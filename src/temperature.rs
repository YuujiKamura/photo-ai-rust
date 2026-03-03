//! 温度管理フォルダ専用の後処理ロジック
//!
//! `analysis.rs` のパイプラインから呼ばれ、温度管理フォルダ内の写真に対して
//! 温度種別の分類・測点補完・台目伝搬などの専用処理を行う。

use crate::analyzer::AnalysisResult;
use crate::domain::*;
use crate::normalizer::measurements::{
    extract_dump_number, extract_temperature, extract_temperature_for_remarks, is_temperature_photo,
};
use crate::master_matcher::date_to_month_day;
use crate::normalizer;

/// 温度管理の測定種別
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TemperatureKind {
    Arrival,           // 到着温度
    Spreading,         // 敷均し温度
    InitialCompaction, // 初期締固め前温度
    Opening,           // 開放温度
    OutsideAir,        // 舗装日外気温
}

impl TemperatureKind {
    /// 日本語ラベル（「測定」suffix付き）
    pub fn label(&self) -> &'static str {
        match self {
            Self::Arrival => "到着温度測定",
            Self::Spreading => "敷均し温度測定",
            Self::InitialCompaction => "初期締固め前温度測定",
            Self::Opening => "開放温度測定",
            Self::OutsideAir => "舗装日外気温測定",
        }
    }

    /// テキストから温度種別を判定（focusTarget / detectedText 両対応）
    pub fn from_text(text: &str) -> Option<Self> {
        if text.is_empty() {
            return None;
        }
        if text.contains("到着温度") {
            Some(Self::Arrival)
        } else if text.contains("敷均し温度") || text.contains("敷きならし") {
            Some(Self::Spreading)
        } else if text.contains("初期転圧前") || text.contains("初期締固め前") {
            Some(Self::InitialCompaction)
        } else if text.contains("開放温度") || text.contains("解放温度") {
            Some(Self::Opening)
        } else if text.contains("舗装日外気温") || text.contains("外気温") {
            Some(Self::OutsideAir)
        } else {
            None
        }
    }

    /// 有効な温度種別ラベル一覧（「測定」suffix付き・なし両方）
    pub fn all_labels() -> &'static [&'static str] {
        &[
            "舗装日外気温測定",
            "到着温度測定",
            "敷均し温度測定",
            "初期締固め前温度測定",
            "開放温度測定",
            "到着温度",
            "敷均し温度",
            "初期締固め前温度",
            "開放温度",
        ]
    }
}

pub fn apply_temperature_folder_postprocess(result: &mut AnalysisResult, folder_name: &str) {
    if !folder_name.contains("温度管理") {
        return;
    }
    result.photo_category = PHOTO_CAT_QUALITY.to_string();
    result.work_type = "舗装工".to_string();
    result.variety = VARIETY_PAVEMENT_REPLACE.to_string();
    result.subphase = SUBPHASE_SURFACE.to_string();
}

/// detectedText・focusTargetから温度測定種別を分類する。
/// 常に「測定」suffix付きの正規化名を返す。
fn classify_temperature_remarks(
    remarks: &str,
    detected_text: &str,
    focus_target: &str,
) -> String {
    // 1. focusTarget（photo-taggerの判定、最優先）
    if let Some(kind) = TemperatureKind::from_text(focus_target) {
        return kind.label().to_string();
    }
    // 2. detectedText（focusTarget未設定時のフォールバック）
    if let Some(kind) = TemperatureKind::from_text(detected_text) {
        return kind.label().to_string();
    }
    // 3. 元のremarks（温度種別なら「測定」suffix付きに正規化、それ以外はそのまま）
    if let Some(kind) = TemperatureKind::from_text(remarks) {
        kind.label().to_string()
    } else {
        remarks.to_string()
    }
}

fn extract_month_day_from_text(text: &str) -> Option<String> {
    let compact = text.replace([' ', '　'], "");
    let month_pos = compact.find('月')?;
    let month_digits_rev: String = compact[..month_pos]
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if month_digits_rev.is_empty() {
        return None;
    }
    let month: String = month_digits_rev.chars().rev().collect();
    let after_month = &compact[month_pos + '月'.len_utf8()..];
    let day_pos = after_month.find('日')?;
    let day: String = after_month[..day_pos]
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if day.is_empty() {
        return None;
    }
    Some(format!("{}月{}日", month, day))
}

/// フォルダ名から施工日(MMDD)を抽出して「M月D日」形式で返す。
/// 夜間工事で日付をまたいでも、フォルダ名の施工日（作業開始日）を使う。
fn extract_month_day_from_folder_context(folder_context: &str) -> Option<String> {
    let (mm, dd) = extract_mmdd_digits(folder_context)?;
    Some(format!("{}月{}日", mm, dd))
}

/// フォルダ名から最初の有効なMMDD(4桁連続数字)を検出して(month, day)を返す。
fn extract_mmdd_digits(text: &str) -> Option<(u32, u32)> {
    let chars: Vec<char> = text.chars().collect();
    for window in chars.windows(4) {
        if window.iter().all(|c| c.is_ascii_digit()) {
            let mmdd: String = window.iter().collect();
            let mm: u32 = mmdd[0..2].parse().ok()?;
            let dd: u32 = mmdd[2..4].parse().ok()?;
            if (1..=12).contains(&mm) && (1..=31).contains(&dd) {
                return Some((mm, dd));
            }
        }
    }
    None
}

fn extract_dump_number_from_station(station: &str) -> Option<String> {
    extract_dump_number(station)
}

fn fill_missing_dump_numbers(results: &mut [AnalysisResult]) {
    // ±1隣接ウィンドウで台目を推定（全写真共通）
    // group内検索は台目が撮影イベント単位ではなくダンプ1台分（=station）単位なので不要
    for i in 0..results.len() {
        if results[i].station.contains("台目") || !is_temperature_photo(&results[i].remarks) {
            continue;
        }
        let Some(base_day) = extract_month_day_from_text(&results[i].station) else {
            continue;
        };
        let mut inferred_dump = None;
        if i > 0
            && results[i - 1].remarks == results[i].remarks
            && extract_month_day_from_text(&results[i - 1].station).as_deref()
                == Some(base_day.as_str())
        {
            inferred_dump = extract_dump_number_from_station(&results[i - 1].station);
        }
        if inferred_dump.is_none()
            && i + 1 < results.len()
            && results[i + 1].remarks == results[i].remarks
            && extract_month_day_from_text(&results[i + 1].station).as_deref()
                == Some(base_day.as_str())
        {
            inferred_dump = extract_dump_number_from_station(&results[i + 1].station);
        }
        if let Some(dump) = inferred_dump {
            results[i].station = format!("{} {}", base_day, dump);
        }
    }
}

fn rebalance_initial_temperature_labels(results: &mut [AnalysisResult]) {
    use std::collections::BTreeMap;
    let initial = TemperatureKind::InitialCompaction.label();
    let spread = TemperatureKind::Spreading.label();
    // stationのみでキー化（ダンプ1台分=station単位のリバランス。groupは撮影イベント単位なので不使用）
    let mut by_station: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (idx, r) in results.iter().enumerate() {
        if r.station.contains("台目") {
            by_station.entry(r.station.clone()).or_default().push(idx);
        }
    }
    for indices in by_station.values() {
        let mut spread_indices: Vec<usize> = indices
            .iter()
            .copied()
            .filter(|&i| results[i].remarks == spread)
            .collect();
        let initial_count = indices.iter().filter(|&&i| results[i].remarks == initial).count();
        if initial_count == 2 && spread_indices.len() == 3 {
            if let Some(&idx) = spread_indices.last() {
                results[idx].remarks = initial.to_string();
            }
            continue;
        }
        if initial_count >= 3 || spread_indices.len() <= 3 {
            continue;
        }
        let move_count = (3 - initial_count).min(spread_indices.len() - 2);
        spread_indices.reverse();
        for &idx in spread_indices.iter().take(move_count) {
            results[idx].remarks = initial.to_string();
        }
    }
}

fn repair_orphan_temperature_entries(results: &mut [AnalysisResult]) {
    // 2ステップ後退ウィンドウで孤立温度エントリを修復（全写真共通）
    // 台目はダンプ1台分（=station）単位なのでgroupではなく隣接で検索
    for i in 2..results.len() {
        if results[i].station.contains("台目") || !is_temperature_photo(&results[i].remarks) {
            continue;
        }
        let prev_station = results[i - 1].station.clone();
        let prev_remarks = results[i - 1].remarks.clone();
        let prev_measurements = results[i - 1].measurements.clone();
        let prev2_station = results[i - 2].station.clone();
        let prev2_remarks = results[i - 2].remarks.clone();
        let prev2_measurements = results[i - 2].measurements.clone();
        if !prev_station.contains("台目")
            || prev_station != prev2_station
            || prev_remarks != prev2_remarks
            || prev_station.is_empty()
        {
            continue;
        }
        let m = results[i].measurements.trim();
        if m.is_empty() {
            continue;
        }
        if prev_measurements.trim() == m || prev2_measurements.trim() == m {
            results[i].station = prev_station;
            results[i].remarks = prev_remarks;
        }
    }
}

fn propagate_temperature_measurements(results: &mut [AnalysisResult]) {
    use std::collections::BTreeMap;
    let valid = TemperatureKind::all_labels();

    for r in results.iter_mut() {
        if !valid.contains(&r.remarks.as_str()) {
            continue;
        }
        let key = if !r.focus_target.is_empty() && valid.contains(&r.focus_target.as_str()) {
            &r.focus_target
        } else {
            &r.remarks
        };
        if let Some(extracted) = extract_temperature_for_remarks(&r.detected_text, key) {
            r.measurements = extracted;
        }
    }

    // (station, focus_target) でグループ化（focus_targetはremarksより正確）
    let mut groups: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
    for (idx, r) in results.iter().enumerate() {
        let group_key = if !r.focus_target.is_empty() && valid.contains(&r.focus_target.as_str()) {
            &r.focus_target
        } else if valid.contains(&r.remarks.as_str()) {
            &r.remarks
        } else {
            continue;
        };
        groups
            .entry((r.station.clone(), group_key.clone()))
            .or_default()
            .push(idx);
    }

    for ((_, group_key), idxs) in &groups {
        let Some(_first_idx) = idxs.first().copied() else {
            continue;
        };
        let group_remarks = group_key.clone();
        let mut value_counts: BTreeMap<String, usize> = BTreeMap::new();
        for &i in idxs {
            let m = results[i].measurements.trim();
            if !m.is_empty() {
                *value_counts.entry(m.to_string()).or_insert(0) += 1;
            }
        }
        if value_counts.is_empty() {
            continue;
        }
        let source_value = if group_remarks == TemperatureKind::OutsideAir.label() {
            value_counts
                .keys()
                .filter_map(|v| extract_temperature(v).map(|t| (t, v.clone())))
                .min_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(_, v)| v)
                .unwrap_or_else(|| value_counts.keys().next().cloned().unwrap_or_default())
        } else {
            value_counts
                .iter()
                .max_by_key(|(_, c)| **c)
                .map(|(v, _)| v.clone())
                .unwrap_or_default()
        };
        for &i in idxs {
            results[i].measurements = source_value.clone();
        }
    }
}

/// 温度管理フォルダ専用の最終調整。
///
/// フォルダ名に依存せず、detectedText（黒板OCR）を正とする統一ロジック。
/// - station: 黒板の日付 → フォルダ名の日付 → EXIF日付（最終手段）
/// - 台目: blackboard → 既存station
/// - remarks: classify_temperature_remarks() で正規化
/// - measurements: detectedTextから抽出→グループ内伝搬
/// - variety: この関数では設定しない（apply_temperature_folder_postprocessの値を維持）
pub fn apply_temperature_folder_final_adjustments(
    results: &mut [AnalysisResult],
    folder_context: &str,
) {
    if !folder_context.contains("温度管理") {
        return;
    }

    for result in results.iter_mut() {
        // station: 黒板日付 → フォルダ名日付 → 既存station日付 → EXIF日付
        let base_day = extract_month_day_from_text(&result.detected_text)
            .or_else(|| extract_month_day_from_folder_context(folder_context))
            .or_else(|| extract_month_day_from_text(&result.station))
            .unwrap_or_else(|| date_to_month_day(&result.date));
        let dump = extract_dump_number(&result.detected_text)
            .or_else(|| extract_dump_number_from_station(&result.station));
        result.station = match dump {
            Some(d) => format!("{} {}", base_day, d),
            None => base_day,
        };

        // remarks: detectedText + focusTarget から温度種別を分類
        result.remarks = classify_temperature_remarks(
            &result.remarks,
            &result.detected_text,
            &result.focus_target,
        );

        // 黒板に複数温度並記時の到着温度優先ルール
        // focusTargetで温度種別が確定している場合はスキップ（累積記入式黒板対応）
        if TemperatureKind::from_text(&result.focus_target).is_none() {
            let has_arrival = result.detected_text.contains("到着温度");
            let has_spread = result.detected_text.contains("敷均し温度");
            if has_arrival && has_spread && result.remarks == TemperatureKind::Spreading.label() {
                result.remarks = TemperatureKind::Arrival.label().to_string();
            }
        }
    }

    // 後処理: 全温度管理フォルダ共通
    normalizer::append_dump_number_to_station(results);
    fill_missing_dump_numbers(results);
    repair_orphan_temperature_entries(results);
    rebalance_initial_temperature_labels(results);
    propagate_temperature_measurements(results);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temperature_unified_station_from_folder_context() {
        // フォルダ名 "0209切削" → 2月9日（EXIF日付 2月10日ではなくフォルダの施工日を使う）
        let mut result = AnalysisResult {
            file_name: "T1.JPG".to_string(),
            photo_category: PHOTO_CAT_QUALITY.to_string(),
            work_type: "舗装補修工".to_string(),
            variety: "アスファルト舗装補修工".to_string(),
            subphase: SUBPHASE_SURFACE.to_string(),
            station: "".to_string(),
            remarks: "到着温度測定".to_string(),
            measurements: "159.7℃".to_string(),
            date: "2026-02-10 01:00:09".to_string(),
            ..Default::default()
        };

        apply_temperature_folder_postprocess(&mut result, "0209切削_温度管理");
        let mut results = vec![result];
        apply_temperature_folder_final_adjustments(&mut results, "0209切削_温度管理");
        let result = &results[0];

        assert_eq!(result.work_type, "舗装工");
        assert_eq!(result.variety, VARIETY_PAVEMENT_REPLACE);
        assert_eq!(result.station, "2月9日");           // folder_contextから（EXIF 2月10日ではない）
        assert_eq!(result.remarks, "到着温度測定");      // 「測定」suffix付き（統一）
        assert_eq!(result.measurements, "159.7℃");      // 保持（クリアしない）
    }

    #[test]
    fn test_temperature_unified_station_from_folder_0212() {
        // フォルダ名 "0212切削" → 2月12日
        let mut result = AnalysisResult {
            file_name: "T2.JPG".to_string(),
            photo_category: PHOTO_CAT_QUALITY.to_string(),
            work_type: "舗装補修工".to_string(),
            variety: "アスファルト舗装補修工".to_string(),
            subphase: SUBPHASE_SURFACE.to_string(),
            station: "2月13日".to_string(),
            remarks: "到着温度".to_string(),
            measurements: "160.1℃".to_string(),
            date: "2026-02-13 10:00:00".to_string(),
            ..Default::default()
        };

        apply_temperature_folder_postprocess(&mut result, "0212切削_温度管理");
        let mut results = vec![result];
        apply_temperature_folder_final_adjustments(&mut results, "0212切削_温度管理");
        let result = &results[0];

        assert_eq!(result.work_type, "舗装工");
        assert_eq!(result.variety, VARIETY_PAVEMENT_REPLACE);
        assert_eq!(result.station, "2月12日");           // folder_contextから
        assert_eq!(result.remarks, "到着温度測定");      // 「測定」suffix付き
        assert_eq!(result.measurements, "160.1℃");      // 保持
    }

    #[test]
    fn test_extract_mmdd_digits_various_patterns() {
        // "切削"マーカーなしでもMMDD抽出できる
        assert_eq!(extract_mmdd_digits("0209_温度管理"), Some((2, 9)));
        assert_eq!(extract_mmdd_digits("温度管理_0213"), Some((2, 13)));
        assert_eq!(extract_mmdd_digits("0212切削_温度管理"), Some((2, 12)));
        // 数字4桁だがMMDDとして無効（月>12）
        assert_eq!(extract_mmdd_digits("1399_フォルダ"), None);
        // 数字なし
        assert_eq!(extract_mmdd_digits("温度管理"), None);
        // 4桁ちょうど
        assert_eq!(extract_mmdd_digits("0101"), Some((1, 1)));
        assert_eq!(extract_mmdd_digits("1231"), Some((12, 31)));
    }
}
