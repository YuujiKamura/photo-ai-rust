//! マスタ照合モジュール
//!
//! 既存の constructionHierarchyData.ts と同じJSON構造を読み込み、
//! AI解析結果と照合して工種・種別・細別を特定する。
//!
//! ## 階層構造
//! ```text
//! 直接工事費
//!   └─ 写真区分（品質管理写真、施工状況写真...）
//!       └─ 工種（舗装工、区画線工...）
//!           └─ 種別（舗装打換え工...）
//!               └─ 細別（表層工、上層路盤工...）
//!                   └─ 備考キー or { matchPatterns: [...] }
//! ```

use crate::analyzer::AnalysisResult;
use crate::error::{PhotoAiError, Result};
use serde_json::Value;
use std::path::Path;

/// 照合結果
#[derive(Debug, Clone, Default)]
pub struct MatchResult {
    pub photo_category: String,
    pub work_type: String,
    pub variety: String,
    pub detail: String,
    pub remark: String,
    pub matched_patterns: Vec<String>,
}

/// 階層走査時のコンテキスト
#[derive(Clone)]
struct TraverseContext {
    photo_category: String,
    work_type: String,
    variety: String,
    detail: String,
}

/// マスタJSONを読み込み
fn load_master(master_path: &Path) -> Result<Value> {
    let content = std::fs::read_to_string(master_path)?;
    let master: Value = serde_json::from_str(&content)
        .map_err(|e| PhotoAiError::InvalidMaster(format!("JSONパースエラー: {}", e)))?;
    Ok(master)
}

/// 階層を再帰的に走査し、matchPatternsを持つエントリを収集
fn collect_match_entries(
    value: &Value,
    ctx: &TraverseContext,
    depth: usize,
    entries: &mut Vec<(TraverseContext, String, Vec<String>)>,
) {
    let Some(obj) = value.as_object() else {
        return;
    };

    for (key, child) in obj {
        // matchPatternsキーは特別扱い
        if key == "matchPatterns" {
            if let Some(arr) = child.as_array() {
                let patterns: Vec<String> = arr
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                if !patterns.is_empty() {
                    // 親キー（細別または備考）をremarkとして記録
                    entries.push((ctx.clone(), String::new(), patterns));
                }
            }
            continue;
        }

        // 階層に応じてコンテキストを更新
        let new_ctx = match depth {
            0 => TraverseContext {
                photo_category: key.clone(),
                ..ctx.clone()
            },
            1 => TraverseContext {
                work_type: key.clone(),
                ..ctx.clone()
            },
            2 => TraverseContext {
                variety: key.clone(),
                ..ctx.clone()
            },
            3 => TraverseContext {
                detail: key.clone(),
                ..ctx.clone()
            },
            _ => ctx.clone(),
        };

        // 子ノードがオブジェクトで matchPatterns を持つ場合
        if let Some(child_obj) = child.as_object() {
            if let Some(patterns_val) = child_obj.get("matchPatterns") {
                if let Some(arr) = patterns_val.as_array() {
                    let patterns: Vec<String> = arr
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();
                    if !patterns.is_empty() {
                        let ctx_with_remark = TraverseContext {
                            detail: if depth >= 3 { new_ctx.detail.clone() } else { key.clone() },
                            ..new_ctx.clone()
                        };
                        entries.push((ctx_with_remark, key.clone(), patterns));
                    }
                }
                // matchPatternsのみのノードは子を持たないのでスキップ
                if child_obj.len() == 1 {
                    continue;
                }
            }
        }

        // 再帰
        collect_match_entries(child, &new_ctx, depth + 1, entries);
    }
}

/// 解析結果とマスタをマッチング
fn match_entry(
    result: &AnalysisResult,
    entries: &[(TraverseContext, String, Vec<String>)],
) -> Option<MatchResult> {
    let mut best_match: Option<MatchResult> = None;
    let mut best_score = 0;

    // 検索対象テキスト（OCR + 説明文 + 写真区分）
    let search_text = format!(
        "{} {} {}",
        result.detected_text,
        result.description,
        result.photo_category
    ).to_lowercase();

    for (ctx, remark, patterns) in entries {
        // 写真区分が一致するかチェック（部分一致）
        let category_match = result.photo_category.is_empty()
            || ctx.photo_category.contains(&result.photo_category)
            || result.photo_category.contains(&ctx.photo_category);

        if !category_match {
            continue;
        }

        // パターンマッチング
        let mut matched_patterns = Vec::new();
        for pattern in patterns {
            if search_text.contains(&pattern.to_lowercase()) {
                matched_patterns.push(pattern.clone());
            }
        }

        let score = matched_patterns.len();
        if score > best_score {
            best_score = score;
            best_match = Some(MatchResult {
                photo_category: ctx.photo_category.clone(),
                work_type: ctx.work_type.clone(),
                variety: ctx.variety.clone(),
                detail: ctx.detail.clone(),
                remark: remark.clone(),
                matched_patterns,
            });
        }
    }

    best_match
}

pub fn match_with_master(
    results: &[AnalysisResult],
    master_path: &Path,
) -> Result<Vec<AnalysisResult>> {
    if !master_path.exists() {
        return Err(PhotoAiError::FileNotFound(master_path.display().to_string()));
    }

    let master = load_master(master_path)?;

    // 「直接工事費」キーがあればその下を使う
    let root = master
        .get("直接工事費")
        .unwrap_or(&master);

    // matchPatternsを持つエントリを収集
    let mut entries = Vec::new();
    let initial_ctx = TraverseContext {
        photo_category: String::new(),
        work_type: String::new(),
        variety: String::new(),
        detail: String::new(),
    };
    collect_match_entries(root, &initial_ctx, 0, &mut entries);

    if entries.is_empty() {
        eprintln!("警告: マスタにmatchPatternsが見つかりません");
        return Ok(results.to_vec());
    }

    let matched_results: Vec<AnalysisResult> = results
        .iter()
        .map(|r| {
            let mut updated = r.clone();

            if let Some(m) = match_entry(r, &entries) {
                // 既存の値が空の場合のみ更新
                if updated.work_type.is_empty() {
                    updated.work_type = m.work_type;
                }
                if updated.variety.is_empty() {
                    updated.variety = m.variety;
                }
                if updated.detail.is_empty() {
                    updated.detail = m.detail;
                }
            }

            updated
        })
        .collect();

    Ok(matched_results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_master() -> Value {
        serde_json::json!({
            "直接工事費": {
                "品質管理写真": {
                    "舗装工": {
                        "舗装打換え工": {
                            "表層工": {
                                "アスファルト混合物温度測定": {
                                    "matchPatterns": ["温度管理", "合材温度", "到着温度", "敷均し温度"]
                                }
                            },
                            "上層路盤工": {
                                "現場密度測定": {
                                    "matchPatterns": ["密度測定", "RI計器", "砂置換法"]
                                }
                            }
                        }
                    }
                },
                "出来形管理写真": {
                    "舗装工": {
                        "舗装打換え工": {
                            "上層路盤工": {
                                "不陸整正出来形": {
                                    "matchPatterns": ["路盤出来形", "出来形検測", "基準高"]
                                }
                            }
                        }
                    }
                }
            }
        })
    }

    #[test]
    fn test_collect_match_entries() {
        let master = create_test_master();
        let root = master.get("直接工事費").unwrap();

        let mut entries = Vec::new();
        let initial_ctx = TraverseContext {
            photo_category: String::new(),
            work_type: String::new(),
            variety: String::new(),
            detail: String::new(),
        };
        collect_match_entries(root, &initial_ctx, 0, &mut entries);

        assert_eq!(entries.len(), 3);

        // 温度測定エントリを確認
        let temp_entry = entries.iter().find(|(ctx, _, _)| ctx.detail == "表層工");
        assert!(temp_entry.is_some());
        let (ctx, remark, patterns) = temp_entry.unwrap();
        assert_eq!(ctx.photo_category, "品質管理写真");
        assert_eq!(ctx.work_type, "舗装工");
        assert_eq!(ctx.variety, "舗装打換え工");
        assert_eq!(remark, "アスファルト混合物温度測定");
        assert!(patterns.contains(&"温度管理".to_string()));
    }

    #[test]
    fn test_match_entry_temperature() {
        let master = create_test_master();
        let root = master.get("直接工事費").unwrap();

        let mut entries = Vec::new();
        let initial_ctx = TraverseContext {
            photo_category: String::new(),
            work_type: String::new(),
            variety: String::new(),
            detail: String::new(),
        };
        collect_match_entries(root, &initial_ctx, 0, &mut entries);

        let result = AnalysisResult {
            file_name: "test.jpg".to_string(),
            detected_text: "到着温度 160.4℃".to_string(),
            photo_category: "品質管理".to_string(),
            ..Default::default()
        };

        let matched = match_entry(&result, &entries);
        assert!(matched.is_some());

        let m = matched.unwrap();
        assert_eq!(m.work_type, "舗装工");
        assert_eq!(m.variety, "舗装打換え工");
        assert_eq!(m.detail, "表層工");
        assert!(m.matched_patterns.contains(&"到着温度".to_string()));
    }

    #[test]
    fn test_match_entry_density() {
        let master = create_test_master();
        let root = master.get("直接工事費").unwrap();

        let mut entries = Vec::new();
        let initial_ctx = TraverseContext {
            photo_category: String::new(),
            work_type: String::new(),
            variety: String::new(),
            detail: String::new(),
        };
        collect_match_entries(root, &initial_ctx, 0, &mut entries);

        let result = AnalysisResult {
            file_name: "test.jpg".to_string(),
            detected_text: "RI計器による密度測定".to_string(),
            photo_category: "品質管理".to_string(),
            ..Default::default()
        };

        let matched = match_entry(&result, &entries);
        assert!(matched.is_some());

        let m = matched.unwrap();
        assert_eq!(m.detail, "上層路盤工");
        assert!(m.matched_patterns.len() >= 2); // "密度測定" と "RI計器"
    }

    #[test]
    fn test_match_entry_no_match() {
        let master = create_test_master();
        let root = master.get("直接工事費").unwrap();

        let mut entries = Vec::new();
        let initial_ctx = TraverseContext {
            photo_category: String::new(),
            work_type: String::new(),
            variety: String::new(),
            detail: String::new(),
        };
        collect_match_entries(root, &initial_ctx, 0, &mut entries);

        let result = AnalysisResult {
            file_name: "test.jpg".to_string(),
            detected_text: "関係ないテキスト".to_string(),
            photo_category: "施工状況".to_string(),
            ..Default::default()
        };

        let matched = match_entry(&result, &entries);
        // 写真区分が一致しないのでマッチしない
        assert!(matched.is_none());
    }
}
