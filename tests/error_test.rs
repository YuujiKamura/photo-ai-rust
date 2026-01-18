//! エラーケーステスト
//!
//! 各種エラー条件でのエラーハンドリングを検証

use photo_ai_rust::error::PhotoAiError;
use photo_ai_rust::scanner;
use std::path::Path;
use tempfile::tempdir;

/// 存在しないフォルダをスキャンした場合
#[test]
fn test_scan_nonexistent_folder() {
    let result = scanner::scan_folder(Path::new("/nonexistent/path/12345"));
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(matches!(err, PhotoAiError::FolderNotFound(_)));
}

/// 空のフォルダをスキャンした場合
#[test]
fn test_scan_empty_folder() {
    let dir = tempdir().expect("Failed to create temp dir");
    let result = scanner::scan_folder(dir.path());

    // 空フォルダはエラーではなく空のVecを返す
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

/// 画像のないフォルダをスキャンした場合
#[test]
fn test_scan_folder_no_images() {
    let dir = tempdir().expect("Failed to create temp dir");

    // テキストファイルのみ作成
    std::fs::write(dir.path().join("test.txt"), "hello").unwrap();
    std::fs::write(dir.path().join("data.json"), "{}").unwrap();

    let result = scanner::scan_folder(dir.path());
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

/// PhotoAiErrorのDisplay実装確認
#[test]
fn test_error_display() {
    let errors = vec![
        PhotoAiError::Config("テスト設定エラー".to_string()),
        PhotoAiError::FileNotFound("test.jpg".to_string()),
        PhotoAiError::FolderNotFound("/path/to/folder".to_string()),
        PhotoAiError::ApiCall("API呼び出し失敗".to_string()),
        PhotoAiError::PdfGeneration("PDF生成エラー".to_string()),
        PhotoAiError::ExcelGeneration("Excel生成エラー".to_string()),
        PhotoAiError::InvalidMaster("不正なマスタ".to_string()),
        PhotoAiError::NoImagesFound("フォルダ".to_string()),
    ];

    for err in errors {
        let display = format!("{}", err);
        assert!(!display.is_empty(), "エラーメッセージが空: {:?}", err);
    }
}

/// MissingApiKeyエラーのメッセージ確認
#[test]
fn test_missing_api_key_message() {
    let err = PhotoAiError::MissingApiKey;
    let display = format!("{}", err);

    assert!(display.contains("APIキー"));
    assert!(display.contains("photo-ai config"));
}

/// エラーのDebug実装確認
#[test]
fn test_error_debug() {
    let err = PhotoAiError::Config("テスト".to_string());
    let debug = format!("{:?}", err);

    assert!(debug.contains("Config"));
    assert!(debug.contains("テスト"));
}

/// IOエラーからの変換
#[test]
fn test_io_error_conversion() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let err: PhotoAiError = io_err.into();

    assert!(matches!(err, PhotoAiError::Io(_)));
    let display = format!("{}", err);
    assert!(display.contains("IO"));
}

/// JSONエラーからの変換
#[test]
fn test_json_error_conversion() {
    let json_err = serde_json::from_str::<serde_json::Value>("{ invalid }").unwrap_err();
    let err: PhotoAiError = json_err.into();

    assert!(matches!(err, PhotoAiError::JsonParse(_)));
}

/// common::Errorからの変換
#[test]
fn test_common_error_conversion() {
    let common_err = photo_ai_common::Error::Parse("パースエラー".to_string());
    let err: PhotoAiError = common_err.into();

    assert!(matches!(err, PhotoAiError::Common(_)));
}

/// エラーチェーン（透過的エラー）
#[test]
fn test_error_chain_transparent() {
    let common_err = photo_ai_common::Error::Config("設定エラー".to_string());
    let err: PhotoAiError = common_err.into();

    // 透過的エラーなのでメッセージがそのまま表示される
    let display = format!("{}", err);
    assert!(display.contains("設定エラー") || display.contains("Config"));
}
