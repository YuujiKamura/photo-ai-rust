//! Claude CLI連携モジュール
//!
//! 2段階解析処理:
//! - Step1 (Vision): 画像から生データを抽出（OCR、数値、シーン説明）
//! - Step2 (Text): 階層マスタとの照合で分類
//!
//! 共通ロジックは photo_ai_common から使用

use crate::error::{PhotoAiError, Result};
use crate::scanner::ImageInfo;
use std::path::PathBuf;
use std::process::Command;

// 共通モジュールから型と関数をインポート
use photo_ai_common::{
    AnalysisResult, RawImageData, Step2Result, HierarchyMaster, ImageMeta,
    build_step1_prompt, build_step2_prompt,
    parse_step1_response as common_parse_step1,
    parse_step2_response as common_parse_step2,
    detect_work_types, merge_results as common_merge_results,
};

/// Step1: 画像認識を実行
pub async fn analyze_batch_step1(images: &[ImageInfo], verbose: bool) -> Result<Vec<RawImageData>> {
    // 画像をtemp-imagesにコピー
    let temp_dir = get_temp_dir()?;
    let local_paths = copy_to_temp(images, &temp_dir)?;

    // プロンプト構築
    let image_list = local_paths
        .iter()
        .map(|p| p.display().to_string().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join(", ");

    // 共通プロンプト生成を使用
    let image_meta: Vec<(&str, Option<&str>)> = images
        .iter()
        .map(|img| (img.file_name.as_str(), img.date.as_deref()))
        .collect();
    let step1_prompt = build_step1_prompt(&image_meta);

    // プロンプト構築（改行をスペースに置換してcmd経由で渡す）
    let raw_prompt = format!(
        "Read the following image files and analyze them: {}\n\n{}",
        image_list, step1_prompt
    );
    let full_prompt = raw_prompt.replace('\n', " ").replace('"', "\\\"");

    if verbose {
        println!("  [Step1] プロンプト長: {} chars", full_prompt.len());
    }

    // Claude CLI呼び出し
    let response = run_claude_cli(&full_prompt, verbose)?;

    if verbose {
        println!("  [Step1] レスポンス長: {} chars", response.len());
    }

    // 共通パーサーを使用
    parse_step1_response(&response)
}

/// Step2: マスタ照合を実行
pub async fn analyze_batch_step2(
    raw_data: &[RawImageData],
    master: &HierarchyMaster,
    verbose: bool,
) -> Result<Vec<Step2Result>> {
    // 共通プロンプト生成を使用
    let step2_prompt = build_step2_prompt(raw_data, master);
    let full_prompt = step2_prompt.replace('\n', " ").replace('"', "\\\"");

    if verbose {
        println!("  [Step2] プロンプト長: {} chars", full_prompt.len());
    }

    // Claude CLI呼び出し（画像なし）
    let response = run_claude_cli(&full_prompt, verbose)?;

    if verbose {
        println!("  [Step2] レスポンス長: {} chars", response.len());
    }

    // 共通パーサーを使用
    parse_step2_response(&response)
}

/// Step1とStep2の結果をマージ
pub fn merge_results(
    raw_data: &[RawImageData],
    step2_results: &[Step2Result],
    images: &[ImageInfo],
) -> Vec<AnalysisResult> {
    // ImageInfoからImageMetaへ変換
    let image_metas: Vec<ImageMeta> = images
        .iter()
        .map(|img| ImageMeta {
            file_name: img.file_name.clone(),
            file_path: img.path.display().to_string(),
            date: img.date.clone().unwrap_or_default(),
        })
        .collect();

    // 共通マージ関数を使用
    let common_results = common_merge_results(raw_data, step2_results, &image_metas);

    // photo_ai_common::AnalysisResult から crate::analyzer::AnalysisResult へ変換
    common_results
        .into_iter()
        .map(|r| AnalysisResult {
            file_name: r.file_name,
            file_path: r.file_path,
            date: r.date,
            work_type: r.work_type,
            variety: r.variety,
            detail: r.detail,
            station: r.station,
            remarks: r.remarks,
            description: r.description,
            has_board: r.has_board,
            detected_text: r.detected_text,
            measurements: r.measurements,
            photo_category: r.photo_category,
            reasoning: r.reasoning,
        })
        .collect()
}

/// 2段階解析を実行（後方互換性のため維持）
pub async fn analyze_batch(images: &[ImageInfo], verbose: bool) -> Result<Vec<AnalysisResult>> {
    // Step1のみ実行（マスタなし）
    let raw_data = analyze_batch_step1(images, verbose).await?;

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

/// 2段階解析を実行（マスタあり）
pub async fn analyze_batch_with_master(
    images: &[ImageInfo],
    master: &HierarchyMaster,
    verbose: bool,
) -> Result<Vec<AnalysisResult>> {
    // Step1: 画像認識
    if verbose {
        println!("  Step1: 画像認識開始...");
    }
    let raw_data = analyze_batch_step1(images, verbose).await?;
    if verbose {
        println!("  Step1: 完了 ({}件)", raw_data.len());
    }

    // Step1結果から工種を自動判定してマスタをフィルタ（共通関数を使用）
    let detected_types = detect_work_types(&raw_data);
    let filtered_master = if detected_types.is_empty() {
        if verbose {
            println!("  工種判定: 該当なし → 全マスタ使用 ({}件)", master.rows().len());
        }
        master.clone()
    } else {
        let filtered = master.filter_by_work_types(&detected_types);
        if verbose {
            println!("  工種判定: {:?} → マスタ絞込み ({}件 → {}件)",
                detected_types, master.rows().len(), filtered.rows().len());
        }
        filtered
    };

    // Step2: マスタ照合（フィルタ済みマスタを使用）
    if verbose {
        println!("  Step2: マスタ照合開始...");
    }
    let step2_results = analyze_batch_step2(&raw_data, &filtered_master, verbose).await?;
    if verbose {
        println!("  Step2: 完了 ({}件)", step2_results.len());
    }

    // 結果マージ
    let results = merge_results(&raw_data, &step2_results, images);
    Ok(results)
}

// =============================================
// CLI固有の関数
// =============================================

fn get_temp_dir() -> Result<PathBuf> {
    let temp_dir = std::env::current_dir()?.join("temp-images");
    std::fs::create_dir_all(&temp_dir)?;
    Ok(temp_dir)
}

fn copy_to_temp(images: &[ImageInfo], temp_dir: &std::path::Path) -> Result<Vec<PathBuf>> {
    let mut local_paths = Vec::new();

    for img in images {
        let dest = temp_dir.join(&img.file_name);
        std::fs::copy(&img.path, &dest)?;
        // 絶対パスに変換
        let abs_path = std::fs::canonicalize(&dest)?;
        local_paths.push(abs_path);
    }

    Ok(local_paths)
}

fn run_claude_cli(prompt: &str, verbose: bool) -> Result<String> {
    // Claude CLI呼び出し（Windowsではcmd /c経由）
    #[cfg(windows)]
    let output = Command::new("cmd")
        .args(["/c", "claude", "-p", prompt, "--output-format", "text"])
        .output()
        .map_err(|e| PhotoAiError::ApiCall(format!("Claude CLI実行エラー: {}", e)))?;

    #[cfg(not(windows))]
    let output = Command::new("claude")
        .args(["-p", prompt, "--output-format", "text"])
        .output()
        .map_err(|e| PhotoAiError::ApiCall(format!("Claude CLI実行エラー: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PhotoAiError::ApiCall(format!(
            "Claude CLI failed (code {:?}): {}",
            output.status.code(),
            stderr
        )));
    }

    let response = String::from_utf8_lossy(&output.stdout).to_string();

    if verbose {
        let preview: String = response.chars().take(500).collect();
        println!("  レスポンス: {}", preview);
    }

    Ok(response)
}

/// Step1レスポンスをパース（共通パーサーをラップ）
fn parse_step1_response(response: &str) -> Result<Vec<RawImageData>> {
    common_parse_step1(response)
        .map_err(|e| PhotoAiError::ApiParse(format!("Step1 JSONパースエラー: {}", e)))
}

/// Step2レスポンスをパース（共通パーサーをラップ）
fn parse_step2_response(response: &str) -> Result<Vec<Step2Result>> {
    common_parse_step2(response)
        .map_err(|e| PhotoAiError::ApiParse(format!("Step2 JSONパースエラー: {}", e)))
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
    fn test_parse_step2_response() {
        let response = r#"```json
[
  {
    "fileName": "test.jpg",
    "workType": "舗装工",
    "variety": "舗装打換え工",
    "detail": "表層工",
    "station": "No.10",
    "description": "舗設状況"
  }
]
```"#;
        let result = parse_step2_response(response).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].work_type, "舗装工");
        assert_eq!(result[0].variety, "舗装打換え工");
        assert_eq!(result[0].detail, "表層工");
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
        assert!(prompt.contains("到着温度"));
        assert!(prompt.contains("JSON配列のみ出力"));
    }

    #[test]
    fn test_merge_results_with_image_info() {
        let raw_data = vec![RawImageData {
            file_name: "test.jpg".to_string(),
            has_board: true,
            detected_text: "温度測定".to_string(),
            measurements: "160℃".to_string(),
            scene_description: "舗装工事".to_string(),
            photo_category: "到着温度".to_string(),
        }];

        let step2_results = vec![Step2Result {
            file_name: "test.jpg".to_string(),
            work_type: "舗装工".to_string(),
            variety: "舗装打換え工".to_string(),
            detail: "表層工".to_string(),
            station: "No.10".to_string(),
            remarks: "備考".to_string(),
            description: "舗設状況".to_string(),
            reasoning: "温度測定のため".to_string(),
        }];

        let images = vec![ImageInfo {
            path: PathBuf::from("/path/to/test.jpg"),
            file_name: "test.jpg".to_string(),
            date: Some("2025-01-18".to_string()),
        }];

        let results = merge_results(&raw_data, &step2_results, &images);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_name, "test.jpg");
        assert_eq!(results[0].work_type, "舗装工");
        assert_eq!(results[0].variety, "舗装打換え工");
        assert!(results[0].has_board);
        assert_eq!(results[0].detected_text, "温度測定");
        assert_eq!(results[0].date, "2025-01-18");
    }
}
