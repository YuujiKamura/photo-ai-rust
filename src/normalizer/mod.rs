//! 後解析（正規化）モジュール
//!
//! 個別画像解析後に、グループ単位で計測値を統一する。
//!
//! ## 処理フロー（予定）
//! - 温度管理: 3枚単位（全景+ボードアップ+温度計アップ）で統一
//! - 出来形管理: 同一測点のセット全体で統一

pub mod alias;
pub mod dekigata;
pub mod measurements;

pub use alias::apply_aliases;
pub use dekigata::Lane;

use crate::analyzer::AnalysisResult;

/// 正規化結果
#[derive(Debug, Clone)]
pub struct NormalizationResult {
    /// 修正内容のリスト
    pub corrections: Vec<NormalizationCorrection>,
    /// 統計情報
    pub stats: NormalizationStats,
}

/// 個別の修正内容
#[derive(Debug, Clone)]
pub struct NormalizationCorrection {
    /// ファイル名
    pub file_name: String,
    /// 修正対象フィールド
    pub field: CorrectionField,
    /// 修正前の値
    pub original: String,
    /// 修正後の値
    pub corrected: String,
    /// 修正理由
    pub reason: String,
}

/// 修正対象フィールド
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrectionField {
    Measurements,
    Remarks,
}

impl std::fmt::Display for CorrectionField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CorrectionField::Measurements => write!(f, "計測値"),
            CorrectionField::Remarks => write!(f, "備考"),
        }
    }
}

/// 正規化の統計情報
#[derive(Debug, Clone, Default)]
pub struct NormalizationStats {
    /// 処理したレコード数
    pub total_records: usize,
    /// 修正したレコード数
    pub corrected_records: usize,
    /// 計測値の修正数
    pub measurement_corrections: usize,
}

/// 正規化オプション
#[derive(Debug, Clone)]
pub struct NormalizationOptions {
    /// 計測値グループ統一を有効にする
    pub unify_measurements: bool,
    /// 出来形管理写真の測定値統一を有効にする
    pub unify_dekigata: bool,
    /// 出来形の車線指定（Noneなら自動判定）
    pub dekigata_lane: Option<Lane>,
    /// 出来形の備考統一テキスト（例: "路面切削工出来形測定"）
    pub dekigata_remarks: Option<String>,
}

impl Default for NormalizationOptions {
    fn default() -> Self {
        Self {
            unify_measurements: true,
            unify_dekigata: true,
            dekigata_lane: None,
            dekigata_remarks: None,
        }
    }
}

/// 解析結果を正規化する
///
/// # Arguments
/// * `results` - 解析結果のスライス
/// * `options` - 正規化オプション
///
/// # Returns
/// 正規化結果（修正内容と統計情報）
pub fn normalize_results(
    results: &[AnalysisResult],
    options: &NormalizationOptions,
) -> NormalizationResult {
    let mut corrections = Vec::new();
    let mut stats = NormalizationStats {
        total_records: results.len(),
        ..Default::default()
    };

    if options.unify_measurements {
        // Step 1: detected_textから温度値を抽出してmeasurementsに設定
        let populate_corrections = populate_temperature_measurements(results);
        for correction in populate_corrections {
            stats.measurement_corrections += 1;
            stats.corrected_records += 1;
            corrections.push(correction);
        }

        // Step 2: 誤分類修正（温度範囲チェック + 隣接写真からのremarks伝搬）
        let reclassify_corrections = fix_misclassified_temperature(results);
        for correction in reclassify_corrections {
            stats.corrected_records += 1;
            corrections.push(correction);
        }

        // Step 3: 温度値のバリデーションと修正
        for result in results {
            let combined_text = format!("{} {} {}", result.remarks, result.description, result.detected_text);
            if measurements::is_temperature_photo(&combined_text) {
                let temp_type = measurements::TemperatureType::from_text(&combined_text);

                if !result.measurements.is_empty() {
                    if let Some(corrected) = measurements::validate_temperature(&result.measurements, temp_type.clone()) {
                        corrections.push(NormalizationCorrection {
                            file_name: result.file_name.clone(),
                            field: CorrectionField::Measurements,
                            original: result.measurements.clone(),
                            corrected: corrected.clone(),
                            reason: format!("温度値修正 ({:?}の妥当範囲外)", temp_type),
                        });
                        stats.measurement_corrections += 1;
                        stats.corrected_records += 1;
                    }
                }
            }
        }

        // Step 4: グループ単位での計測値統一
        // Step 1-3のcorrectionsを作業コピーに適用してからunifyを実行
        // （remarks修正がunifyのグループ化に影響するため）
        let mut working = results.to_vec();
        apply_corrections(&mut working, &corrections);
        let group_corrections = unify_measurements_by_group(&working);
        for correction in group_corrections {
            if !corrections.iter().any(|c| c.file_name == correction.file_name && c.field == CorrectionField::Measurements) {
                stats.measurement_corrections += 1;
                stats.corrected_records += 1;
                corrections.push(correction);
            }
        }
    }

    if options.unify_dekigata {
        let dekigata_corrections = unify_dekigata_measurements(
            results,
            options.dekigata_lane,
            options.dekigata_remarks.as_deref(),
        );
        for correction in dekigata_corrections {
            if !corrections.iter().any(|c| c.file_name == correction.file_name && c.field == correction.field) {
                match correction.field {
                    CorrectionField::Measurements => stats.measurement_corrections += 1,
                    _ => {}
                }
                stats.corrected_records += 1;
                corrections.push(correction);
            }
        }
    }

    NormalizationResult { corrections, stats }
}

/// detected_textから温度値を抽出してmeasurementsに設定する
///
/// measurementsが空の温度写真に対して、detected_textから対応する温度値を抽出する。
fn populate_temperature_measurements(results: &[AnalysisResult]) -> Vec<NormalizationCorrection> {
    let mut corrections = Vec::new();

    for result in results {
        // measurementsが既にあればスキップ
        if !result.measurements.is_empty() {
            continue;
        }

        // 温度写真かどうか判定
        let is_temp = measurements::is_temperature_photo(&result.remarks)
            || (result.photo_category == "品質管理写真"
                && measurements::is_temperature_photo(&result.detected_text));

        if !is_temp {
            continue;
        }

        // detected_textから備考に対応する温度値を抽出
        if let Some(value) = measurements::extract_temperature_for_remarks(
            &result.detected_text,
            &result.remarks,
        ) {
            corrections.push(NormalizationCorrection {
                file_name: result.file_name.clone(),
                field: CorrectionField::Measurements,
                original: String::new(),
                corrected: value,
                reason: "detected_textから温度値抽出".to_string(),
            });
        }
    }

    corrections
}

/// 温度写真の誤分類を修正する
///
/// 隣接する写真のremarksから、孤立した写真のremarksを修正する。
/// 例: 前後が「舗装日外気温」の写真が「開放温度」になっている場合、
/// detected_textに温度キーワードがなければ「舗装日外気温」に修正。
fn fix_misclassified_temperature(results: &[AnalysisResult]) -> Vec<NormalizationCorrection> {
    let mut corrections = Vec::new();

    for i in 0..results.len() {
        let r = &results[i];

        // 温度写真でなければスキップ
        if !measurements::is_temperature_photo(&r.remarks) {
            continue;
        }

        // detected_textに温度キーワード（到着/敷均し/初期/開放/外気温）が含まれていれば確定済み
        let dt = &r.detected_text;
        let has_strong_keyword = dt.contains("到着温度") || dt.contains("敷均し温度")
            || dt.contains("初期転圧前温度") || dt.contains("初期締固め前温度")
            || dt.contains("開放温度") || dt.contains("解放温度")
            || dt.contains("舗装日外気温") || dt.contains("外気温");

        if has_strong_keyword {
            continue;
        }

        // focusTargetが自身のremarksを支持している場合はスキップ
        // （focusTargetは視覚的判定なので、remarksと一致していれば正しい可能性が高い）
        if !r.focus_target.is_empty() && measurements::is_temperature_photo(&r.focus_target) {
            let ft_base = r.focus_target.trim_end_matches("測定");
            let rem_base = r.remarks.trim_end_matches("測定");
            if ft_base == rem_base {
                continue;
            }
        }

        // 前の写真のremarksを確認
        let prev_remarks = if i > 0 {
            let prev = &results[i - 1];
            if measurements::is_temperature_photo(&prev.remarks) {
                Some(prev.remarks.as_str())
            } else {
                None
            }
        } else {
            None
        };

        // 前の写真と同じremarks型であるべき場合、修正
        if let Some(prev_rem) = prev_remarks {
            if r.remarks != prev_rem {
                corrections.push(NormalizationCorrection {
                    file_name: r.file_name.clone(),
                    field: CorrectionField::Remarks,
                    original: r.remarks.clone(),
                    corrected: prev_rem.to_string(),
                    reason: format!("隣接写真({})からremarks伝搬", results[i - 1].file_name),
                });
            }
        }
    }

    corrections
}

/// 3枚セット内で黒板アップの計測値に統一する
///
/// 連続する同一remarks（温度種別）の写真をグループ化し、
/// focusTarget="黒板アップ"の値を他の写真に適用する
fn unify_measurements_by_group(results: &[AnalysisResult]) -> Vec<NormalizationCorrection> {
    let mut corrections = Vec::new();

    // 連続する同一remarksでグループ化
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current_group: Vec<usize> = Vec::new();
    let mut current_remarks: Option<&str> = None;

    for (i, result) in results.iter().enumerate() {
        // 温度写真判定: remarksまたはphoto_categoryで判定
        let is_temp = measurements::is_temperature_photo(&result.remarks)
            || (result.photo_category == "品質管理写真"
                && measurements::is_temperature_photo(&result.detected_text));

        if !is_temp {
            if !current_group.is_empty() {
                groups.push(std::mem::take(&mut current_group));
                current_remarks = None;
            }
            continue;
        }

        let remarks = result.remarks.as_str();
        if current_remarks == Some(remarks) {
            current_group.push(i);
        } else {
            if !current_group.is_empty() {
                groups.push(std::mem::take(&mut current_group));
            }
            current_group.push(i);
            current_remarks = Some(remarks);
        }
    }
    if !current_group.is_empty() {
        groups.push(current_group);
    }

    // 各グループでソース写真の値に統一
    for group in groups {
        // 黒板アップを優先、なければmeasurementsを持つ任意の写真をソースにする
        let source_idx = group.iter().find(|&&i| {
            results[i].focus_target == "黒板アップ" && !results[i].measurements.is_empty()
        }).or_else(|| {
            group.iter().find(|&&i| !results[i].measurements.is_empty())
        });

        if let Some(&src_idx) = source_idx {
            let source_value = results[src_idx].measurements.clone();

            // 他の写真の値を統一（空のものにも適用）
            for &idx in &group {
                if idx == src_idx {
                    continue;
                }
                let target = &results[idx];
                if target.measurements != source_value {
                    corrections.push(NormalizationCorrection {
                        file_name: target.file_name.clone(),
                        field: CorrectionField::Measurements,
                        original: target.measurements.clone(),
                        corrected: source_value.clone(),
                        reason: format!(
                            "グループ内統一({})",
                            results[src_idx].file_name
                        ),
                    });
                }
            }
        }
    }

    corrections
}

/// ダンプ台数を測点に付加する
///
/// 温度管理写真のdetected_textから「N台目」を抽出し、stationに追加。
/// 3枚セット（全景+黒板+温度計）内で、黒板写真の台数を全写真に伝搬する。
pub fn append_dump_number_to_station(results: &mut [AnalysisResult]) {
    // 隣接3枚セットで同一remarksのグループを処理
    let len = results.len();
    let mut i = 0;
    while i < len {
        // 温度写真でなければスキップ
        if !measurements::is_temperature_photo(&results[i].remarks) {
            i += 1;
            continue;
        }

        // 同一remarksの連続写真の範囲を特定
        let remarks = results[i].remarks.clone();
        let mut j = i + 1;
        while j < len && results[j].remarks == remarks {
            j += 1;
        }
        let group = &results[i..j];

        // グループ内のどれかのdetected_textから台数を抽出
        let dump_num = group.iter()
            .filter_map(|r| measurements::extract_dump_number(&r.detected_text))
            .next();

        if let Some(num) = dump_num {
            for r in &mut results[i..j] {
                // 既に台目が付いていればスキップ
                if r.station.contains("台目") {
                    continue;
                }
                let base = r.station.split('\n').next().unwrap_or(&r.station).to_string();
                r.station = format!("{} {}", base, num);
            }
        }

        i = j;
    }
}


/// 3枚セットを超える温度管理写真にskipフラグを設定する
///
/// 連続する同一remarksの温度写真グループが3枚を超える場合、
/// 4枚目以降にskip=trueを設定してエクスポートから除外する。
pub fn dedup_temperature_groups(results: &mut [AnalysisResult]) -> usize {
    let mut count = 0;
    let len = results.len();
    let mut i = 0;
    while i < len {
        if !measurements::is_temperature_photo(&results[i].remarks) {
            i += 1;
            continue;
        }

        // 同一remarksの連続写真の範囲を特定
        let remarks = results[i].remarks.clone();
        let mut j = i + 1;
        while j < len && results[j].remarks == remarks {
            j += 1;
        }

        // 3枚超過分をスキップ
        if j - i > 3 {
            for k in (i + 3)..j {
                eprintln!(
                    "  スキップ: {} [{}] ({}枚中{}枚目)",
                    results[k].file_name, remarks, j - i, k - i + 1
                );
                results[k].skip = true;
                count += 1;
            }
        }

        i = j;
    }
    count
}

/// 修正を適用する
///
/// # Arguments
/// * `results` - 解析結果（変更される）
/// * `corrections` - 適用する修正リスト
pub fn apply_corrections(
    results: &mut [AnalysisResult],
    corrections: &[NormalizationCorrection],
) {
    for correction in corrections {
        if let Some(result) = results.iter_mut().find(|r| r.file_name == correction.file_name) {
            match correction.field {
                CorrectionField::Measurements => result.measurements = correction.corrected.clone(),
                CorrectionField::Remarks => result.remarks = correction.corrected.clone(),
            }
        }
    }
}

/// 出来形管理写真のmeasurementsを統一する
///
/// 同一stationの出来形管理写真をグループ化し、
/// 管理用紙アップ写真のdetected_textから値をパースして
/// 全写真に統一フォーマットのmeasurementsを設定する。
fn unify_dekigata_measurements(
    results: &[AnalysisResult],
    lane_override: Option<Lane>,
    remarks_override: Option<&str>,
) -> Vec<NormalizationCorrection> {
    let mut corrections = Vec::new();

    // 出来形管理写真のインデックスを収集
    let dekigata_indices: Vec<usize> = results
        .iter()
        .enumerate()
        .filter(|(_, r)| dekigata::is_dekigata_photo(r))
        .map(|(i, _)| i)
        .collect();

    if dekigata_indices.is_empty() {
        return corrections;
    }

    // station でグループ化
    let mut station_groups: std::collections::BTreeMap<String, Vec<usize>> =
        std::collections::BTreeMap::new();
    for &idx in &dekigata_indices {
        let station = results[idx].station.clone();
        if !station.is_empty() {
            station_groups.entry(station).or_default().push(idx);
        }
    }

    // 各測点グループで統一
    for (_station, group) in &station_groups {
        if let Some(measurements) =
            dekigata::unify_dekigata_set(results, group, lane_override)
        {
            for &idx in group {
                let r = &results[idx];
                if r.measurements != measurements {
                    corrections.push(NormalizationCorrection {
                        file_name: r.file_name.clone(),
                        field: CorrectionField::Measurements,
                        original: r.measurements.clone(),
                        corrected: measurements.clone(),
                        reason: "出来形管理用紙OCRから統一".to_string(),
                    });
                }
            }
        }

        // 備考統一
        if let Some(remarks) = remarks_override {
            for &idx in group {
                let r = &results[idx];
                if r.remarks != remarks {
                    corrections.push(NormalizationCorrection {
                        file_name: r.file_name.clone(),
                        field: CorrectionField::Remarks,
                        original: r.remarks.clone(),
                        corrected: remarks.to_string(),
                        reason: "出来形管理写真の備考統一".to_string(),
                    });
                }
            }
        }
    }

    corrections
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_results_returns_empty() {
        let results = vec![
            AnalysisResult {
                file_name: "photo1.jpg".to_string(),
                measurements: "温度: 160℃".to_string(),
                ..Default::default()
            },
        ];

        let options = NormalizationOptions::default();
        let result = normalize_results(&results, &options);

        assert_eq!(result.stats.total_records, 1);
        assert!(result.corrections.is_empty());
    }

    #[test]
    fn test_unify_measurements_by_group() {
        // 3枚セット: 全景(147.6℃)、黒板アップ(149.6℃)、温度計アップ(149.6℃)
        let results = vec![
            AnalysisResult {
                file_name: "RIMG0188.JPG".to_string(),
                remarks: "初期締固め前温度".to_string(),
                measurements: "147.6℃".to_string(),
                focus_target: "全景".to_string(),
                photo_category: "品質管理写真".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                file_name: "RIMG0189.JPG".to_string(),
                remarks: "初期締固め前温度".to_string(),
                measurements: "149.6℃".to_string(),
                focus_target: "黒板アップ".to_string(),
                photo_category: "品質管理写真".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                file_name: "RIMG0190.JPG".to_string(),
                remarks: "初期締固め前温度".to_string(),
                measurements: "149.6℃".to_string(),
                focus_target: "温度計アップ".to_string(),
                photo_category: "品質管理写真".to_string(),
                ..Default::default()
            },
        ];

        let corrections = unify_measurements_by_group(&results);

        // 全景の値が黒板アップの値に修正されるべき
        assert_eq!(corrections.len(), 1);
        assert_eq!(corrections[0].file_name, "RIMG0188.JPG");
        assert_eq!(corrections[0].original, "147.6℃");
        assert_eq!(corrections[0].corrected, "149.6℃");
    }

    #[test]
    fn test_unify_measurements_multiple_groups() {
        // 2つの温度グループ
        let results = vec![
            // 到着温度グループ
            AnalysisResult {
                file_name: "IMG001.JPG".to_string(),
                remarks: "到着温度".to_string(),
                measurements: "160.0℃".to_string(),
                focus_target: "全景".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                file_name: "IMG002.JPG".to_string(),
                remarks: "到着温度".to_string(),
                measurements: "160.7℃".to_string(),
                focus_target: "黒板アップ".to_string(),
                ..Default::default()
            },
            // 敷均し温度グループ
            AnalysisResult {
                file_name: "IMG003.JPG".to_string(),
                remarks: "敷均し温度".to_string(),
                measurements: "155.0℃".to_string(),
                focus_target: "全景".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                file_name: "IMG004.JPG".to_string(),
                remarks: "敷均し温度".to_string(),
                measurements: "155.4℃".to_string(),
                focus_target: "黒板アップ".to_string(),
                ..Default::default()
            },
        ];

        let corrections = unify_measurements_by_group(&results);

        // 各グループで1件ずつ修正
        assert_eq!(corrections.len(), 2);
        assert_eq!(corrections[0].file_name, "IMG001.JPG");
        assert_eq!(corrections[0].corrected, "160.7℃");
        assert_eq!(corrections[1].file_name, "IMG003.JPG");
        assert_eq!(corrections[1].corrected, "155.4℃");
    }
}
