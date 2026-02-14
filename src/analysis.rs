//! 解析関連のヘルパー関数とデータ構造
//!
//! AI写真解析のためのスキャン、解析、正規化処理を提供する。
//! OCR解析・マスタ照合・線種判定は各専用モジュールに委譲し、
//! このモジュールはパイプラインのオーケストレーションに専念する。

use crate::{ai_provider::AiProvider, analyzer, error, scanner};
use crate::normalizer::{self, NormalizationOptions};
use crate::ocr_parser::{extract_kv_from_text, normalize_station};
use crate::master_matcher::{
    date_to_month_day, match_master_from_detected_texts, role_priority,
    safety_remarks_from_machine_type,
};
use crate::line_type_detector::detect_line_type;
use photo_ai_common::{HierarchyMaster, LineTypeEntry};
use photo_tagger::GroupRecords;
use std::path::{Path, PathBuf};

use crate::error::Result;

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

/// AI解析のオプション
pub struct AnalysisOptions<'a> {
    pub folder: &'a Path,
    pub batch_size: usize,
    pub verbose: bool,
    pub master: Option<&'a Path>,
    pub photo_type: Option<&'a str>,
    pub use_cache: bool,
    pub provider: AiProvider,
    pub work_type: Option<&'a str>,
    pub variety: Option<&'a str>,
    pub step_prefix: &'a str,
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
        // マスタパスなし: デフォルトマスタをフォールバック
        let default = PathBuf::from("master/construction_hierarchy.csv");
        if default.exists() {
            HierarchyMaster::from_csv(&default)
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

    // 2. マスタ読み込み（語彙リスト抽出のため先にロード）
    let master = load_master_from_config(config.master_config)?;
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
    let mut results = convert_groups_to_results(&grouped_images, &group_records, &master, folder_name, config.line_types);
    println!("✔ マスタ照合完了（{}枚）\n", results.len());

    // 4. 正規化
    normalize_results_with_station(
        &mut results,
        config.station.map(|s| s.as_str()),
        config.verbose,
    );

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
    println!("✔ {}枚の写真を検出\n", images.len());

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

/// AI解析を実行（常に1ステップ解析）
pub async fn run_analysis(
    images: &[scanner::ImageInfo],
    options: &AnalysisOptions<'_>,
) -> Result<Vec<analyzer::AnalysisResult>> {
    // マスタパスを決定
    let master_path_buf = resolve_master_path(options.master, options.work_type)?;

    // 工種指定なし + by_work_typeディレクトリあり → マージ読み込み
    let hierarchy = if options.work_type.is_none() {
        if let Some(all_paths) = crate::master_selector::collect_all_master_paths() {
            HierarchyMaster::from_csv_files(&all_paths)
                .map_err(|e| error::PhotoAiError::MasterLoad(e.to_string()))?
        } else {
            HierarchyMaster::from_csv(&master_path_buf)
                .map_err(|e| error::PhotoAiError::MasterLoad(e.to_string()))?
        }
    } else {
        HierarchyMaster::from_csv(&master_path_buf)
            .map_err(|e| error::PhotoAiError::MasterLoad(e.to_string()))?
    };

    // photo_type指定時はそちらでフィルタ、なければwork_typeでフィルタ
    let master = if let Some(pt) = options.photo_type {
        println!("{} 1ステップ解析中 (写真種類: {})...", options.step_prefix, pt);
        let filtered = hierarchy.filter_by_photo_type(pt);
        println!("  マスタ読み込み: {}件 (写真種類: {})", filtered.rows().len(), pt);
        filtered
    } else if let Some(wt) = options.work_type {
        println!("{} 1ステップ解析中 (工種: {})...", options.step_prefix, wt);
        let filtered = hierarchy.filter_by_work_types(&[wt.to_string()]);
        println!("  マスタ読み込み: {}件 (工種: {})", filtered.rows().len(), wt);
        filtered
    } else {
        println!("{} 1ステップ解析中 (全工種)...", options.step_prefix);
        println!("  マスタ読み込み: {}件", hierarchy.rows().len());
        hierarchy
    };

    analyzer::analyze_images_single_step(
        images,
        &master,
        options.work_type,
        options.variety,
        options.photo_type,
        options.batch_size,
        options.verbose,
        options.provider,
    ).await
}

/// マスタファイルのパスを解決する
///
/// 優先順位:
/// 1. 明示的に指定されたマスタパス
/// 2. 工種指定時: 工種別マスタ (master/by_work_type/{work_type}.csv)
/// 3. デフォルトマスタ (master/construction_hierarchy.csv)
pub fn resolve_master_path(master: Option<&Path>, work_type: Option<&str>) -> Result<PathBuf> {
    if let Some(mp) = master {
        return Ok(mp.to_path_buf());
    }

    // 工種指定時は工種別マスタを優先
    if let Some(wt) = work_type {
        let by_work_type = PathBuf::from("master/by_work_type").join(format!("{}.csv", wt));
        if by_work_type.exists() {
            return Ok(by_work_type);
        }
    }

    // デフォルトマスタ
    let default = PathBuf::from("master/construction_hierarchy.csv");
    if default.exists() {
        return Ok(default);
    }

    Err(error::PhotoAiError::MasterLoad("マスタファイルが見つかりません".to_string()))
}

/// 後方互換: 工種指定必須版
pub fn resolve_master_for_work_type(master: Option<&Path>, work_type: &str) -> Result<PathBuf> {
    resolve_master_path(master, Some(work_type))
}

/// photo-groups.json のレコードを AnalysisResult に変換し、マスタ照合する
/// グループ番号→全景優先でソート
fn convert_groups_to_results(
    images: &[scanner::ImageInfo],
    groups: &GroupRecords,
    master: &HierarchyMaster,
    folder_name: &str,
    line_types: Option<&[LineTypeEntry]>,
) -> Vec<analyzer::AnalysisResult> {
    // フォルダ名でのフォールバック照合（detected_textが空の写真用）
    let folder_fallback_row = {
        let empty: Vec<&str> = Vec::new();
        match_master_from_detected_texts(master, &empty, folder_name)
    };

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
        result.station = normalize_station(&photo_station);

        // 安全管理系: taggerのmachine_typeから直接判定（黒板なし写真に対応）
        let safety_remarks = safety_remarks_from_machine_type(&rec.machine_type);
        if let Some(remarks) = safety_remarks {
            result.photo_category = "安全管理写真".to_string();
            result.remarks = remarks;
            // 安全管理写真のstationに日付を設定（測点がない場合）
            if result.station.is_empty() {
                if let Some(d) = result.date.split(' ').next() {
                    // "2026-02-11" → "2月11日"
                    let parts: Vec<&str> = d.split('-').collect();
                    if parts.len() == 3 {
                        if let (Ok(m), Ok(day)) = (parts[1].parse::<u32>(), parts[2].parse::<u32>()) {
                            result.station = format!("{}月{}日", m, day);
                        }
                    }
                }
            }
        } else {
            // 写真ごとにマスタ照合（混在フォルダ対応）
            // machine_typeもキーワードに追加して照合精度を向上
            let photo_matched_row = if !rec.detected_text.is_empty() {
                let combined = if !rec.machine_type.is_empty() {
                    format!("{}\n{}", rec.detected_text, rec.machine_type)
                } else {
                    rec.detected_text.clone()
                };
                let texts = vec![combined.as_str()];
                match_master_from_detected_texts(master, &texts, folder_name)
            } else {
                folder_fallback_row.clone()
            };

            // マスタ照合結果を適用
            if let Some(row) = &photo_matched_row {
                result.photo_category = row.photo_type.clone();
                result.work_type = row.work_type.clone();
                result.variety = row.variety.clone();
                result.subphase = row.subphase.clone();
                result.remarks = row.remarks.clone();
            } else {
                // マスタ照合失敗: 工種キーワードだけ埋める
                let extracted_wt = kvs.iter()
                    .find(|(k, _)| k == "工種")
                    .map(|(_, v)| v.clone());
                if let Some(wt) = extracted_wt {
                    result.work_type = wt;
                }
                result.remarks = folder_name.replace('_', " ");
            }
        }

        result.reasoning = format!("photo-groups.json: {} / {}", rec.machine_type, rec.machine_id);

        Some((rec.group, role_priority(&rec.role), result))
    }).collect();

    // 日付→ファイル名の時系列ソート（グループ順ではなく撮影順）
    results.sort_by(|a, b| {
        a.2.date.cmp(&b.2.date).then(a.2.file_name.cmp(&b.2.file_name))
    });
    let mut final_results: Vec<analyzer::AnalysisResult> = results.into_iter().map(|(_, _, r)| r).collect();

    // ポスト処理: フォルダ内の文脈で種別を補正
    // 路面切削工が存在する場合、舗装打換え工/表層工 → 切削オーバーレイ工/表層工
    let has_road_cutting = final_results.iter().any(|r| r.variety == "路面切削工");
    if has_road_cutting {
        for r in &mut final_results {
            if r.variety == "舗装打換え工" && r.subphase == "表層工" {
                r.variety = "切削オーバーレイ工".to_string();
            }
        }
    }

    // 区画線工の線種判定: line_typesが指定されている場合のみ
    if let Some(lt) = line_types {
        if !lt.is_empty() {
            for r in &mut final_results {
                if r.work_type == "区画線工" && !r.file_path.is_empty() {
                    if let Some(detected) = detect_line_type(&r.file_path, lt) {
                        r.station = detected;
                    }
                }
            }
        }
    }

    final_results
}

/// 測点を一括適用
/// - 安全管理写真・品質管理写真 → 日付（◯月◯日）
/// - 区画線工 → スキップ（線種ごとに撮影するため固定測点は不適切）
pub fn apply_station(results: &mut [analyzer::AnalysisResult], station: &str) {
    for result in results {
        match result.photo_category.as_str() {
            "安全管理写真" | "品質管理写真" => {
                result.station = date_to_month_day(&result.date);
            }
            _ if result.work_type == "区画線工" => {
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
                photo_category: "施工状況写真".to_string(),
                date: "2026-02-09 23:34:47".to_string(),
                ..Default::default()
            },
            analyzer::AnalysisResult {
                photo_category: "安全管理写真".to_string(),
                date: "2026-02-09 21:23:53".to_string(),
                ..Default::default()
            },
            analyzer::AnalysisResult {
                photo_category: "品質管理写真".to_string(),
                date: "2026-02-10 01:15:00".to_string(),
                ..Default::default()
            },
            analyzer::AnalysisResult {
                photo_category: "施工状況写真".to_string(),
                work_type: "区画線工".to_string(),
                station: "".to_string(),
                date: "2026-02-10 03:44:26".to_string(),
                ..Default::default()
            },
            // 以前の-Sで誤設定された区画線工
            analyzer::AnalysisResult {
                photo_category: "施工状況写真".to_string(),
                work_type: "区画線工".to_string(),
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
}
