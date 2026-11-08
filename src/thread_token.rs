//! Cybion Thread identity.
//!
//! A Ticket's two Threads prove who they are with a short-lived JWT that Cybion
//! signs: `sub` is the Thread id, `iss` is Cybion, and the public keys are
//! published at `{cybion}/.well-known/jwks.json`. Tickets keeps no agent secret
//! of its own — the only thing it stores is the Thread id it created, and that id
//! is what a token is looked up against.
//!
//! The scheme is the one Auth Mini uses one level up: Ed25519, an `EdDSA` JWT
//! naming its key with `kid`, and a JWKS document the verifier caches. It is not
//! `auth-mini-axum`, because Auth Mini's tokens are about a *session* (`sid`,
//! `typ`) and a Thread has none.

use std::{collections::HashMap, sync::Arc};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde_json::Value;
use tokio::sync::RwLock;

use crate::{
    crypto::now,
    error::{AppError, AppResult},
};

/// What Cybion puts in `iss`. Both public entries serve the same deployment, so
/// this is a constant rather than a setting: the issuer is the deployment, not
/// the hostname the browser happened to use.
pub const ISSUER: &str = "https://cybion.ntnl.io";
/// At most one key fetch per window. A caller with a junk `kid` must not be able
/// to turn every request into an outbound call.
const REFRESH_FLOOR_SECONDS: i64 = 15;

/// Verifies Cybion Thread tokens and answers with the Thread id they prove.
#[derive(Clone)]
pub struct ThreadTokens {
    issuer: String,
    jwks_url: Option<String>,
    cached: Arc<RwLock<Cached>>,
}

#[derive(Default)]
struct Cached {
    keys: HashMap<String, VerifyingKey>,
    /// When the last fetch was *attempted*, successfully or not.
    attempted_at: Option<i64>,
}

impl ThreadTokens {
    /// Fetches keys from `{cybion}/.well-known/jwks.json`. Nothing is fetched
    /// until the first token arrives, so a Cybion outage cannot stop the process
    /// from starting.
    pub fn new(cybion_base: &str) -> Self {
        Self {
            issuer: ISSUER.to_owned(),
            jwks_url: Some(format!(
                "{}/.well-known/jwks.json",
                cybion_base.trim_end_matches('/')
            )),
            cached: Arc::new(RwLock::new(Cached::default())),
        }
    }

    /// Trusts a fixed key set and never fetches. Tests use this so they can sign
    /// their own tokens instead of standing up a JWKS endpoint.
    pub fn pinned(keys: HashMap<String, VerifyingKey>) -> Self {
        Self {
            issuer: ISSUER.to_owned(),
            jwks_url: None,
            cached: Arc::new(RwLock::new(Cached {
                keys,
                attempted_at: None,
            })),
        }
    }

    /// The Thread id this token proves, or why it proves nothing.
    pub async fn verify(&self, token: &str) -> AppResult<String> {
        let parsed = Parsed::parse(token)?;
        let key = self.key(&parsed.kid).await?;
        key.verify(parsed.signing_input.as_bytes(), &parsed.signature)
            .map_err(|_| AppError::unauthorized("Agent 令牌的签名无效"))?;
        if parsed.claims.get("iss").and_then(Value::as_str) != Some(self.issuer.as_str()) {
            return Err(AppError::unauthorized("Agent 令牌不是 Cybion 签发的"));
        }
        let expires_at = parsed
            .claims
            .get("exp")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        if expires_at <= now() {
            return Err(AppError::unauthorized(
                "Agent 令牌已过期：调用 create_thread_token 重新签发一张",
            ));
        }
        parsed
            .claims
            .get("sub")
            .and_then(Value::as_str)
            .filter(|subject| !subject.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| AppError::unauthorized("Agent 令牌没有指向任何 Thread"))
    }

    async fn key(&self, kid: &str) -> AppResult<VerifyingKey> {
        if let Some(key) = self.cached.read().await.keys.get(kid).cloned() {
            return Ok(key);
        }
        self.refresh().await?;
        self.cached
            .read()
            .await
            .keys
            .get(kid)
            .cloned()
            .ok_or_else(|| AppError::unauthorized("Agent 令牌的签名密钥不在 Cybion 公布的密钥集里"))
    }

    async fn refresh(&self) -> AppResult<()> {
        let Some(url) = self.jwks_url.as_deref() else {
            return Ok(());
        };
        {
            let cached = self.cached.read().await;
            if let Some(attempted_at) = cached.attempted_at
                && now() - attempted_at < REFRESH_FLOOR_SECONDS
            {
                return Ok(());
            }
        }
        self.cached.write().await.attempted_at = Some(now());
        let response = reqwest::get(url)
            .await
            .map_err(|error| AppError::unavailable(format!("Cybion 公钥不可用: {error}")))?;
        if !response.status().is_success() {
            return Err(AppError::unavailable(format!(
                "Cybion 公钥不可用: HTTP {}",
                response.status()
            )));
        }
        let document: Value = response
            .json()
            .await
            .map_err(|error| AppError::unavailable(format!("Cybion 公钥无法解析: {error}")))?;
        let keys = parse_jwks(&document);
        if keys.is_empty() {
            return Err(AppError::unavailable("Cybion 公布的密钥集是空的"));
        }
        self.cached.write().await.keys = keys;
        Ok(())
    }
}

struct Parsed {
    kid: String,
    signing_input: String,
    signature: Signature,
    claims: Value,
}

impl Parsed {
    fn parse(token: &str) -> AppResult<Self> {
        let mut segments = token.split('.');
        let (Some(header), Some(payload), Some(signature), None) = (
            segments.next(),
            segments.next(),
            segments.next(),
            segments.next(),
        ) else {
            return Err(AppError::unauthorized("Agent 令牌的格式不正确"));
        };
        let header_json: Value = decode_json(header)?;
        if header_json.get("alg").and_then(Value::as_str) != Some("EdDSA") {
            return Err(AppError::unauthorized("Agent 令牌的签名算法不受支持"));
        }
        let kid = header_json
            .get("kid")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        if kid.is_empty() {
            return Err(AppError::unauthorized("Agent 令牌没有指明签名密钥"));
        }
        let claims = decode_json(payload)?;
        let signature = URL_SAFE_NO_PAD
            .decode(signature)
            .ok()
            .and_then(|bytes| Signature::from_slice(&bytes).ok())
            .ok_or_else(|| AppError::unauthorized("Agent 令牌的签名不可读"))?;
        Ok(Self {
            kid,
            // The signature covers the two segments exactly as they arrived, so
            // the signing input is rebuilt from the original text, not from the
            // re-serialized JSON.
            signing_input: format!("{header}.{payload}"),
            signature,
            claims,
        })
    }
}

fn decode_json(segment: &str) -> AppResult<Value> {
    URL_SAFE_NO_PAD
        .decode(segment)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or_else(|| AppError::unauthorized("Agent 令牌的内容不可读"))
}

/// Keeps every key the deployment publishes — the signing one and its standby —
/// because the standby is exactly the key the next token will name.
fn parse_jwks(document: &Value) -> HashMap<String, VerifyingKey> {
    let mut keys = HashMap::new();
    for key in document
        .get("keys")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        if key.get("kty").and_then(Value::as_str) != Some("OKP")
            || key.get("crv").and_then(Value::as_str) != Some("Ed25519")
        {
            continue;
        }
        let (Some(kid), Some(x)) = (
            key.get("kid").and_then(Value::as_str),
            key.get("x").and_then(Value::as_str),
        ) else {
            continue;
        };
        let Some(public) = URL_SAFE_NO_PAD
            .decode(x)
            .ok()
            .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
        else {
            continue;
        };
        if let Ok(verifying) = VerifyingKey::from_bytes(&public) {
            keys.insert(kid.to_owned(), verifying);
        }
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::json;

    fn sign(key: &SigningKey, kid: &str, claims: Value) -> String {
        let header =
            URL_SAFE_NO_PAD.encode(json!({"alg":"EdDSA","kid":kid,"typ":"JWT"}).to_string());
        let payload = URL_SAFE_NO_PAD.encode(claims.to_string());
        let signature = key.sign(format!("{header}.{payload}").as_bytes());
        format!(
            "{header}.{payload}.{}",
            URL_SAFE_NO_PAD.encode(signature.to_bytes())
        )
    }

    fn gate(key: &SigningKey, kid: &str) -> ThreadTokens {
        ThreadTokens::pinned(HashMap::from([(kid.to_owned(), key.verifying_key())]))
    }

    fn claims(subject: &str) -> Value {
        json!({"iss": ISSUER, "sub": subject, "iat": now(), "exp": now() + 60})
    }

    #[tokio::test]
    async fn a_valid_token_names_its_thread() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let token = sign(&key, "kid-1", claims("thread-1"));
        assert_eq!(
            gate(&key, "kid-1").verify(&token).await.unwrap(),
            "thread-1"
        );
    }

    #[tokio::test]
    async fn an_unknown_key_is_rejected() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let token = sign(&key, "kid-2", claims("thread-1"));
        assert_eq!(
            gate(&key, "kid-1").verify(&token).await.unwrap_err().status,
            401
        );
    }

    #[tokio::test]
    async fn a_token_signed_by_another_key_is_rejected() {
        let signer = SigningKey::from_bytes(&[7u8; 32]);
        let trusted = SigningKey::from_bytes(&[9u8; 32]);
        let token = sign(&signer, "kid-1", claims("thread-1"));
        let gate = ThreadTokens::pinned(HashMap::from([(
            "kid-1".to_owned(),
            trusted.verifying_key(),
        )]));
        assert_eq!(gate.verify(&token).await.unwrap_err().status, 401);
    }

    #[tokio::test]
    async fn an_expired_token_is_rejected() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let token = sign(
            &key,
            "kid-1",
            json!({"iss": ISSUER, "sub": "t", "exp": now() - 1}),
        );
        assert_eq!(
            gate(&key, "kid-1").verify(&token).await.unwrap_err().status,
            401
        );
    }

    #[tokio::test]
    async fn a_token_from_another_issuer_is_rejected() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let token = sign(
            &key,
            "kid-1",
            json!({"iss": "https://evil.example", "sub": "t", "exp": now() + 60}),
        );
        assert_eq!(
            gate(&key, "kid-1").verify(&token).await.unwrap_err().status,
            401
        );
    }

    #[tokio::test]
    async fn a_token_without_a_subject_is_rejected() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let token = sign(&key, "kid-1", json!({"iss": ISSUER, "exp": now() + 60}));
        assert_eq!(
            gate(&key, "kid-1").verify(&token).await.unwrap_err().status,
            401
        );
    }

    #[test]
    fn a_published_document_yields_both_slots() {
        let first = SigningKey::from_bytes(&[1u8; 32]);
        let second = SigningKey::from_bytes(&[2u8; 32]);
        let document = json!({"keys": [
            {"kid":"a","kty":"OKP","crv":"Ed25519","alg":"EdDSA","use":"sig","x": URL_SAFE_NO_PAD.encode(first.verifying_key().to_bytes())},
            {"kid":"b","kty":"OKP","crv":"Ed25519","alg":"EdDSA","use":"sig","x": URL_SAFE_NO_PAD.encode(second.verifying_key().to_bytes())},
            {"kid":"ignored","kty":"RSA","crv":"","x":"nope"},
        ]});
        let keys = parse_jwks(&document);
        assert_eq!(keys.len(), 2);
        assert!(keys.contains_key("a") && keys.contains_key("b"));
    }
}
