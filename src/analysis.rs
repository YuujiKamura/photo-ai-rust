//! 解析関連のヘルパー関数とデータ構造
//!
//! AI写真解析のためのスキャン、解析、正規化処理を提供する。
//! OCR解析・マスタ照合・線種判定は各専用モジュールに委譲し、
//! このモジュールはパイプラインのオーケストレーションに専念する。

use crate::{analyzer, engine, error, scanner};
use crate::domain::*;
use crate::folder_rules::FolderRule;
use crate::grouping::{CarrierConfig, GroupRecords};
use crate::normalizer::{self, NormalizationOptions};
use crate::ocr_parser::{extract_kv_from_text, normalize_station};
use crate::master_matcher::{
    date_to_month_day, match_master_from_detected_texts, role_priority,
};
use crate::line_type_detector::detect_line_type;
use photo_ai_common::{HierarchyMaster, LineTypeEntry};
use std::path::{Path, PathBuf};

use crate::error::Result;

use crate::domain::policy::{extract_tonnage_from_text, should_auto_date_station};

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
    pub variety: Option<&'a String>,
    pub station: Option<&'a String>,
    pub recursive: bool,
    pub include_all: bool,
    pub step_prefix_scan: &'a str,
    pub step_prefix_analyze: &'a str,
    pub line_types: Option<&'a [LineTypeEntry]>,
    pub folder_rules: Option<&'a [FolderRule]>,
    pub carrier: CarrierConfig,
}

/// マスタ選択結果
pub struct MasterConfig {
    pub master_path: Option<PathBuf>,
    pub effective_work_type: Option<String>,
    /// 全工種時のマージ用パス一覧（by_work_type/*.csv + メインCSV）
    pub all_paths: Option<Vec<PathBuf>>,
}

impl MasterConfig {
    pub fn selection_label(&self) -> &'static str {
        if self.all_paths.is_some() { "all" } else { "single" }
    }

    pub fn primary_path_display(&self) -> String {
        if let Some(paths) = &self.all_paths {
            let joined = paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(";");
            return joined;
        }
        self.master_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    }
}

/// マスタ選択と検証を行う共通関数
pub fn prepare_analysis(
    master: Vec<PathBuf>,
    work_type: Option<String>,
    variety: Option<&String>,
    resolve_master_path: impl FnOnce(Vec<PathBuf>, bool) -> Option<crate::master_selector::MasterSelection>,
) -> Result<MasterConfig> {
    // マスタ選択（対話式または引数から）
    let has_master_arg = !master.is_empty();
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
        master = master.filter_by_work_types(std::slice::from_ref(wt));
        println!("  マスタフィルタ: 工種={} ({} → {}件)", wt, before, master.rows().len());
    }
    if let Some(pt) = config.photo_type {
        let before = master.rows().len();
        master = master.filter_by_photo_type(pt);
        println!("  マスタフィルタ: 写真区分={} ({} -> {}件)", pt, before, master.rows().len());
    }
    let vocabulary = master.extract_vocabulary();

    // 3. 解析engine（語彙リスト付き）
    println!("{} photo-analysis-engine実行中...", config.step_prefix_analyze);
    let vocab_ref = if vocabulary.is_empty() { None } else { Some(vocabulary.as_slice()) };
    let group_records = engine::run_tag_groups(config.folder, config.batch_size, vocab_ref, config.carrier)?;

    if group_records.is_empty() {
        return Err(error::PhotoAiError::NoImagesFound(
            format!("photo-analysis-engine の結果が空: {}", config.folder.display())
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

    // マスタに直接マッチするエントリがあるかチェック
    if !crate::master_matcher::folder_has_master_entry(&master, folder_name) {
        // tagger結果からdescriptionを集約して提案材料にする
        let descriptions: Vec<&str> = group_records.values()
            .map(|r| r.core.description.as_str())
            .filter(|d| !d.is_empty())
            .collect();
        let desc_summary = if descriptions.is_empty() {
            "(tagger結果なし)".to_string()
        } else {
            descriptions.join(" / ")
        };
        eprintln!();
        eprintln!("⚠ マスタに「{}」に対応するエントリがありません。", folder_name);
        eprintln!("  照合が的外れになる可能性があります。");
        eprintln!("  以下のような行をマスタCSV (master/by_work_type/**.csv) に追加してください:");
        eprintln!("  \"直接工事費\",\"(写真区分)\",\"(工種)\",\"(種別)\",\"(細別)\",\"{}\",\"(検索パターン)\"", folder_name);
        eprintln!("  tagger推定内容: {}", desc_summary);
        eprintln!();
    }

    let folder_context = config.folder.display().to_string();
    let mut results = convert_groups_to_results(
        &grouped_images,
        &group_records,
        &master,
        folder_name,
        &folder_context,
        config.line_types,
        config.folder_rules,
    );
    println!("✓ マスタ照合完了（{}枚）\n", results.len());

    // 4. 正規化
    normalize_results_with_station(
        &mut results,
        config.station.map(|s| s.as_str()),
        config.verbose,
    );

    apply_folder_specific_corrections(&mut results, &folder_context, config.folder_rules);
    // 温度管理フォルダ: Phase 2 — 正規化後の最終調整
    // focusTarget→remarks正規化、黒板日付→station、温度値抽出→measurements伝搬
    crate::temperature::apply_temperature_folder_final_adjustments(&mut results, &folder_context);

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

#[cfg(test)]
mod pipeline_checkpoint_tests {
    use super::*;
    use crate::grouping::{GroupCore, GroupRecord};
    use crate::scanner::ImageInfo;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;

    fn unique_temp_dir(test_name: &str) -> PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("photo-ai-analysis-{}-{}", test_name, timestamp))
    }

    struct TempDirGuard(PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_master_csv(path: &Path, rows: &[&str]) {
        let header = "費目,写真区分,工種,種別,細別,備考,検索パターン\n";
        let body = rows.join("\n");
        fs::write(path, format!("{header}{body}\n")).unwrap();
    }

    fn test_master(rows: &[&str]) -> HierarchyMaster {
        let dir = unique_temp_dir("master");
        let _guard = TempDirGuard(dir.clone());
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("master.csv");
        write_master_csv(&path, rows);
        HierarchyMaster::from_csv(&path).unwrap()
    }

    fn test_image(file_name: &str, date: &str) -> ImageInfo {
        ImageInfo {
            path: std::env::temp_dir().join(file_name),
            file_name: file_name.to_string(),
            date: Some(date.to_string()),
        }
    }

    fn test_group(
        file_name: &str,
        group: u32,
        role: &str,
        machine_type: &str,
        machine_id: &str,
        detected_text: &str,
        description: &str,
    ) -> GroupRecords {
        let mut groups = HashMap::new();
        groups.insert(
            file_name.to_string(),
            GroupRecord {
                core: GroupCore {
                    role: role.to_string(),
                    machine_type: machine_type.to_string(),
                    machine_id: machine_id.to_string(),
                    has_board: true,
                    detected_text: detected_text.to_string(),
                    description: description.to_string(),
                },
                group,
                captured_at: None,
            },
        );
        groups
    }

    #[test]
    fn prepare_analysis_rejects_variety_without_work_type() {
        let result = prepare_analysis(
            Vec::new(),
            None,
            Some(&"舗装打換え工".to_string()),
            |_master, _interactive| None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn prepare_analysis_rejects_work_type_without_master() {
        let result = prepare_analysis(
            Vec::new(),
            Some("舗装工".to_string()),
            None,
            |_master, _interactive| None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn load_master_from_config_merges_all_paths() {
        let dir = unique_temp_dir("merge-master");
        let _guard = TempDirGuard(dir.clone());
        fs::create_dir_all(&dir).unwrap();

        let path1 = dir.join("a.csv");
        let path2 = dir.join("b.csv");
        write_master_csv(
            &path1,
            &[
                "直接工事費,施工状況写真,舗装工,舗装打換え工,切削工,切削状況,切削",
            ],
        );
        write_master_csv(
            &path2,
            &[
                "直接工事費,品質管理写真,舗装工,舗装打換え工,表層工,温度管理,温度",
            ],
        );

        let config = MasterConfig {
            master_path: None,
            effective_work_type: None,
            all_paths: Some(vec![path1, path2]),
        };

        let master = load_master_from_config(&config).unwrap();
        assert_eq!(master.rows().len(), 2);
        assert!(master.extract_vocabulary().contains(&"切削状況".to_string()));
        assert!(master.extract_vocabulary().contains(&"温度管理".to_string()));
    }

    #[test]
    fn scan_images_excludes_non_use_folder_by_default() {
        let dir = unique_temp_dir("scan-exclude");
        let _guard = TempDirGuard(dir.clone());
        fs::create_dir_all(dir.join("非使用")).unwrap();
        fs::write(dir.join("a.jpg"), b"ok").unwrap();
        fs::write(dir.join("非使用").join("b.jpg"), b"skip").unwrap();

        let images = scan_images(&dir, true, false, "[test]").unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].file_name, "a.jpg");
    }

    #[test]
    fn convert_groups_to_results_extracts_station_from_ocr_key() {
        let master = test_master(&[
            "直接工事費,施工状況写真,舗装工,舗装打換え工,切削工,切削状況,切削"
        ]);
        let images = vec![test_image("a.jpg", "2026:02:13 08:00:00")];
        let groups = test_group(
            "a.jpg",
            1,
            "全景",
            "",
            "",
            "場所: No.9\n切削",
            "切削状況",
        );

        let results = convert_groups_to_results(
            &images,
            &groups,
            &master,
            "施工状況",
            r"C:\fixtures\施工状況",
            None,
            None,
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].station, "No.9");
    }

    #[test]
    fn convert_groups_to_results_extracts_dekigata_station_without_key() {
        let master = test_master(&[
            "直接工事費,出来形管理写真,舗装工,舗装打換え工,切削工,出来形管理,出来形"
        ]);
        let images = vec![test_image("a.jpg", "2026:02:13 08:00:00")];
        let groups = test_group(
            "a.jpg",
            1,
            "黒板アップ",
            "",
            "",
            "出来形管理用紙 No.9",
            "出来形管理",
        );

        let results = convert_groups_to_results(
            &images,
            &groups,
            &master,
            "切削出来形",
            r"C:\fixtures\切削出来形",
            None,
            None,
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].station, "No.9");
    }

    #[test]
    fn convert_groups_to_results_matches_machine_type_when_detected_text_empty() {
        let master = test_master(&[
            "直接工事費,その他,舗装工,,,使用機械,路面切削機"
        ]);
        let images = vec![test_image("a.jpg", "2026:02:10 08:00:00")];
        let groups = test_group(
            "a.jpg",
            1,
            "全景",
            "路面切削機",
            "ER552F",
            "",
            "使用機械",
        );

        let results = convert_groups_to_results(
            &images,
            &groups,
            &master,
            "使用機械",
            r"C:\fixtures\使用機械",
            None,
            None,
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].remarks, "使用機械");
        assert_eq!(results[0].station, "路面切削機 ER552F");
    }

    #[test]
    fn convert_groups_to_results_auto_dates_specific_safety_remarks() {
        let master = test_master(&[
            "直接工事費,安全管理写真,,,,安全朝礼実施状況,朝礼"
        ]);
        let images = vec![test_image("a.jpg", "2026:02:13 08:00:00")];
        let groups = test_group(
            "a.jpg",
            1,
            "全景",
            "",
            "",
            "朝礼実施",
            "朝礼",
        );

        let results = convert_groups_to_results(
            &images,
            &groups,
            &master,
            "朝礼",
            r"C:\fixtures\朝礼",
            None,
            None,
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].photo_category, PHOTO_CAT_SAFETY);
        assert_eq!(results[0].station, "2月13日");
    }

    #[test]
    fn convert_groups_to_results_overrides_cutting_machine_folder_output() {
        let master = test_master(&[
            "直接工事費,その他,舗装工,,,使用機械,路面切削機"
        ]);
        let images = vec![test_image("a.jpg", "2026:02:10 08:00:00")];
        let groups = test_group(
            "a.jpg",
            1,
            "全景",
            "路面切削機",
            "ER552F 1234",
            "",
            "使用機械",
        );

        let results = convert_groups_to_results(
            &images,
            &groups,
            &master,
            "切削機",
            r"C:\fixtures\切削機",
            None,
            None,
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].photo_category, PHOTO_CAT_OTHER);
        assert_eq!(results[0].work_type, "舗装工");
        assert_eq!(results[0].remarks, "使用機械（路面切削機 ER552F）");
        assert_eq!(results[0].station, "");
    }

    #[test]
    fn normalize_results_with_station_applies_manual_station() {
        let mut results = vec![
            analyzer::AnalysisResult {
                file_name: "a.jpg".to_string(),
                station: "".to_string(),
                ..Default::default()
            },
            analyzer::AnalysisResult {
                file_name: "b.jpg".to_string(),
                station: "No.1".to_string(),
                ..Default::default()
            },
        ];

        normalize_results_with_station(&mut results, Some("No.9"), false);

        assert_eq!(results[0].station, "No.9");
        assert_eq!(results[1].station, "No.9");
    }

    #[test]
    fn convert_groups_to_results_sorts_full_view_before_others() {
        let master = test_master(&[
            "直接工事費,施工状況写真,舗装工,舗装打換え工,切削工,切削状況,切削"
        ]);
        let images = vec![
            test_image("b.jpg", "2026:02:13 08:00:00"),
            test_image("a.jpg", "2026:02:13 08:01:00"),
        ];
        let mut groups = HashMap::new();
        groups.insert(
            "b.jpg".to_string(),
            GroupRecord {
                core: GroupCore {
                    role: "黒板アップ".to_string(),
                    machine_type: "".to_string(),
                    machine_id: "".to_string(),
                    has_board: true,
                    detected_text: "切削".to_string(),
                    description: "".to_string(),
                },
                group: 1,
                captured_at: None,
            },
        );
        groups.insert(
            "a.jpg".to_string(),
            GroupRecord {
                core: GroupCore {
                    role: "全景".to_string(),
                    machine_type: "".to_string(),
                    machine_id: "".to_string(),
                    has_board: true,
                    detected_text: "切削".to_string(),
                    description: "".to_string(),
                },
                group: 1,
                captured_at: None,
            },
        );

        let results = convert_groups_to_results(
            &images,
            &groups,
            &master,
            "施工状況",
            r"C:\fixtures\施工状況",
            None,
            None,
        );

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].file_name, "a.jpg");
        assert_eq!(results[1].file_name, "b.jpg");
    }
}

/// 測点適用と正規化を実行
///
/// # Arguments
/// * `results` - 解析結果（変更される）
/// * `station` - 適用する測点（Noneの場合はスキップ）
/// * `verbose` - 詳細出力するか
pub fn normalize_results_with_station(
    results: &mut [analyzer::AnalysisResult],
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
pub fn convert_groups_to_results(
    images: &[scanner::ImageInfo],
    groups: &GroupRecords,
    master: &HierarchyMaster,
    folder_name: &str,
    folder_context: &str,
    line_types: Option<&[LineTypeEntry]>,
    folder_rules: Option<&[FolderRule]>,
) -> Vec<analyzer::AnalysisResult> {
    let mut results: Vec<(u32, u8, analyzer::AnalysisResult)> = images.iter().filter_map(|img| {
        let rec = groups.get(&img.file_name)?;
        let mut result = analyzer::AnalysisResult {
            file_name: img.file_name.clone(),
            file_path: img.path.display().to_string(),
            date: img.date.as_deref().unwrap_or("").to_string(),
            ..Default::default()
        };

        result.has_board = rec.core.has_board;
        result.detected_text = rec.core.detected_text.clone();
        result.description = rec.core.description.clone();
        result.focus_target = rec.core.role.clone();

        // 写真ごとのdetected_textからキー:値を抽出
        let kvs = extract_kv_from_text(&rec.core.detected_text);

        // 測点: この写真の黒板OCRから「場所」を取得
        let photo_station = kvs.iter()
            .find(|(k, _)| k == "場所" || k == "測点")
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        // 出来形管理用紙からNo.Xを抽出（場所/測点キーがない場合）
        let photo_station = if photo_station.is_empty() {
            extract_dekigata_station(&rec.core.detected_text).unwrap_or_default()
        } else {
            photo_station
        };
        result.station = normalize_station(&photo_station);

        // 写真ごとにマスタ照合（混在フォルダ対応）
        // machine_typeもキーワードに追加して照合精度を向上
        let ft = if rec.core.role.is_empty() { None } else { Some(rec.core.role.as_str()) };
        let photo_matched_row = if !rec.core.detected_text.is_empty() {
            let combined = if !rec.core.machine_type.is_empty() {
                format!("{}\n{}", rec.core.detected_text, rec.core.machine_type)
            } else {
                rec.core.detected_text.clone()
            };
            let texts = vec![combined.as_str()];
            match_master_from_detected_texts(master, &texts, folder_name, ft)
        } else if !rec.core.machine_type.is_empty() {
            // detected_text空でもmachine_typeがあればキーワードとして使う
            let texts = vec![rec.core.machine_type.as_str()];
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
                && rec.core.machine_type.contains("路面切削機")
            {
                let id_head = rec.core.machine_id.split_whitespace().next().unwrap_or("");
                if !id_head.is_empty() {
                    result.remarks = format!("使用機械（{} {}）", rec.core.machine_type.trim(), id_head);
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
                && !rec.core.machine_type.trim().is_empty()
            {
                if result.remarks == "使用機械" && !rec.core.machine_id.trim().is_empty() {
                    result.station = format!("{} {}", rec.core.machine_type.trim(), rec.core.machine_id.trim());
                } else {
                    result.station = rec.core.machine_type.trim().to_string();
                }
            }

            // 切削機フォルダの使用機械は運用値に合わせて固定する。
            if folder_name.contains("切削機") && rec.core.machine_type.contains("路面切削機") {
                let id_head = rec.core.machine_id.split_whitespace().next().unwrap_or("");
                result.photo_category = PHOTO_CAT_OTHER.to_string();
                result.work_type = "舗装工".to_string();
                result.variety.clear();
                result.subphase.clear();
                result.station.clear();
                if !id_head.is_empty() {
                    result.remarks = format!("使用機械（{} {}）", rec.core.machine_type.trim(), id_head);
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
                && rec.core.detected_text.contains("処分状況")
                && rec.core.detected_text.contains("車番調査")
            {
                result.photo_category = PHOTO_CAT_CONSTRUCTION.to_string();
                result.work_type = "舗装工".to_string();
                result.variety = "路面切削工".to_string();
                result.subphase = "殻処分".to_string();
                if rec.core.detected_text.contains("車番") {
                    result.remarks = "As殻処分状況　車番調査（黒板日付訂正）".to_string();
                } else {
                    result.remarks = "As殻処分状況　車番調査".to_string();
                }
            }
            if folder_name.contains("処分状況_車番調査") && result.measurements.is_empty() {
                if let Some(m) = extract_tonnage_from_text(&rec.core.detected_text) {
                    result.measurements = m;
                }
            }
        }
        // マスタ照合失敗時は全フィールド空のまま（ゴミを埋めない）
        // 温度管理フォルダ: Phase 1 — 写真区分・工種・種別・細別を固定値に設定
        // （Phase 2 は正規化完了後の apply_temperature_folder_final_adjustments で
        //   remarks/station/measurements を最終調整）
        crate::temperature::apply_temperature_folder_postprocess(&mut result, folder_name);

        result.reasoning = format!("photo-groups.json: {} / {}", rec.core.machine_type, rec.core.machine_id);

        Some((rec.group, role_priority(&rec.core.role), result))
    }).collect();

    // ソート順:
    //   1. group 番号（同一グループの写真はまとめる）
    //   2. group 内では role_priority（全景→証票→ナンバー→その他）
    //   3. 同 role 内では date / file_name で安定化
    //
    // 以前は date / file_name のみで並べていたが、group と role_priority を
    // タプルに載せておきながら未使用だった。テスト
    // `convert_groups_to_results_sorts_full_view_before_others` が期待する
    // 「全景が同グループ内で先頭」を満たすよう、タプル要素をそのまま使う。
    results.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.date.cmp(&b.2.date))
            .then_with(|| a.2.file_name.cmp(&b.2.file_name))
    });
    let mut final_results: Vec<analyzer::AnalysisResult> = results.into_iter().map(|(group, _, mut r)| {
        r.group = group;
        r
    }).collect();

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
    apply_folder_specific_corrections(&mut final_results, folder_context, folder_rules);
    propagate_master_match_within_groups(&mut final_results);

    final_results
}

/// グループ内でマスタ照合結果を伝搬する
///
/// 解析 engine の machine_id 由来グループ番号を利用して、
/// マスタ照合に成功した写真(リーダー)の結果を、同一グループ内の
/// マスタ照合失敗写真(remarks空)に伝搬する。
///
/// ゲート: グループサイズ2〜6のみ対象（machine_id=""の巨大グループを除外）
fn propagate_master_match_within_groups(results: &mut [analyzer::AnalysisResult]) {
    use std::collections::HashMap;
    let mut groups: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, r) in results.iter().enumerate() {
        if r.group > 0 {
            groups.entry(r.group).or_default().push(i);
        }
    }
    for indices in groups.values() {
        if indices.len() < 2 || indices.len() > 6 {
            continue;
        }
        // リーダー: remarks非空 & detected_text最長
        let leader = indices
            .iter()
            .filter(|&&i| !results[i].remarks.is_empty())
            .max_by_key(|&&i| results[i].detected_text.len())
            .copied();
        let Some(leader) = leader else { continue };
        let snap_work_type = results[leader].work_type.clone();
        let snap_variety = results[leader].variety.clone();
        let snap_subphase = results[leader].subphase.clone();
        let snap_photo_category = results[leader].photo_category.clone();
        let snap_remarks = results[leader].remarks.clone();
        // マッチ失敗写真（remarks空 & photo_category空）にリーダーの結果を伝搬
        for &idx in indices {
            if idx == leader {
                continue;
            }
            if results[idx].remarks.is_empty() && results[idx].photo_category.is_empty() {
                results[idx].work_type = snap_work_type.clone();
                results[idx].variety = snap_variety.clone();
                results[idx].subphase = snap_subphase.clone();
                results[idx].photo_category = snap_photo_category.clone();
                results[idx].remarks = snap_remarks.clone();
            }
        }
    }
}

/// フォルダ内の文脈でドメイン固有の種別補正を適用
///
/// - 路面切削工が存在する場合、舗装打換え工/表層工 → 切削オーバーレイ工/表層工
/// - 区画線工の線種判定（line_typesが指定されている場合のみ）
fn apply_domain_corrections(
    results: &mut [analyzer::AnalysisResult],
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
        let cat = result.photo_category_enum();
        if cat.map(photo_ai_common::domain::policy::category_uses_date_station).unwrap_or(false) {
            // 安全管理・品質管理: 既に日付形式のstationがあれば上書きしない（tagger由来の作業日を優先）
            let already_has_date = photo_ai_common::domain::Station::parse(&result.station).is_date();
            if !already_has_date {
                result.station = if station_is_date {
                    station.to_string()
                } else {
                    date_to_month_day(&result.date)
                };
            }
            continue;
        }
        if result.work_type == WORK_LANE_MARKING {
            // 区画線工は線種ごとに撮影するため測点を一律適用しない
            // 以前のnormalize -Sで誤設定された値もクリア
            if result.station == station {
                result.station.clear();
            }
        } else {
            result.station = station.to_string();
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

}

fn apply_folder_specific_corrections(
    results: &mut [analyzer::AnalysisResult],
    folder_name: &str,
    folder_rules: Option<&[FolderRule]>,
) {
    if let Some(rules) = folder_rules {
        crate::folder_rules::apply_folder_specific_corrections(results, folder_name, rules);
    }
}

// 温度管理ロジックは crate::temperature モジュールに移動済み
