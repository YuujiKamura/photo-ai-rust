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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_io() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let error = Error::Io(io_error);
        let display = format!("{}", error);
        assert!(display.contains("IO error"));
        assert!(display.contains("file not found"));
    }

    #[test]
    fn test_error_display_json() {
        let json_str = "invalid json";
        let json_error = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
        let error = Error::Json(json_error);
        let display = format!("{}", error);
        assert!(display.contains("JSON error"));
    }

    #[test]
    fn test_error_display_config() {
        let error = Error::Config("設定ファイルが見つかりません".to_string());
        let display = format!("{}", error);
        assert_eq!(display, "Config error: 設定ファイルが見つかりません");
    }

    #[test]
    fn test_error_from_io() {
        let io_error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let error: Error = io_error.into();
        assert!(matches!(error, Error::Io(_)));
    }

    #[test]
    fn test_error_from_json() {
        let json_error = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        let error: Error = json_error.into();
        assert!(matches!(error, Error::Json(_)));
    }

    #[test]
    fn test_error_debug() {
        let error = Error::Config("テスト".to_string());
        let debug = format!("{:?}", error);
        assert!(debug.contains("Config"));
        assert!(debug.contains("テスト"));
    }
}
