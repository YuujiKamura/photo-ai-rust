//! CSVパース関連ユーティリティ
//!
//! 工種階層マスタCSVの行パースとフィールド抽出を担当する。

use super::HierarchyRow;
use crate::error::{Error, HierarchyError};
use serde::Deserialize;

/// CSV列名に対応する内部デシリアライズ用構造体
///
/// HierarchyRowのJSON serde互換性を壊さないよう、CSV専用の型を使う。
#[derive(Deserialize)]
struct CsvRow {
    #[serde(rename = "費目")]
    photo_division: String,
    #[serde(rename = "写真区分")]
    photo_type: String,
    #[serde(rename = "工種")]
    work_type: String,
    #[serde(rename = "種別")]
    variety: String,
    #[serde(rename = "細別")]
    subphase: String,
    #[serde(rename = "備考", alias = "撮影内容")]
    remarks: String,
    #[serde(rename = "検索パターン")]
    search_patterns: String,
}

impl From<CsvRow> for HierarchyRow {
    fn from(row: CsvRow) -> Self {
        Self {
            photo_division: row.photo_division,
            photo_type: row.photo_type,
            work_type: row.work_type,
            variety: row.variety,
            subphase: row.subphase,
            remarks: row.remarks,
            search_patterns: row.search_patterns,
        }
    }
}

/// CSV文字列からHierarchyRow配列をパース
pub(crate) fn parse_rows_from_csv(content: &str) -> Result<Vec<HierarchyRow>, Error> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(content.as_bytes());

    let mut rows = Vec::new();
    for (i, result) in reader.deserialize::<CsvRow>().enumerate() {
        let csv_row = result.map_err(|e| HierarchyError::CsvParse {
            line: i + 2, // 1-based, +1 for header
            detail: e.to_string(),
        })?;
        rows.push(HierarchyRow::from(csv_row));
    }

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rows_basic() {
        let csv = "費目,写真区分,工種,種別,細別,備考,検索パターン\n\
                   直接工事費,施工状況写真,舗装工,舗装打換え工,表層工,舗設状況,\n";
        let rows = parse_rows_from_csv(csv).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].photo_division, "直接工事費");
        assert_eq!(rows[0].work_type, "舗装工");
        assert_eq!(rows[0].remarks, "舗設状況");
    }

    #[test]
    fn test_parse_rows_quoted() {
        let csv = "費目,写真区分,工種,種別,細別,備考,検索パターン\n\
                   \"直接工事費\",\"施工状況写真\",\"舗装工\",\"舗装打換え工\",\"表層工\",\"舗設状況\",\"\"\n";
        let rows = parse_rows_from_csv(csv).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].photo_division, "直接工事費");
    }

    #[test]
    fn test_parse_rows_with_search_patterns() {
        let csv = "費目,写真区分,工種,種別,細別,備考,検索パターン\n\
                   直接工事費,品質管理写真,舗装工,舗装打換え工,表層工,温度測定,温度管理|到着温度\n";
        let rows = parse_rows_from_csv(csv).unwrap();
        assert_eq!(rows[0].search_patterns, "温度管理|到着温度");
    }

    #[test]
    fn test_parse_rows_empty_fields() {
        let csv = "費目,写真区分,工種,種別,細別,備考,検索パターン\n\
                   直接工事費,施工状況写真,舗装工,,,, \n";
        let rows = parse_rows_from_csv(csv).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].variety, "");
        assert_eq!(rows[0].subphase, "");
    }

    #[test]
    fn test_parse_rows_alternative_header() {
        let csv = "費目,写真区分,工種,種別,細別,撮影内容,検索パターン\n\
                   直接工事費,施工状況写真,舗装工,舗装打換え工,表層工,舗設状況,\n";
        let rows = parse_rows_from_csv(csv).unwrap();
        assert_eq!(rows[0].remarks, "舗設状況");
    }

    #[test]
    fn test_parse_rows_skips_header() {
        let csv = "費目,写真区分,工種,種別,細別,備考,検索パターン\na,b,c,d,e,f,g\n";
        let rows = parse_rows_from_csv(csv).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].photo_division, "a");
    }

    #[test]
    fn test_parse_rows_error_on_bad_csv() {
        let csv = "費目,写真区分,工種\n\
                   a,b,c\n";
        let result = parse_rows_from_csv(csv);
        assert!(result.is_err());
    }
}
