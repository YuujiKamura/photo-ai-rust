//! エラー型定義

use thiserror::Error;

/// 共通エラー型
#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Config error: {0}")]
    Config(String),
}

/// Result型エイリアス
pub type Result<T> = std::result::Result<T, Error>;
