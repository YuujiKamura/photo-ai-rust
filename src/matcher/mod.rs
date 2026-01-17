mod types;

pub use types::{MasterEntry, MatchResult};

use crate::analyzer::AnalysisResult;
use crate::error::{PhotoAiError, Result};
use calamine::{open_workbook, Reader, Xlsx};
use std::path::Path;

/// Excelマスタを読み込み、MasterEntryリストに変換
///
/// 想定シート構造:
/// | 写真区分 | 工種 | 種別 | 細別 | 照合パターン |
/// |---------|------|------|------|-------------|
/// | 品質管理 | 舗装工 | 表層工 | - | 温度,密度 |
fn load_master_from_excel(master_path: &Path) -> Result<Vec<MasterEntry>> {
    let mut workbook: Xlsx<_> = open_workbook(master_path)
        .map_err(|e| PhotoAiError::InvalidMaster(format!("Excel読み込みエラー: {}", e)))?;

    // 最初のシートを使用
    let sheet_name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| PhotoAiError::InvalidMaster("シートが見つかりません".into()))?;

    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|e| PhotoAiError::InvalidMaster(format!("シート読み込みエラー: {}", e)))?;

    let mut entries = Vec::new();
    let mut header_row = true;

    for row in range.rows() {
        // ヘッダー行をスキップ
        if header_row {
            header_row = false;
            continue;
        }

        // 最低5列必要（写真区分, 工種, 種別, 細別, 照合パターン）
        if row.len() < 5 {
            continue;
        }

        let photo_category = cell_to_string(&row[0]);
        let work_type = cell_to_string(&row[1]);
        let variety = cell_to_string(&row[2]);
        let detail = cell_to_string(&row[3]);
        let patterns_str = cell_to_string(&row[4]);

        // 空行をスキップ
        if photo_category.is_empty() && work_type.is_empty() {
            continue;
        }

        // 照合パターンをカンマ区切りで分割
        let match_patterns: Vec<String> = patterns_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if !match_patterns.is_empty() {
            entries.push(MasterEntry {
                photo_category,
                work_type,
                variety,
                detail,
                match_patterns,
            });
        }
    }

    Ok(entries)
}

/// セル値を文字列に変換
fn cell_to_string(cell: &calamine::Data) -> String {
    match cell {
        calamine::Data::String(s) => s.clone(),
        calamine::Data::Int(i) => i.to_string(),
        calamine::Data::Float(f) => f.to_string(),
        calamine::Data::Bool(b) => b.to_string(),
        calamine::Data::Empty => String::new(),
        _ => String::new(),
    }
}

/// 解析結果とマスタをマッチング
fn match_entry(result: &AnalysisResult, entries: &[MasterEntry]) -> Option<MatchResult> {
    let mut best_match: Option<MatchResult> = None;
    let mut best_score = 0;

    // 検索対象テキスト（OCR + 説明文 + 写真区分）
    let search_text = format!(
        "{} {} {}",
        result.detected_text.to_lowercase(),
        result.description.to_lowercase(),
        result.photo_category.to_lowercase()
    );

    for entry in entries {
        // 写真区分が一致するかチェック（部分一致）
        let category_match = result.photo_category.is_empty()
            || entry.photo_category.contains(&result.photo_category)
            || result.photo_category.contains(&entry.photo_category);

        if !category_match {
            continue;
        }

        // パターンマッチング
        let mut matched_patterns = Vec::new();
        for pattern in &entry.match_patterns {
            if search_text.contains(&pattern.to_lowercase()) {
                matched_patterns.push(pattern.clone());
            }
        }

        let score = matched_patterns.len();
        if score > best_score {
            best_score = score;
            let confidence = if entry.match_patterns.is_empty() {
                0.0
            } else {
                score as f32 / entry.match_patterns.len() as f32
            };

            best_match = Some(MatchResult {
                work_type: entry.work_type.clone(),
                variety: entry.variety.clone(),
                detail: entry.detail.clone(),
                matched_patterns,
                confidence,
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

    // 拡張子でExcelかJSONか判定
    let ext = master_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let entries = match ext.as_str() {
        "xlsx" | "xls" => load_master_from_excel(master_path)?,
        _ => {
            return Err(PhotoAiError::InvalidMaster(
                "マスタファイルはExcel形式（.xlsx）で指定してください".into(),
            ));
        }
    };

    if entries.is_empty() {
        eprintln!("警告: マスタにマッチングパターンが見つかりません");
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

    fn create_test_entries() -> Vec<MasterEntry> {
        vec![
            MasterEntry {
                photo_category: "品質管理".to_string(),
                work_type: "舗装工".to_string(),
                variety: "表層工".to_string(),
                detail: "".to_string(),
                match_patterns: vec!["温度".to_string(), "密度".to_string()],
            },
            MasterEntry {
                photo_category: "出来形管理".to_string(),
                work_type: "舗装工".to_string(),
                variety: "路盤工".to_string(),
                detail: "".to_string(),
                match_patterns: vec!["厚さ".to_string(), "幅".to_string()],
            },
        ]
    }

    #[test]
    fn test_match_entry_temperature() {
        let entries = create_test_entries();
        let result = AnalysisResult {
            file_name: "test.jpg".to_string(),
            detected_text: "温度 160.4℃".to_string(),
            photo_category: "品質管理".to_string(),
            ..Default::default()
        };

        let matched = match_entry(&result, &entries);
        assert!(matched.is_some());

        let m = matched.unwrap();
        assert_eq!(m.work_type, "舗装工");
        assert_eq!(m.variety, "表層工");
        assert!(m.matched_patterns.contains(&"温度".to_string()));
    }

    #[test]
    fn test_match_entry_thickness() {
        let entries = create_test_entries();
        let result = AnalysisResult {
            file_name: "test.jpg".to_string(),
            detected_text: "厚さ 50mm".to_string(),
            photo_category: "出来形".to_string(),
            ..Default::default()
        };

        let matched = match_entry(&result, &entries);
        assert!(matched.is_some());

        let m = matched.unwrap();
        assert_eq!(m.variety, "路盤工");
    }

    #[test]
    fn test_match_entry_no_match() {
        let entries = create_test_entries();
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
