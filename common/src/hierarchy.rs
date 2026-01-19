//! 工種階層マスタモジュール
//!
//! 工事写真の分類に使用する階層マスタデータを管理する。
//! CSVから読み込み、Step2のAI解析でマスタ照合を行う。

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// CSVの1行を表す構造体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HierarchyRow {
    /// 写真区分（直接工事費など）
    pub photo_division: String,
    /// 写真種別（施工状況写真、品質管理写真など）
    pub photo_type: String,
    /// 工種
    pub work_type: String,
    /// 種別
    pub variety: String,
    /// 細別
    pub detail: String,
    /// 備考（マスタの最下層）
    pub remarks: String,
    /// 検索パターン（|区切り）
    pub search_patterns: String,
}

/// 階層マスタ全体を管理する構造体
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HierarchyMaster {
    /// 全行データ
    rows: Vec<HierarchyRow>,
    /// 工種の一覧
    work_types: HashSet<String>,
    /// 工種→種別のマッピング
    work_type_to_varieties: HashMap<String, HashSet<String>>,
    /// (工種, 種別)→細別のマッピング
    variety_to_details: HashMap<(String, String), HashSet<String>>,
}

impl HierarchyMaster {
    /// CSVファイルから読み込み
    pub fn from_csv(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Self::from_csv_str(&content)
    }

    /// CSV文字列から読み込み
    pub fn from_csv_str(content: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut rows = Vec::new();
        let mut work_types = HashSet::new();
        let mut work_type_to_varieties: HashMap<String, HashSet<String>> = HashMap::new();
        let mut variety_to_details: HashMap<(String, String), HashSet<String>> = HashMap::new();

        // ヘッダーをスキップ
        for line in content.lines().skip(1) {
            let fields: Vec<&str> = parse_csv_line(line);
            if fields.len() < 7 {
                continue;
            }

            let row = HierarchyRow {
                photo_division: fields[0].to_string(),
                photo_type: fields[1].to_string(),
                work_type: fields[2].to_string(),
                variety: fields[3].to_string(),
                detail: fields[4].to_string(),
                remarks: fields[5].to_string(),
                search_patterns: fields[6].to_string(),
            };

            // インデックス構築
            if !row.work_type.is_empty() {
                work_types.insert(row.work_type.clone());

                if !row.variety.is_empty() {
                    work_type_to_varieties
                        .entry(row.work_type.clone())
                        .or_default()
                        .insert(row.variety.clone());

                    if !row.detail.is_empty() {
                        variety_to_details
                            .entry((row.work_type.clone(), row.variety.clone()))
                            .or_default()
                            .insert(row.detail.clone());
                    }
                }
            }

            rows.push(row);
        }

        Ok(Self {
            rows,
            work_types,
            work_type_to_varieties,
            variety_to_details,
        })
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

    /// (工種, 種別)に対応する細別一覧を取得
    pub fn get_details(&self, work_type: &str, variety: &str) -> Vec<&str> {
        self.variety_to_details
            .get(&(work_type.to_string(), variety.to_string()))
            .map(|set| {
                let mut v: Vec<_> = set.iter().map(|s| s.as_str()).collect();
                v.sort();
                v
            })
            .unwrap_or_default()
    }

    /// Step2プロンプト用の階層JSONを生成
    pub fn to_hierarchy_json(&self) -> serde_json::Value {
        let mut hierarchy: HashMap<String, HashMap<String, Vec<String>>> = HashMap::new();

        for work_type in &self.work_types {
            let mut varieties_map: HashMap<String, Vec<String>> = HashMap::new();

            if let Some(varieties) = self.work_type_to_varieties.get(work_type) {
                for variety in varieties {
                    let details = self.get_details(work_type, variety);
                    varieties_map.insert(variety.clone(), details.iter().map(|s| s.to_string()).collect());
                }
            }

            hierarchy.insert(work_type.clone(), varieties_map);
        }

        serde_json::to_value(hierarchy).unwrap_or(serde_json::Value::Null)
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

        let filtered_rows: Vec<HierarchyRow> = self.rows
            .iter()
            .filter(|row| work_types.contains(&row.work_type))
            .cloned()
            .collect();

        let mut work_types_set = HashSet::new();
        let mut work_type_to_varieties: HashMap<String, HashSet<String>> = HashMap::new();
        let mut variety_to_details: HashMap<(String, String), HashSet<String>> = HashMap::new();

        for row in &filtered_rows {
            if !row.work_type.is_empty() {
                work_types_set.insert(row.work_type.clone());

                if !row.variety.is_empty() {
                    work_type_to_varieties
                        .entry(row.work_type.clone())
                        .or_default()
                        .insert(row.variety.clone());

                    if !row.detail.is_empty() {
                        variety_to_details
                            .entry((row.work_type.clone(), row.variety.clone()))
                            .or_default()
                            .insert(row.detail.clone());
                    }
                }
            }
        }

        Self {
            rows: filtered_rows,
            work_types: work_types_set,
            work_type_to_varieties,
            variety_to_details,
        }
    }

    /// 指定した工種・種別のみに絞ったマスタを返す
    pub fn filter_by_work_type_and_variety(&self, work_type: &str, variety: Option<&str>) -> Self {
        let filtered_rows: Vec<HierarchyRow> = self.rows
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

        let mut work_types_set = HashSet::new();
        let mut work_type_to_varieties: HashMap<String, HashSet<String>> = HashMap::new();
        let mut variety_to_details: HashMap<(String, String), HashSet<String>> = HashMap::new();

        for row in &filtered_rows {
            if !row.work_type.is_empty() {
                work_types_set.insert(row.work_type.clone());

                if !row.variety.is_empty() {
                    work_type_to_varieties
                        .entry(row.work_type.clone())
                        .or_default()
                        .insert(row.variety.clone());

                    if !row.detail.is_empty() {
                        variety_to_details
                            .entry((row.work_type.clone(), row.variety.clone()))
                            .or_default()
                            .insert(row.detail.clone());
                    }
                }
            }
        }

        Self {
            rows: filtered_rows,
            work_types: work_types_set,
            work_type_to_varieties,
            variety_to_details,
        }
    }
}

/// CSV行をパース（ダブルクォート対応）
fn parse_csv_line(line: &str) -> Vec<&str> {
    let mut fields = Vec::new();
    let mut in_quotes = false;
    let mut field_start = 0;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c == '"' {
            in_quotes = !in_quotes;
        } else if c == ',' && !in_quotes {
            // フィールド終了
            let field = &line[field_start..byte_index(line, i)];
            fields.push(trim_quotes(field));
            field_start = byte_index(line, i + 1);
        }
        i += 1;
    }

    // 最後のフィールド
    if field_start <= line.len() {
        let field = &line[field_start..];
        fields.push(trim_quotes(field));
    }

    fields
}

fn byte_index(s: &str, char_index: usize) -> usize {
    s.char_indices()
        .nth(char_index)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

fn trim_quotes(s: &str) -> &str {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CSV: &str = r#"写真区分,写真種別,工種,種別,細別,撮影内容,検索パターン
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
    fn test_get_details() {
        let master = HierarchyMaster::from_csv_str(TEST_CSV).unwrap();
        let details = master.get_details("舗装工", "舗装打換え工");
        assert!(details.contains(&"表層工"));
    }

    #[test]
    fn test_find_by_pattern() {
        let master = HierarchyMaster::from_csv_str(TEST_CSV).unwrap();
        let matches = master.find_by_pattern("到着温度");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].photo_type, "品質管理写真");
    }

    #[test]
    fn test_to_hierarchy_json() {
        let master = HierarchyMaster::from_csv_str(TEST_CSV).unwrap();
        let json = master.to_hierarchy_json();
        assert!(json.is_object());
        assert!(json.get("舗装工").is_some());
    }
}
