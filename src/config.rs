use std::path::PathBuf;

use crate::error::{EvergeekError, Result};

const CREDENTIALS_FILE: &str = "credentials.toml";

#[derive(Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Credentials {
    pub auth_token: String, // SECRET
    pub note_store_url: String,
    pub web_api_url_prefix: String,
    pub user_id: i32,
    pub shard: String,
    pub expires: i64, // ms since epoch
    pub sandbox: bool,
}

// Manual Debug that redacts the auth token — this is a public repo and the
// token grants full account access, so it must never reach logs/panics.
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("auth_token", &"[REDACTED]")
            .field("note_store_url", &self.note_store_url)
            .field("web_api_url_prefix", &self.web_api_url_prefix)
            .field("user_id", &self.user_id)
            .field("shard", &self.shard)
            .field("expires", &self.expires)
            .field("sandbox", &self.sandbox)
            .finish()
    }
}

/// Resolve the config directory. Honors `EVERGEEK_CONFIG_DIR`, else uses
/// platform-appropriate `directories`. Creates the directory if missing.
pub fn config_dir() -> Result<PathBuf> {
    let dir = match std::env::var_os("EVERGEEK_CONFIG_DIR") {
        Some(path) => PathBuf::from(path),
        None => directories::ProjectDirs::from("io", "evergeek", "evergeek")
            .map(|dirs| dirs.config_dir().to_path_buf())
            .ok_or_else(|| {
                EvergeekError::Config("could not determine a config directory".to_string())
            })?,
    };
    std::fs::create_dir_all(&dir).map_err(|e| {
        EvergeekError::Config(format!(
            "failed to create config directory {}: {e}",
            dir.display()
        ))
    })?;
    Ok(dir)
}

/// Path to the credentials file inside the config directory.
fn credentials_path() -> Result<PathBuf> {
    Ok(config_dir()?.join(CREDENTIALS_FILE))
}

/// Load persisted credentials, if any (`Ok(None)` when the file is absent).
pub fn load_credentials() -> Result<Option<Credentials>> {
    let path = credentials_path()?;
    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(EvergeekError::Config(format!(
                "failed to read credentials file {}: {e}",
                path.display()
            )))
        }
    };
    let creds = toml::from_str(&contents).map_err(|e| {
        EvergeekError::Config(format!(
            "failed to parse credentials file {}: {e}",
            path.display()
        ))
    })?;
    Ok(Some(creds))
}

/// Persist credentials to `credentials.toml`. On unix the file is created with
/// mode `0600` from the start and swapped in atomically via a temp file +
/// rename, so the token is never momentarily readable by other users.
pub fn save_credentials(creds: &Credentials) -> Result<()> {
    let path = credentials_path()?;
    let contents = toml::to_string_pretty(creds)
        .map_err(|e| EvergeekError::Config(format!("failed to serialize credentials: {e}")))?;
    let write_err = |e: std::io::Error| {
        EvergeekError::Config(format!(
            "failed to write credentials file {}: {e}",
            path.display()
        ))
    };

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        // Write to a sibling temp file created 0600, then atomically rename over
        // the target. The renamed inode keeps its 0600 mode regardless of
        // whether the target previously existed with looser permissions.
        let tmp_path = path.with_extension("toml.tmp");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp_path)
            .map_err(write_err)?;
        file.write_all(contents.as_bytes()).map_err(write_err)?;
        file.sync_all().map_err(write_err)?;
        std::fs::rename(&tmp_path, &path).map_err(write_err)?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&path, contents).map_err(write_err)?;
    }

    Ok(())
}

/// Remove the stored credentials file. Succeeds even if it is already gone.
pub fn delete_credentials() -> Result<()> {
    let path = credentials_path()?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(EvergeekError::Config(format!(
            "failed to delete credentials file {}: {e}",
            path.display()
        ))),
    }
}

/// Read `EVERGEEK_CONSUMER_KEY` from the environment.
pub fn consumer_key() -> Result<String> {
    std::env::var("EVERGEEK_CONSUMER_KEY")
        .map_err(|_| EvergeekError::Config("EVERGEEK_CONSUMER_KEY is not set".to_string()))
}

/// Read `EVERGEEK_CONSUMER_SECRET` from the environment.
pub fn consumer_secret() -> Result<String> {
    std::env::var("EVERGEEK_CONSUMER_SECRET")
        .map_err(|_| EvergeekError::Config("EVERGEEK_CONSUMER_SECRET is not set".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Env vars are process-global and tests run in parallel threads, so any
    /// test that mutates `EVERGEEK_*` env vars must hold this lock first.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn sample_credentials() -> Credentials {
        Credentials {
            auth_token: "S=s1:U=abc:E=18a:C=...:P=...:A=evergeek:V=2:H=deadbeef".to_string(),
            note_store_url: "https://sandbox.evernote.com/shard/s1/notestore".to_string(),
            web_api_url_prefix: "https://sandbox.evernote.com/shard/s1/".to_string(),
            user_id: 12345,
            shard: "s1".to_string(),
            expires: 1_700_000_000_000,
            sandbox: true,
        }
    }

    #[test]
    fn save_then_load_round_trips() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("EVERGEEK_CONFIG_DIR", tmp.path());

        let creds = sample_credentials();
        save_credentials(&creds).unwrap();

        let loaded = load_credentials().unwrap();
        assert_eq!(loaded, Some(creds));

        std::env::remove_var("EVERGEEK_CONFIG_DIR");
    }

    #[test]
    fn missing_file_loads_as_none() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("EVERGEEK_CONFIG_DIR", tmp.path());

        let loaded = load_credentials().unwrap();
        assert_eq!(loaded, None);

        std::env::remove_var("EVERGEEK_CONFIG_DIR");
    }

    #[cfg(unix)]
    #[test]
    fn saved_file_mode_is_0600_on_unix() {
        use std::os::unix::fs::PermissionsExt;

        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("EVERGEEK_CONFIG_DIR", tmp.path());

        save_credentials(&sample_credentials()).unwrap();
        let path = tmp.path().join("credentials.toml");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);

        std::env::remove_var("EVERGEEK_CONFIG_DIR");
    }

    #[test]
    fn delete_removes_file_and_load_returns_none() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("EVERGEEK_CONFIG_DIR", tmp.path());

        save_credentials(&sample_credentials()).unwrap();
        let path = tmp.path().join("credentials.toml");
        assert!(path.exists());

        delete_credentials().unwrap();
        assert!(!path.exists());
        assert_eq!(load_credentials().unwrap(), None);

        // Deleting again is a no-op (file already gone).
        delete_credentials().unwrap();

        std::env::remove_var("EVERGEEK_CONFIG_DIR");
    }

    #[test]
    fn config_dir_honors_env_var_and_creates_it() {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("nested").join("evergeek");
        std::env::set_var("EVERGEEK_CONFIG_DIR", &nested);

        let dir = config_dir().unwrap();
        assert_eq!(dir, nested);
        assert!(dir.is_dir());

        std::env::remove_var("EVERGEEK_CONFIG_DIR");
    }

    #[test]
    fn debug_redacts_auth_token() {
        let creds = sample_credentials();
        let rendered = format!("{creds:?}");
        assert!(
            !rendered.contains(&creds.auth_token),
            "Debug output leaked the auth token: {rendered}"
        );
        assert!(rendered.contains("[REDACTED]"));
        // Non-secret fields should still be visible for debugging.
        assert!(rendered.contains("s1"));
    }

    #[test]
    fn consumer_key_and_secret_env_handling() {
        let _guard = ENV_LOCK.lock().unwrap();

        // Missing → Config error.
        std::env::remove_var("EVERGEEK_CONSUMER_KEY");
        std::env::remove_var("EVERGEEK_CONSUMER_SECRET");
        assert!(matches!(consumer_key(), Err(EvergeekError::Config(_))));
        assert!(matches!(consumer_secret(), Err(EvergeekError::Config(_))));

        // Present → value returned.
        std::env::set_var("EVERGEEK_CONSUMER_KEY", "my-key");
        std::env::set_var("EVERGEEK_CONSUMER_SECRET", "my-secret");
        assert_eq!(consumer_key().unwrap(), "my-key");
        assert_eq!(consumer_secret().unwrap(), "my-secret");

        std::env::remove_var("EVERGEEK_CONSUMER_KEY");
        std::env::remove_var("EVERGEEK_CONSUMER_SECRET");
    }
}
