use std::env;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Debug, Deserialize, Serialize)]
struct AuthConfig {
    api_key: String,
}

pub fn config_path() -> Result<PathBuf, AppError> {
    if let Ok(custom_path) = env::var("PEXELS_AGENT_CONFIG_PATH") {
        return Ok(PathBuf::from(custom_path));
    }

    if let Ok(config_home) = env::var("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(config_home)
            .join("pexels-agent")
            .join("config.json"));
    }

    let home = env::var("HOME").map(PathBuf::from).map_err(|_| {
        AppError::message("HOME is not set and no config path override was provided")
    })?;

    Ok(home
        .join(".config")
        .join("pexels-agent")
        .join("config.json"))
}

pub fn load_stored_api_key() -> Result<Option<String>, AppError> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(None);
    }

    let payload: AuthConfig = serde_json::from_str(&fs::read_to_string(path)?)?;
    if payload.api_key.trim().is_empty() {
        return Ok(None);
    }

    Ok(Some(payload.api_key))
}

pub fn save_api_key(api_key: &str) -> Result<PathBuf, AppError> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_string_pretty(&AuthConfig {
        api_key: api_key.to_owned(),
    })?;
    write_secret_atomic(&path, contents.as_bytes())?;
    Ok(path)
}

pub fn remove_stored_api_key() -> Result<bool, AppError> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(false);
    }
    fs::remove_file(path)?;
    Ok(true)
}

fn write_secret_atomic(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "config".to_owned());
    let tmp = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));

    let _ = fs::remove_file(&tmp);

    let mut opts = fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }

    let write_result = (|| -> Result<(), AppError> {
        let mut file = opts.open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    })();

    if let Err(error) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(error);
    }

    if let Err(error) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(error.into());
    }
    Ok(())
}
