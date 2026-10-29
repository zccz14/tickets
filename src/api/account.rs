use axum::{Extension, Json, extract::State};
use serde_json::{Value, json};
use sqlx::Row;

use crate::{
    AppState,
    auth::CurrentUser,
    crypto::{now, token_hint},
    error::{AppError, AppResult},
    model::{ConnectCybion, Me},
};

pub async fn me(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
) -> AppResult<Json<Me>> {
    let row = sqlx::query("SELECT key_hint,last_error FROM cybion_accounts WHERE user_id=?")
        .bind(&user.id)
        .fetch_optional(&state.db)
        .await?;
    Ok(Json(Me {
        id: user.id.clone(),
        display_name: user.display_name.clone(),
        cybion_connected: row.is_some(),
        cybion_key_hint: row.as_ref().map(|row| row.get(0)),
        cybion_last_error: row.and_then(|row| row.get::<Option<String>, _>(1)),
    }))
}

/// Connects the signed-in person to Cybion.
///
/// The browser never asks for a key: Tickets forwards the person's Auth Mini
/// token (whose audience set includes `cybion.ntnl.io`) to Cybion's browser API
/// and mints a `cyb_…` key there. A manual key is still accepted for recovery
/// and tests, but it is not part of the UI.
pub async fn connect(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Json(input): Json<ConnectCybion>,
) -> AppResult<Json<Me>> {
    let manual = input
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty());
    let (api_key, key_id) = match manual {
        Some(api_key) => {
            if !api_key.starts_with("cyb_") || api_key.chars().count() < 16 {
                return Err(AppError::bad_request(
                    "Cybion API Key 形如 cyb_<user-id>_<secret>，请从 Cybion 的 API Keys 页面复制",
                ));
            }
            // Probe before storing: a key that cannot read anything is a key the
            // user pasted wrong, and telling them now beats failing later.
            if let Err(error) = state.cybion.probe_key(api_key).await {
                return Err(AppError::bad_request(error));
            }
            (api_key.to_owned(), None)
        }
        None => {
            let token = user
                .token
                .as_deref()
                .ok_or_else(|| AppError::unauthorized("缺少 Auth Mini 访问令牌"))?;
            let created = state
                .cybion
                .create_api_key(token, "Tickets")
                .await
                .map_err(AppError::unavailable)?;
            (created.secret, Some(created.key.id))
        }
    };

    let previous = sqlx::query_scalar::<_, Option<String>>(
        "SELECT key_id FROM cybion_accounts WHERE user_id=?",
    )
    .bind(&user.id)
    .fetch_optional(&state.db)
    .await?
    .flatten();

    sqlx::query(
        "INSERT INTO cybion_accounts(user_id,api_key_cipher,key_hint,key_id,created_at,last_checked_at,last_error)
         VALUES(?,?,?,?,?,?,NULL)
         ON CONFLICT(user_id) DO UPDATE SET api_key_cipher=excluded.api_key_cipher,key_hint=excluded.key_hint,key_id=excluded.key_id,created_at=excluded.created_at,last_checked_at=excluded.last_checked_at,last_error=NULL",
    )
    .bind(&user.id)
    .bind(state.secrets.seal(&api_key))
    .bind(token_hint(&api_key))
    .bind(key_id.as_deref())
    .bind(now())
    .bind(now())
    .execute(&state.db)
    .await?;

    // One live key per person: the one we just replaced is useless now.
    if let (Some(token), Some(previous)) = (user.token.as_deref(), previous)
        && Some(previous.as_str()) != key_id.as_deref()
    {
        let _ = state.cybion.delete_api_key(token, &previous).await;
    }

    me(State(state), Extension(user)).await
}

pub async fn disconnect(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
) -> AppResult<Json<Value>> {
    let key_id = sqlx::query_scalar::<_, Option<String>>(
        "SELECT key_id FROM cybion_accounts WHERE user_id=?",
    )
    .bind(&user.id)
    .fetch_optional(&state.db)
    .await?
    .flatten();

    let mut revoked = false;
    if let (Some(token), Some(key_id)) = (user.token.as_deref(), key_id) {
        revoked = state.cybion.delete_api_key(token, &key_id).await.is_ok();
    }
    sqlx::query("DELETE FROM cybion_accounts WHERE user_id=?")
        .bind(&user.id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "disconnected": true, "revoked": revoked })))
}

/// The reporter's Cybion key, decrypted, for server-side Thread calls.
pub async fn reporter_key(state: &AppState, user_id: &str) -> AppResult<Option<String>> {
    let Some(cipher) = sqlx::query_scalar::<_, String>(
        "SELECT api_key_cipher FROM cybion_accounts WHERE user_id=?",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?
    else {
        return Ok(None);
    };
    Ok(Some(state.secrets.open(&cipher)?))
}
