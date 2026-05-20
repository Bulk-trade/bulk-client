use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const DEFAULT_API_URL: &str = "http://localhost:12000/api/v1";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CliConfig {
    pub api_url: Option<String>,
}

impl CliConfig {
    pub fn load() -> eyre::Result<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn save(&self) -> eyre::Result<()> {
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(self)?;
        fs::write(path, raw)?;
        Ok(())
    }
}

pub fn resolve_api_url(cli_flag: Option<&str>, config: &CliConfig) -> String {
    if let Some(url) = cli_flag {
        return url.trim_end_matches('/').to_string();
    }
    if let Some(url) = config.api_url.as_deref() {
        return url.trim_end_matches('/').to_string();
    }
    DEFAULT_API_URL.to_string()
}

pub fn config_path() -> eyre::Result<PathBuf> {
    let home = std::env::var("HOME").map_err(|_| eyre::eyre!("HOME is not set"))?;
    Ok(PathBuf::from(home).join(".bulk-cli").join("config.json"))
}
