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
use chrono::NaiveDateTime;

const DEKIGATA_CONTIGUOUS_GAP_SECS: i64 = 5 * 60;

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
    Station,
}

impl std::fmt::Display for CorrectionField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CorrectionField::Measurements => write!(f, "計測値"),
            CorrectionField::Remarks => write!(f, "備考"),
            CorrectionField::Station => write!(f, "測点"),
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
                if correction.field == CorrectionField::Measurements { stats.measurement_corrections += 1 }
                stats.corrected_records += 1;
                corrections.push(correction);
            }
        }
    }

    // Step 5: グループ単位でのステーション統一
    {
        let mut working = results.to_vec();
        apply_corrections(&mut working, &corrections);
        let station_corrections = unify_station_by_group(&working);
        for correction in station_corrections {
            if !corrections.iter().any(|c| c.file_name == correction.file_name && c.field == CorrectionField::Station) {
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

        // detected_textから温度値を抽出（focus_targetを優先、なければremarks）
        let extraction_key = if crate::temperature::TemperatureKind::from_text(&result.focus_target).is_some() {
            &result.focus_target
        } else {
            &result.remarks
        };
        if let Some(value) = measurements::extract_temperature_for_remarks(
            &result.detected_text,
            extraction_key,
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

/// ボードアップ写真かどうかを判定する
///
/// ボードアップ = 黒板・管理用紙等の計測データをクローズアップした写真。
/// 温度管理なら"黒板アップ"、出来形なら"管理値"を含むfocus_target。
fn is_board_up_photo(focus_target: &str) -> bool {
    focus_target == "黒板アップ" || focus_target.contains("管理値")
}

fn parse_result_datetime(date: &str) -> Option<NaiveDateTime> {
    let formats = [
        "%Y-%m-%d %H:%M:%S",
        "%Y/%m/%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y/%m/%d %H:%M",
    ];
    for fmt in formats {
        if let Ok(dt) = NaiveDateTime::parse_from_str(date, fmt) {
            return Some(dt);
        }
    }
    None
}

fn split_group_by_time_gap(
    results: &[AnalysisResult],
    indices: &[usize],
    gap_secs: i64,
) -> Vec<Vec<usize>> {
    if indices.is_empty() {
        return Vec::new();
    }

    let mut ordered: Vec<usize> = indices.to_vec();
    ordered.sort_by(|&a, &b| {
        results[a]
            .date
            .cmp(&results[b].date)
            .then(results[a].file_name.cmp(&results[b].file_name))
    });

    if ordered
        .iter()
        .any(|&i| parse_result_datetime(&results[i].date).is_none())
    {
        return vec![ordered];
    }

    let mut chunks: Vec<Vec<usize>> = Vec::new();
    let mut current = vec![ordered[0]];
    let mut prev = parse_result_datetime(&results[ordered[0]].date)
        .expect("datetime checked above");

    for &idx in ordered.iter().skip(1) {
        let dt = parse_result_datetime(&results[idx].date)
            .expect("datetime checked above");
        let gap = (dt - prev).num_seconds().abs();
        if gap > gap_secs {
            chunks.push(std::mem::take(&mut current));
        }
        current.push(idx);
        prev = dt;
    }

    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn extract_station_no(text: &str) -> Option<String> {
    for marker in ["No.", "No ", "NO.", "NO "] {
        if let Some(pos) = text.find(marker) {
            let rest = &text[pos + marker.len()..];
            let digits: String = rest
                .chars()
                .skip_while(|c| !c.is_ascii_digit())
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if !digits.is_empty() {
                return Some(format!("No.{}", digits));
            }
        }
    }
    None
}

fn station_hint_from_result(result: &AnalysisResult) -> Option<String> {
    // 機械系写真は検査番号等を測点と誤認するためスキップ
    if result.photo_category == "その他" {
        return None;
    }
    let merged = format!(
        "{} {} {}",
        result.detected_text, result.description, result.remarks
    );
    let no = extract_station_no(&merged)?;
    if merged.contains("取付道路") || merged.contains("取付") {
        Some(format!("取付道路 {}", no))
    } else {
        Some(no)
    }
}

/// グループ内でボードアップ写真の計測値に統一する
///
/// Phase 1: real group番号(解析 engine)によるグループ化 → 全写真対象
/// Phase 2: group=0の温度写真は連続同一remarksでグループ化（後方互換）
///
/// ボードアップ写真がなければ伝播しない（値なし）。
fn unify_measurements_by_group(results: &[AnalysisResult]) -> Vec<NormalizationCorrection> {
    let mut corrections = Vec::new();
    let mut groups: Vec<Vec<usize>> = Vec::new();

    // Phase 1: real group番号によるグループ化（全写真対象）
    let mut real_groups: std::collections::HashMap<u32, Vec<usize>> =
        std::collections::HashMap::new();
    let mut ungrouped_temp: Vec<usize> = Vec::new();

    for (i, result) in results.iter().enumerate() {
        if dekigata::is_dekigata_photo(result) {
            continue;
        }
        if result.group > 0 {
            real_groups
                .entry(result.group)
                .or_default()
                .push(i);
        } else if measurements::is_temperature_photo(&result.remarks) {
            ungrouped_temp.push(i);
        }
    }

    for indices in real_groups.values() {
        if indices.len() >= 2 {
            groups.push(indices.clone());
        }
    }

    // Phase 2: group=0 の温度写真は既存の連続同一remarksロジックでグループ化（後方互換）
    let mut current_group: Vec<usize> = Vec::new();
    let mut current_remarks: Option<&str> = None;
    for &i in &ungrouped_temp {
        let remarks = results[i].remarks.as_str();
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

    // 各グループでボードアップ写真の値に統一
    for group in groups {
        // ボードアップ写真を優先ソースとする。なければ伝播しない
        let source_idx = group.iter().find(|&&i| {
            is_board_up_photo(&results[i].focus_target) && !results[i].measurements.is_empty()
        }).or_else(|| {
            // 後方互換: ボードアップがなくてもmeasurementsを持つ写真があればソースに
            // （温度管理では黒板アップ以外にも温度計アップ等が値を持つ）
            group.iter().find(|&&i| !results[i].measurements.is_empty())
        });

        if let Some(&src_idx) = source_idx {
            let source_value = results[src_idx].measurements.clone();

            for &idx in &group {
                if idx == src_idx { continue; }
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

/// グループ内でステーションを統一する
///
/// 解析 engine のグループ内で、ボードアップ写真またはhas_board=trueの写真の
/// stationを代表値として全写真に適用する。
fn unify_station_by_group(results: &[AnalysisResult]) -> Vec<NormalizationCorrection> {
    let mut corrections = Vec::new();

    // real group番号によるグループ化（全写真対象）
    let mut real_groups: std::collections::HashMap<u32, Vec<usize>> =
        std::collections::HashMap::new();

    for (i, result) in results.iter().enumerate() {
        if result.group > 0 {
            real_groups.entry(result.group).or_default().push(i);
        }
    }

    for indices in real_groups.values() {
        for chunk in split_group_by_time_gap(results, indices, DEKIGATA_CONTIGUOUS_GAP_SECS) {
            if chunk.len() < 2 {
                continue;
            }

            // ステーション代表値の選定:
            // 1. ボードアップ写真で非空station
            // 2. has_board=trueの写真で非空station（最長値＝最も詳細）
            // 3. 任意の非空station（最長値）
            let hint_stations: Vec<String> = chunk
                .iter()
                .filter_map(|&i| station_hint_from_result(&results[i]))
                .collect();
            let representative_station = hint_stations
                .iter()
                .find(|s| s.contains("取付道路"))
                .cloned()
                .or_else(|| hint_stations.into_iter().next())
                .or_else(|| {
                    chunk
                        .iter()
                        .filter(|&&i| {
                            is_board_up_photo(&results[i].focus_target)
                                && !results[i].station.is_empty()
                        })
                        .map(|&i| results[i].station.as_str().to_string())
                        .next()
                })
                .or_else(|| {
                    chunk
                        .iter()
                        .filter(|&&i| results[i].has_board && !results[i].station.is_empty())
                        .map(|&i| results[i].station.as_str().to_string())
                        .max_by_key(|s| s.len())
                })
                .or_else(|| {
                    chunk
                        .iter()
                        .filter(|&&i| !results[i].station.is_empty())
                        .map(|&i| results[i].station.as_str().to_string())
                        .max_by_key(|s| s.len())
                });

            if let Some(station_owned) = representative_station {
                for &idx in &chunk {
                    if results[idx].station != station_owned {
                        corrections.push(NormalizationCorrection {
                            file_name: results[idx].file_name.clone(),
                            field: CorrectionField::Station,
                            original: results[idx].station.clone(),
                            corrected: station_owned.clone(),
                            reason: "グループ内ステーション統一".to_string(),
                        });
                    }
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
    // real group番号によるグループ化
    let mut real_groups: std::collections::HashMap<(u32, String), Vec<usize>> =
        std::collections::HashMap::new();
    let mut ungrouped_temp_indices: Vec<usize> = Vec::new();

    for (i, r) in results.iter().enumerate() {
        if !measurements::is_temperature_photo(&r.remarks) {
            continue;
        }
        if r.group > 0 {
            real_groups
                .entry((r.group, r.remarks.clone()))
                .or_default()
                .push(i);
        } else {
            ungrouped_temp_indices.push(i);
        }
    }

    // real groupで処理
    for indices in real_groups.values() {
        let dump_num = indices
            .iter()
            .filter_map(|&i| measurements::extract_dump_number(&results[i].detected_text))
            .next();
        if let Some(num) = dump_num {
            for &i in indices {
                if results[i].station.contains("台目") {
                    continue;
                }
                let base = results[i].station.split('\n').next().unwrap_or(&results[i].station).to_string();
                results[i].station = format!("{} {}", base, num);
            }
        }
    }

    // group=0: 既存の連続同一remarksロジック
    let mut i = 0;
    while i < ungrouped_temp_indices.len() {
        let idx = ungrouped_temp_indices[i];
        let remarks = results[idx].remarks.clone();
        let mut j = i + 1;
        while j < ungrouped_temp_indices.len()
            && results[ungrouped_temp_indices[j]].remarks == remarks
        {
            j += 1;
        }
        let group_indices = &ungrouped_temp_indices[i..j];
        let dump_num = group_indices
            .iter()
            .filter_map(|&gi| measurements::extract_dump_number(&results[gi].detected_text))
            .next();
        if let Some(num) = dump_num {
            for &gi in group_indices {
                if results[gi].station.contains("台目") {
                    continue;
                }
                let base = results[gi].station.split('\n').next().unwrap_or(&results[gi].station).to_string();
                results[gi].station = format!("{} {}", base, num);
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

    // real group番号によるグループ化
    let mut real_groups: std::collections::HashMap<(u32, String), Vec<usize>> =
        std::collections::HashMap::new();
    let mut ungrouped_temp_indices: Vec<usize> = Vec::new();

    for (i, r) in results.iter().enumerate() {
        if !measurements::is_temperature_photo(&r.remarks) {
            continue;
        }
        if r.group > 0 {
            real_groups
                .entry((r.group, r.remarks.clone()))
                .or_default()
                .push(i);
        } else {
            ungrouped_temp_indices.push(i);
        }
    }

    // real groupで3枚超過分をスキップ
    for ((_, remarks), indices) in &real_groups {
        if indices.len() > 3 {
            for (offset, &idx) in indices[3..].iter().enumerate() {
                eprintln!(
                    "  スキップ: {} [{}] ({}枚中{}枚目)",
                    results[idx].file_name, remarks, indices.len(), offset + 4
                );
                results[idx].skip = true;
                count += 1;
            }
        }
    }

    // group=0: 既存の連続同一remarksロジック
    let mut i = 0;
    while i < ungrouped_temp_indices.len() {
        let idx = ungrouped_temp_indices[i];
        let remarks = results[idx].remarks.clone();
        let mut j = i + 1;
        while j < ungrouped_temp_indices.len()
            && results[ungrouped_temp_indices[j]].remarks == remarks
        {
            j += 1;
        }
        let group_indices = &ungrouped_temp_indices[i..j];
        if group_indices.len() > 3 {
            for (offset, &gi) in group_indices[3..].iter().enumerate() {
                eprintln!(
                    "  スキップ: {} [{}] ({}枚中{}枚目)",
                    results[gi].file_name, remarks, group_indices.len(), offset + 4
                );
                results[gi].skip = true;
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
                CorrectionField::Station => result.station = correction.corrected.clone(),
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

// 解析 engine の group番号でグループ化（唯一の基準）
    let mut tagger_groups: std::collections::BTreeMap<u32, Vec<usize>> =
        std::collections::BTreeMap::new();
    for &idx in &dekigata_indices {
        let g = results[idx].group;
        if g > 0 {
            tagger_groups.entry(g).or_default().push(idx);
        }
    }

// 各解析 engine グループで統一
    for group in tagger_groups.values() {
        // 同一groupに別測点セットが混在するケースを避けるため、
        // 時刻差5分超でサブグループに分割して統一する。
        for chunk in split_group_by_time_gap(results, group, DEKIGATA_CONTIGUOUS_GAP_SECS) {
            // ラベル判定:
            // サブグループ内のどれか1枚でも「表層」分類なら「計画高」、
            // それ以外（切削等）は「切削基準高」。
            let is_surface_layer = chunk.iter().any(|&idx| {
                let r = &results[idx];
                r.subphase.contains("表層")
                    || r.remarks.contains("表層")
                    || r.description.contains("表層")
                    || r.focus_target.contains("表層")
            });
            let _label = if is_surface_layer { "計画高" } else { "切削基準高" };

            if let Some(measurements) =
                dekigata::unify_dekigata_set(results, &chunk, lane_override)
            {
                for &idx in &chunk {
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
                for &idx in &chunk {
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
    fn test_unify_measurements_general_group_core() {
        // コアー測定グループ: group=2, ボードアップ(管理値)の値がグループ全体に伝播
        let results = vec![
            AnalysisResult {
                file_name: "R0010584.JPG".to_string(),
                remarks: "舗装厚測定".to_string(),
                measurements: "".to_string(),
                focus_target: "舗装厚測定".to_string(),
                has_board: true,
                station: "No.1".to_string(),
                group: 2,
                ..Default::default()
            },
            AnalysisResult {
                file_name: "R0010585.JPG".to_string(),
                remarks: "舗装厚測定".to_string(),
                measurements: "".to_string(),
                focus_target: "舗装厚測定・接写".to_string(),
                has_board: false,
                station: "".to_string(),
                group: 2,
                ..Default::default()
            },
            AnalysisResult {
                file_name: "R0010586.JPG".to_string(),
                remarks: "舗装厚測定".to_string(),
                measurements: "設計厚 t=50mm, 実測平均 52.50mm".to_string(),
                focus_target: "舗装厚測定・管理値".to_string(),
                has_board: true,
                station: "No.1".to_string(),
                group: 2,
                ..Default::default()
            },
        ];

        let corrections = unify_measurements_by_group(&results);

        // R0010584とR0010585が管理値写真の値に統一される
        assert_eq!(corrections.len(), 2, "corrections: {:?}", corrections);
        assert!(corrections.iter().any(|c|
            c.file_name == "R0010584.JPG"
            && c.corrected == "設計厚 t=50mm, 実測平均 52.50mm"
        ));
        assert!(corrections.iter().any(|c|
            c.file_name == "R0010585.JPG"
            && c.corrected == "設計厚 t=50mm, 実測平均 52.50mm"
        ));
    }

    #[test]
    fn test_unify_station_by_group() {
        // コアー写真グループ: has_board=trueの写真のstationをグループ全体に伝播
        let results = vec![
            AnalysisResult {
                file_name: "R0010582.JPG".to_string(),
                station: "No.1 右車線".to_string(),
                has_board: true,
                focus_target: "コアー採取前".to_string(),
                group: 2,
                ..Default::default()
            },
            AnalysisResult {
                file_name: "R0010583.JPG".to_string(),
                station: "No.1 右車線".to_string(),
                has_board: true,
                focus_target: "コアー採取状況".to_string(),
                group: 2,
                ..Default::default()
            },
            AnalysisResult {
                file_name: "R0010585.JPG".to_string(),
                station: "".to_string(),
                has_board: false,
                focus_target: "舗装厚測定・接写".to_string(),
                group: 2,
                ..Default::default()
            },
        ];

        let corrections = unify_station_by_group(&results);

        // 接写写真(station空)にstationが伝播
        assert_eq!(corrections.len(), 1);
        assert_eq!(corrections[0].file_name, "R0010585.JPG");
        assert_eq!(corrections[0].corrected, "No.1 右車線");
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

    #[test]
    fn test_unify_measurements_by_group_skips_dekigata_photos() {
        let results = vec![
            AnalysisResult {
                file_name: "D1.JPG".to_string(),
                photo_category: "出来形管理写真".to_string(),
                group: 1,
                measurements: "A".to_string(),
                focus_target: "出来形管理".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                file_name: "D2.JPG".to_string(),
                photo_category: "出来形管理写真".to_string(),
                group: 1,
                measurements: "B".to_string(),
                focus_target: "出来形管理".to_string(),
                ..Default::default()
            },
        ];
        let corrections = unify_measurements_by_group(&results);
        assert!(corrections.is_empty());
    }

    #[test]
    fn test_unify_dekigata_uses_design_height_for_surface_group() {
        let ocr = "出来形管理用紙 No.1, 切削高(設計) V1=9.819 V2=9.842 V3=9.861 V4=9.826 V5=9.785, \
                   切削高(実施) V1=9.815 V2=9.842 V3=9.860 V4=9.825 V5=9.780, \
                   左幅員 設計4.20 実測4.20, 右幅員 設計4.13 実測4.13";

        let results = vec![
            AnalysisResult {
                file_name: "surface_overview.jpg".to_string(),
                photo_category: "出来形管理写真".to_string(),
                station: "No.1".to_string(),
                subphase: "表層工".to_string(),
                group: 1,
                ..Default::default()
            },
            AnalysisResult {
                file_name: "surface_board.jpg".to_string(),
                photo_category: "出来形管理写真".to_string(),
                station: "No.1".to_string(),
                detected_text: ocr.to_string(),
                group: 1,
                ..Default::default()
            },
        ];

        let corrections = unify_dekigata_measurements(&results, Some(Lane::Left), None);
        assert!(!corrections.is_empty());
        assert!(corrections
            .iter()
            .filter(|c| c.field == CorrectionField::Measurements)
            .all(|c| c.corrected.contains("V1=9.819")));
    }

    #[test]
    fn test_unify_dekigata_skips_group_zero() {
        let ocr = "出来形管理用紙 No.1, 計画高(設計) V1=9.869 V2=9.892 V3=9.911 V4=9.876 V5=9.835, \
                   計画高(実施) V1=9.870 V2=9.895 V3=9.913 V4=9.878 V5=9.835, \
                   左幅員 設計4.20 実測4.20, 右幅員 設計4.13 実測4.13";
        let results = vec![
            AnalysisResult {
                file_name: "g0_overview.jpg".to_string(),
                photo_category: "出来形管理写真".to_string(),
                subphase: "表層工".to_string(),
                group: 0,
                ..Default::default()
            },
            AnalysisResult {
                file_name: "g0_board.jpg".to_string(),
                photo_category: "出来形管理写真".to_string(),
                detected_text: ocr.to_string(),
                group: 0,
                ..Default::default()
            },
        ];

        let corrections = unify_dekigata_measurements(&results, Some(Lane::Both), None);
        assert!(corrections.is_empty());
    }

    #[test]
    fn test_unify_dekigata_splits_same_group_by_time_gap() {
        let ocr_main = "出来形管理用紙 No.1, 計画高(設計) V1=9.869 V2=9.892 V3=9.911 V4=9.876 V5=9.835, \
                        計画高(実施) V1=9.870 V2=9.895 V3=9.913 V4=9.878 V5=9.835, \
                        左幅員 設計4.20 実測4.20, 右幅員 設計4.13 実測4.13";
        let ocr_attach = "出来形管理用紙 No.1, 計画高(設計) V1=10.609 V2=10.636 V3=10.678 V4=10.632 V5=10.605, \
                          計画高(実施) V1=10.611 V2=10.637 V3=10.678 V4=10.635 V5=10.607, \
                          左幅員 設計3.23 実測3.23, 右幅員 設計3.15 実測3.15";
        let results = vec![
            AnalysisResult {
                file_name: "A_overview.jpg".to_string(),
                photo_category: "出来形管理写真".to_string(),
                subphase: "表層工".to_string(),
                date: "2026-02-18 00:36:13".to_string(),
                group: 1,
                ..Default::default()
            },
            AnalysisResult {
                file_name: "A_board.jpg".to_string(),
                photo_category: "出来形管理写真".to_string(),
                subphase: "表層工".to_string(),
                detected_text: ocr_main.to_string(),
                date: "2026-02-18 00:36:43".to_string(),
                group: 1,
                ..Default::default()
            },
            AnalysisResult {
                file_name: "B_overview.jpg".to_string(),
                photo_category: "出来形管理写真".to_string(),
                subphase: "表層工".to_string(),
                date: "2026-02-18 01:47:25".to_string(),
                group: 1,
                ..Default::default()
            },
            AnalysisResult {
                file_name: "B_board.jpg".to_string(),
                photo_category: "出来形管理写真".to_string(),
                subphase: "表層工".to_string(),
                detected_text: ocr_attach.to_string(),
                date: "2026-02-18 01:48:20".to_string(),
                group: 1,
                ..Default::default()
            },
        ];

        let corrections = unify_dekigata_measurements(&results, Some(Lane::Both), None);
        let a = corrections
            .iter()
            .find(|c| c.file_name == "A_overview.jpg" && c.field == CorrectionField::Measurements)
            .unwrap();
        let b = corrections
            .iter()
            .find(|c| c.file_name == "B_overview.jpg" && c.field == CorrectionField::Measurements)
            .unwrap();
        assert!(a.corrected.contains("V1=9.869"));
        assert!(b.corrected.contains("V1=10.609"));
    }

    #[test]
    fn test_unify_station_splits_same_group_by_time_gap() {
        let results = vec![
            AnalysisResult {
                file_name: "A1.jpg".to_string(),
                group: 1,
                has_board: true,
                station: "No.1".to_string(),
                date: "2026-02-18 00:36:13".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                file_name: "A2.jpg".to_string(),
                group: 1,
                station: "".to_string(),
                date: "2026-02-18 00:36:43".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                file_name: "B1.jpg".to_string(),
                group: 1,
                has_board: true,
                station: "取付道路 No.1".to_string(),
                date: "2026-02-18 01:47:25".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                file_name: "B2.jpg".to_string(),
                group: 1,
                station: "".to_string(),
                date: "2026-02-18 01:48:20".to_string(),
                ..Default::default()
            },
        ];

        let corrections = unify_station_by_group(&results);
        let a2 = corrections.iter().find(|c| c.file_name == "A2.jpg").unwrap();
        let b2 = corrections.iter().find(|c| c.file_name == "B2.jpg").unwrap();
        assert_eq!(a2.corrected, "No.1");
        assert_eq!(b2.corrected, "取付道路 No.1");
    }

    #[test]
    fn test_unify_station_uses_detected_text_hint() {
        let results = vec![
            AnalysisResult {
                file_name: "main_board.jpg".to_string(),
                group: 1,
                station: "取付道路 No.1".to_string(),
                detected_text: "出来形管理用紙 No.1 計画高(設計)".to_string(),
                date: "2026-02-18 00:36:43".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                file_name: "main_overview.jpg".to_string(),
                group: 1,
                station: "".to_string(),
                date: "2026-02-18 00:36:13".to_string(),
                ..Default::default()
            },
        ];

        let corrections = unify_station_by_group(&results);
        assert!(corrections.iter().any(|c| c.file_name == "main_board.jpg" && c.corrected == "No.1"));
        assert!(corrections.iter().any(|c| c.file_name == "main_overview.jpg" && c.corrected == "No.1"));
    }
}
