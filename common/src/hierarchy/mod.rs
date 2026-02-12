//! 工種階層マスタモジュール
//!
//! 工事写真の分類に使用する階層マスタデータを管理する。
//! CSVから読み込み、Step2のAI解析でマスタ照合を行う。

mod csv_parser;

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// CSVの1行を表す構造体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchyRow {
    /// 費目（直接工事費、現場管理費など）
    pub photo_division: String,
    /// 写真区分（施工状況写真、安全管理写真、品質管理写真など）
    pub photo_type: String,
    /// 工種
    pub work_type: String,
    /// 種別
    pub variety: String,
    /// 作業段階
    pub subphase: String,
    /// 備考（マスタの最下層）
    pub remarks: String,
    /// 検索パターン（|区切り）
    pub search_patterns: String,
}

/// 行データから構築されるインデックス
struct Indexes {
    work_types: HashSet<String>,
    work_type_to_varieties: HashMap<String, HashSet<String>>,
    variety_to_subphases: HashMap<(String, String), HashSet<String>>,
}

/// 行データからインデックスを構築する共通関数
fn build_indexes(rows: &[HierarchyRow]) -> Indexes {
    let mut work_types = HashSet::new();
    let mut work_type_to_varieties: HashMap<String, HashSet<String>> = HashMap::new();
    let mut variety_to_subphases: HashMap<(String, String), HashSet<String>> = HashMap::new();

    for row in rows {
        if !row.work_type.is_empty() {
            work_types.insert(row.work_type.clone());

            if !row.variety.is_empty() {
                work_type_to_varieties
                    .entry(row.work_type.clone())
                    .or_default()
                    .insert(row.variety.clone());

                if !row.subphase.is_empty() {
                    variety_to_subphases
                        .entry((row.work_type.clone(), row.variety.clone()))
                        .or_default()
                        .insert(row.subphase.clone());
                }
            }
        }
    }

    Indexes {
        work_types,
        work_type_to_varieties,
        variety_to_subphases,
    }
}

/// 階層マスタ全体を管理する構造体
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HierarchyMaster {
    /// 全行データ
    rows: Vec<HierarchyRow>,
    /// 工種の一覧
    pub(crate) work_types: HashSet<String>,
    /// 工種→種別のマッピング
    pub(crate) work_type_to_varieties: HashMap<String, HashSet<String>>,
    /// (工種, 種別)→作業段階のマッピング
    variety_to_subphases: HashMap<(String, String), HashSet<String>>,
}

impl HierarchyMaster {
    /// 行データとインデックスからSelfを構築
    fn from_rows(rows: Vec<HierarchyRow>) -> Self {
        let idx = build_indexes(&rows);
        Self {
            rows,
            work_types: idx.work_types,
            work_type_to_varieties: idx.work_type_to_varieties,
            variety_to_subphases: idx.variety_to_subphases,
        }
    }

    /// CSVファイルから読み込み
    pub fn from_csv(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Self::from_csv_str(&content)
    }

    /// CSV文字列から読み込み
    pub fn from_csv_str(content: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let rows = csv_parser::parse_rows_from_csv(content)?;
        Ok(Self::from_rows(rows))
    }

    /// 工種一覧を取得
    pub fn get_work_types(&self) -> Vec<&str> {
        let mut types: Vec<_> = self.work_types.iter().map(|s| s.as_str()).collect();
        types.sort();
        types
    }

    /// 工種に対応する種別一覧を取得
    pub fn get_varieties(&self, work_type: &str) -> Vec<&str> {
        self.work_type_to_varieties
            .get(work_type)
            .map(|set| {
                let mut v: Vec<_> = set.iter().map(|s| s.as_str()).collect();
                v.sort();
                v
            })
            .unwrap_or_default()
    }

    /// (工種, 種別)に対応する作業段階一覧を取得
    pub fn get_subphases(&self, work_type: &str, variety: &str) -> Vec<&str> {
        self.variety_to_subphases
            .get(&(work_type.to_string(), variety.to_string()))
            .map(|set| {
                let mut v: Vec<_> = set.iter().map(|s| s.as_str()).collect();
                v.sort();
                v
            })
            .unwrap_or_default()
    }

    /// 写真種別の一覧を取得
    pub fn get_photo_types(&self) -> Vec<String> {
        let mut types: HashSet<String> = HashSet::new();
        for row in &self.rows {
            if !row.photo_type.is_empty() {
                types.insert(row.photo_type.clone());
            }
        }
        let mut v: Vec<_> = types.into_iter().collect();
        v.sort();
        v
    }

    /// 検索パターンでマッチする行を検索
    pub fn find_by_pattern(&self, text: &str) -> Vec<&HierarchyRow> {
        self.rows
            .iter()
            .filter(|row| {
                if row.search_patterns.is_empty() {
                    return false;
                }
                row.search_patterns
                    .split('|')
                    .any(|pattern| text.contains(pattern))
            })
            .collect()
    }

    /// 全行を取得
    pub fn rows(&self) -> &[HierarchyRow] {
        &self.rows
    }

    /// 指定した工種のみに絞ったマスタを返す
    pub fn filter_by_work_types(&self, work_types: &[String]) -> Self {
        if work_types.is_empty() {
            return self.clone();
        }

        let filtered_rows: Vec<HierarchyRow> = self
            .rows
            .iter()
            .filter(|row| work_types.contains(&row.work_type))
            .cloned()
            .collect();

        Self::from_rows(filtered_rows)
    }

    /// 写真種類で絞ったマスタを返す
    ///
    /// 写真種別カラムのみでフィルタ（備考へのフォールバックはしない）
    /// 例: "その他" -> 使用機械等の行がヒット
    /// 例: "安全管理写真" -> 朝礼・KY活動等の行がヒット
    pub fn filter_by_photo_type(&self, photo_type: &str) -> Self {
        let filtered_rows: Vec<HierarchyRow> = self
            .rows
            .iter()
            .filter(|row| row.photo_type == photo_type)
            .cloned()
            .collect();

        Self::from_rows(filtered_rows)
    }

    /// 指定した工種・種別のみに絞ったマスタを返す
    pub fn filter_by_work_type_and_variety(&self, work_type: &str, variety: Option<&str>) -> Self {
        let filtered_rows: Vec<HierarchyRow> = self
            .rows
            .iter()
            .filter(|row| {
                if row.work_type != work_type {
                    return false;
                }
                match variety {
                    Some(v) => row.variety == v,
                    None => true,
                }
            })
            .cloned()
            .collect();

        Self::from_rows(filtered_rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CSV: &str = r#"費目,写真区分,工種,種別,細別,撮影内容,検索パターン
"直接工事費","施工状況写真","舗装工","舗装打換え工","表層工","舗設状況",""
"直接工事費","品質管理写真","舗装工","舗装打換え工","表層工","アスファルト混合物温度測定","温度管理|到着温度|敷均し温度"
"直接工事費","施工状況写真","区画線工","区画線工","溶融式区画線","区画線設置状況",""
"#;

    #[test]
    fn test_load_csv() {
        let master = HierarchyMaster::from_csv_str(TEST_CSV).unwrap();
        assert_eq!(master.rows.len(), 3);
    }

    #[test]
    fn test_get_work_types() {
        let master = HierarchyMaster::from_csv_str(TEST_CSV).unwrap();
        let types = master.get_work_types();
        assert!(types.contains(&"舗装工"));
        assert!(types.contains(&"区画線工"));
    }

    #[test]
    fn test_get_varieties() {
        let master = HierarchyMaster::from_csv_str(TEST_CSV).unwrap();
        let varieties = master.get_varieties("舗装工");
        assert!(varieties.contains(&"舗装打換え工"));
    }

    #[test]
    fn test_get_subphases() {
        let master = HierarchyMaster::from_csv_str(TEST_CSV).unwrap();
        let subphases = master.get_subphases("舗装工", "舗装打換え工");
        assert!(subphases.contains(&"表層工"));
    }

    #[test]
    fn test_find_by_pattern() {
        let master = HierarchyMaster::from_csv_str(TEST_CSV).unwrap();
        let matches = master.find_by_pattern("到着温度");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].photo_type, "品質管理写真");
    }

}
