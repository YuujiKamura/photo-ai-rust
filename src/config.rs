use crate::error::{PhotoAiError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub api_key: Option<String>,
    pub model: String,
    pub max_image_size: u32,
    pub default_batch_size: usize,
    pub timeout_seconds: u64,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: Config = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Self::default_config())
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    pub fn config_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| PhotoAiError::Config("ホームディレクトリが見つかりません".into()))?;
        Ok(home.join(".config").join("photo-ai").join("config.json"))
    }

    fn default_config() -> Self {
        Self {
            api_key: None,
            model: "claude-sonnet-4-20250514".into(),
            max_image_size: 1568,  // Claude Vision推奨サイズ
            default_batch_size: 5,
            timeout_seconds: 120,
        }
    }

    #[allow(dead_code)]
    pub fn get_api_key(&self) -> Result<String> {
        // 環境変数を優先
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            return Ok(key);
        }

        self.api_key.clone().ok_or(PhotoAiError::MissingApiKey)
    }

    pub fn set_api_key(&mut self, key: String) -> Result<()> {
        self.api_key = Some(key);
        self.save()
    }
}
