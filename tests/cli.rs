//! CLI E2Eテスト
//!
//! 実際のバイナリを実行してCLIの動作を検証

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// テスト用のコマンドを取得
fn cmd() -> Command {
    Command::cargo_bin("photo-ai-rust").unwrap()
}

// =============================================
// ヘルプ・基本コマンド
// =============================================

#[test]
fn test_help_displays_usage() {
    cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("工事写真AI解析"))
        .stdout(predicate::str::contains("analyze"))
        .stdout(predicate::str::contains("export"))
        .stdout(predicate::str::contains("run"));
}

#[test]
fn test_subcommand_help_analyze() {
    cmd()
        .args(["analyze", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("写真フォルダを解析"));
}

#[test]
fn test_subcommand_help_export() {
    cmd()
        .args(["export", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PDF"));
}

#[test]
fn test_subcommand_help_normalize() {
    cmd()
        .args(["normalize", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("正規化"));
}

#[test]
fn test_config_show() {
    cmd()
        .args(["config", "--show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("設定:"))
        .stdout(predicate::str::contains("モデル"));
}

#[test]
fn test_cache_info() {
    let temp = TempDir::new().unwrap();
    cmd()
        .args(["cache", "--info", "-f", temp.path().to_str().unwrap()])
        .assert()
        .success();
}

// =============================================
// エラーケース
// =============================================

#[test]
fn test_analyze_nonexistent_folder() {
    cmd()
        .args(["analyze", "/nonexistent/folder/path", "--work-type", "舗装工"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("FolderNotFound"));
}

#[test]
fn test_export_invalid_json() {
    let temp = TempDir::new().unwrap();
    let invalid_json = temp.path().join("invalid.json");
    fs::write(&invalid_json, "{ invalid json }").unwrap();

    cmd()
        .args(["export", invalid_json.to_str().unwrap()])
        .assert()
        .failure();
}

#[test]
fn test_export_nonexistent_file() {
    cmd()
        .args(["export", "/nonexistent/result.json"])
        .assert()
        .failure();
}

#[test]
fn test_analyze_empty_folder() {
    let temp = TempDir::new().unwrap();

    cmd()
        .args(["analyze", temp.path().to_str().unwrap(), "--work-type", "舗装工"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("NoImagesFound"));
}

// =============================================
// 正常系（AI呼び出しなし）
// =============================================

#[test]
fn test_normalize_valid_json() {
    let temp = TempDir::new().unwrap();
    let json_file = temp.path().join("results.json");

    // 最小限の有効なJSON（camelCase形式）
    let valid_json = r#"[
        {
            "fileName": "test.jpg",
            "filePath": "/path/to/test.jpg",
            "date": "2024-01-01",
            "hasBoard": true,
            "detectedText": "テスト",
            "photoCategory": "施工状況写真",
            "workType": "舗装工",
            "variety": "",
            "detail": "",
            "measurements": ""
        }
    ]"#;
    fs::write(&json_file, valid_json).unwrap();

    cmd()
        .args(["normalize", json_file.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn test_cache_clear() {
    let temp = TempDir::new().unwrap();
    cmd()
        .args(["cache", "--clear", "-f", temp.path().to_str().unwrap()])
        .assert()
        .success();
}

// =============================================
// フラグ組み合わせ
// =============================================

#[test]
fn test_verbose_flag() {
    cmd()
        .args(["--verbose", "config", "--show"])
        .assert()
        .success();
}

#[test]
fn test_ai_provider_flag() {
    cmd()
        .args(["--ai-provider", "gemini", "config", "--show"])
        .assert()
        .success();
}

#[test]
fn test_invalid_ai_provider() {
    cmd()
        .args(["--ai-provider", "invalid_provider", "config", "--show"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid"));
}
