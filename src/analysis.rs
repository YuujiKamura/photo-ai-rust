//! 解析関連のヘルパー関数とデータ構造
//!
//! AI写真解析のためのスキャン、解析、正規化処理を提供する。

use crate::{ai_provider::AiProvider, analyzer, error, scanner};
use crate::normalizer::{self, NormalizationOptions};
use photo_ai_common::HierarchyMaster;
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
    })
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

    // 2. AI解析（1ステップ解析）
    let analysis_options = AnalysisOptions {
        folder: config.folder,
        batch_size: config.batch_size,
        verbose: config.verbose,
        master: config.master_config.master_path.as_deref(),
        photo_type: config.photo_type,
        use_cache: config.use_cache,
        provider: config.provider,
        work_type: config.master_config.effective_work_type.as_deref(),
        variety: config.variety.map(|s| s.as_str()),
        step_prefix: config.step_prefix_analyze,
    };
    let mut results = run_analysis(&images, &analysis_options).await?;
    println!("✔ 解析完了\n");

    // 3. 測点適用と正規化
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

    let hierarchy = HierarchyMaster::from_csv(&master_path_buf)
        .map_err(|e| error::PhotoAiError::MasterLoad(e.to_string()))?;

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

/// 測点を一括適用
pub fn apply_station(results: &mut [analyzer::AnalysisResult], station: &str) {
    for result in results {
        result.station = station.to_string();
    }
}
