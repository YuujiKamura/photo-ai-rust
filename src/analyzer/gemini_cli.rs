//! Gemini CLI連携モジュール
//!
//! 解析処理:
//! - 1ステップ解析: 工種指定時、1回のAI呼び出しで画像認識と分類を実行
//! - 基本解析: 工種未指定時、画像認識のみ実行
//!
//! 共通ロジックは photo_ai_common から使用

use crate::engine;
use crate::error::{PhotoAiError, Result};
use crate::scanner::ImageInfo;
use std::path::Path;

// 共通モジュールから型と関数をインポート
use photo_ai_common::{
    AnalysisResult, RawImageData, HierarchyMaster,
    build_step1_prompt, build_prompt_for_category,
};
use photo_ai_common::domain::{Matched, Photo, Raw};

/// `analyze_batch_single_step_typed` の返り値
///
/// `photo_category` が既知ラベルで埋まっているレコードは `Photo<Matched>` に昇格され、
/// 未知ラベル・空ラベルの場合は `AnalysisResult` のまま `unmatched` に残る。
/// 型で「分類済」と「未分類」を分離することでゴミ混入を防ぐ。
pub struct ClassifiedBatch {
    pub matched: Vec<Photo<Matched>>,
    pub unmatched: Vec<AnalysisResult>,
}

/// `TryFrom<AnalysisResult> for Photo<Matched>` を用いて結果を分類する純粋ヘルパー
///
/// 相対順序は各グループ（matched / unmatched）内で保持される。
pub(crate) fn partition_classified(results: Vec<AnalysisResult>) -> ClassifiedBatch {
    let mut matched = Vec::new();
    let mut unmatched = Vec::new();
    for ar in results {
        match Photo::<Matched>::try_from(ar) {
            Ok(photo) => matched.push(photo),
            Err(original) => unmatched.push(original),
        }
    }
    ClassifiedBatch { matched, unmatched }
}

/// Step1: 画像認識を実行
pub async fn analyze_batch_step1(
    images: &[ImageInfo],
    verbose: bool,
) -> Result<Vec<RawImageData>> {
    let folder = images
        .first()
        .and_then(|img| img.path.parent())
        .ok_or_else(|| PhotoAiError::FolderNotFound("画像フォルダを特定できません".to_string()))?;

    // 共通プロンプト生成を使用
    let image_meta: Vec<(&str, Option<&str>)> = images
        .iter()
        .map(|img| (img.file_name.as_str(), img.date.as_deref()))
        .collect();
    let step1_prompt = build_step1_prompt(&image_meta);

    if verbose {
        println!("  [Step1] プロンプト長: {} chars", step1_prompt.len());
    }

    if verbose {
        println!("  [Step1] photo-analysis-engine 呼び出し");
    }

    engine::run_step1(folder)
}

/// 基本解析を実行（マスタなし、TypeState 版）
///
/// Step1 の出力を `Photo<Raw>` としてそのまま返す。呼出元が型安全な
/// パイプラインを組み立てたい場合はこちらを使う。
pub async fn analyze_batch_raw(
    images: &[ImageInfo],
    verbose: bool,
) -> Result<Vec<Photo<Raw>>> {
    // Step1のみ実行（マスタなし）
    let raw_data = analyze_batch_step1(images, verbose).await?;

    // ImageInfo から file_path / date を補完して Photo<Raw> に組み立てる
    let info_map: std::collections::HashMap<&str, &ImageInfo> = images
        .iter()
        .map(|img| (img.file_name.as_str(), img))
        .collect();

    let photos = raw_data
        .into_iter()
        .map(|raw| {
            let img_info = info_map.get(raw.file_name.as_str());
            let file_path = img_info
                .map(|i| i.path.display().to_string())
                .unwrap_or_default();
            let date = img_info
                .and_then(|i| i.date.clone())
                .unwrap_or_default();
            Photo::<Raw>::from_raw_data(raw, file_path, date)
        })
        .collect();

    Ok(photos)
}

// `analyze_batch` は Step 7b で削除。
// 呼出元（analyzer::analyze_images）は analyze_batch_raw に直接委譲するように変更済。
// 外部 API `analyze_images` の契約（`Vec<AnalysisResult>`）は維持されている。

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
    let folder = images
        .first()
        .and_then(|img| img.path.parent())
        .ok_or_else(|| PhotoAiError::FolderNotFound("画像フォルダを特定できません".to_string()))?;
    let master_csv = std::env::temp_dir().join(format!("photo-ai-master-{}.csv", std::process::id()));
    write_master_csv(master, &master_csv)?;

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

    if verbose {
        println!("  [1ステップ解析] photo-analysis-engine 呼び出し");
    }

    let mut results = engine::run_single_step(
        folder,
        &master_csv,
        work_type,
        variety,
        photo_type,
    )?;
    let _ = std::fs::remove_file(&master_csv);

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

/// `analyze_batch_single_step` の型付き版
///
/// 既存の `analyze_batch_single_step` に委譲し、結果を `ClassifiedBatch` に畳み込む。
/// シグネチャは `Result<Vec<AnalysisResult>, _>` → `Result<ClassifiedBatch, _>` 以外
/// 完全一致。呼出元は `matched` を型安全に扱い、`unmatched` は既存の AnalysisResult
/// パスにフォールバックできる。
pub async fn analyze_batch_single_step_typed(
    images: &[ImageInfo],
    master: &HierarchyMaster,
    work_type: Option<&str>,
    variety: Option<&str>,
    photo_type: Option<&str>,
    verbose: bool,
) -> Result<ClassifiedBatch> {
    let results =
        analyze_batch_single_step(images, master, work_type, variety, photo_type, verbose).await?;
    Ok(partition_classified(results))
}

fn write_master_csv(master: &HierarchyMaster, path: &Path) -> Result<()> {
    let mut out = String::from("費目,写真区分,工種,種別,細別,撮影内容,検索パターン\n");
    for row in master.rows() {
        let line = [
            row.photo_division.as_str(),
            row.photo_type.as_str(),
            row.work_type.as_str(),
            row.variety.as_str(),
            row.subphase.as_str(),
            row.remarks.as_str(),
            row.search_patterns.as_str(),
        ]
        .into_iter()
        .map(csv_escape)
        .collect::<Vec<_>>()
        .join(",");
        out.push_str(&line);
        out.push('\n');
    }

    std::fs::write(path, out).map_err(PhotoAiError::Io)
}

fn csv_escape(value: &str) -> String {
    let escaped = value.replace('"', "\"\"");
    format!("\"{escaped}\"")
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
    use photo_ai_common::parse_step1_response;
    use std::path::PathBuf;

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

    // === partition_classified ===

    fn known_ar(name: &str) -> AnalysisResult {
        AnalysisResult {
            file_name: name.to_string(),
            photo_category: "施工状況写真".to_string(),
            work_type: "舗装工".to_string(),
            variety: "舗装打換え工".to_string(),
            subphase: "表層工".to_string(),
            remarks: "舗設状況".to_string(),
            ..Default::default()
        }
    }

    fn unknown_ar(name: &str) -> AnalysisResult {
        AnalysisResult {
            file_name: name.to_string(),
            photo_category: "不明区分".to_string(),
            ..Default::default()
        }
    }

    fn empty_category_ar(name: &str) -> AnalysisResult {
        AnalysisResult {
            file_name: name.to_string(),
            photo_category: String::new(),
            ..Default::default()
        }
    }

    #[test]
    fn partition_classified_separates_known_from_unknown() {
        let input = vec![
            known_ar("ok1.jpg"),
            unknown_ar("bad1.jpg"),
            empty_category_ar("empty.jpg"),
            known_ar("ok2.jpg"),
        ];
        let batch = partition_classified(input);
        assert_eq!(batch.matched.len(), 2);
        assert_eq!(batch.unmatched.len(), 2);
        assert_eq!(batch.matched[0].file_name(), "ok1.jpg");
        assert_eq!(batch.matched[1].file_name(), "ok2.jpg");
        assert_eq!(batch.unmatched[0].file_name, "bad1.jpg");
        assert_eq!(batch.unmatched[1].file_name, "empty.jpg");
    }

    #[test]
    fn partition_classified_preserves_order_within_groups() {
        let input = vec![
            known_ar("a.jpg"),
            known_ar("b.jpg"),
            unknown_ar("x.jpg"),
            known_ar("c.jpg"),
            unknown_ar("y.jpg"),
            unknown_ar("z.jpg"),
        ];
        let batch = partition_classified(input);
        // matched の相対順序は a → b → c
        let matched_names: Vec<&str> = batch.matched.iter().map(|p| p.file_name()).collect();
        assert_eq!(matched_names, vec!["a.jpg", "b.jpg", "c.jpg"]);
        // unmatched の相対順序は x → y → z
        let unmatched_names: Vec<&str> =
            batch.unmatched.iter().map(|ar| ar.file_name.as_str()).collect();
        assert_eq!(unmatched_names, vec!["x.jpg", "y.jpg", "z.jpg"]);
    }

    #[test]
    fn partition_classified_empty_input_returns_empty_batch() {
        let batch = partition_classified(Vec::new());
        assert!(batch.matched.is_empty());
        assert!(batch.unmatched.is_empty());
    }
}
