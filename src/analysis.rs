//! 解析関連のヘルパー関数とデータ構造
//!
//! AI写真解析のためのスキャン、解析、正規化処理を提供する。
//! OCR解析・マスタ照合・線種判定は各専用モジュールに委譲し、
//! このモジュールはパイプラインのオーケストレーションに専念する。

use crate::{ai_provider::AiProvider, analyzer, error, scanner};
use crate::domain::*;
use crate::normalizer::{self, NormalizationOptions};
use crate::normalizer::measurements::{
    extract_dump_number, extract_temperature, extract_temperature_for_remarks, is_temperature_photo,
};
use crate::ocr_parser::{extract_kv_from_text, normalize_station};
use crate::master_matcher::{
    date_to_month_day, match_master_from_detected_texts, role_priority,
};
use crate::line_type_detector::detect_line_type;
use photo_ai_common::{HierarchyMaster, LineTypeEntry};
use photo_tagger::GroupRecords;
use std::path::{Path, PathBuf};

use crate::error::Result;

fn should_auto_date_station(remarks: &str) -> bool {
    matches!(
        remarks,
        "安全朝礼実施状況" | "KY活動状況" | "新規入場者教育状況" | "安全訓練実施状況"
    )
}

fn extract_tonnage_from_text(text: &str) -> Option<String> {
    for key in ["計量積載量", "積載量", "数量"] {
        if let Some(pos) = text.find(key) {
            let tail = &text[pos + key.len()..];
            let tail = tail.trim_start_matches(&[':', '：', ' ', '　'][..]);
            let num: String = tail
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if !num.is_empty() {
                return Some(format!("積載量：{}ｔ", num));
            }
        }
    }
    None
}

/// detected_textから「出来形管理用紙 No.X」の測点を抽出
fn extract_dekigata_station(text: &str) -> Option<String> {
    crate::folder_rules::extract_dekigata_station(text)
}

/// スキャンと解析の設定
pub struct ScanAnalysisConfig<'a> {
    pub folder: &'a Path,
    pub batch_size: usize,
    pub verbose: bool,
    pub master_config: &'a MasterConfig,
    pub photo_type: Option<&'a str>,
    pub use_cache: bool,
    pub provider: AiProvider,
    pub variety: Option<&'a String>,
    pub station: Option<&'a String>,
    pub recursive: bool,
    pub include_all: bool,
    pub step_prefix_scan: &'a str,
    pub step_prefix_analyze: &'a str,
    pub line_types: Option<&'a [LineTypeEntry]>,
}

/// マスタ選択結果
pub struct MasterConfig {
    pub master_path: Option<PathBuf>,
    pub effective_work_type: Option<String>,
    /// 全工種時のマージ用パス一覧（by_work_type/*.csv + メインCSV）
    pub all_paths: Option<Vec<PathBuf>>,
}

/// マスタ選択と検証を行う共通関数
pub fn prepare_analysis(
    master: Option<PathBuf>,
    work_type: Option<String>,
    variety: Option<&String>,
    resolve_master_path: impl FnOnce(Option<PathBuf>, bool) -> Option<crate::master_selector::MasterSelection>,
) -> Result<MasterConfig> {
    // マスタ選択（対話式または引数から）
    let has_master_arg = master.is_some();
    let selection = resolve_master_path(master, !has_master_arg && work_type.is_none());

    // work_type: CLI引数優先、なければ選択結果から
    let effective_work_type = work_type.or_else(|| selection.as_ref().and_then(|s| s.work_type.clone()));
    let all_paths = selection.as_ref().and_then(|s| s.all_paths.clone());
    let master_path = selection.map(|s| s.path);

    // 検証
    if variety.is_some() && effective_work_type.is_none() {
        return Err(error::PhotoAiError::InvalidMaster(
            "variety指定にはwork_typeが必要です".to_string(),
        ));
    }
    if effective_work_type.is_some() && master_path.is_none() {
        return Err(error::PhotoAiError::MasterLoad(
            "work_type指定にはマスタが必要です".to_string(),
        ));
    }

    Ok(MasterConfig {
        master_path,
        effective_work_type,
        all_paths,
    })
}

/// MasterConfigからHierarchyMasterを読み込む
///
/// all_pathsがある場合はマージ読み込み、なければ単一ファイル読み込み
fn load_master_from_config(config: &MasterConfig) -> Result<HierarchyMaster> {
    if let Some(ref all_paths) = config.all_paths {
        HierarchyMaster::from_csv_files(all_paths)
            .map_err(|e| error::PhotoAiError::MasterLoad(e.to_string()))
    } else if let Some(ref path) = config.master_path {
        HierarchyMaster::from_csv(path)
            .map_err(|e| error::PhotoAiError::MasterLoad(e.to_string()))
    } else {
        // マスタパスなし: by_work_type全ファイルをマージ読み込み
        let all_paths = crate::master_selector::collect_all_master_paths();
        if let Some(paths) = all_paths {
            HierarchyMaster::from_csv_files(&paths)
                .map_err(|e| error::PhotoAiError::MasterLoad(e.to_string()))
        } else {
            Err(error::PhotoAiError::MasterLoad("マスタファイルが見つかりません".to_string()))
        }
    }
}

/// スキャンから正規化までを行う共通関数
pub async fn scan_and_analyze(config: &ScanAnalysisConfig<'_>) -> Result<Vec<analyzer::AnalysisResult>> {
    // 1. 画像スキャン
    let images = scan_images(
        config.folder,
        config.recursive,
        config.include_all,
        config.step_prefix_scan,
    )?;

    // 2. マスタ読み込み + work_type/photo_typeでフィルタ
    let mut master = load_master_from_config(config.master_config)?;
    if let Some(ref wt) = config.master_config.effective_work_type {
        let before = master.rows().len();
        master = master.filter_by_work_types(&[wt.clone()]);
        println!("  マスタフィルタ: 工種={} ({} → {}件)", wt, before, master.rows().len());
    }
    if let Some(pt) = config.photo_type {
        let before = master.rows().len();
        master = master.filter_by_photo_type(pt);
        println!("  マスタフィルタ: 写真区分={} ({} -> {}件)", pt, before, master.rows().len());
    }
    let vocabulary = master.extract_vocabulary();

    // 3. photo-tagger（語彙リスト付き）
    println!("{} photo-tagger実行中...", config.step_prefix_analyze);
    let vocab_ref = if vocabulary.is_empty() { None } else { Some(vocabulary.as_slice()) };
    let group_records = photo_tagger::run_grouping(config.folder, config.batch_size, vocab_ref)
        .map_err(|e| error::PhotoAiError::ApiCall(format!("photo-tagger: {}", e)))?;

    if group_records.is_empty() {
        return Err(error::PhotoAiError::NoImagesFound(
            format!("photo-taggerの結果が空: {}", config.folder.display())
        ));
    }

    let grouped_images: Vec<_> = images.iter()
        .filter(|img| group_records.contains_key(&img.file_name))
        .cloned()
        .collect();
    let skipped = images.len() - grouped_images.len();
    if skipped > 0 {
        println!("  {} 枚をスキップ（グループ未登録）", skipped);
    }

    let folder_name = config.folder.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let folder_context = config.folder.display().to_string();
    let mut results = convert_groups_to_results(
        &grouped_images,
        &group_records,
        &master,
        folder_name,
        &folder_context,
        config.line_types,
    );
    println!("✓ マスタ照合完了（{}枚）\n", results.len());

    // 4. 正規化
    normalize_results_with_station(
        &mut results,
        config.station.map(|s| s.as_str()),
        config.verbose,
    );

    apply_folder_specific_corrections(&mut results, &folder_context);
    apply_temperature_folder_final_adjustments(&mut results, &folder_context);

    Ok(results)
}

/// 画像スキャンのみを実行
///
/// # Arguments
/// * `folder` - スキャン対象フォルダ
/// * `recursive` - 再帰スキャンするか
/// * `include_all` - 全ファイルを含めるか（falseの場合はJPEG/PNGのみ）
/// * `step_prefix` - ステップ表示用プレフィックス（例: "[1/3]"）
///
/// # Returns
/// スキャンされた画像情報のベクタ
pub fn scan_images(
    folder: &Path,
    recursive: bool,
    include_all: bool,
    step_prefix: &str,
) -> Result<Vec<scanner::ImageInfo>> {
    println!("{} 写真をスキャン中...{}", step_prefix, if recursive { " (再帰)" } else { "" });
    let images = scanner::scan_folder_full(folder, recursive, !include_all)?;
    println!("✓ {}枚の写真を検出\n", images.len());

    if images.is_empty() {
        return Err(error::PhotoAiError::NoImagesFound(
            folder.display().to_string()
        ));
    }

    Ok(images)
}

/// 測点適用と正規化を実行
///
/// # Arguments
/// * `results` - 解析結果（変更される）
/// * `station` - 適用する測点（Noneの場合はスキップ）
/// * `verbose` - 詳細出力するか
pub fn normalize_results_with_station(
    results: &mut Vec<analyzer::AnalysisResult>,
    station: Option<&str>,
    verbose: bool,
) {
    // 測点一括適用
    if let Some(st) = station {
        println!("  測点を一括適用: {}", st);
        apply_station(results, st);
    }

    // 正規化（3枚セット内で黒板アップの値に統一）
    let norm_options = NormalizationOptions::default();
    let norm_result = normalizer::normalize_results(results, &norm_options);
    if !norm_result.corrections.is_empty() {
        if verbose {
            println!("  計測値統一: {}件", norm_result.stats.measurement_corrections);
            for c in &norm_result.corrections {
                println!("    {} → {} ({})", c.file_name, c.corrected, c.reason);
            }
        }
        normalizer::apply_corrections(results, &norm_result.corrections);
    }
}

/// photo-groups.json のレコードを AnalysisResult に変換し、マスタ照合する
/// グループ番号→全景優先でソート
fn convert_groups_to_results(
    images: &[scanner::ImageInfo],
    groups: &GroupRecords,
    master: &HierarchyMaster,
    folder_name: &str,
    folder_context: &str,
    line_types: Option<&[LineTypeEntry]>,
) -> Vec<analyzer::AnalysisResult> {
    let mut results: Vec<(u32, u8, analyzer::AnalysisResult)> = images.iter().filter_map(|img| {
        let rec = groups.get(&img.file_name)?;
        let mut result = analyzer::AnalysisResult {
            file_name: img.file_name.clone(),
            file_path: img.path.display().to_string(),
            date: img.date.as_deref().unwrap_or("").to_string(),
            ..Default::default()
        };

        result.has_board = rec.has_board;
        result.detected_text = rec.detected_text.clone();
        result.description = rec.description.clone();
        result.focus_target = rec.role.clone();

        // 写真ごとのdetected_textからキー:値を抽出
        let kvs = extract_kv_from_text(&rec.detected_text);

        // 測点: この写真の黒板OCRから「場所」を取得
        let photo_station = kvs.iter()
            .find(|(k, _)| k == "場所" || k == "測点")
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        // 出来形管理用紙からNo.Xを抽出（場所/測点キーがない場合）
        let photo_station = if photo_station.is_empty() {
            extract_dekigata_station(&rec.detected_text).unwrap_or_default()
        } else {
            photo_station
        };
        result.station = normalize_station(&photo_station);

        // 写真ごとにマスタ照合（混在フォルダ対応）
        // machine_typeもキーワードに追加して照合精度を向上
        let ft = if rec.role.is_empty() { None } else { Some(rec.role.as_str()) };
        let photo_matched_row = if !rec.detected_text.is_empty() {
            let combined = if !rec.machine_type.is_empty() {
                format!("{}\n{}", rec.detected_text, rec.machine_type)
            } else {
                rec.detected_text.clone()
            };
            let texts = vec![combined.as_str()];
            match_master_from_detected_texts(master, &texts, folder_name, ft)
        } else if !rec.machine_type.is_empty() {
            // detected_text空でもmachine_typeがあればキーワードとして使う
            let texts = vec![rec.machine_type.as_str()];
            match_master_from_detected_texts(master, &texts, folder_name, ft)
        } else {
            // detected_text空、machine_type空ならフォルダ名のみで照合
            match_master_from_detected_texts(master, &[], folder_name, ft)
        };

        // マスタ照合結果を適用
        if let Some(row) = &photo_matched_row {
            result.photo_category = row.photo_type.clone();
            result.work_type = row.work_type.clone();
            result.variety = row.variety.clone();
            result.subphase = row.subphase.clone();
            result.remarks = row.remarks.clone();

            // 「切削機」フォルダの使用機械は、機械名を備考へ埋め込む。
            if result.remarks == "使用機械"
                && folder_name.contains("切削機")
                && rec.machine_type.contains("路面切削機")
            {
                let id_head = rec.machine_id.split_whitespace().next().unwrap_or("");
                if !id_head.is_empty() {
                    result.remarks = format!("使用機械（{} {}）", rec.machine_type.trim(), id_head);
                }
            }

            // 安全管理写真の一部のみ日付測点を自動設定する。
            if result.photo_category == PHOTO_CAT_SAFETY
                && result.station.is_empty()
                && should_auto_date_station(&result.remarks)
            {
                result.station = date_to_month_day(&result.date);
            }

            // 機械系写真は測点欄へ機械名を補完する。
            if result.station.is_empty()
                && (result.remarks == "使用機械" || result.remarks == "重機始業前点検")
                && !rec.machine_type.trim().is_empty()
            {
                if result.remarks == "使用機械" && !rec.machine_id.trim().is_empty() {
                    result.station = format!("{} {}", rec.machine_type.trim(), rec.machine_id.trim());
                } else {
                    result.station = rec.machine_type.trim().to_string();
                }
            }

            // 切削機フォルダの使用機械は運用値に合わせて固定する。
            if folder_name.contains("切削機") && rec.machine_type.contains("路面切削機") {
                let id_head = rec.machine_id.split_whitespace().next().unwrap_or("");
                result.photo_category = PHOTO_CAT_OTHER.to_string();
                result.work_type = "舗装工".to_string();
                result.variety.clear();
                result.subphase.clear();
                result.station.clear();
                if !id_head.is_empty() {
                    result.remarks = format!("使用機械（{} {}）", rec.machine_type.trim(), id_head);
                } else {
                    result.remarks = "使用機械".to_string();
                }
            }

            // 既存運用: 「重機始業前点検」フォルダは工種を舗装工として扱う。
            if folder_name.contains("重機始業前点検")
                && result.remarks == "重機始業前点検"
                && result.work_type.is_empty()
            {
                result.work_type = "舗装工".to_string();
            }

            // 処分状況_車番調査は日付測点を補完し、備考を運用値へ寄せる。
            if folder_name.contains("処分状況_車番調査") && result.station.is_empty() {
                result.station = date_to_month_day(&result.date);
            }
            if folder_name.contains("処分状況_車番調査")
                && rec.detected_text.contains("処分状況")
                && rec.detected_text.contains("車番調査")
            {
                result.photo_category = PHOTO_CAT_CONSTRUCTION.to_string();
                result.work_type = "舗装工".to_string();
                result.variety = "路面切削工".to_string();
                result.subphase = "殻処分".to_string();
                if rec.detected_text.contains("車番") {
                    result.remarks = "As殻処分状況　車番調査（黒板日付訂正）".to_string();
                } else {
                    result.remarks = "As殻処分状況　車番調査".to_string();
                }
            }
            if folder_name.contains("処分状況_車番調査") && result.measurements.is_empty() {
                if let Some(m) = extract_tonnage_from_text(&rec.detected_text) {
                    result.measurements = m;
                }
            }
        }
        // マスタ照合失敗時は全フィールド空のまま（ゴミを埋めない）
        apply_temperature_folder_postprocess(&mut result, folder_name);

        result.reasoning = format!("photo-groups.json: {} / {}", rec.machine_type, rec.machine_id);

        Some((rec.group, role_priority(&rec.role), result))
    }).collect();

    // 日付・ファイル名の時系列ソート（グループ順ではなく撮影順）
    results.sort_by(|a, b| {
        a.2.date.cmp(&b.2.date).then(a.2.file_name.cmp(&b.2.file_name))
    });
    let mut final_results: Vec<analyzer::AnalysisResult> = results.into_iter().map(|(_, _, r)| r).collect();

    if folder_name.contains("処分状況_車番調査") {
        let mut shared_measurement: Option<String> = None;
        for r in &final_results {
            if r.remarks.contains("As殻処分状況") && !r.measurements.is_empty() {
                shared_measurement = Some(r.measurements.clone());
                break;
            }
        }
        if let Some(m) = shared_measurement {
            for r in &mut final_results {
                if r.remarks.contains("As殻処分状況") && r.measurements.is_empty() {
                    r.measurements = m.clone();
                }
            }
        }
    }

    apply_domain_corrections(&mut final_results, line_types);
    apply_folder_specific_corrections(&mut final_results, folder_context);

    final_results
}

/// フォルダ内の文脈でドメイン固有の種別補正を適用
///
/// - 路面切削工が存在する場合、舗装打換え工/表層工 → 切削オーバーレイ工/表層工
/// - 区画線工の線種判定（line_typesが指定されている場合のみ）
fn apply_domain_corrections(
    results: &mut Vec<analyzer::AnalysisResult>,
    line_types: Option<&[LineTypeEntry]>,
) {
    // 路面切削工が存在する場合、舗装打換え工/表層工 → 切削オーバーレイ工/表層工
    let has_road_cutting = results.iter().any(|r| r.variety == VARIETY_ROAD_CUTTING);
    if has_road_cutting {
        for r in results.iter_mut() {
            if r.photo_category == PHOTO_CAT_CONSTRUCTION
                && r.variety == VARIETY_PAVEMENT_REPLACE
                && r.subphase == SUBPHASE_SURFACE
            {
                r.variety = VARIETY_CUTTING_OVERLAY.to_string();
            }
        }
    }

    // 区画線工の線種判定: line_typesが指定されている場合のみ
    if let Some(lt) = line_types {
        if !lt.is_empty() {
            for r in results.iter_mut() {
                if r.work_type == WORK_LANE_MARKING && !r.file_path.is_empty() {
                    if let Some(detected) = detect_line_type(&r.file_path, lt) {
                        r.station = detected;
                    }
                }
            }
        }
    }
}

/// 測点を一括適用
/// - 安全管理写真・品質管理写真 → 日付（◯月◯日）
/// - 区画線工 → スキップ（線種ごとに撮影するため固定測点は不適切）
pub fn apply_station(results: &mut [analyzer::AnalysisResult], station: &str) {
    // -S で日付形式（"X月Y日"）が渡された場合、品質管理・安全管理にもそのまま使う
    let station_is_date = station.contains('月') && station.contains('日');

    for result in results {
        match result.photo_category.as_str() {
            PHOTO_CAT_SAFETY | PHOTO_CAT_QUALITY => {
                // 既に日付形式のstationがあれば上書きしない（tagger由来の作業日を優先）
                let already_has_date = result.station.contains('月') && result.station.contains('日');
                if !already_has_date {
                    result.station = if station_is_date {
                        station.to_string()
                    } else {
                        date_to_month_day(&result.date)
                    };
                }
            }
            _ if result.work_type == WORK_LANE_MARKING => {
                // 区画線工は線種ごとに撮影するため測点を一律適用しない
                // 以前のnormalize -Sで誤設定された値もクリア
                if result.station == station {
                    result.station.clear();
                }
            }
            _ => {
                result.station = station.to_string();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_station_skips_safety_and_marking() {
        let mut results = vec![
            analyzer::AnalysisResult {
                photo_category: PHOTO_CAT_CONSTRUCTION.to_string(),
                date: "2026-02-09 23:34:47".to_string(),
                ..Default::default()
            },
            analyzer::AnalysisResult {
                photo_category: PHOTO_CAT_SAFETY.to_string(),
                date: "2026-02-09 21:23:53".to_string(),
                ..Default::default()
            },
            analyzer::AnalysisResult {
                photo_category: PHOTO_CAT_QUALITY.to_string(),
                date: "2026-02-10 01:15:00".to_string(),
                ..Default::default()
            },
            analyzer::AnalysisResult {
                photo_category: PHOTO_CAT_CONSTRUCTION.to_string(),
                work_type: WORK_LANE_MARKING.to_string(),
                station: "".to_string(),
                date: "2026-02-10 03:44:26".to_string(),
                ..Default::default()
            },
            // 以前の-Sで誤設定された区画線工
            analyzer::AnalysisResult {
                photo_category: PHOTO_CAT_CONSTRUCTION.to_string(),
                work_type: WORK_LANE_MARKING.to_string(),
                station: "No.4 左車線".to_string(),
                date: "2026-02-10 03:52:16".to_string(),
                ..Default::default()
            },
        ];
        apply_station(&mut results, "No.4 左車線");
        assert_eq!(results[0].station, "No.4 左車線");
        assert_eq!(results[1].station, "2月9日");
        assert_eq!(results[2].station, "2月10日");
        assert_eq!(results[3].station, ""); // 区画線工はスキップ
        assert_eq!(results[4].station, ""); // 区画線工: 以前の-S値をクリア
    }

    #[test]
    fn test_apply_domain_corrections_does_not_convert_quality_temperature() {
        let mut results = vec![
            analyzer::AnalysisResult {
                file_name: "A.JPG".to_string(),
                photo_category: PHOTO_CAT_CONSTRUCTION.to_string(),
                work_type: "舗装工".to_string(),
                variety: VARIETY_ROAD_CUTTING.to_string(),
                subphase: "路面切削".to_string(),
                ..Default::default()
            },
            analyzer::AnalysisResult {
                file_name: "B.JPG".to_string(),
                photo_category: PHOTO_CAT_QUALITY.to_string(),
                work_type: "舗装工".to_string(),
                variety: VARIETY_PAVEMENT_REPLACE.to_string(),
                subphase: SUBPHASE_SURFACE.to_string(),
                remarks: "到着温度測定".to_string(),
                date: "2026-02-10 01:00:09".to_string(),
                ..Default::default()
            },
        ];

        apply_domain_corrections(&mut results, None);
        assert_eq!(results[1].variety, VARIETY_PAVEMENT_REPLACE);
    }

    #[test]
    fn test_temperature_folder_postprocess_fills_station_and_strips_measurement_suffix() {
        let mut result = analyzer::AnalysisResult {
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
        assert_eq!(result.station, "2月10日");
        assert_eq!(result.remarks, "到着温度");
        assert_eq!(result.measurements, "");
    }

    #[test]
    fn test_temperature_folder_postprocess_keeps_modern_format() {
        let mut result = analyzer::AnalysisResult {
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
        assert_eq!(result.station, "");
        assert_eq!(result.remarks, "到着温度測定");
        assert_eq!(result.measurements, "160.1℃");
    }
}

fn apply_folder_specific_corrections(
    results: &mut Vec<analyzer::AnalysisResult>,
    folder_name: &str,
) {
    crate::folder_rules::apply_folder_specific_corrections(results, folder_name);
}
fn apply_temperature_folder_postprocess(result: &mut analyzer::AnalysisResult, folder_name: &str) {
    if !folder_name.contains("温度管理") {
        return;
    }
    result.photo_category = PHOTO_CAT_QUALITY.to_string();
    result.work_type = "舗装工".to_string();
    result.variety = VARIETY_PAVEMENT_REPLACE.to_string();
    result.subphase = SUBPHASE_SURFACE.to_string();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TemperatureMode {
    Legacy0209,
    Overlay0211Or0213,
    Standard0212,
    Other,
}

fn detect_temperature_mode(folder_context: &str) -> TemperatureMode {
    if folder_context.contains("0209切削") {
        TemperatureMode::Legacy0209
    } else if folder_context.contains("0211切削") || folder_context.contains("0213切削") {
        TemperatureMode::Overlay0211Or0213
    } else if folder_context.contains("0212切削") {
        TemperatureMode::Standard0212
    } else {
        TemperatureMode::Other
    }
}

fn classify_temperature_remarks(
    remarks: &str,
    detected_text: &str,
    focus_target: &str,
    mode: TemperatureMode,
) -> String {
    let text = format!("{} {}", detected_text, focus_target);
    let has_open = text.contains("開放温度") || text.contains("解放温度");
    let has_outside = text.contains("舗装日外気温") || text.contains("外気温");
    let has_arrival = text.contains("到着温度");
    let has_spread = text.contains("敷均し温度") || text.contains("敷きならし");
    let has_initial = text.contains("初期転圧前温度")
        || text.contains("初期締固め前温度")
        || text.contains("初期転圧前")
        || text.contains("初期締固め前");

    let mut mapped = if remarks.contains("舗装厚測定") && focus_target.contains("開放温度") {
        "開放温度測定".to_string()
    } else if has_initial {
        "初期締固め前温度測定".to_string()
    } else if has_spread {
        "敷均し温度測定".to_string()
    } else if has_arrival {
        "到着温度測定".to_string()
    } else if has_open {
        "開放温度測定".to_string()
    } else if has_outside {
        "舗装日外気温測定".to_string()
    } else {
        remarks.to_string()
    };

    match mode {
        TemperatureMode::Legacy0209 => {
            mapped = mapped.trim_end_matches("測定").to_string();
            if mapped == "舗装日外気温" || mapped == "舗装厚測定・管理値" {
                mapped = "開放温度".to_string();
            }
        }
        _ => {
            if matches!(
                mapped.as_str(),
                "到着温度" | "敷均し温度" | "初期締固め前温度" | "開放温度" | "舗装日外気温"
            ) {
                mapped.push_str("測定");
            }
        }
    }
    mapped
}

fn extract_month_day_from_text(text: &str) -> Option<String> {
    let compact = text.replace(' ', "").replace('　', "");
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

fn extract_month_day_from_folder_context(folder_context: &str) -> Option<String> {
    let marker = "切削";
    let marker_pos = folder_context.find(marker)?;
    let before = &folder_context[..marker_pos];
    let digits_rev: String = before
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if digits_rev.len() < 4 {
        return None;
    }
    let digits: String = digits_rev.chars().rev().collect();
    let mm = digits[digits.len() - 4..digits.len() - 2].parse::<u32>().ok()?;
    let dd = digits[digits.len() - 2..].parse::<u32>().ok()?;
    Some(format!("{}月{}日", mm, dd))
}

fn extract_dump_number_from_station(station: &str) -> Option<String> {
    extract_dump_number(station)
}

fn fill_missing_dump_numbers(results: &mut [analyzer::AnalysisResult]) {
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
            && extract_month_day_from_text(&results[i - 1].station).as_deref() == Some(base_day.as_str())
        {
            inferred_dump = extract_dump_number_from_station(&results[i - 1].station);
        }
        if inferred_dump.is_none()
            && i + 1 < results.len()
            && results[i + 1].remarks == results[i].remarks
            && extract_month_day_from_text(&results[i + 1].station).as_deref() == Some(base_day.as_str())
        {
            inferred_dump = extract_dump_number_from_station(&results[i + 1].station);
        }
        if let Some(dump) = inferred_dump {
            results[i].station = format!("{} {}", base_day, dump);
        }
    }
}

fn rebalance_initial_temperature_labels(results: &mut [analyzer::AnalysisResult]) {
    use std::collections::BTreeMap;
    let initial = "初期締固め前温度測定";
    let spread = "敷均し温度測定";
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

fn repair_orphan_temperature_entries(results: &mut [analyzer::AnalysisResult]) {
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

fn propagate_temperature_measurements(results: &mut [analyzer::AnalysisResult]) {
    use std::collections::BTreeMap;
    let valid = [
        "舗装日外気温測定",
        "到着温度測定",
        "敷均し温度測定",
        "初期締固め前温度測定",
        "開放温度測定",
        "到着温度",
        "敷均し温度",
        "初期締固め前温度",
        "開放温度",
    ];

    for r in results.iter_mut() {
        if !valid.contains(&r.remarks.as_str()) {
            continue;
        }
        if let Some(extracted) = extract_temperature_for_remarks(&r.detected_text, &r.remarks) {
            r.measurements = extracted;
        }
    }

    let mut groups: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
    for (idx, r) in results.iter().enumerate() {
        if valid.contains(&r.remarks.as_str()) {
            groups.entry((r.station.clone(), r.remarks.clone())).or_default().push(idx);
        }
    }

    for idxs in groups.values() {
        let Some(first_idx) = idxs.first().copied() else { continue; };
        let group_remarks = results[first_idx].remarks.clone();
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
        let source_value = if group_remarks == "舗装日外気温測定" {
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

fn apply_temperature_folder_final_adjustments(
    results: &mut [analyzer::AnalysisResult],
    folder_context: &str,
) {
    if !folder_context.contains("温度管理") {
        return;
    }

    let mode = detect_temperature_mode(folder_context);
    for result in results.iter_mut() {
        if mode == TemperatureMode::Legacy0209 {
            if result.station.is_empty() {
                result.station = date_to_month_day(&result.date);
            }
            result.measurements.clear();
            let mut normalized = result.remarks.trim_end_matches("測定").to_string();
            if normalized == "舗装日外気温" || normalized == "舗装厚測定・管理値" {
                normalized = "開放温度".to_string();
            }
            let has_arrival = result.detected_text.contains("到着温度");
            let has_spread = result.detected_text.contains("敷均し温度");
            let has_initial = result.detected_text.contains("初期転圧前温度")
                || result.detected_text.contains("初期締固め前温度");
            if has_arrival && (has_spread || has_initial) {
                normalized = "到着温度".to_string();
            }
            result.remarks = normalized;
        } else if mode == TemperatureMode::Standard0212
            && result.station.contains('月')
            && result.station.contains('日')
        {
            result.station.clear();
            result.remarks = classify_temperature_remarks(
                &result.remarks,
                &result.detected_text,
                &result.focus_target,
                mode,
            );
            let has_arrival = result.detected_text.contains("到着温度");
            let has_spread = result.detected_text.contains("敷均し温度");
            if has_arrival && has_spread && result.remarks == "敷均し温度測定" {
                result.remarks = "到着温度測定".to_string();
            }
        }

        if mode == TemperatureMode::Overlay0211Or0213 {
            result.variety = VARIETY_CUTTING_OVERLAY.to_string();
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
            result.remarks = classify_temperature_remarks(
                &result.remarks,
                &result.detected_text,
                &result.focus_target,
                mode,
            );
        }
    }

    if mode == TemperatureMode::Overlay0211Or0213 {
        normalizer::append_dump_number_to_station(results);
        fill_missing_dump_numbers(results);
        repair_orphan_temperature_entries(results);
        rebalance_initial_temperature_labels(results);
        propagate_temperature_measurements(results);
    }
}
