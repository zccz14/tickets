use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, OsRng, rand_core::RngCore},
};
use anyhow::Context;
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use rand::Rng;
use std::path::Path;
use std::sync::Arc;

/// Seals user-held provider credentials at rest.
///
/// The key lives beside the database (`secret.key`, mode 0600) rather than
/// inside it, so handing someone a database file does not hand them the Cybion
/// API keys of every reporter. Rotating the file invalidates stored keys, which
/// the UI surfaces as "reconnect your account".
pub struct SecretBox {
    cipher: Arc<Aes256Gcm>,
}

impl SecretBox {
    pub fn load_or_create(directory: &Path) -> anyhow::Result<Self> {
        std::fs::create_dir_all(directory)
            .with_context(|| format!("创建目录失败: {}", directory.display()))?;
        let path = directory.join("secret.key");
        let raw = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => {
                let mut bytes = vec![0u8; 32];
                OsRng.fill_bytes(&mut bytes);
                std::fs::write(&path, &bytes)
                    .with_context(|| format!("写入密钥失败: {}", path.display()))?;
                restrict_permissions(&path)?;
                bytes
            }
        };
        anyhow::ensure!(raw.len() == 32, "secret.key 长度必须为 32 字节");
        let cipher = Aes256Gcm::new_from_slice(&raw).expect("32 字节密钥");
        Ok(Self {
            cipher: Arc::new(cipher),
        })
    }

    pub fn seal(&self, plaintext: &str) -> String {
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let ciphertext = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
            .expect("AES-GCM 加密不会失败");
        let mut packed = Vec::with_capacity(12 + ciphertext.len());
        packed.extend_from_slice(&nonce);
        packed.extend_from_slice(&ciphertext);
        B64.encode(packed)
    }

    pub fn open(&self, sealed: &str) -> anyhow::Result<String> {
        let packed = B64.decode(sealed).context("凭据不是合法的 base64")?;
        anyhow::ensure!(packed.len() > 12, "凭据长度非法");
        let (nonce, ciphertext) = packed.split_at(12);
        let plaintext = self
            .cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| anyhow::anyhow!("凭据无法解密，可能需要重新连接 Cybion"))?;
        String::from_utf8(plaintext).context("凭据不是 UTF-8")
    }
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

/// `tk_` + 48 hex characters. Long enough to be unguessable, short enough to
/// paste into a Cybion Thread prompt without wrapping.
pub fn new_token() -> String {
    let mut bytes = [0u8; 24];
    rand::rng().fill(&mut bytes);
    format!("tk_{}", hex::encode(bytes))
}

pub fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

/// `cyb_abc123…` — the first characters only, for "which key is this" display.
pub fn token_hint(token: &str) -> String {
    let head: String = token.chars().take(12).collect();
    format!("{head}…")
}

pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_round_trip_with_a_fresh_nonce_each_time() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = SecretBox::load_or_create(dir.path()).unwrap();
        let first = secrets.seal("cyb_owner_secret");
        let second = secrets.seal("cyb_owner_secret");
        assert_ne!(first, second, "每次加密都应使用新的 nonce");
        assert_eq!(secrets.open(&first).unwrap(), "cyb_owner_secret");
        assert_eq!(secrets.open(&second).unwrap(), "cyb_owner_secret");
        assert!(secrets.open("显然不是 base64").is_err());
        assert!(secrets.open("aGVsbG8=").is_err(), "长度不足的密文应被拒绝");
    }

    #[test]
    fn the_key_file_is_private_and_stable_across_restarts() {
        let dir = tempfile::tempdir().unwrap();
        let first = SecretBox::load_or_create(dir.path()).unwrap();
        let sealed = first.seal("hello");
        let path = dir.path().join("secret.key");
        assert_eq!(std::fs::read(&path).unwrap().len(), 32);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let second = SecretBox::load_or_create(dir.path()).unwrap();
        assert_eq!(second.open(&sealed).unwrap(), "hello");
    }

    #[test]
    fn a_different_secret_cannot_open_old_ciphertext() {
        let one = tempfile::tempdir().unwrap();
        let two = tempfile::tempdir().unwrap();
        let sealed = SecretBox::load_or_create(one.path()).unwrap().seal("hello");
        let other = SecretBox::load_or_create(two.path()).unwrap();
        assert!(other.open(&sealed).is_err());
    }

    #[test]
    fn agent_tokens_are_prefixed_and_only_ever_stored_hashed() {
        let token = new_token();
        assert!(token.starts_with("tk_"));
        assert_eq!(token.len(), 51);
        assert_eq!(hash_token(&token).len(), 64);
        assert_ne!(hash_token(&token), token);
        assert_eq!(
            hash_token("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn hints_keep_the_first_twelve_characters() {
        assert_eq!(token_hint("cyb_1234567890abcdef"), "cyb_12345678…");
    }
}
