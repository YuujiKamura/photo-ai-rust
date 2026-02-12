//! エラー型定義
//!
//! 各モジュールで定義されたサブエラーをここで合成する。
//! - ExportError:    export/mod.rs で定義
//! - HierarchyError: hierarchy/mod.rs で定義
//! - AnalyzerError:  analyzer.rs で定義

use std::path::PathBuf;
use thiserror::Error;

// サブエラーの re-export
pub use crate::export::ExportError;
pub use crate::hierarchy::HierarchyError;
pub use crate::analyzer::AnalyzerError;

/// 共通エラー型
#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// ファイルが見つからない
    #[error("File not found: {0}")]
    MissingFile(PathBuf),

    /// 設定形式が不正
    #[error("Invalid format for '{key}': {detail}")]
    InvalidFormat { key: String, detail: String },

    /// AI解析エラー
    #[error(transparent)]
    Analyzer(#[from] AnalyzerError),

    /// エクスポートエラー
    #[error(transparent)]
    Export(#[from] ExportError),

    /// 階層マスタエラー
    #[error(transparent)]
    Hierarchy(#[from] HierarchyError),
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
        let error = Error::MissingFile(PathBuf::from("/tmp/test"));
        let debug = format!("{:?}", error);
        assert!(debug.contains("MissingFile"));
    }

    #[test]
    fn test_error_missing_file() {
        let error = Error::MissingFile(PathBuf::from("/tmp/test.csv"));
        let display = format!("{}", error);
        assert!(display.contains("File not found"));
        assert!(display.contains("/tmp/test.csv"));
    }

    #[test]
    fn test_error_invalid_format() {
        let error = Error::InvalidFormat {
            key: "batch_size".to_string(),
            detail: "must be positive".to_string(),
        };
        let display = format!("{}", error);
        assert!(display.contains("batch_size"));
        assert!(display.contains("must be positive"));
    }

    #[test]
    fn test_error_from_analyzer_response_parse() {
        let error: Error = AnalyzerError::ResponseParse("test".to_string()).into();
        assert!(matches!(error, Error::Analyzer(AnalyzerError::ResponseParse(_))));
    }

    #[test]
    fn test_error_from_hierarchy_csv_parse() {
        let error: Error = HierarchyError::CsvParse {
            line: 5,
            detail: "test".to_string(),
        }.into();
        assert!(matches!(error, Error::Hierarchy(HierarchyError::CsvParse { .. })));
    }

    #[test]
    fn test_error_from_hierarchy_master_not_found() {
        let error: Error = HierarchyError::MasterNotFound("test".to_string()).into();
        assert!(matches!(error, Error::Hierarchy(HierarchyError::MasterNotFound(_))));
    }

    #[test]
    fn test_error_from_analyzer_tagger_empty() {
        let error: Error = AnalyzerError::TaggerEmpty.into();
        assert!(matches!(error, Error::Analyzer(AnalyzerError::TaggerEmpty)));
    }

    #[test]
    fn test_error_from_export_failed() {
        let error: Error = ExportError::Failed {
            format: "PDF".to_string(),
            detail: "test".to_string(),
        }.into();
        assert!(matches!(error, Error::Export(ExportError::Failed { .. })));
    }

    #[test]
    fn test_export_error_from_conversion() {
        let export_err = ExportError::Failed {
            format: "Excel".to_string(),
            detail: "test".to_string(),
        };
        let error: Error = export_err.into();
        assert!(matches!(error, Error::Export(_)));
    }

    #[test]
    fn test_hierarchy_error_from_conversion() {
        let hier_err = HierarchyError::MasterNotFound("test".to_string());
        let error: Error = hier_err.into();
        assert!(matches!(error, Error::Hierarchy(_)));
    }

    #[test]
    fn test_analyzer_error_from_conversion() {
        let analyzer_err = AnalyzerError::ResponseParse("test".to_string());
        let error: Error = analyzer_err.into();
        assert!(matches!(error, Error::Analyzer(_)));
    }
}
