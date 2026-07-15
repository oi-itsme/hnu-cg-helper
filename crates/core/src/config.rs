use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit, OsRng},
};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::CoreError;

// ── Keyring ──────────────────────────────────────────────────────────

const KEYRING_SERVICE: &str = "hnu-cg-helper";
const KEYRING_ACCOUNT: &str = "encryption-key";

/// Generate a random 256-bit AES key.
fn generate_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    key
}

/// Obtain the encryption key from the OS keyring.
///
/// Returns `(key, keyring_available)`.
/// - If the keyring is accessible and a key exists, returns that key + `true`.
/// - If the keyring is accessible but no key exists, generates and persists a new key + `true`.
/// - If the keyring is **not** accessible, returns `(None, false)`.
fn load_key_from_keyring() -> (Option<[u8; 32]>, bool) {
    let entry = match keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT) {
        Ok(e) => e,
        Err(_) => {
            tracing::warn!("OS 密钥环不可用，凭据加密功能已禁用");
            return (None, false);
        }
    };

    // 检测实际使用的密钥环后端
    let backend_name = entry
        .get_credential()
        .downcast_ref::<keyring::mock::MockCredential>()
        .map(|_| "mock (in-memory)")
        .unwrap_or("平台原生密钥环");

    match entry.get_password() {
        Ok(stored) => {
            // Decode stored base64 key
            match BASE64.decode(&stored) {
                Ok(bytes) if bytes.len() == 32 => {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&bytes);
                    tracing::info!("已从 {backend_name} 读取加密密钥");
                    (Some(key), true)
                }
                _ => {
                    // Corrupted key data — regenerate
                    tracing::warn!("{backend_name} 中的密钥已损坏，正在重新生成");
                    let key = generate_key();
                    let _ = entry.set_password(&BASE64.encode(key));
                    (Some(key), true)
                }
            }
        }
        Err(keyring::Error::NoEntry) => {
            // First run — generate and persist a new key
            let key = generate_key();
            match entry.set_password(&BASE64.encode(key)) {
                Ok(()) => {
                    tracing::info!("已在 {backend_name} 中生成新的加密密钥");
                    (Some(key), true)
                }
                Err(e) => {
                    tracing::warn!("无法将密钥写入 {backend_name}: {e}，凭据将无法持久化");
                    (Some(key), false)
                }
            }
        }
        Err(e) => {
            tracing::warn!("无法访问 {backend_name}: {e}，凭据加密功能已禁用");
            (None, false)
        }
    }
}

// ── AES-256-GCM helpers ──────────────────────────────────────────────

/// Encrypt `plaintext` with AES-256-GCM.
///
/// Returns base64-encoded `nonce (12 bytes) || ciphertext`.
/// Returns `None` when `key` is `None` (keyring unavailable).
fn encrypt_field(key: Option<&[u8; 32]>, plaintext: &str) -> Result<Option<String>, CoreError> {
    let key = match key {
        Some(k) => k,
        None => return Ok(None),
    };
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| CoreError::Config(format!("加密失败: {e}")))?;

    // Prepend nonce to ciphertext, then base64-encode
    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);

    Ok(Some(BASE64.encode(&combined)))
}

/// Decrypt `encoded` (base64-encoded `nonce || ciphertext`) with AES-256-GCM.
///
/// Returns `None` when `key` is `None` or `encoded` is empty.
fn decrypt_field(key: Option<&[u8; 32]>, encoded: &str) -> Result<Option<String>, CoreError> {
    let key = match key {
        Some(k) => k,
        None => return Ok(None),
    };
    if encoded.is_empty() {
        return Ok(None);
    }

    let data = BASE64
        .decode(encoded)
        .map_err(|e| CoreError::Config(format!("Base64 解码失败: {e}")))?;

    if data.len() < 12 + 16 {
        // At minimum: 12-byte nonce + 16-byte GCM tag
        return Ok(None); // treat as "not set"
    }

    let (nonce_bytes, ciphertext) = data.split_at(12);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| CoreError::Config(format!("解密失败: {e}")))?;

    String::from_utf8(plaintext)
        .map(Some)
        .map_err(|e| CoreError::Config(format!("UTF-8 解码失败: {e}")))
}

// ── Configuration structs (TOML representation) ──────────────────────

fn default_base_url() -> String {
    "https://api.deepseek.com".into()
}

fn default_model() -> String {
    "deepseek-v4-flash".into()
}

fn default_port() -> u16 {
    20365
}

/// Raw on-disk AI config (some fields may be encrypted).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawAiConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default = "default_model")]
    pub model: String,
}

impl Default for RawAiConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: default_base_url(),
            model: default_model(),
        }
    }
}

/// Raw on-disk auth config.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RawAuthConfig {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
}

/// Raw on-disk server config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawServerConfig {
    #[serde(default = "default_port")]
    pub port: u16,
}

impl Default for RawServerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
        }
    }
}

/// The full configuration as stored in TOML.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RawConfig {
    #[serde(default)]
    pub ai: RawAiConfig,
    #[serde(default)]
    pub auth: RawAuthConfig,
    #[serde(default)]
    pub server: RawServerConfig,
}

// ── Public view structs (for API responses) ──────────────────────────

/// AI configuration returned to the client.
/// API key is never sent to the frontend in plaintext.
#[derive(Debug, Clone, Serialize)]
pub struct AiConfigView {
    pub has_api_key: bool,
    pub base_url: String,
    pub model: String,
}

// ── ConfigManager ────────────────────────────────────────────────────

/// Manages configuration lifecycle: load, decrypt, mutate, save.
///
/// # Thread safety
///
/// This struct is **not** `Clone`. Wrap it in `Arc<RwLock<ConfigManager>>`
/// in your application state to share across request handlers.
pub struct ConfigManager {
    config_path: PathBuf,
    encryption_key: Option<[u8; 32]>,
    pub keyring_available: bool,
    raw: RawConfig,
    /// API key stored only in memory when keyring is unavailable.
    /// Persisted (non-empty) on disk takes precedence over this field.
    session_api_key: Option<String>,
}

impl ConfigManager {
    /// Load configuration from `config_path`, fall back to defaults if the
    /// file does not exist.  Also initialises (or reads) the OS keyring entry.
    pub fn load(config_path: impl Into<PathBuf>) -> Result<Self, CoreError> {
        let config_path: PathBuf = config_path.into();

        // Ensure parent directory exists
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CoreError::Config(format!("无法创建配置目录: {e}")))?;
        }

        let raw = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .map_err(|e| CoreError::Config(format!("读取配置文件失败: {e}")))?;
            toml::from_str(&content).unwrap_or_default()
        } else {
            RawConfig::default()
        };

        let (encryption_key, keyring_available) = load_key_from_keyring();

        Ok(Self {
            config_path,
            encryption_key,
            keyring_available,
            raw,
            session_api_key: None,
        })
    }

    // ── AI config accessors ──────────────────────────────────────

    /// Return a view of the current AI config (never exposes plaintext API key).
    pub fn ai_config_view(&self) -> AiConfigView {
        let has_api_key = self.ai_api_key().is_some_and(|k| !k.is_empty());

        AiConfigView {
            has_api_key,
            base_url: self.raw.ai.base_url.clone(),
            model: self.raw.ai.model.clone(),
        }
    }

    /// Return the decrypted API key (for server-side use in AI requests).
    ///
    /// Resolution order:
    /// 1. Decrypt from persisted config (when keyring is available).
    /// 2. In-memory session key (when keyring is unavailable).
    pub fn ai_api_key(&self) -> Option<String> {
        // Try encrypted persisted key first
        if let Some(decrypted) = self
            .encryption_key
            .and_then(|key| decrypt_field(Some(&key), &self.raw.ai.api_key).ok())
            .flatten()
        {
            return Some(decrypted);
        }
        // Fall back to session-only key
        self.session_api_key.clone()
    }

    /// Set AI config fields.  `api_key` is encrypted before writing.
    ///
    /// When the OS keyring is unavailable, `api_key` is stored in memory only
    /// (via `session_api_key`) for the lifetime of this process.
    pub fn set_ai_config(
        &mut self,
        api_key: Option<&str>,
        base_url: Option<&str>,
        model: Option<&str>,
    ) -> Result<(), CoreError> {
        if let Some(url) = base_url {
            self.raw.ai.base_url = url.to_string();
        }
        if let Some(m) = model {
            self.raw.ai.model = m.to_string();
        }
        if let Some(key) = api_key {
            if self.encryption_key.is_some() {
                let encrypted = encrypt_field(self.encryption_key.as_ref(), key)?;
                self.raw.ai.api_key = encrypted.unwrap_or_default();
            } else {
                // Keyring unavailable — keep in memory only
                tracing::info!("密钥环不可用，API Key 仅保存在本次会话内存中");
                self.raw.ai.api_key.clear();
                self.session_api_key = Some(key.to_string());
            }
        }
        Ok(())
    }
    // ── Persistence ──────────────────────────────────────────────

    /// Write the current (raw) configuration to disk as TOML.
    pub fn save_to_disk(&self) -> Result<(), CoreError> {
        let content =
            toml::to_string_pretty(&self.raw).map_err(|e| CoreError::Config(e.to_string()))?;
        std::fs::write(&self.config_path, content)
            .map_err(|e| CoreError::Config(format!("写入配置文件失败: {e}")))?;
        tracing::debug!("配置已保存到 {}", self.config_path.display());
        Ok(())
    }

    /// Return the configured server port.
    pub fn server_port(&self) -> u16 {
        self.raw.server.port
    }

    /// Re-read the config file from disk (used when the file was updated externally).
    pub fn reload(&mut self) -> Result<(), CoreError> {
        if self.config_path.exists() {
            let content = std::fs::read_to_string(&self.config_path)
                .map_err(|e| CoreError::Config(format!("读取配置文件失败: {e}")))?;
            self.raw = toml::from_str(&content).unwrap_or_default();
        }
        Ok(())
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = generate_key();
        let plaintext = "sk-test-secret-api-key-12345";

        let encrypted = encrypt_field(Some(&key), plaintext)
            .expect("encrypt should succeed")
            .expect("should not be None when key is provided");

        assert!(!encrypted.is_empty());
        assert_ne!(encrypted, plaintext); // not plaintext

        let decrypted = decrypt_field(Some(&key), &encrypted)
            .expect("decrypt should succeed")
            .expect("should not be None when key is provided");

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_empty_string_is_none() {
        let key = generate_key();
        let result = decrypt_field(Some(&key), "").expect("decrypt should succeed");
        assert!(result.is_none());
    }

    #[test]
    fn test_encrypt_without_key_returns_none() {
        let plaintext = "secret";
        let result = encrypt_field(None, plaintext).expect("encrypt should succeed");
        assert!(result.is_none());
    }

    #[test]
    fn test_decrypt_with_wrong_key_fails() {
        let key1 = generate_key();
        let key2 = generate_key();

        let encrypted = encrypt_field(Some(&key1), "secret")
            .expect("encrypt ok")
            .unwrap();

        let result = decrypt_field(Some(&key2), &encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn test_ai_config_view_empty_defaults() {
        // Create a minimal in-memory config
        let tmp = std::env::temp_dir().join("hnu-cg-test-config.toml");
        let _ = std::fs::remove_file(&tmp); // clean up from previous runs
        let mgr = ConfigManager::load(&tmp).expect("load should succeed");
        let view = mgr.ai_config_view();

        assert!(!view.has_api_key);
        assert_eq!(view.base_url, "https://api.deepseek.com");
        assert_eq!(view.model, "deepseek-v4-flash");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_save_and_reload() {
        let tmp = std::env::temp_dir().join("hnu-cg-test-save.toml");
        let _ = std::fs::remove_file(&tmp);

        {
            let mut mgr = ConfigManager::load(&tmp).expect("load ok");
            mgr.set_ai_config(None, Some("https://custom.api.com"), Some("gpt-4"))
                .expect("set_ai_config ok");
            // Without keyring, set with api_key won't encrypt — just store empty
            mgr.save_to_disk().expect("save ok");
        }

        {
            let mgr2 = ConfigManager::load(&tmp).expect("reload ok");
            let view = mgr2.ai_config_view();
            assert_eq!(view.base_url, "https://custom.api.com");
            assert_eq!(view.model, "gpt-4");
        }

        let _ = std::fs::remove_file(&tmp);
    }

    /// Setting an API key when keyring is available encrypts it (non-empty
    /// ciphertext).  When keyring is unavailable, it is stored in
    /// `session_api_key` and `ai_api_key()` still returns it.
    #[test]
    fn test_set_api_key_behaviour() {
        let tmp = std::env::temp_dir().join("hnu-cg-test-apikey.toml");
        let _ = std::fs::remove_file(&tmp);

        let mut mgr = ConfigManager::load(&tmp).expect("load ok");
        mgr.set_ai_config(Some("secret-key"), None, None)
            .expect("set_ai_config ok");

        if mgr.keyring_available {
            // Keyring available → key should be encrypted (non-empty ciphertext)
            assert!(!mgr.raw.ai.api_key.is_empty());
        } else {
            // Keyring unavailable → persisted field is empty, in-memory field set
            assert!(mgr.raw.ai.api_key.is_empty());
            assert_eq!(mgr.session_api_key.as_deref(), Some("secret-key"));
        }

        // In both cases `ai_api_key()` returns the key
        let decrypted = mgr.ai_api_key();
        assert_eq!(decrypted.as_deref(), Some("secret-key"));

        let _ = std::fs::remove_file(&tmp);
    }
}
