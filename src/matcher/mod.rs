//! マスタ照合モジュール
//!
//! 既存の constructionHierarchyData.ts と同じJSON構造を読み込み、
//! AI解析結果と照合して工種・種別・作業段階を特定する。
//!
//! ## 階層構造
//! ```text
//! 直接工事費
//!   └─ 写真区分（品質管理写真、施工状況写真...）
//!       └─ 工種（舗装工、区画線工...）
//!           └─ 種別（舗装打換え工...）
//!               └─ 作業段階（表層工、上層路盤工...）
//!                   └─ 備考キー or { matchPatterns: [...] }
//! ```
//!
//! ## マスタ形式
//! - JSON (.json): 上記の階層構造を直接記述
//! - Excel (.xlsx/.xls): フラット形式で記述 → 内部でJSON構造に変換

pub mod alias;

pub use alias::apply_aliases;

use crate::analyzer::AnalysisResult;
use crate::error::{PhotoAiError, Result};
use calamine::{open_workbook, Reader, Xlsx};
use serde_json::{json, Map, Value};
use std::path::Path;

/// 照合結果
#[derive(Debug, Clone, Default)]
pub struct MatchResult {
    pub photo_category: String,
    pub work_type: String,
    pub variety: String,
    pub subphase: String,
    pub remark: String,
    pub matched_patterns: Vec<String>,
}

/// 階層走査時のコンテキスト
#[derive(Clone)]
struct TraverseContext {
    photo_category: String,
    work_type: String,
    variety: String,
    subphase: String,
}

/// マスタJSONを読み込み
fn load_master_json(master_path: &Path) -> Result<Value> {
    let content = std::fs::read_to_string(master_path)?;
    let master: Value = serde_json::from_str(&content)
        .map_err(|e| PhotoAiError::InvalidMaster(format!("JSONパースエラー: {}", e)))?;
    Ok(master)
}

/// Excelマスタを読み込み、JSON構造に変換
///
/// ## Excel形式
/// | 写真区分 | 工種 | 種別 | 作業段階 | 備考 | matchPatterns |
/// |---------|------|------|------|------|---------------|
/// | 品質管理写真 | 舗装工 | 舗装打換え工 | 表層工 | アスファルト混合物温度測定 | 温度管理,到着温度,敷均し温度 |
fn load_master_excel(master_path: &Path) -> Result<Value> {
    let mut workbook: Xlsx<_> = open_workbook(master_path)
        .map_err(|e| PhotoAiError::InvalidMaster(format!("Excel読み込みエラー: {}", e)))?;

    // 最初のシートを取得
    let sheet_name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| PhotoAiError::InvalidMaster("シートが見つかりません".to_string()))?;

    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|e| PhotoAiError::InvalidMaster(format!("シート読み込みエラー: {}", e)))?;

    // ヘッダー行を取得してカラムインデックスを特定
    let headers: Vec<String> = range
        .rows()
        .next()
        .ok_or_else(|| PhotoAiError::InvalidMaster("ヘッダー行がありません".to_string()))?
        .iter()
        .map(|cell| cell.to_string().trim().to_string())
        .collect();

    let col_photo_category = find_column(&headers, &["写真区分"])?;
    let col_work_type = find_column(&headers, &["工種"])?;
    let col_variety = find_column(&headers, &["種別"])?;
    let col_subphase = find_column(&headers, &["細別", "作業段階"])?;
    let col_remark = find_column(&headers, &["備考"]).ok();
    let col_patterns = find_column(&headers, &["matchPatterns", "マッチパターン", "パターン"])?;

    // 階層構造を構築
    let mut root: Map<String, Value> = Map::new();

    for row in range.rows().skip(1) {
        let photo_category = get_cell_string(row, col_photo_category);
        let work_type = get_cell_string(row, col_work_type);
        let variety = get_cell_string(row, col_variety);
        let subphase = get_cell_string(row, col_subphase);
        let remark = col_remark.map_or(String::new(), |i| get_cell_string(row, i));
        let patterns_str = get_cell_string(row, col_patterns);

        // 空行はスキップ
        if photo_category.is_empty() || patterns_str.is_empty() {
            continue;
        }

        // matchPatterns をカンマ区切りでパース
        let patterns: Vec<Value> = patterns_str
            .split(',')
            .map(|s| Value::String(s.trim().to_string()))
            .filter(|v| !v.as_str().unwrap_or("").is_empty())
            .collect();

        if patterns.is_empty() {
            continue;
        }

        // 階層構造に挿入
        let category_obj = root
            .entry(photo_category)
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .unwrap();

        let work_type_obj = category_obj
            .entry(work_type)
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .unwrap();

        let variety_obj = work_type_obj
            .entry(variety)
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .unwrap();

        let subphase_obj = variety_obj
            .entry(subphase)
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .unwrap();

        // 備考がある場合は備考キー下に、なければ直接matchPatternsを設定
        if !remark.is_empty() {
            subphase_obj.insert(
                remark,
                json!({ "matchPatterns": patterns }),
            );
        } else {
            subphase_obj.insert("matchPatterns".to_string(), Value::Array(patterns));
        }
    }

    Ok(json!({ "直接工事費": root }))
}

/// ヘッダー行からカラムインデックスを検索
fn find_column(headers: &[String], names: &[&str]) -> Result<usize> {
    for name in names {
        if let Some(idx) = headers.iter().position(|h| h == *name) {
            return Ok(idx);
        }
    }
    Err(PhotoAiError::InvalidMaster(format!(
        "カラムが見つかりません: {:?}",
        names
    )))
}

/// セルから文字列を取得
fn get_cell_string(row: &[calamine::Data], idx: usize) -> String {
    row.get(idx)
        .map(|cell| cell.to_string().trim().to_string())
        .unwrap_or_default()
}

/// ファイル拡張子に応じてマスタを読み込み
fn load_master(master_path: &Path) -> Result<Value> {
    let ext = master_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "xlsx" | "xls" => load_master_excel(master_path),
        "json" => load_master_json(master_path),
        _ => Err(PhotoAiError::InvalidMaster(format!(
            "未対応のファイル形式: {} (json, xlsx, xlsのみ対応)",
            ext
        ))),
    }
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
                    // 親キー（作業段階または備考）をremarkとして記録
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
                subphase: key.clone(),
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
                            subphase: if depth >= 3 { new_ctx.subphase.clone() } else { key.clone() },
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
                subphase: ctx.subphase.clone(),
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
        subphase: String::new(),
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
                if updated.subphase.is_empty() {
                    updated.subphase = m.subphase;
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
            subphase: String::new(),
        };
        collect_match_entries(root, &initial_ctx, 0, &mut entries);

        assert_eq!(entries.len(), 3);

        // 温度測定エントリを確認
        let temp_entry = entries.iter().find(|(ctx, _, _)| ctx.subphase == "表層工");
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
            subphase: String::new(),
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
        assert_eq!(m.subphase, "表層工");
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
            subphase: String::new(),
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
        assert_eq!(m.subphase, "上層路盤工");
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
            subphase: String::new(),
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

    #[test]
    fn test_load_master_excel() {
        use rust_xlsxwriter::Workbook;

        // テスト用Excelファイルを作成
        let temp_dir = std::env::temp_dir();
        let excel_path = temp_dir.join("test_master.xlsx");

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        // ヘッダー行
        worksheet.write_string(0, 0, "写真区分").unwrap();
        worksheet.write_string(0, 1, "工種").unwrap();
        worksheet.write_string(0, 2, "種別").unwrap();
        worksheet.write_string(0, 3, "細別").unwrap();
        worksheet.write_string(0, 4, "備考").unwrap();
        worksheet.write_string(0, 5, "matchPatterns").unwrap();

        // データ行1
        worksheet.write_string(1, 0, "品質管理写真").unwrap();
        worksheet.write_string(1, 1, "舗装工").unwrap();
        worksheet.write_string(1, 2, "舗装打換え工").unwrap();
        worksheet.write_string(1, 3, "表層工").unwrap();
        worksheet.write_string(1, 4, "アスファルト混合物温度測定").unwrap();
        worksheet.write_string(1, 5, "温度管理,到着温度,敷均し温度").unwrap();

        // データ行2
        worksheet.write_string(2, 0, "品質管理写真").unwrap();
        worksheet.write_string(2, 1, "舗装工").unwrap();
        worksheet.write_string(2, 2, "舗装打換え工").unwrap();
        worksheet.write_string(2, 3, "上層路盤工").unwrap();
        worksheet.write_string(2, 4, "現場密度測定").unwrap();
        worksheet.write_string(2, 5, "密度測定,RI計器").unwrap();

        workbook.save(&excel_path).unwrap();

        // Excelから読み込み
        let master = load_master_excel(&excel_path).unwrap();

        // 構造を検証
        let root = master.get("直接工事費").unwrap();
        let category = root.get("品質管理写真").unwrap();
        let work_type = category.get("舗装工").unwrap();
        let variety = work_type.get("舗装打換え工").unwrap();
        let subphase = variety.get("表層工").unwrap();
        let remark = subphase.get("アスファルト混合物温度測定").unwrap();
        let patterns = remark.get("matchPatterns").unwrap().as_array().unwrap();

        assert!(patterns.iter().any(|p| p.as_str() == Some("温度管理")));
        assert!(patterns.iter().any(|p| p.as_str() == Some("到着温度")));

        // クリーンアップ
        std::fs::remove_file(&excel_path).ok();
    }

    #[test]
    fn test_excel_and_json_produce_same_entries() {
        use rust_xlsxwriter::Workbook;

        // テスト用Excelファイルを作成
        let temp_dir = std::env::temp_dir();
        let excel_path = temp_dir.join("test_compare.xlsx");

        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        // ヘッダー行
        worksheet.write_string(0, 0, "写真区分").unwrap();
        worksheet.write_string(0, 1, "工種").unwrap();
        worksheet.write_string(0, 2, "種別").unwrap();
        worksheet.write_string(0, 3, "細別").unwrap();
        worksheet.write_string(0, 4, "備考").unwrap();
        worksheet.write_string(0, 5, "matchPatterns").unwrap();

        // データ行（JSONテストと同じ内容）
        worksheet.write_string(1, 0, "品質管理写真").unwrap();
        worksheet.write_string(1, 1, "舗装工").unwrap();
        worksheet.write_string(1, 2, "舗装打換え工").unwrap();
        worksheet.write_string(1, 3, "表層工").unwrap();
        worksheet.write_string(1, 4, "アスファルト混合物温度測定").unwrap();
        worksheet.write_string(1, 5, "温度管理,合材温度,到着温度,敷均し温度").unwrap();

        workbook.save(&excel_path).unwrap();

        // Excel版とJSON版の両方からエントリを収集
        let excel_master = load_master_excel(&excel_path).unwrap();
        let json_master = create_test_master();

        let excel_root = excel_master.get("直接工事費").unwrap();
        let json_root = json_master.get("直接工事費").unwrap();

        let mut excel_entries = Vec::new();
        let mut json_entries = Vec::new();
        let initial_ctx = TraverseContext {
            photo_category: String::new(),
            work_type: String::new(),
            variety: String::new(),
            subphase: String::new(),
        };

        collect_match_entries(excel_root, &initial_ctx, 0, &mut excel_entries);
        collect_match_entries(json_root, &initial_ctx, 0, &mut json_entries);

        // Excelから読み込んだエントリが1つあること
        assert_eq!(excel_entries.len(), 1);

        // 温度測定のエントリを比較
        let excel_temp = excel_entries.iter().find(|(ctx, _, _)| ctx.subphase == "表層工");
        let json_temp = json_entries.iter().find(|(ctx, _, _)| ctx.subphase == "表層工");

        assert!(excel_temp.is_some());
        assert!(json_temp.is_some());

        let (excel_ctx, excel_remark, excel_patterns) = excel_temp.unwrap();
        let (json_ctx, json_remark, json_patterns) = json_temp.unwrap();

        assert_eq!(excel_ctx.photo_category, json_ctx.photo_category);
        assert_eq!(excel_ctx.work_type, json_ctx.work_type);
        assert_eq!(excel_ctx.variety, json_ctx.variety);
        assert_eq!(excel_ctx.subphase, json_ctx.subphase);
        assert_eq!(excel_remark, json_remark);
        assert_eq!(excel_patterns.len(), json_patterns.len());

        // クリーンアップ
        std::fs::remove_file(&excel_path).ok();
    }
}
