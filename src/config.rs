use std::path::PathBuf;

use crate::error::Result;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Credentials {
    pub auth_token: String, // SECRET
    pub note_store_url: String,
    pub web_api_url_prefix: String,
    pub user_id: i32,
    pub shard: String,
    pub expires: i64, // ms since epoch
    pub sandbox: bool,
}

/// Resolve the config directory. Honors `EVERGEEK_CONFIG_DIR`, else uses
/// platform-appropriate `directories`.
///
/// Implemented in Task 5.
pub fn config_dir() -> Result<PathBuf> {
    todo!("config_dir is implemented in Task 5")
}

/// Load persisted credentials, if any (`Ok(None)` when the file is absent).
///
/// Implemented in Task 5.
pub fn load_credentials() -> Result<Option<Credentials>> {
    todo!("load_credentials is implemented in Task 5")
}

/// Persist credentials to `credentials.toml` (chmod `0600` on unix).
///
/// Implemented in Task 5.
pub fn save_credentials(_creds: &Credentials) -> Result<()> {
    todo!("save_credentials is implemented in Task 5")
}

/// Remove the stored credentials file.
///
/// Implemented in Task 5.
pub fn delete_credentials() -> Result<()> {
    todo!("delete_credentials is implemented in Task 5")
}

/// Read `EVERGEEK_CONSUMER_KEY` from the environment.
///
/// Implemented in Task 5.
pub fn consumer_key() -> Result<String> {
    todo!("consumer_key is implemented in Task 5")
}

/// Read `EVERGEEK_CONSUMER_SECRET` from the environment.
///
/// Implemented in Task 5.
pub fn consumer_secret() -> Result<String> {
    todo!("consumer_secret is implemented in Task 5")
}
