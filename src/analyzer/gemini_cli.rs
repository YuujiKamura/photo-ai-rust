//! Gemini CLI連携モジュール
//!
//! 解析処理:
//! - 1ステップ解析: 工種指定時、1回のAI呼び出しで画像認識と分類を実行
//! - 基本解析: 工種未指定時、画像認識のみ実行
//!
//! 共通ロジックは photo_ai_common から使用

use crate::error::{PhotoAiError, Result};
use crate::scanner::ImageInfo;
use std::path::PathBuf;

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
) -> Result<Vec<RawImageData>> {
    // 元画像パスをそのまま使用（cli-ai-analyzerが内部でASCIIパスにコピーする）
    let image_paths: Vec<PathBuf> = images.iter().map(|img| img.path.clone()).collect();

    // 共通プロンプト生成を使用
    let image_meta: Vec<(&str, Option<&str>)> = images
        .iter()
        .map(|img| (img.file_name.as_str(), img.date.as_deref()))
        .collect();
    let step1_prompt = build_step1_prompt(&image_meta);

    if verbose {
        println!("  [Step1] プロンプト長: {} chars", step1_prompt.len());
    }

    // cli-ai-analyzer経由でGemini CLI呼び出し
    let options = cli_ai_analyzer::AnalyzeOptions::default();
    let response = cli_ai_analyzer::analyze(&step1_prompt, &image_paths, options)
        .map_err(|e| PhotoAiError::ApiCall(format!("Gemini解析エラー: {}", e)))?;

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
) -> Result<Vec<AnalysisResult>> {
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
) -> Result<Vec<AnalysisResult>> {
    // 元画像パスをそのまま使用（cli-ai-analyzerが内部でASCIIパスにコピーする）
    let image_paths: Vec<PathBuf> = images.iter().map(|img| img.path.clone()).collect();

    // 画像メタデータ
    let image_meta: Vec<(&str, Option<&str>)> = images
        .iter()
        .map(|img| (img.file_name.as_str(), img.date.as_deref()))
        .collect();

    // 1ステップ解析プロンプト生成
    let single_step_prompt = build_prompt_for_category(&image_meta, master, work_type, variety, photo_type);

    if verbose {
        println!("  [1ステップ解析] プロンプト長: {} chars", single_step_prompt.len());
    }

    // cli-ai-analyzer経由でGemini CLI呼び出し
    let options = cli_ai_analyzer::AnalyzeOptions::default();
    let response = cli_ai_analyzer::analyze(&single_step_prompt, &image_paths, options)
        .map_err(|e| PhotoAiError::ApiCall(format!("Gemini解析エラー: {}", e)))?;

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
