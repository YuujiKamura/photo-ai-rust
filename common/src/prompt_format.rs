//! AI向けマスタ整形モジュール
//!
//! HierarchyMasterをAIプロンプトに埋め込むための各種フォーマッタ。
//! HierarchyMasterの`impl`から独立した関数として提供する。

use crate::error::Result;
use crate::hierarchy::HierarchyMaster;
use std::collections::BTreeMap;

/// Step2プロンプト用の階層JSONを生成
pub fn hierarchy_to_json(master: &HierarchyMaster) -> Result<serde_json::Value> {
    let mut hierarchy: BTreeMap<String, BTreeMap<String, Vec<String>>> = BTreeMap::new();

    for work_type in master.get_work_types() {
        let mut varieties_map: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for variety in master.get_varieties(work_type) {
            let subphases: Vec<String> = master
                .get_subphases(work_type, variety)
                .iter()
                .map(|s| s.to_string())
                .collect();
            varieties_map.insert(variety.to_string(), subphases);
        }

        hierarchy.insert(work_type.to_string(), varieties_map);
    }

    Ok(serde_json::to_value(hierarchy)?)
}

/// 1ステップ解析用のチェーンレコードJSONを生成
/// (photoType/workType/variety/subphase/remarks/patterns)
pub fn hierarchy_to_chain_records(master: &HierarchyMaster) -> Result<serde_json::Value> {
    let records: Vec<serde_json::Value> = master
        .rows()
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

    Ok(serde_json::to_value(records)?)
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
pub fn hierarchy_to_compact_text(master: &HierarchyMaster) -> String {
    // (photoType, workType, variety, subphase) -> Vec<remarks>
    // BTreeMap for stable ordering
    let mut groups: BTreeMap<(String, String, String, String), Vec<String>> = BTreeMap::new();

    for row in master.rows() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchy::HierarchyMaster;

    const TEST_CSV: &str = r#"費目,写真区分,工種,種別,細別,撮影内容,検索パターン
"直接工事費","施工状況写真","舗装工","舗装打換え工","表層工","舗設状況",""
"直接工事費","品質管理写真","舗装工","舗装打換え工","表層工","アスファルト混合物温度測定","温度管理|到着温度|敷均し温度"
"直接工事費","施工状況写真","区画線工","区画線工","溶融式区画線","区画線設置状況",""
"#;

    #[test]
    fn test_hierarchy_to_json() {
        let master = HierarchyMaster::from_csv_str(TEST_CSV).unwrap();
        let json = hierarchy_to_json(&master).unwrap();
        assert!(json.is_object());
        assert!(json.get("舗装工").is_some());
    }

    #[test]
    fn test_hierarchy_to_compact_text_basic() {
        let master = HierarchyMaster::from_csv_str(TEST_CSV).unwrap();
        let text = hierarchy_to_compact_text(&master);
        assert!(text.contains("舗設状況"));
        assert!(text.contains("区画線設置状況"));
        assert!(text.contains("品質管理写真"));
        assert!(text.contains("アスファルト混合物温度測定"));
    }

    #[test]
    fn test_hierarchy_to_compact_text_grouping() {
        let csv = r#"費目,写真区分,工種,種別,細別,撮影内容,検索パターン
"直接工事費","施工状況写真","舗装工","舗装打換え工","表層工","舗設状況",""
"直接工事費","施工状況写真","舗装工","舗装打換え工","表層工","初期転圧状況",""
"直接工事費","施工状況写真","舗装工","舗装打換え工","表層工","施工完了",""
"現場管理費","安全管理写真","","","","朝礼",""
"現場管理費","安全管理写真","","","","KY活動",""
"直接工事費","その他","舗装工","","","使用機械",""
"#;
        let master = HierarchyMaster::from_csv_str(csv).unwrap();
        let text = hierarchy_to_compact_text(&master);

        assert!(text.contains("舗設状況, 初期転圧状況, 施工完了"));
        assert!(text.contains("安全管理写真: 朝礼, KY活動"));
        assert!(text.contains("その他 > 舗装工: 使用機械"));
    }
}
