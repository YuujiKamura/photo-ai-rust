//! Claude CLI連携モジュール
//!
//! 解析処理:
//! - 1ステップ解析: 工種指定時、1回のAI呼び出しで画像認識と分類を実行
//! - 基本解析: 工種未指定時、画像認識のみ実行
//!
//! 共通ロジックは photo_ai_common から使用

use crate::error::{PhotoAiError, Result};
use crate::scanner::ImageInfo;
use crate::ai_provider::AiProvider;
use std::path::PathBuf;
use std::process::Command;

// 共通モジュールから型と関数をインポート
use photo_ai_common::{
    AnalysisResult, RawImageData, HierarchyMaster,
    build_step1_prompt, build_prompt_for_category,
    parse_step1_response as common_parse_step1,
    parse_single_step_response as common_parse_single_step,
};

/// Step1: 画像認識を実行
pub async fn analyze_batch_step1(
    images: &[ImageInfo],
    verbose: bool,
    provider: AiProvider,
) -> Result<Vec<RawImageData>> {
    // 元画像パスをそのまま使用（run_gemini_cliが内部でASCIIパスにコピーする）
    let image_paths: Vec<PathBuf> = images.iter().map(|img| img.path.clone()).collect();

    // 共通プロンプト生成を使用
    let image_meta: Vec<(&str, Option<&str>)> = images
        .iter()
        .map(|img| (img.file_name.as_str(), img.date.as_deref()))
        .collect();
    let step1_prompt = build_step1_prompt(&image_meta);

    let full_prompt = step1_prompt.replace('\n', " ").replace('"', "\\\"");

    if verbose {
        println!("  [Step1] プロンプト長: {} chars", full_prompt.len());
    }

    // AI CLI呼び出し（画像パスはrun_gemini_cliが@fileで付加する）
    let response = run_ai_cli(&full_prompt, Some(&image_paths), verbose, provider)?;

    if verbose {
        println!("  [Step1] レスポンス長: {} chars", response.len());
    }

    // 共通パーサーを使用
    parse_step1_response(&response)
}

/// 基本解析を実行（マスタなし）
pub async fn analyze_batch(
    images: &[ImageInfo],
    verbose: bool,
    provider: AiProvider,
) -> Result<Vec<AnalysisResult>> {
    // Step1のみ実行（マスタなし）
    let raw_data = analyze_batch_step1(images, verbose, provider).await?;

    // マスタなしの場合はStep1結果をそのまま変換
    let info_map: std::collections::HashMap<&str, &ImageInfo> = images
        .iter()
        .map(|img| (img.file_name.as_str(), img))
        .collect();

    let results = raw_data
        .iter()
        .map(|raw| {
            let img_info = info_map.get(raw.file_name.as_str());
            let file_path = img_info
                .map(|i| i.path.display().to_string())
                .unwrap_or_default();
            let date = img_info
                .and_then(|i| i.date.clone())
                .unwrap_or_default();

            AnalysisResult {
                file_name: raw.file_name.clone(),
                file_path,
                date,
                has_board: raw.has_board,
                detected_text: raw.detected_text.clone(),
                measurements: raw.measurements.clone(),
                description: raw.scene_description.clone(),
                photo_category: raw.photo_category.clone(),
                ..Default::default()
            }
        })
        .collect();

    Ok(results)
}

/// 1ステップ解析を実行（工種指定版）
///
/// 工種が既知の場合、1回のAI呼び出しで画像認識と分類を実行
pub async fn analyze_batch_single_step(
    images: &[ImageInfo],
    master: &HierarchyMaster,
    work_type: Option<&str>,
    variety: Option<&str>,
    photo_type: Option<&str>,
    verbose: bool,
    provider: AiProvider,
) -> Result<Vec<AnalysisResult>> {
    // 元画像パスをそのまま使用（run_gemini_cliが内部でASCIIパスにコピーする）
    let image_paths: Vec<PathBuf> = images.iter().map(|img| img.path.clone()).collect();

    // 画像メタデータ
    let image_meta: Vec<(&str, Option<&str>)> = images
        .iter()
        .map(|img| (img.file_name.as_str(), img.date.as_deref()))
        .collect();

    // 1ステップ解析プロンプト生成
    let single_step_prompt = build_prompt_for_category(&image_meta, master, work_type, variety, photo_type);

    // プロンプト構築（画像参照は run_gemini_cli が @file で付加する）
    let full_prompt = single_step_prompt.replace('\n', " ").replace('"', "\\\"");

    if verbose {
        println!("  [1ステップ解析] プロンプト長: {} chars", full_prompt.len());
    }

    // AI CLI呼び出し
    let response = run_ai_cli(&full_prompt, Some(&image_paths), verbose, provider)?;

    if verbose {
        println!("  [1ステップ解析] レスポンス長: {} chars", response.len());
    }

    // レスポンスをパース
    let mut results = parse_single_step_response(&response)?;

    // file_path と date を補完
    let info_map: std::collections::HashMap<&str, &ImageInfo> = images
        .iter()
        .map(|img| (img.file_name.as_str(), img))
        .collect();

    for result in &mut results {
        if let Some(img_info) = info_map.get(result.file_name.as_str()) {
            result.file_path = img_info.path.display().to_string();
            result.date = img_info.date.clone().unwrap_or_default();
        }
    }

    // マスタとの整合性チェック
    sanitize_classification(&mut results, master);

    Ok(results)
}

/// parse関数のラッパーマクロ
///
/// 共通パーサー関数をラップし、エラーメッセージを付与した関数を生成する
macro_rules! wrap_parse {
    ($fn_name:ident, $parser:path, $return_type:ty, $error_msg:expr) => {
        fn $fn_name(response: &str) -> Result<$return_type> {
            $parser(response)
                .map_err(|e| PhotoAiError::ApiParse(format!("{}: {}", $error_msg, e)))
        }
    };
}

// マクロを使用してparse関数を生成
wrap_parse!(parse_single_step_response, common_parse_single_step, Vec<AnalysisResult>, "1ステップ解析 JSONパースエラー");
wrap_parse!(parse_step1_response, common_parse_step1, Vec<RawImageData>, "Step1 JSONパースエラー");

// =============================================
// CLI固有の関数
// =============================================

/// CLI実行設定
struct CliConfig {
    /// コマンド名（Windowsではcmd /c経由で実行）
    command: String,
    /// コマンド引数
    args: Vec<String>,
    /// stdin経由で送るプロンプト
    stdin_prompt: Option<String>,
    /// 出力ファイルパス（Codex用）
    output_file: Option<PathBuf>,
    /// プロバイダ名（ログ用）
    provider_name: String,
    /// verbose フラグ
    verbose: bool,
}

/// CLI実行の共通処理
fn run_cli_command(config: CliConfig) -> Result<String> {
    use std::process::Stdio;
    #[cfg(not(windows))]
    use std::io::Write;

    if config.verbose {
        let prompt_len = config.stdin_prompt.as_ref().map(|p| p.len()).unwrap_or(0);
        println!("  [{}] prompt length: {}", config.provider_name, prompt_len);
    }

    // Windows + stdin piping: プロンプトをファイル経由で渡す
    // cmd /c経由だとstdinが正しく子プロセスに転送されない
    #[cfg(windows)]
    let stdin_file = if config.stdin_prompt.is_some() {
        let path = std::env::temp_dir().join(format!("photo-ai-stdin-{}.txt", std::process::id()));
        if let Some(prompt) = &config.stdin_prompt {
            std::fs::write(&path, prompt.as_bytes())
                .map_err(|e| PhotoAiError::ApiCall(format!("stdin一時ファイル作成エラー: {}", e)))?;
        }
        Some(path)
    } else {
        None
    };

    // コマンド構築
    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.arg("/c");
        // "gemini --yolo -o text < C:\Temp\prompt.txt" を1つのコマンド文字列として渡す
        let mut cmd_str = config.command.clone();
        for arg in &config.args {
            cmd_str.push(' ');
            cmd_str.push_str(arg);
        }
        if let Some(ref path) = stdin_file {
            cmd_str.push_str(&format!(" < {}", path.display()));
        }
        c.arg(&cmd_str);
        c
    };

    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = Command::new(&config.command);
        for arg in &config.args {
            c.arg(arg);
        }
        c
    };

    // stdin piping (non-Windows only)
    #[cfg(not(windows))]
    if config.stdin_prompt.is_some() {
        cmd.stdin(Stdio::piped());
    }

    #[allow(unused_mut)]
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| PhotoAiError::ApiCall(format!("{} CLI実行エラー: {}", config.provider_name, e)))?;

    // stdin書き込み (non-Windows)
    #[cfg(not(windows))]
    if let Some(prompt) = &config.stdin_prompt {
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(prompt.as_bytes())
                .map_err(|e| PhotoAiError::ApiCall(format!("{} CLI stdin書き込みエラー: {}", config.provider_name, e)))?;
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|e| PhotoAiError::ApiCall(format!("{} CLI実行エラー: {}", config.provider_name, e)))?;

    // stdin一時ファイル削除
    #[cfg(windows)]
    if let Some(ref path) = stdin_file {
        std::fs::remove_file(path).ok();
    }

    // エラーチェック
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stdout_tail = if stdout.is_empty() {
            String::new()
        } else {
            format!("\nstdout: {}", stdout)
        };
        return Err(PhotoAiError::ApiCall(format!(
            "{} CLI failed (code {:?}): {}{}",
            config.provider_name,
            output.status.code(),
            stderr,
            stdout_tail
        )));
    }

    // 出力取得（output_fileがあればファイルから、なければstdoutから）
    let response = if let Some(output_path) = &config.output_file {
        let content = std::fs::read_to_string(output_path)
            .map_err(|e| PhotoAiError::ApiCall(format!("{}出力読み込みエラー: {}", config.provider_name, e)))?;
        let _ = std::fs::remove_file(output_path);
        content
    } else {
        String::from_utf8_lossy(&output.stdout).to_string()
    };

    if config.verbose {
        let preview: String = response.chars().take(500).collect();
        println!("  [{}] response: {}", config.provider_name, preview);
    }

    Ok(response)
}


fn run_ai_cli(
    prompt: &str,
    image_paths: Option<&[PathBuf]>,
    verbose: bool,
    provider: AiProvider,
) -> Result<String> {
    match provider {
        AiProvider::Claude => run_claude_cli(prompt, verbose),
        AiProvider::Codex => run_codex_cli(prompt, image_paths, verbose),
        AiProvider::Gemini => run_gemini_cli(prompt, image_paths, verbose),
    }
}

fn run_codex_cli(prompt: &str, image_paths: Option<&[PathBuf]>, verbose: bool) -> Result<String> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let temp_dir = std::env::temp_dir();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let output_path = temp_dir.join(format!("photo-ai-codex-{}.txt", ts));

    // 引数構築
    let mut args = vec![
        "exec".to_string(),
        "--output-last-message".to_string(),
        output_path.display().to_string(),
        "-".to_string(),
    ];

    // 画像パス追加
    if let Some(paths) = image_paths {
        for path in paths {
            args.push("-i".to_string());
            args.push(path.display().to_string());
        }
    }

    let config = CliConfig {
        command: "codex".to_string(),
        args,
        stdin_prompt: Some(prompt.to_string()),
        output_file: Some(output_path),
        provider_name: "Codex".to_string(),
        verbose,
    };

    run_cli_command(config)
}

fn run_gemini_cli(prompt: &str, image_paths: Option<&[PathBuf]>, verbose: bool) -> Result<String> {
    // Gemini CLI @file構文はパスにスペースや日本語を含むと動作しない
    // 一時ディレクトリにコピーしてASCIIパスで参照する
    let temp_dir = if image_paths.is_some() {
        let dir = std::env::temp_dir().join(format!("photo-ai-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        Some(dir)
    } else {
        None
    };
    let full_prompt = if let Some(paths) = image_paths {
        let dir = temp_dir.as_ref().unwrap();
        let file_refs: Vec<String> = paths
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("jpg");
                let temp_name = format!("img_{:02}.{}", i, ext);
                let temp_path = dir.join(&temp_name);
                if let Err(e) = std::fs::copy(p, &temp_path) {
                    eprintln!("  Warning: failed to copy {}: {}", p.display(), e);
                }
                format!("@{}", temp_path.display().to_string().replace('\\', "/"))
            })
            .collect();
        format!("{} {}", file_refs.join(" "), prompt)
    } else {
        prompt.to_string()
    };

    let config = CliConfig {
        command: "gemini".to_string(),
        args: vec!["--yolo".to_string(), "-o".to_string(), "text".to_string()],
        stdin_prompt: Some(full_prompt),
        output_file: None,
        provider_name: "Gemini".to_string(),
        verbose,
    };

    let result = run_cli_command(config);
    // 一時ファイル削除
    if let Some(dir) = temp_dir {
        std::fs::remove_dir_all(&dir).ok();
    }
    result
}

fn run_claude_cli(prompt: &str, verbose: bool) -> Result<String> {
    // Claude CLIは常にstdin経由で送信（コマンドライン長制限を回避）
    let config = CliConfig {
        command: "claude".to_string(),
        args: vec!["--output-format".to_string(), "text".to_string()],
        stdin_prompt: Some(prompt.to_string()),
        output_file: None,
        provider_name: "Claude".to_string(),
        verbose,
    };

    run_cli_command(config)
}

fn sanitize_classification(results: &mut [AnalysisResult], master: &HierarchyMaster) {
    for result in results.iter_mut() {
        // remarks から階層を確定（撮影内容ベース）
        if !result.remarks.is_empty() {
            let mut candidates: Vec<_> = master.rows().iter()
                .filter(|row| row.remarks == result.remarks)
                .collect();

            if !candidates.is_empty() {
                if !result.photo_category.is_empty() {
                    let filtered: Vec<_> = candidates
                        .iter()
                        .copied()
                        .filter(|row| row.photo_type == result.photo_category)
                        .collect();
                    if !filtered.is_empty() {
                        candidates = filtered;
                    }
                }
                if !result.work_type.is_empty() {
                    let filtered: Vec<_> = candidates
                        .iter()
                        .copied()
                        .filter(|row| row.work_type == result.work_type)
                        .collect();
                    if !filtered.is_empty() {
                        candidates = filtered;
                    }
                }
                if !result.variety.is_empty() {
                    let filtered: Vec<_> = candidates
                        .iter()
                        .copied()
                        .filter(|row| row.variety == result.variety)
                        .collect();
                    if !filtered.is_empty() {
                        candidates = filtered;
                    }
                }
                if !result.subphase.is_empty() {
                    let filtered: Vec<_> = candidates
                        .iter()
                        .copied()
                        .filter(|row| row.subphase == result.subphase)
                        .collect();
                    if !filtered.is_empty() {
                        candidates = filtered;
                    }
                }

                if let Some(row) = candidates.first() {
                    result.photo_category = row.photo_type.clone();
                    result.work_type = row.work_type.clone();
                    result.variety = row.variety.clone();
                    result.subphase = row.subphase.clone();
                }
            }
        }

        // 未舗装部舗装工は自動選択しない（デフォルトは舗装打換え工）
        if result.work_type == "舗装工" && result.variety == "未舗装部舗装工" {
            result.variety = "舗装打換え工".to_string();
        }

        // 1) photoCategory (写真種別) と workType の整合
        if !result.photo_category.is_empty() && !result.work_type.is_empty() {
            let has_work = master.rows().iter().any(|row| {
                row.photo_type == result.photo_category && row.work_type == result.work_type
            });
            if !has_work {
                result.work_type.clear();
                result.variety.clear();
                result.subphase.clear();
                result.remarks.clear();
                continue;
            }
        }

        // 2) workType の存在チェック
        if !result.work_type.is_empty() {
            let work_types = master.get_work_types();
            if !work_types.contains(&result.work_type.as_str()) {
                result.work_type.clear();
                result.variety.clear();
                result.subphase.clear();
                result.remarks.clear();
                continue;
            }
        }

        // 3) variety の整合
        if !result.work_type.is_empty() && !result.variety.is_empty() {
            let has_variety = master.rows().iter().any(|row| {
                row.work_type == result.work_type
                    && row.variety == result.variety
                    && (result.photo_category.is_empty()
                        || row.photo_type == result.photo_category)
            });
            if !has_variety {
                result.variety.clear();
                result.subphase.clear();
                result.remarks.clear();
            }
        } else {
            // 工種が空のケース（安全管理写真、使用機械等）は remarks を保持
            result.variety.clear();
            result.subphase.clear();
        }

        // 4) subphase の整合
        if !result.work_type.is_empty() && !result.variety.is_empty() && !result.subphase.is_empty() {
            let has_detail = master.rows().iter().any(|row| {
                row.work_type == result.work_type
                    && row.variety == result.variety
                    && row.subphase == result.subphase
                    && (result.photo_category.is_empty()
                        || row.photo_type == result.photo_category)
            });
            if !has_detail {
                result.subphase.clear();
                result.remarks.clear();
            }
        } else if !result.work_type.is_empty() {
            // 工種ありで種別なし → subphaseクリア、remarksはマスタに存在すれば保持
            result.subphase.clear();
            let has_remarks_in_master = !result.remarks.is_empty() && master.rows().iter().any(|row| {
                row.work_type == result.work_type
                    && row.variety.is_empty()
                    && row.remarks == result.remarks
            });
            if !has_remarks_in_master {
                result.remarks.clear();
            }
        } else {
            // 工種なし（安全管理写真等） → subphaseのみクリア、remarksは保持
            result.subphase.clear();
        }

        // 5) remarks の整合（同一の photoCategory/work/var/subphase の行に存在する備考のみ許可）
        if !result.remarks.is_empty() {
            let has_remarks = master.rows().iter().any(|row| {
                row.remarks == result.remarks
                    && row.work_type == result.work_type
                    && row.variety == result.variety
                    && row.subphase == result.subphase
                    && (result.photo_category.is_empty()
                        || row.photo_type == result.photo_category)
            });
            if !has_remarks {
                result.remarks.clear();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_step1_response_with_json_block() {
        let response = r#"Here is the analysis:
```json
[
  {
    "fileName": "test.jpg",
    "hasBoard": true,
    "detectedText": "温度 160.4℃",
    "measurements": "160.4℃",
    "sceneDescription": "アスファルト舗装工事",
    "photoCategory": "品質管理"
  }
]
```
"#;
        let result = parse_step1_response(response).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_name, "test.jpg");
        assert!(result[0].has_board);
        assert_eq!(result[0].detected_text, "温度 160.4℃");
        assert_eq!(result[0].photo_category, "品質管理");
    }

    #[test]
    fn test_parse_step1_response_raw_json() {
        let response = r#"[{"fileName": "photo1.jpg", "hasBoard": false, "sceneDescription": "道路工事"}]"#;
        let result = parse_step1_response(response).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_name, "photo1.jpg");
        assert!(!result[0].has_board);
    }

    #[test]
    fn test_build_step1_prompt() {
        let images = vec![ImageInfo {
            path: PathBuf::from("test.jpg"),
            file_name: "test.jpg".to_string(),
            date: Some("2025-01-18".to_string()),
        }];
        let image_meta: Vec<(&str, Option<&str>)> = images
            .iter()
            .map(|img| (img.file_name.as_str(), img.date.as_deref()))
            .collect();
        let prompt = build_step1_prompt(&image_meta);
        assert!(prompt.contains("test.jpg"));
        assert!(prompt.contains("施工状況写真")); // PHOTO_CATEGORIESから
        assert!(prompt.contains("JSON配列のみ出力"));
    }

}
