use crate::analyzer::AnalysisResult;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn extract_dekigata_station(text: &str) -> Option<String> {
    let markers = ["出来形管理用紙", "出来形管理用具"];
    for marker in markers {
        if let Some(idx) = text.find(marker) {
            let after = &text[idx + marker.len()..];
            let after = after.trim_start_matches(&[' ', '\u{3000}', ','][..]).trim_start();
            if after.starts_with("No.") || after.starts_with("NO.") || after.starts_with("no.") {
                let num_start = 3;
                let num_end = after[num_start..]
                    .find(|c: char| !c.is_ascii_digit())
                    .unwrap_or(after.len() - num_start)
                    + num_start;
                if num_end > num_start {
                    return Some(after[..num_end].to_string());
                }
            }
        }
    }

    if let Some(pos) = text.find("No.") {
        let tail = &text[pos + 3..];
        let digits: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            if text.contains("取付道路") {
                return Some(format!("取付道路 No.{}", digits));
            }
            return Some(format!("No.{}", digits));
        }
    }
    None
}

// === Serde structs ===

#[derive(Debug, Deserialize, Clone)]
pub struct FolderRule {
    #[allow(dead_code)]
    pub id: String,
    pub match_all: Vec<String>,
    #[serde(default)]
    pub apply: Option<FieldOverrides>,
    #[serde(default)]
    pub per_file: Vec<PerFileRule>,
    #[serde(default)]
    pub station_replacements: HashMap<String, String>,
    #[serde(default)]
    pub focus_target_renames: HashMap<String, String>,
    #[serde(default)]
    pub focus_target_merge: Option<FocusTargetMerge>,
    #[serde(default)]
    pub remarks_variety_override: Option<RemarksVarietyOverride>,
    #[serde(default)]
    pub tackcoat_counter: Option<TackcoatCounter>,
    #[serde(default)]
    pub index_focus_target: Option<HashMap<String, String>>,
    #[serde(default)]
    pub dekigata_station_extraction: bool,
    #[serde(default)]
    pub station_propagation: bool,
    #[serde(default)]
    pub clear_subphase_for: Vec<String>,
    #[serde(default)]
    pub default_subphase: Option<String>,
    #[serde(default)]
    pub measurement_maps: HashMap<String, HashMap<String, String>>,
    #[serde(default)]
    pub station_renames: HashMap<String, HashMap<String, String>>,
    #[serde(default)]
    pub file_overrides: HashMap<String, FieldOverrides>,
    #[serde(default)]
    pub clear_fields: Vec<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct FieldOverrides {
    pub photo_category: Option<String>,
    pub work_type: Option<String>,
    pub variety: Option<String>,
    pub subphase: Option<String>,
    pub remarks: Option<String>,
    pub station: Option<String>,
    pub measurements: Option<String>,
    pub focus_target: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PerFileRule {
    pub condition: PerFileCondition,
    #[serde(default)]
    pub apply: FieldOverrides,
    #[serde(default)]
    pub clear_fields: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PerFileCondition {
    #[serde(default)]
    pub detected_text_contains: Option<String>,
    #[serde(default)]
    pub detected_text_empty: Option<bool>,
    #[serde(default)]
    pub station_empty: Option<bool>,
    #[serde(default)]
    pub remarks_eq: Option<String>,
    #[serde(default)]
    pub default: Option<bool>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FocusTargetMerge {
    pub from: Vec<String>,
    pub to: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RemarksVarietyOverride {
    pub remarks_list: Vec<String>,
    pub work_type: String,
    pub variety: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TackcoatCounter {
    pub remarks: String,
    pub first_focus_target: String,
    pub rest_focus_target: String,
}

// === Loading ===

pub fn load_folder_rules(path: &Path) -> anyhow::Result<Vec<FolderRule>> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read folder rules from {}: {}", path.display(), e))?;
    let rules: Vec<FolderRule> = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse folder rules JSON from {}: {}", path.display(), e))?;
    Ok(rules)
}

/// Resolve the path to folder_rules.json:
/// 1. CLI-provided path
/// 2. config/folder_rules.json relative to the executable
/// 3. config/folder_rules.json relative to CARGO_MANIFEST_DIR (dev)
pub fn resolve_rules_path(cli_path: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = cli_path {
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }

    // Relative to executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let candidate = exe_dir.join("config").join("folder_rules.json");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // Relative to CARGO_MANIFEST_DIR (for development)
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let candidate = Path::new(&manifest_dir).join("config").join("folder_rules.json");
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}

// === Rule engine ===

fn matches_rule(folder_name: &str, rule: &FolderRule) -> bool {
    rule.match_all.iter().all(|s| folder_name.contains(s.as_str()))
}

fn apply_overrides(r: &mut AnalysisResult, overrides: &FieldOverrides) {
    if let Some(ref v) = overrides.photo_category {
        r.photo_category = v.clone();
    }
    if let Some(ref v) = overrides.work_type {
        r.work_type = v.clone();
    }
    if let Some(ref v) = overrides.variety {
        r.variety = v.clone();
    }
    if let Some(ref v) = overrides.subphase {
        r.subphase = v.clone();
    }
    if let Some(ref v) = overrides.remarks {
        r.remarks = v.clone();
    }
    if let Some(ref v) = overrides.station {
        r.station = v.clone();
    }
    if let Some(ref v) = overrides.measurements {
        r.measurements = v.clone();
    }
    if let Some(ref v) = overrides.focus_target {
        r.focus_target = v.clone();
    }
}

fn clear_field(r: &mut AnalysisResult, field: &str) {
    match field {
        "photo_category" => r.photo_category.clear(),
        "work_type" => r.work_type.clear(),
        "variety" => r.variety.clear(),
        "subphase" => r.subphase.clear(),
        "remarks" => r.remarks.clear(),
        "station" => r.station.clear(),
        "measurements" => r.measurements.clear(),
        "focus_target" => r.focus_target.clear(),
        _ => {}
    }
}

fn matches_condition(r: &AnalysisResult, cond: &PerFileCondition) -> bool {
    if cond.default == Some(true) {
        return true;
    }

    // All specified conditions must be true (AND logic)
    let mut has_any = false;

    if let Some(ref text) = cond.detected_text_contains {
        has_any = true;
        if !r.detected_text.contains(text.as_str()) {
            return false;
        }
    }

    if let Some(true) = cond.detected_text_empty {
        has_any = true;
        if !r.detected_text.trim().is_empty() {
            return false;
        }
    }

    if let Some(true) = cond.station_empty {
        has_any = true;
        if !r.station.is_empty() {
            return false;
        }
    }

    if let Some(ref eq) = cond.remarks_eq {
        has_any = true;
        if r.remarks != *eq {
            return false;
        }
    }

    // If no conditions were specified (shouldn't happen), don't match
    has_any
}

fn apply_single_rule(results: &mut [AnalysisResult], folder_name: &str, rule: &FolderRule) {
    // 1. Apply global overrides to all photos
    if let Some(ref overrides) = rule.apply {
        for r in results.iter_mut() {
            apply_overrides(r, overrides);
        }
    }

    // 2. Clear fields on all photos
    if !rule.clear_fields.is_empty() {
        for r in results.iter_mut() {
            for field in &rule.clear_fields {
                clear_field(r, field);
            }
        }
    }

    // 3. Station replacements
    if !rule.station_replacements.is_empty() {
        for r in results.iter_mut() {
            for (old, new) in &rule.station_replacements {
                if r.station.contains(old.as_str()) {
                    r.station = r.station.replace(old.as_str(), new.as_str());
                }
            }
        }
    }

    // 4. Focus target renames
    if !rule.focus_target_renames.is_empty() {
        for r in results.iter_mut() {
            if let Some(new_name) = rule.focus_target_renames.get(&r.focus_target) {
                r.focus_target = new_name.clone();
            }
        }
    }

    // 5. Focus target merge
    if let Some(ref merge) = rule.focus_target_merge {
        for r in results.iter_mut() {
            if merge.from.contains(&r.focus_target) {
                r.focus_target = merge.to.clone();
            }
        }
    }

    // 6. Per-file rules (first matching condition wins)
    for r in results.iter_mut() {
        for pf in &rule.per_file {
            if matches_condition(r, &pf.condition) {
                apply_overrides(r, &pf.apply);
                for field in &pf.clear_fields {
                    clear_field(r, field);
                }
                break;
            }
        }
    }

    // 7. Remarks variety override
    if let Some(ref rvo) = rule.remarks_variety_override {
        for r in results.iter_mut() {
            if rvo.remarks_list.contains(&r.remarks) {
                r.work_type = rvo.work_type.clone();
                r.variety = rvo.variety.clone();
            }
        }
    }

    // 8. Tackcoat counter
    if let Some(ref tc) = rule.tackcoat_counter {
        let mut seen = 0usize;
        for r in results.iter_mut() {
            if r.remarks == tc.remarks {
                seen += 1;
                r.focus_target = if seen == 1 {
                    tc.first_focus_target.clone()
                } else {
                    tc.rest_focus_target.clone()
                };
            }
        }
    }

    // 9. Index-based focus_target
    if let Some(ref idx_map) = rule.index_focus_target {
        let default_ft = idx_map.get("default");
        for (i, r) in results.iter_mut().enumerate() {
            let key = i.to_string();
            if let Some(ft) = idx_map.get(&key) {
                r.focus_target = ft.clone();
            } else if let Some(ft) = default_ft {
                r.focus_target = ft.clone();
            }
        }
    }

    // 10. Dekigata station extraction
    if rule.dekigata_station_extraction {
        for r in results.iter_mut() {
            if r.station.is_empty() {
                r.station = extract_dekigata_station(&r.detected_text).unwrap_or_default();
            }
        }
    }

    // 11. Clear subphase / default subphase
    if !rule.clear_subphase_for.is_empty() || rule.default_subphase.is_some() {
        let should_clear = rule.clear_subphase_for.iter().any(|pat| folder_name.contains(pat.as_str()));
        for r in results.iter_mut() {
            if should_clear {
                r.subphase.clear();
            } else if let Some(ref sp) = rule.default_subphase {
                r.subphase = sp.clone();
            }
        }
    }

    // 12. Station propagation
    if rule.station_propagation {
        propagate_stations(results);
    }

    // 13. Station renames (before measurement maps)
    for (pattern, rename_map) in &rule.station_renames {
        if folder_name.contains(pattern.as_str()) {
            for r in results.iter_mut() {
                if let Some(new_st) = rename_map.get(&r.station) {
                    r.station = new_st.clone();
                }
            }
        }
    }

    // 14. Measurement maps
    for (pattern, m_map) in &rule.measurement_maps {
        if folder_name.contains(pattern.as_str()) {
            for r in results.iter_mut() {
                if let Some(m) = m_map.get(&r.station) {
                    r.measurements = m.clone();
                }
            }
        }
    }

    // 15. File overrides
    for (file_name, overrides) in &rule.file_overrides {
        for r in results.iter_mut() {
            if r.file_name == *file_name {
                apply_overrides(r, overrides);
            }
        }
    }
}

fn propagate_stations(results: &mut [AnalysisResult]) {
    let mut groups: HashMap<u32, Vec<usize>> = HashMap::new();
    let mut ungrouped: Vec<usize> = Vec::new();
    for (i, r) in results.iter().enumerate() {
        if r.group > 0 {
            groups.entry(r.group).or_default().push(i);
        } else {
            ungrouped.push(i);
        }
    }

    // Real groups: propagate station from leader to members
    for indices in groups.values() {
        let station = indices
            .iter()
            .find(|&&i| !results[i].station.is_empty())
            .map(|&i| results[i].station.clone());
        if let Some(st) = station {
            for &i in indices {
                if results[i].station.is_empty() {
                    results[i].station = st.clone();
                }
            }
        }
    }

    // group=0: forward propagation
    for wi in 1..ungrouped.len() {
        let i = ungrouped[wi];
        let prev_i = ungrouped[wi - 1];
        if results[i].station.is_empty() && !results[prev_i].station.is_empty() {
            results[i].station = results[prev_i].station.clone();
        }
    }
    // group=0: backward propagation
    for wi in (0..ungrouped.len().saturating_sub(1)).rev() {
        let i = ungrouped[wi];
        let next_i = ungrouped[wi + 1];
        if results[i].station.is_empty() && !results[next_i].station.is_empty() {
            results[i].station = results[next_i].station.clone();
        }
    }
}

pub fn apply_folder_specific_corrections(
    results: &mut [AnalysisResult],
    folder_name: &str,
    rules: &[FolderRule],
) {
    for rule in rules {
        if matches_rule(folder_name, rule) {
            apply_single_rule(results, folder_name, rule);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_folder_rules_json() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("config")
            .join("folder_rules.json");
        let rules = load_folder_rules(&path).expect("Failed to load folder_rules.json");
        assert!(!rules.is_empty(), "Rules should not be empty");
        assert_eq!(rules[0].id, "出荷指示確認");
        assert_eq!(rules[0].match_all, vec!["Photomanager", "20260211", "出荷指示確認"]);
        assert!(rules[0].apply.is_some());
        assert_eq!(rules[0].per_file.len(), 2);
    }

    #[test]
    fn test_matches_rule() {
        let rule = FolderRule {
            id: "test".to_string(),
            match_all: vec!["abc".to_string(), "def".to_string()],
            apply: None,
            per_file: vec![],
            station_replacements: HashMap::new(),
            focus_target_renames: HashMap::new(),
            focus_target_merge: None,
            remarks_variety_override: None,
            tackcoat_counter: None,
            index_focus_target: None,
            dekigata_station_extraction: false,
            station_propagation: false,
            clear_subphase_for: vec![],
            default_subphase: None,
            measurement_maps: HashMap::new(),
            station_renames: HashMap::new(),
            file_overrides: HashMap::new(),
            clear_fields: vec![],
        };
        assert!(matches_rule("foo/abc/def/bar", &rule));
        assert!(!matches_rule("foo/abc/bar", &rule));
    }

    #[test]
    fn test_apply_global_overrides() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("config")
            .join("folder_rules.json");
        let rules = load_folder_rules(&path).unwrap();

        let mut results = vec![AnalysisResult::default()];
        let folder = "something/Photomanager/20260211/出荷指示確認/photos";
        apply_folder_specific_corrections(&mut results, folder, &rules);

        assert_eq!(results[0].photo_category, "品質管理写真");
        assert_eq!(results[0].work_type, "舗装工");
        assert_eq!(results[0].variety, "切削オーバーレイ工");
        assert_eq!(results[0].subphase, "表層工");
        assert_eq!(results[0].remarks, "出荷指示確認");
        // Default per-file (no 外観検査 in detected_text)
        assert_eq!(results[0].station, "大林道路㈱熊本As混合所");
        assert_eq!(results[0].measurements, "出荷温度：163℃\n積載量：3.5ｔ\n保温シート確認");
    }

    #[test]
    fn test_per_file_detected_text_condition() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("config")
            .join("folder_rules.json");
        let rules = load_folder_rules(&path).unwrap();

        let mut results = vec![AnalysisResult {
            detected_text: "外観検査の記録".to_string(),
            ..Default::default()
        }];
        let folder = "Photomanager/20260211/出荷指示確認";
        apply_folder_specific_corrections(&mut results, folder, &rules);

        assert_eq!(results[0].station, "No.0右車線");
        assert_eq!(results[0].measurements, "出荷温度：163℃\n積載量：3.5ｔ\n到着時外観検査");
    }

    #[test]
    fn test_station_replacements_and_focus_target() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("config")
            .join("folder_rules.json");
        let rules = load_folder_rules(&path).unwrap();

        let mut results = vec![
            AnalysisResult {
                station: "No.0R".to_string(),
                focus_target: "朝礼".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                station: "No.6R".to_string(),
                focus_target: "切断・清掃状況".to_string(),
                ..Default::default()
            },
        ];
        let folder = "0211切削/施工状況";
        apply_folder_specific_corrections(&mut results, folder, &rules);

        assert_eq!(results[0].station, "No.0 右車線");
        assert_eq!(results[0].focus_target, "安全活動");
        assert_eq!(results[1].station, "No.6 右車線");
        assert_eq!(results[1].focus_target, "作業状況");
    }

    #[test]
    fn test_tackcoat_counter() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("config")
            .join("folder_rules.json");
        let rules = load_folder_rules(&path).unwrap();

        let mut results = vec![
            AnalysisResult {
                remarks: "タックコート乳剤散布状況".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                remarks: "舗設状況".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                remarks: "タックコート乳剤散布状況".to_string(),
                ..Default::default()
            },
        ];
        let folder = "0211切削/施工状況";
        apply_folder_specific_corrections(&mut results, folder, &rules);

        assert_eq!(results[0].focus_target, "乳剤散布状況");
        assert_eq!(results[2].focus_target, "作業状況");
        // remarks_variety_override should have set these
        assert_eq!(results[0].work_type, "舗装工");
        assert_eq!(results[0].variety, "切削オーバーレイ工");
    }

    #[test]
    fn test_clear_fields() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("config")
            .join("folder_rules.json");
        let rules = load_folder_rules(&path).unwrap();

        let mut results = vec![AnalysisResult {
            station: "No.5".to_string(),
            measurements: "some data".to_string(),
            subphase: "old".to_string(),
            ..Default::default()
        }];
        let folder = "0213切削/切削出来形/No.9";
        apply_folder_specific_corrections(&mut results, folder, &rules);

        assert_eq!(results[0].station, "");
        assert_eq!(results[0].measurements, "");
        assert_eq!(results[0].subphase, "路面切削");
    }

    #[test]
    fn test_index_focus_target() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("config")
            .join("folder_rules.json");
        let rules = load_folder_rules(&path).unwrap();

        let mut results = vec![
            AnalysisResult::default(),
            AnalysisResult::default(),
            AnalysisResult::default(),
        ];
        let folder = "Photomanager/20260211/トラックスケール";
        apply_folder_specific_corrections(&mut results, folder, &rules);

        assert_eq!(results[0].focus_target, "計量値");    // index 0 -> default
        assert_eq!(results[1].focus_target, "計量状況");  // index 1 -> "1"
        assert_eq!(results[2].focus_target, "計量値");    // index 2 -> default
        assert_eq!(results[0].station, "2月11日（黒板日付訂正）");
    }
}
