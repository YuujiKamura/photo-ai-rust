//! AI向けマスタ整形モジュール
//!
//! HierarchyMasterをAIプロンプトに埋め込むための各種フォーマッタ。

use super::HierarchyMaster;
use std::collections::{BTreeMap, HashMap};

impl HierarchyMaster {
    /// Step2プロンプト用の階層JSONを生成
    pub fn to_hierarchy_json(&self) -> serde_json::Value {
        let mut hierarchy: HashMap<String, HashMap<String, Vec<String>>> = HashMap::new();

        for work_type in &self.work_types {
            let mut varieties_map: HashMap<String, Vec<String>> = HashMap::new();

            if let Some(varieties) = self.work_type_to_varieties.get(work_type) {
                for variety in varieties {
                    let subphases = self.get_subphases(work_type, variety);
                    varieties_map.insert(
                        variety.clone(),
                        subphases.iter().map(|s| s.to_string()).collect(),
                    );
                }
            }

            hierarchy.insert(work_type.clone(), varieties_map);
        }

        serde_json::to_value(hierarchy).unwrap_or(serde_json::Value::Null)
    }

    /// 1ステップ解析用のチェーンレコードJSONを生成
    /// (photoType/workType/variety/subphase/remarks/patterns)
    pub fn to_chain_records_json(&self) -> serde_json::Value {
        let records: Vec<serde_json::Value> = self
            .rows
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
                lines.push(hierarchy);
            } else {
                let remarks_str = remarks.join(", ");
                lines.push(format!("{}: {}", hierarchy, remarks_str));
            }
        }

        lines.join("\n")
    }
}
