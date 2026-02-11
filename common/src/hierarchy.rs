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
    /// 作業段階
    pub subphase: String,
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
    /// (工種, 種別)→作業段階のマッピング
    variety_to_subphases: HashMap<(String, String), HashSet<String>>,
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
        let mut variety_to_subphases: HashMap<(String, String), HashSet<String>> = HashMap::new();

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
                subphase: fields[4].to_string(),
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

                    if !row.subphase.is_empty() {
                        variety_to_subphases
                            .entry((row.work_type.clone(), row.variety.clone()))
                            .or_default()
                            .insert(row.subphase.clone());
                    }
                }
            }

            rows.push(row);
        }

        Ok(Self {
            rows,
            work_types,
            work_type_to_varieties,
            variety_to_subphases,
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

    /// Step2プロンプト用の階層JSONを生成
    pub fn to_hierarchy_json(&self) -> serde_json::Value {
        let mut hierarchy: HashMap<String, HashMap<String, Vec<String>>> = HashMap::new();

        for work_type in &self.work_types {
            let mut varieties_map: HashMap<String, Vec<String>> = HashMap::new();

            if let Some(varieties) = self.work_type_to_varieties.get(work_type) {
                for variety in varieties {
                    let subphases = self.get_subphases(work_type, variety);
                    varieties_map.insert(variety.clone(), subphases.iter().map(|s| s.to_string()).collect());
                }
            }

            hierarchy.insert(work_type.clone(), varieties_map);
        }

        serde_json::to_value(hierarchy).unwrap_or(serde_json::Value::Null)
    }

    /// 1ステップ解析用のチェーンレコードJSONを生成
    /// (photoType/workType/variety/subphase/remarks/patterns)
    pub fn to_chain_records_json(&self) -> serde_json::Value {
        let records: Vec<serde_json::Value> = self.rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "photoType": row.photo_type,
                    "workType": row.work_type,
                    "variety": row.variety,
                    "subphase": row.subphase,
                    "remarks": row.remarks,
                    "patterns": row.search_patterns,
                })
            })
            .collect();

        serde_json::to_value(records).unwrap_or(serde_json::Value::Null)
    }

    /// コンパクトなテキスト形式でマスタを出力
    ///
    /// 同じ写真種別>工種>種別>細別のグループを1行にまとめ、
    /// 備考（remarks）をカンマ区切りで列挙する。
    ///
    /// 例:
    /// ```text
    /// 施工状況写真 > 舗装工 > 舗装打換え工 > 表層工: 舗設状況, 初期転圧状況, 施工完了
    /// 安全管理写真: 朝礼, KY活動, 新規入場者教育
    /// その他 > 舗装工: 使用機械
    /// ```
    pub fn to_compact_text(&self) -> String {
        use std::collections::BTreeMap;

        // (photoType, workType, variety, subphase) -> Vec<remarks>
        // BTreeMap for stable ordering
        let mut groups: BTreeMap<(String, String, String, String), Vec<String>> = BTreeMap::new();

        for row in &self.rows {
            let key = (
                row.photo_type.clone(),
                row.work_type.clone(),
                row.variety.clone(),
                row.subphase.clone(),
            );
            let remarks = row.remarks.trim().to_string();
            if !remarks.is_empty() {
                groups.entry(key).or_default().push(remarks);
            } else {
                // Ensure the group exists even if remarks is empty
                groups.entry(key).or_default();
            }
        }

        let mut lines = Vec::new();
        for ((photo_type, work_type, variety, subphase), remarks) in &groups {
            let mut parts = Vec::new();
            if !photo_type.is_empty() {
                parts.push(photo_type.as_str());
            }
            if !work_type.is_empty() {
                parts.push(work_type.as_str());
            }
            if !variety.is_empty() {
                parts.push(variety.as_str());
            }
            if !subphase.is_empty() {
                parts.push(subphase.as_str());
            }

            let hierarchy = parts.join(" > ");

            if remarks.is_empty() {
                // No remarks - just show the hierarchy path
                lines.push(hierarchy);
            } else {
                let remarks_str = remarks.join(", ");
                lines.push(format!("{}: {}", hierarchy, remarks_str));
            }
        }

        lines.join("\n")
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
        let mut variety_to_subphases: HashMap<(String, String), HashSet<String>> = HashMap::new();

        for row in &filtered_rows {
            if !row.work_type.is_empty() {
                work_types_set.insert(row.work_type.clone());

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

        Self {
            rows: filtered_rows,
            work_types: work_types_set,
            work_type_to_varieties,
            variety_to_subphases,
        }
    }

    /// 写真種類で絞ったマスタを返す
    ///
    /// まず写真種別（photo_type）でマッチし、ヒットしなければ備考（remarks）でマッチする。
    /// 例: "安全管理写真" → 写真種別="安全管理写真" でフィルタ
    /// 例: "使用機械" → 備考="使用機械" でフィルタ
    pub fn filter_by_photo_type(&self, photo_type: &str) -> Self {
        // まず写真種別で検索
        let by_type: Vec<HierarchyRow> = self.rows
            .iter()
            .filter(|row| row.photo_type == photo_type)
            .cloned()
            .collect();

        let filtered_rows = if !by_type.is_empty() {
            by_type
        } else {
            // 写真種別にヒットしなければ備考で検索
            self.rows
                .iter()
                .filter(|row| row.remarks == photo_type)
                .cloned()
                .collect()
        };

        Self::from_rows(filtered_rows)
    }

    /// 行リストからインデックスを再構築
    fn from_rows(rows: Vec<HierarchyRow>) -> Self {
        let mut work_types = HashSet::new();
        let mut work_type_to_varieties: HashMap<String, HashSet<String>> = HashMap::new();
        let mut variety_to_subphases: HashMap<(String, String), HashSet<String>> = HashMap::new();

        for row in &rows {
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

        Self { rows, work_types, work_type_to_varieties, variety_to_subphases }
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
        let mut variety_to_subphases: HashMap<(String, String), HashSet<String>> = HashMap::new();

        for row in &filtered_rows {
            if !row.work_type.is_empty() {
                work_types_set.insert(row.work_type.clone());

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

        Self {
            rows: filtered_rows,
            work_types: work_types_set,
            work_type_to_varieties,
            variety_to_subphases,
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

    #[test]
    fn test_to_hierarchy_json() {
        let master = HierarchyMaster::from_csv_str(TEST_CSV).unwrap();
        let json = master.to_hierarchy_json();
        assert!(json.is_object());
        assert!(json.get("舗装工").is_some());
    }

    #[test]
    fn test_to_compact_text_basic() {
        let master = HierarchyMaster::from_csv_str(TEST_CSV).unwrap();
        let text = master.to_compact_text();
        // 施工状況写真の舗装工行にremarksが含まれる
        assert!(text.contains("舗設状況"));
        assert!(text.contains("区画線設置状況"));
        // 品質管理写真の行
        assert!(text.contains("品質管理写真"));
        assert!(text.contains("アスファルト混合物温度測定"));
    }

    #[test]
    fn test_to_compact_text_grouping() {
        let csv = r#"写真区分,写真種別,工種,種別,細別,撮影内容,検索パターン
"直接工事費","施工状況写真","舗装工","舗装打換え工","表層工","舗設状況",""
"直接工事費","施工状況写真","舗装工","舗装打換え工","表層工","初期転圧状況",""
"直接工事費","施工状況写真","舗装工","舗装打換え工","表層工","施工完了",""
"現場管理費","安全管理写真","","","","朝礼",""
"現場管理費","安全管理写真","","","","KY活動",""
"直接工事費","その他","舗装工","","","使用機械",""
"#;
        let master = HierarchyMaster::from_csv_str(csv).unwrap();
        let text = master.to_compact_text();

        // 同じグループの備考がカンマ区切りで1行にまとまる
        assert!(text.contains("舗設状況, 初期転圧状況, 施工完了"));
        // 安全管理写真は工種なし
        assert!(text.contains("安全管理写真: 朝礼, KY活動"));
        // 使用機械
        assert!(text.contains("その他 > 舗装工: 使用機械"));
    }
}
