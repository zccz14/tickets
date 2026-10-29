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

pub async fn connect(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Json(input): Json<ConnectCybion>,
) -> AppResult<Json<Me>> {
    let api_key = input.api_key.trim().to_owned();
    if !api_key.starts_with("cyb_") || api_key.chars().count() < 16 {
        return Err(AppError::bad_request(
            "Cybion API Key 形如 cyb_<user-id>_<secret>，请从 Cybion 的 API Keys 页面复制",
        ));
    }
    // Probe before storing: a key that cannot read anything is a key the user
    // pasted wrong, and telling them now beats failing on their first Ticket.
    if let Err(error) = state.cybion.probe_key(&api_key).await {
        return Err(AppError::bad_request(error));
    }
    sqlx::query(
        "INSERT INTO cybion_accounts(user_id,api_key_cipher,key_hint,created_at,last_checked_at,last_error)
         VALUES(?,?,?,?,?,NULL)
         ON CONFLICT(user_id) DO UPDATE SET api_key_cipher=excluded.api_key_cipher,key_hint=excluded.key_hint,created_at=excluded.created_at,last_checked_at=excluded.last_checked_at,last_error=NULL",
    )
    .bind(&user.id)
    .bind(state.secrets.seal(&api_key))
    .bind(token_hint(&api_key))
    .bind(now())
    .bind(now())
    .execute(&state.db)
    .await?;
    me(State(state), Extension(user)).await
}

pub async fn disconnect(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
) -> AppResult<Json<Value>> {
    sqlx::query("DELETE FROM cybion_accounts WHERE user_id=?")
        .bind(&user.id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "disconnected": true })))
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
