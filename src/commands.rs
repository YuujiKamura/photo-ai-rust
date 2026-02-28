//! コマンドハンドラモジュール
//!
//! CLIコマンドの実行ロジックを提供します。

use crate::analysis::{apply_station, ScanAnalysisConfig, prepare_analysis, scan_and_analyze};
use crate::cli::{Commands, ExportFormat, GtAction, PdfQuality, ReviewBackendArg};
use crate::config::Config;
use crate::error::{self, Result};
use crate::normalizer::{self, NormalizationOptions};
use crate::{analyzer, export, master_selector};
use ai_code_review::{CodeReviewer, Backend as ReviewBackend};
use photo_ai_common::{LineTypeEntry, LineTypesConfig};
use photo_tagger::{ApiKeyGuard, UsageMode};
use std::path::{Path, PathBuf};

// MasterConfig を再エクスポート
pub use crate::analysis::MasterConfig;

/// 共通CLI引数
#[derive(Clone)]
pub struct CommonCliArgs {
    pub verbose: bool,
}

/// Analyzeコマンドの引数
pub struct AnalyzeCommandArgs {
    pub folder: PathBuf,
    pub output: Option<PathBuf>,
    pub batch_size: usize,
    pub master: Option<PathBuf>,
    pub work_type: Option<String>,
    pub photo_type: Option<String>,
    pub variety: Option<String>,
    pub station: Option<String>,
    pub use_cache: bool,
    pub recursive: bool,
    pub include_all: bool,
    pub line_types: Option<Vec<LineTypeEntry>>,
    pub folder_rules: Option<PathBuf>,
    pub usage_mode: UsageMode,
    pub cli_args: CommonCliArgs,
}

/// Runコマンドの引数
pub struct RunCommandArgs {
    pub folder: PathBuf,
    pub output: Option<PathBuf>,
    pub format: ExportFormat,
    pub batch_size: usize,
    pub master: Option<PathBuf>,
    pub work_type: Option<String>,
    pub photo_type: Option<String>,
    pub variety: Option<String>,
    pub station: Option<String>,
    pub pdf_quality: PdfQuality,
    pub use_cache: bool,
    pub recursive: bool,
    pub include_all: bool,
    pub line_types: Option<Vec<LineTypeEntry>>,
    pub folder_rules: Option<PathBuf>,
    pub usage_mode: UsageMode,
    pub cli_args: CommonCliArgs,
}

/// Exportコマンドの引数
pub struct ExportCommandArgs {
    pub input: PathBuf,
    pub format: ExportFormat,
    pub output: Option<PathBuf>,
    pub photos_per_page: u8,
    pub pdf_quality: PdfQuality,
    pub preset: Option<String>,
    pub alias: Option<PathBuf>,
}

/// Reviewコマンドの引数
pub struct ReviewCommandArgs {
    pub path: PathBuf,
    pub watch: bool,
    pub model: Option<String>,
    pub backend: ReviewBackendArg,
    pub cli_args: CommonCliArgs,
}

/// Normalizeコマンドの引数
pub struct NormalizeCommandArgs {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub station: Option<String>,
    pub lane: Option<normalizer::Lane>,
    pub dekigata_remarks: Option<String>,
    pub dry_run: bool,
    pub line_types: Option<Vec<LineTypeEntry>>,
}

/// Configコマンドの引数
pub struct ConfigCommandArgs {
    pub set_api_key: Option<String>,
    pub show: bool,
    pub config: crate::config::Config,
}

/// Cacheコマンドの引数
pub struct CacheCommandArgs {
    pub clear: bool,
    pub folder: Option<PathBuf>,
    pub info: bool,
}

/// Evaluateコマンドの引数
pub struct EvaluateCommandArgs {
    pub input: PathBuf,
    pub gt: PathBuf,
    pub fields: Option<String>,
    pub json: bool,
}

/// PairCompletionコマンドの引数
pub struct PairCompletionCommandArgs {
    pub before: PathBuf,
    pub after: PathBuf,
    pub output: Option<PathBuf>,
    pub project_name: Option<String>,
    pub build: bool,
    pub cli_args: CommonCliArgs,
}

/// PairReplaceコマンドの引数
pub struct PairReplaceCommandArgs {
    pub folder: PathBuf,
    pub pairs: String,
    pub new_after: PathBuf,
    pub project_name: String,
    pub output: Option<PathBuf>,
}

/// PairPdfコマンドの引数
pub struct PairPdfCommandArgs {
    pub folder: Option<PathBuf>,
    pub json: Option<PathBuf>,
    pub project_name: String,
    pub output: Option<PathBuf>,
}

/// Stationコマンドの引数
pub struct StationCommandArgs {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
}

/// 出力パスの解決結果
pub struct OutputPaths {
    /// result.json を保存するディレクトリ
    pub json_dir: PathBuf,
    /// エクスポート先パス
    pub export_path: PathBuf,
}

/// 出力パスを解決する
///
/// output がファイルパス(.pdf等)の場合、result.json は入力フォルダに保存
/// output がディレクトリの場合、両方とも指定ディレクトリに保存
/// output が未指定の場合、両方とも入力フォルダに保存
pub fn resolve_output_paths(folder: &Path, output: Option<&PathBuf>) -> OutputPaths {
    if let Some(out) = output {
        if out.extension().is_some() && !out.is_dir() {
            // ファイルパス指定: result.json は入力フォルダ、エクスポートは指定パス
            OutputPaths {
                json_dir: folder.to_path_buf(),
                export_path: out.clone(),
            }
        } else {
            // ディレクトリ指定
            OutputPaths {
                json_dir: out.clone(),
                export_path: out.clone(),
            }
        }
    } else {
        // 未指定: 入力フォルダを使用
        OutputPaths {
            json_dir: folder.to_path_buf(),
            export_path: folder.to_path_buf(),
        }
    }
}

/// 共通解析パラメータ（RunとAnalyzeで共有）
struct CommonAnalysisParams {
    folder: PathBuf,
    batch_size: usize,
    master: Option<PathBuf>,
    work_type: Option<String>,
    photo_type: Option<String>,
    variety: Option<String>,
    station: Option<String>,
    use_cache: bool,
    recursive: bool,
    include_all: bool,
    verbose: bool,
    line_types: Option<Vec<LineTypeEntry>>,
    folder_rules: Option<PathBuf>,
    usage_mode: UsageMode,
}

/// CLIで指定されたマスタパスからMasterSelectionを解決（純粋ロジック、UI無し）
fn resolve_master_from_cli(master: Option<PathBuf>) -> Option<master_selector::MasterSelection> {
    let path = master?;
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if stem == "construction_hierarchy" {
        // 共通マスタ指定 → 全工種マージ読み込み
        let all_paths = master_selector::collect_all_master_paths();
        Some(master_selector::MasterSelection { path, work_type: None, all_paths })
    } else {
        let work_type = Some(stem.to_string());
        Some(master_selector::MasterSelection { path, work_type, all_paths: None })
    }
}

fn resolve_master_path(master: Option<PathBuf>, interactive: bool) -> Option<master_selector::MasterSelection> {
    // 1. CLI指定があればそれを使う
    if let Some(selection) = resolve_master_from_cli(master) {
        return Some(selection);
    }

    // 2. 対話式選択
    if interactive {
        return master_selector::select_master_interactive();
    }

    // 3. デフォルトマスタ（全工種 → マージ読み込み）
    let all_paths = master_selector::collect_all_master_paths();
    match all_paths {
        Some(paths) => {
            let first = paths[0].clone();
            Some(master_selector::MasterSelection {
                path: first,
                work_type: None,
                all_paths: Some(paths),
            })
        }
        None => None,
    }
}

/// 共通の解析処理を実行（ヘッダー表示→prepare_analysis→scan_and_analyze）
///
/// PayPerUseモードの場合、呼び出し元がApiKeyGuardを保持している前提。
/// guardのlifetime中はenv var GEMINI_API_KEYがセットされている。
async fn run_common_analysis(
    params: &CommonAnalysisParams,
    header: &str,
    step_prefix_scan: &str,
    step_prefix_analyze: &str,
) -> Result<Vec<analyzer::AnalysisResult>> {
    println!("{}\n", header);

    // フォルダルールの読み込み
    let folder_rules = load_folder_rules_from_path(params.folder_rules.as_ref())?;

    // マスタ選択と検証
    let master_config = prepare_analysis(
        params.master.clone(),
        params.work_type.clone(),
        params.variety.as_ref(),
        resolve_master_path,
    )?;

    // スキャンから正規化まで
    let scan_config = ScanAnalysisConfig {
        folder: &params.folder,
        batch_size: params.batch_size,
        verbose: params.verbose,
        master_config: &master_config,
        photo_type: params.photo_type.as_deref(),
        use_cache: params.use_cache,
        variety: params.variety.as_ref(),
        station: params.station.as_ref(),
        recursive: params.recursive,
        include_all: params.include_all,
        step_prefix_scan,
        step_prefix_analyze,
        line_types: params.line_types.as_deref(),
        folder_rules: folder_rules.as_deref(),
        usage_mode: params.usage_mode,
    };

    scan_and_analyze(&scan_config).await
}

/// エクスポート共通パラメータ
pub struct ExportParams<'a> {
    pub format: &'a ExportFormat,
    pub output: &'a Path,
    pub photos_per_page: u8,
    pub quality: PdfQuality,
    pub base_path: &'a Path,
}

/// タイトル導出とエクスポートの共通処理
fn run_export(results: &[analyzer::AnalysisResult], params: &ExportParams<'_>) -> Result<()> {
    let title = derive_export_title(results, params.base_path);
    export::export_results(results, params.format, params.output, params.photos_per_page, &title, params.quality)
}

/// Exportコマンドを処理
pub fn handle_export_command(args: ExportCommandArgs) -> Result<()> {
    println!("📄 photo-ai-rust - エクスポート\n");

    let content = std::fs::read_to_string(&args.input)?;
    let mut results: Vec<analyzer::AnalysisResult> = serde_json::from_str(&content)?;

    // JSONファイルの親ディレクトリを基準に相対パスを解決
    let base_dir = args.input.parent().unwrap_or(Path::new("."));
    for result in &mut results {
        if !result.file_path.is_empty() {
            let path = Path::new(&result.file_path);
            if path.is_relative() {
                if let Ok(abs_path) = base_dir.join(path).canonicalize() {
                    result.file_path = abs_path.to_string_lossy().to_string();
                }
            }
        }
    }

    // エイリアス変換を適用
    if args.preset.is_some() || args.alias.is_some() {
        println!("- エイリアス変換中...");
        results = normalizer::apply_aliases(
            &results,
            args.preset.as_deref(),
            args.alias.as_deref(),
        )?;
        println!("✔ エイリアス変換完了");
    }

    let output_dir = args.output.unwrap_or_else(|| base_dir.to_path_buf());

    run_export(&results, &ExportParams {
        format: &args.format,
        output: &output_dir,
        photos_per_page: args.photos_per_page,
        quality: args.pdf_quality,
        base_path: base_dir,
    })?;

    println!("\n✅ エクスポート完了");
    Ok(())
}

/// Analyzeコマンドを処理
pub async fn handle_analyze_command(args: AnalyzeCommandArgs) -> Result<()> {
    // PayPerUseモード: APIキーを対話入力→暗号化保持
    let _api_key_guard = if args.usage_mode == UsageMode::PayPerUse {
        Some(ApiKeyGuard::prompt().map_err(|e| error::PhotoAiError::Config(e.to_string()))?)
    } else {
        None
    };

    // 共通解析処理
    let params = CommonAnalysisParams {
        folder: args.folder.clone(),
        batch_size: args.batch_size,
        master: args.master,
        work_type: args.work_type,
        photo_type: args.photo_type,
        variety: args.variety,
        station: args.station,
        use_cache: args.use_cache,
        recursive: args.recursive,
        include_all: args.include_all,
        verbose: args.cli_args.verbose,
        line_types: args.line_types,
        folder_rules: args.folder_rules,
        usage_mode: args.usage_mode,
    };
    let results = run_common_analysis(
        &params,
        "📸 photo-ai-rust - 写真解析",
        "[1/3]",
        "[2/3]",
    ).await?;

    // 3. 結果保存
    println!("[3/3] 結果を保存中...");
    let output_path = args.output.unwrap_or_else(|| args.folder.join("result.json"));
    let json = serde_json::to_string_pretty(&results)?;
    std::fs::write(&output_path, json)?;
    println!("✔ 結果を保存: {}", output_path.display());

    println!("\n✅ 解析完了");
    Ok(())
}

/// Runコマンドを処理する（スキャン→解析→保存→エクスポート）
pub async fn handle_run_command(args: RunCommandArgs) -> Result<()> {
    // PayPerUseモード: APIキーを対話入力→暗号化保持
    let _api_key_guard = if args.usage_mode == UsageMode::PayPerUse {
        Some(ApiKeyGuard::prompt().map_err(|e| error::PhotoAiError::Config(e.to_string()))?)
    } else {
        None
    };

    // 共通解析処理
    let params = CommonAnalysisParams {
        folder: args.folder.clone(),
        batch_size: args.batch_size,
        master: args.master,
        work_type: args.work_type,
        photo_type: args.photo_type,
        variety: args.variety,
        station: args.station,
        use_cache: args.use_cache,
        recursive: args.recursive,
        include_all: args.include_all,
        verbose: args.cli_args.verbose,
        line_types: args.line_types,
        folder_rules: args.folder_rules,
        usage_mode: args.usage_mode,
    };
    let results = run_common_analysis(
        &params,
        "🚀 photo-ai-rust - 一括処理",
        "[1/4]",
        "[2/4]",
    ).await?;

    // 3. 結果保存
    let output_paths = resolve_output_paths(&args.folder, args.output.as_ref());
    println!("[3/4] 結果を保存中...");
    let json_path = output_paths.json_dir.join("result.json");
    let json = serde_json::to_string_pretty(&results)?;
    std::fs::write(&json_path, &json)?;
    println!("✔ 結果を保存: {}", json_path.display());

    // 4. Export
    println!("[4/4] エクスポート中...");
    run_export(&results, &ExportParams {
        format: &args.format,
        output: &output_paths.export_path,
        photos_per_page: 3,
        quality: args.pdf_quality,
        base_path: &args.folder,
    })?;

    println!("\n✅ 完了");
    Ok(())
}

/// Reviewコマンドを処理
pub fn handle_review_command(args: ReviewCommandArgs) -> Result<()> {
    println!("🔍 photo-ai-rust - コードレビュー\n");

    let backend = match args.backend {
        ReviewBackendArg::Gemini => ReviewBackend::Gemini,
        ReviewBackendArg::Claude => ReviewBackend::Claude,
        ReviewBackendArg::Codex => ReviewBackend::Codex,
    };

    // ファイル指定の場合は親ディレクトリでReviewerを初期化
    let (base_dir, target_file) = if args.path.is_file() {
        let parent = args.path.parent().unwrap_or(Path::new("."));
        (parent.to_path_buf(), Some(args.path.clone()))
    } else {
        (args.path.clone(), None)
    };

    let reviewer = CodeReviewer::new(&base_dir)
        .map_err(|e| error::PhotoAiError::CodeReview(e.to_string()))?
        .with_backend(backend);

    let mut reviewer = if let Some(ref m) = args.model {
        reviewer.with_model(m)
    } else {
        reviewer
    };

    if args.watch {
        println!("👀 ファイル監視中... (Ctrl+C で終了)\n");
        reviewer.start()
            .map_err(|e| error::PhotoAiError::CodeReview(e.to_string()))?;
    } else if let Some(ref file) = target_file {
        // 単発ファイルレビュー
        let result = reviewer.review_file(file)
            .map_err(|e| error::PhotoAiError::CodeReview(e.to_string()))?;

        println!("=== {} ===", result.path.display());
        println!("重要度: {:?}\n", result.severity);
        println!("{}", result.review);
    } else {
        // フォルダ内の全ファイルをレビュー（ここは簡易実装）
        println!("フォルダレビューは --watch モードをお使いください");
    }

    println!("\n✅ レビュー完了");
    Ok(())
}

/// Configコマンドを処理
pub fn handle_config_command(args: ConfigCommandArgs) -> Result<()> {
    let mut config = args.config;

    if let Some(key) = args.set_api_key {
        config.set_api_key(key)?;
        println!("✔ APIキーを設定しました");
    }

    if args.show {
        println!("設定:");
        println!("  モデル: {}", config.model);
        println!("  最大画像サイズ: {}px", config.max_image_size);
        println!("  バッチサイズ: {}", config.default_batch_size);
        println!("  APIキー: {}", if config.api_key.is_some() { "設定済み" } else { "未設定" });
    }

    Ok(())
}

/// 正規化統計を表示
fn print_normalize_stats(result: &normalizer::NormalizationResult) {
    println!("\n📊 正規化結果:");
    println!("  総レコード数: {}", result.stats.total_records);
    println!("  修正対象: {}件", result.stats.corrected_records);
    println!("  - 計測値修正: {}件", result.stats.measurement_corrections);
}

/// 修正内容を表示
fn print_corrections(corrections: &[normalizer::NormalizationCorrection]) {
    if !corrections.is_empty() {
        println!("\n📝 修正内容:");
        for correction in corrections {
            println!(
                "  {} [{}]: {} → {}",
                correction.file_name,
                correction.field,
                correction.original,
                correction.corrected
            );
        }
    }
}

/// 正規化結果を保存
fn save_normalize_results(results: &[analyzer::AnalysisResult], path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(results)?;
    std::fs::write(path, json)?;
    println!("\n✔ 保存: {}", path.display());
    Ok(())
}

/// Normalizeコマンドを処理
pub fn handle_normalize_command(args: NormalizeCommandArgs) -> Result<()> {
    println!("🔧 photo-ai-rust - 正規化\n");

    // JSONを読み込み
    let content = std::fs::read_to_string(&args.input)?;
    let mut results: Vec<analyzer::AnalysisResult> = serde_json::from_str(&content)?;
    println!("読み込み: {}件", results.len());

    // 測点一括適用
    if let Some(ref st) = args.station {
        println!("測点を一括適用: {}", st);
        apply_station(&mut results, st);
    }

    // 正規化オプション
    let options = NormalizationOptions {
        dekigata_lane: args.lane,
        dekigata_remarks: args.dekigata_remarks,
        ..Default::default()
    };

    // 正規化実行
    let result = normalizer::normalize_results(&results, &options);

    print_normalize_stats(&result);
    print_corrections(&result.corrections);

    // 区画線工の線種判定（差分実行）
    let line_type_changes = if let Some(ref lt) = args.line_types {
        if !lt.is_empty() {
            let changes = crate::line_type_detector::apply_line_type_detection(&mut results, lt);
            println!("線種判定: {}件更新", changes);
            changes
        } else { 0 }
    } else { 0 };

    // ドライランでなければ適用
    if !args.dry_run {
        let has_corrections = args.station.is_some() || !result.corrections.is_empty() || line_type_changes > 0;
        if has_corrections {
            normalizer::apply_corrections(&mut results, &result.corrections);
        }

        // 日付ソートは常に適用
        results.sort_by(|a, b| a.date.cmp(&b.date).then(a.file_name.cmp(&b.file_name)));

        // 3枚セット超過分をスキップ
        let deduped = normalizer::dedup_temperature_groups(&mut results);
        if deduped > 0 {
            println!("超過写真スキップ: {}件", deduped);
        }

        // ダンプ台数を測点に付加
        normalizer::append_dump_number_to_station(&mut results);

        let output_path = args.output.unwrap_or(args.input);
        save_normalize_results(&results, &output_path)?;
    } else {
        println!("\n[ドライラン] 変更は適用されませんでした");
    }

    println!("\n✅ 正規化完了");
    Ok(())
}

/// Stationコマンドを処理
pub fn handle_station_command(args: StationCommandArgs) -> Result<()> {
    use crate::station;

    println!("📍 photo-ai-rust - 測点入力\n");
    station::run_interactive_station(&args.input, args.output.as_deref())?;
    Ok(())
}

/// Cacheコマンドを処理
pub fn handle_cache_command(args: CacheCommandArgs) {
    let target = args.folder.unwrap_or_else(|| PathBuf::from("."));
    let cache_path = analyzer::CacheFile::cache_path(&target);

    if args.info || !args.clear {
        // デフォルトまたは--info: 情報表示
        if cache_path.exists() {
            let cache = analyzer::CacheFile::load(&target);
            println!("キャッシュ情報:");
            println!("  パス: {}", cache_path.display());
            println!("  件数: {}", cache.len());
            if let Ok(meta) = std::fs::metadata(&cache_path) {
                println!("  サイズ: {} bytes", meta.len());
            }
        } else {
            println!("キャッシュファイルが存在しません: {}", cache_path.display());
        }
    }

    if args.clear {
        match analyzer::CacheFile::clear(&target) {
            Ok(true) => println!("✔ キャッシュを削除しました: {}", cache_path.display()),
            Ok(false) => println!("キャッシュファイルが存在しません"),
            Err(e) => println!("キャッシュ削除エラー: {}", e),
        }
    }
}

/// フォルダルールJSONを読み込む
fn load_folder_rules_from_path(cli_path: Option<&PathBuf>) -> Result<Option<Vec<crate::folder_rules::FolderRule>>> {
    use crate::folder_rules;
    let resolved = folder_rules::resolve_rules_path(cli_path.map(|p| p.as_path()));
    match resolved {
        Some(path) => {
            let rules = folder_rules::load_folder_rules(&path)
                .map_err(|e| error::PhotoAiError::Config(format!("フォルダルール読み込み失敗: {}", e)))?;
            println!("フォルダルール読み込み: {}件 ({})", rules.len(), path.display());
            Ok(Some(rules))
        }
        None => Ok(None),
    }
}

/// --line-types オプションからJSONファイルを読み込む
fn load_line_types(path: Option<&PathBuf>) -> Result<Option<Vec<LineTypeEntry>>> {
    let Some(path) = path else { return Ok(None) };
    let content = std::fs::read_to_string(path)
        .map_err(|e| error::PhotoAiError::Config(format!("線種リスト読み込み失敗: {}: {}", path.display(), e)))?;
    let config: LineTypesConfig = serde_json::from_str(&content)
        .map_err(|e| error::PhotoAiError::Config(format!("線種リストJSON解析失敗: {}: {}", path.display(), e)))?;
    println!("線種リスト読み込み: {}種 ({})", config.line_types.len(), path.display());
    Ok(Some(config.line_types))
}

/// 写真区分名を略称に変換（"安全管理写真" → "安全管理"）
fn shorten_photo_category(cat: &str) -> String {
    match cat {
        "安全管理写真" => "安全管理".to_string(),
        "施工状況写真" => "施工状況".to_string(),
        "品質管理写真" => "品質管理".to_string(),
        "出来形管理写真" => "出来形管理".to_string(),
        "使用材料写真" => "使用材料".to_string(),
        "着手前及び完成写真" => "着手前完成".to_string(),
        _ => cat.trim_end_matches("写真").to_string(),
    }
}

/// 解析結果から最頻出の写真区分を取得
fn most_common_category(results: &[analyzer::AnalysisResult]) -> String {
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for r in results {
        if !r.photo_category.is_empty() {
            *counts.entry(r.photo_category.as_str()).or_default() += 1;
        }
    }
    counts.into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(k, _)| k.to_string())
        .unwrap_or_default()
}

/// フォルダ名からMMDD(4桁)を抽出。夜間工事で日付をまたいでも施工日（作業開始日）が正。
fn extract_mmdd_from_folder(folder_name: &str) -> Option<String> {
    let chars: Vec<char> = folder_name.chars().collect();
    for window in chars.windows(4) {
        if window.iter().all(|c| c.is_ascii_digit()) {
            let mmdd: String = window.iter().collect();
            let mm: u32 = mmdd[0..2].parse().ok()?;
            let dd: u32 = mmdd[2..4].parse().ok()?;
            if (1..=12).contains(&mm) && (1..=31).contains(&dd) {
                return Some(mmdd);
            }
        }
    }
    None
}

/// 解析結果の日付から最頻出日のMMDD形式を抽出。
/// 夜間工事では日付をまたぐ写真があるため、最頻出日（=施工日）を使う。
fn extract_mmdd_from_results(results: &[analyzer::AnalysisResult]) -> Option<String> {
    let mut dates: Vec<&str> = results.iter()
        .filter(|r| r.date.len() >= 10)
        .map(|r| &r.date[..10])
        .collect();
    if dates.is_empty() {
        return None;
    }
    dates.sort();
    // Find mode (most common date). On tie, earlier date wins (= work start date).
    let mut best = dates[0];
    let mut best_count = 0usize;
    let mut current = dates[0];
    let mut current_count = 0usize;
    for &d in &dates {
        if d == current {
            current_count += 1;
        } else {
            if current_count > best_count {
                best = current;
                best_count = current_count;
            }
            current = d;
            current_count = 1;
        }
    }
    if current_count > best_count {
        best = current;
    }
    let month = &best[5..7];
    let day = &best[8..10];
    Some(format!("{}{}", month, day))
}

/// 解析結果とフォルダ名から統一命名規則でタイトルを生成
///
/// 命名規則: {写真区分略称}_{活動名}_{MMDD}
/// 例: 出来形管理_切削出来形立会_0212, 施工状況_道路付属施設工_0220
///
/// 活動名の導出優先順位:
/// 1. 結果データの最頻出workType（空でなければ）
/// 2. 結果データの最頻出remarks（空でなければ）
/// 3. 結果データの最頻出subphase（空でなければ）
/// 4. フォルダ名（日付のみのフォルダ名は除外）
fn derive_export_title(results: &[analyzer::AnalysisResult], folder: &Path) -> String {
    let folder_name = folder.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("写真");
    // スペースを除去
    let folder_clean = folder_name.replace(' ', "");

    let category = most_common_category(results);
    if category.is_empty() {
        // 写真区分が不明: フォールバック
        return format!("写真帳_{}", folder_clean);
    }
    let cat_short = shorten_photo_category(&category);
    // MMDD: フォルダ名優先（施工日）、なければEXIF最頻出日
    let mmdd = extract_mmdd_from_folder(&folder_clean)
        .or_else(|| extract_mmdd_from_results(results));

    // 活動名を結果データから導出
    let derived_activity = derive_activity_name(results);

    // フォルダ名が日付パターン(数字のみ)なら活動名としては使わない
    let folder_is_date = folder_clean.chars().all(|c| c.is_ascii_digit());

    let activity_str: String;
    let activity = if let Some(ref act) = derived_activity {
        // 結果データから導出した活動名
        activity_str = act.clone();
        // 写真区分と同じなら省略
        if activity_str == cat_short || cat_short.contains(&activity_str) {
            None
        } else {
            Some(activity_str.as_str())
        }
    } else if !folder_is_date {
        // フォルダ名を使う（日付でない場合）
        if folder_clean == cat_short
            || folder_clean.contains(&cat_short)
            || cat_short.contains(&folder_clean)
        {
            None
        } else {
            Some(folder_clean.as_str())
        }
    } else {
        None
    };

    match (activity, mmdd.as_deref()) {
        (Some(act), Some(d)) => format!("{}_{}_{}", cat_short, act, d),
        (Some(act), None) => format!("{}_{}", cat_short, act),
        (None, Some(d)) => format!("{}_{}", cat_short, d),
        (None, None) => cat_short,
    }
}

/// 解析結果から活動名を導出する
///
/// 優先順位: workType → remarks(写真区分・工種と被らないもの) → subphase
fn derive_activity_name(results: &[analyzer::AnalysisResult]) -> Option<String> {
    // 1. 最頻出workType
    let work_type = most_common_field(results, |r| &r.work_type);
    if let Some(ref wt) = work_type {
        if !wt.is_empty() {
            return Some(wt.clone());
        }
    }

    // 2. 最頻出remarks（ただし写真区分の略称と被る場合は除外）
    let remarks = most_common_field(results, |r| &r.remarks);
    if let Some(ref rm) = remarks {
        if !rm.is_empty() {
            return Some(rm.clone());
        }
    }

    // 3. 最頻出subphase
    let subphase = most_common_field(results, |r| &r.subphase);
    if let Some(ref sp) = subphase {
        if !sp.is_empty() {
            return Some(sp.clone());
        }
    }

    None
}

/// 結果データの指定フィールドから最頻出の値を取得
fn most_common_field<F>(results: &[analyzer::AnalysisResult], field: F) -> Option<String>
where
    F: Fn(&analyzer::AnalysisResult) -> &str,
{
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for r in results {
        let val = field(r);
        if !val.is_empty() {
            *counts.entry(val).or_default() += 1;
        }
    }
    counts.into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(k, _)| k.to_string())
}

/// PairPdfコマンドを処理
pub fn handle_pair_pdf_command(args: PairPdfCommandArgs) -> Result<()> {
    println!("着手前竣工写真帳PDF生成\n");

    // ペア読み込み（JSON or フォルダスキャン）
    let pairs = if let Some(ref json_path) = args.json {
        println!("JSON入力: {}", json_path.display());
        crate::pair_completion::load_pairs_from_json(json_path, args.folder.as_deref())?
    } else if let Some(ref folder) = args.folder {
        crate::pair_completion::scan_pair_folders(folder)?
    } else {
        return Err(error::PhotoAiError::Config("--json か folder のどちらかを指定してください".to_string()));
    };
    println!("ペア数: {}", pairs.len());

    if pairs.is_empty() {
        return Err(error::PhotoAiError::Config("ペアが見つかりません".to_string()));
    }

    // 出力パス
    let default_dir = if let Some(ref json_path) = args.json {
        json_path.parent().unwrap_or(Path::new(".")).to_path_buf()
    } else {
        args.folder.as_ref().unwrap().parent().unwrap_or(Path::new(".")).join("写真帳まとめ")
    };
    let output_dir = args.output.unwrap_or(default_dir);
    let output_path = output_dir.join(format!("着手前竣工_{}.pdf", args.project_name));

    // PDF生成
    export::pair_pdf::generate_pair_pdf(&pairs, &args.project_name, &output_path)?;

    println!("\n✅ PDF生成完了: {}", output_path.display());
    Ok(())
}

/// Doctorコマンドを処理
pub fn handle_doctor_command() -> Result<()> {
    println!("🩺 photo-ai-rust doctor - 前提条件チェック\n");
    let mut all_ok = true;

    // 1. gemini CLI (Windows: gemini.cmd, Unix: gemini)
    print!("  gemini CLI ........ ");
    let gemini_names = if cfg!(windows) {
        vec!["gemini.cmd", "gemini"]
    } else {
        vec!["gemini"]
    };
    let gemini_result = gemini_names.iter().find_map(|name| {
        std::process::Command::new(name).arg("--version").output().ok()
            .filter(|o| o.status.success())
    });
    match gemini_result {
        Some(output) => {
            let ver = String::from_utf8_lossy(&output.stdout);
            let ver = ver.trim();
            println!("OK ({})", if ver.is_empty() { "version unknown" } else { ver });
        }
        None => {
            all_ok = false;
            println!("NG");
            println!("    → gemini がPATHにありません");
            println!("    → 対処: npm install -g @google/gemini-cli && gemini auth");
        }
    }

    // 2. マスタCSV
    print!("  マスタCSV ......... ");
    match master_selector::resolve_master_base_dir() {
        Some(base) => {
            let by_work_type = base.join("by_work_type");
            let count = std::fs::read_dir(&by_work_type)
                .map(|entries| entries.flatten().filter(|e| {
                    e.path().extension().map(|ext| ext == "csv").unwrap_or(false)
                }).count())
                .unwrap_or(0);
            println!("OK ({}ファイル: {})", count, by_work_type.display());
        }
        None => {
            all_ok = false;
            println!("NG");
            println!("    → マスタディレクトリが見つかりません");
            println!("    → 検索パス:");
            for candidate in master_selector::build_master_candidates() {
                println!("      - {}/by_work_type/", candidate.display());
            }
            println!("    → 対処: PHOTO_AI_MASTER_DIR 環境変数を設定 or --master で直接指定");
        }
    }

    // 3. Dropbox認証
    print!("  Dropbox認証 ....... ");
    let dropbox_config = dirs::config_dir()
        .map(|d| d.join("dropbox-fetch"));
    let has_token = dropbox_config
        .as_ref()
        .map(|d| d.join("token.json").exists())
        .unwrap_or(false);
    if has_token {
        println!("OK");
    } else {
        println!("未設定 (オプション)");
        println!("    → dropbox-fetch を使う場合: dropbox-fetch auth");
    }

    println!();
    if all_ok {
        println!("✅ すべてのチェックに合格しました");
    } else {
        println!("⚠ 一部のチェックが失敗しました（上記の対処法を確認してください）");
    }

    Ok(())
}

/// Evaluateコマンドを処理
pub fn handle_evaluate_command(args: EvaluateCommandArgs) -> Result<()> {
    use photo_ai_common::evaluate::{self, EvalField};

    // フィールド解析
    let fields: Vec<EvalField> = if let Some(ref field_str) = args.fields {
        field_str
            .split(',')
            .filter_map(|s| {
                let s = s.trim();
                EvalField::from_name(s).or_else(|| {
                    eprintln!("不明なフィールド: {}", s);
                    None
                })
            })
            .collect()
    } else {
        EvalField::default_fields()
    };

    if fields.is_empty() {
        return Err(error::PhotoAiError::Config(
            "有効な評価フィールドがありません".to_string(),
        ));
    }

    // パイプライン出力読み込み
    let pipe_content = std::fs::read_to_string(&args.input)?;
    let pipeline: Vec<analyzer::AnalysisResult> = serde_json::from_str(&pipe_content)?;

    // GT読み込み
    let gt_content = std::fs::read_to_string(&args.gt)?;
    let gt: Vec<analyzer::AnalysisResult> = serde_json::from_str(&gt_content)?;

    // 評価実行
    let report = evaluate::evaluate(&pipeline, &gt, &fields);

    if args.json {
        // JSON出力（CI連携用）
        let mut json_output = serde_json::Map::new();
        json_output.insert("totalPhotos".to_string(), serde_json::json!(report.total_photos));
        json_output.insert("matchedPhotos".to_string(), serde_json::json!(report.matched_photos));
        json_output.insert("missingPhotos".to_string(), serde_json::json!(report.missing_photos));

        let fields_json: Vec<serde_json::Value> = report.field_results.iter().map(|fr| {
            serde_json::json!({
                "field": fr.field.name(),
                "total": fr.total,
                "matched": fr.matched,
                "accuracy": fr.accuracy(),
            })
        }).collect();
        json_output.insert("fields".to_string(), serde_json::json!(fields_json));

        let diffs_json: Vec<serde_json::Value> = report.diffs.iter().map(|d| {
            serde_json::json!({
                "fileName": d.file_name,
                "field": d.field.name(),
                "got": d.got,
                "expected": d.expected,
            })
        }).collect();
        json_output.insert("diffs".to_string(), serde_json::json!(diffs_json));

        println!("{}", serde_json::to_string_pretty(&json_output)?);
    } else {
        evaluate::print_report(&report);
    }

    Ok(())
}

/// Gtコマンドを処理
pub fn handle_gt_command(action: GtAction) -> Result<()> {
    use crate::gt;

    match action {
        GtAction::Import { source, output_dir, name } => {
            let results = gt::import_from_source(&source)?;
            let stem = name.unwrap_or_else(|| {
                source.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            });
            let file_name = if stem.ends_with("__result") {
                format!("{}.json", stem)
            } else {
                format!("{}__result.json", stem)
            };
            let output_path = output_dir.join(&file_name);
            gt::save_as_gt(&results, &output_path)?;
            println!("✔ GT取り込み: {}枚 → {}", results.len(), output_path.display());
        }
        GtAction::Compare { gt_dir, pipeline_dir } => {
            let report = gt::compare_folders(&gt_dir, &pipeline_dir)?;
            gt::print_report(&report);
        }
    }
    Ok(())
}

impl Commands {
    /// コマンドを実行する
    pub async fn execute(self, cli_args: &CommonCliArgs, config: Config) -> Result<()> {
        match self {
            Commands::Analyze { folder, output, batch_size, master, work_type, photo_type, variety, station, use_cache, recursive, include_all, line_types, folder_rules, pay_per_use } => {
                let line_types = load_line_types(line_types.as_ref())?;
                let usage_mode = if pay_per_use { UsageMode::PayPerUse } else { UsageMode::TimeBasedQuota };
                handle_analyze_command(AnalyzeCommandArgs {
                    folder,
                    output,
                    batch_size,
                    master,
                    work_type,
                    photo_type,
                    variety,
                    station,
                    use_cache,
                    recursive,
                    include_all,
                    line_types,
                    folder_rules,
                    usage_mode,
                    cli_args: cli_args.clone(),
                }).await?;
            }

            Commands::Export { input, format, output, photos_per_page, pdf_quality, preset, alias } => {
                handle_export_command(ExportCommandArgs {
                    input,
                    format,
                    output,
                    photos_per_page,
                    pdf_quality,
                    preset,
                    alias,
                })?;
            }

            Commands::Run { folder, output, format, batch_size, master, work_type, photo_type, variety, station, pdf_quality, use_cache, recursive, include_all, line_types, folder_rules, pay_per_use } => {
                let line_types = load_line_types(line_types.as_ref())?;
                let usage_mode = if pay_per_use { UsageMode::PayPerUse } else { UsageMode::TimeBasedQuota };
                handle_run_command(RunCommandArgs {
                    folder,
                    output,
                    format,
                    batch_size,
                    master,
                    work_type,
                    photo_type,
                    variety,
                    station,
                    pdf_quality,
                    use_cache,
                    recursive,
                    include_all,
                    line_types,
                    folder_rules,
                    usage_mode,
                    cli_args: cli_args.clone(),
                }).await?;
            }

            Commands::Config { set_api_key, show } => {
                handle_config_command(ConfigCommandArgs {
                    set_api_key,
                    show,
                    config,
                })?;
            }

            Commands::Station { input, output } => {
                handle_station_command(StationCommandArgs {
                    input,
                    output,
                })?;
            }

            Commands::Cache { clear, folder, info } => {
                handle_cache_command(CacheCommandArgs {
                    clear,
                    folder,
                    info,
                });
            }

            Commands::Normalize { input, output, station, lane, dekigata_remarks, dry_run, line_types } => {
                let line_types = load_line_types(line_types.as_ref())?;
                handle_normalize_command(NormalizeCommandArgs {
                    input,
                    output,
                    station,
                    lane: lane.map(|l| l.to_lane()),
                    dekigata_remarks,
                    dry_run,
                    line_types,
                })?;
            }

            Commands::Review { path, watch, model, backend } => {
                handle_review_command(ReviewCommandArgs {
                    path,
                    watch,
                    model,
                    backend,
                    cli_args: cli_args.clone(),
                })?;
            }

            Commands::Evaluate { input, gt, fields, json } => {
                handle_evaluate_command(EvaluateCommandArgs {
                    input,
                    gt,
                    fields,
                    json,
                })?;
            }

            Commands::PairPdf { folder, json, project_name, output } => {
                handle_pair_pdf_command(PairPdfCommandArgs {
                    folder,
                    json,
                    project_name,
                    output,
                })?;
            }

            Commands::PairReplace { folder, pairs, new_after, project_name, output } => {
                crate::pair_completion::handle_pair_replace(PairReplaceCommandArgs {
                    folder, pairs, new_after, project_name, output,
                })?;
            }

            Commands::Doctor => {
                handle_doctor_command()?;
            }

            Commands::PairCompletion { before, after, output, project_name, build } => {
                crate::pair_completion::handle_pair_completion(PairCompletionCommandArgs {
                    before, after, output, project_name, build,
                    cli_args: cli_args.clone(),
                }).await?;
            }

            Commands::Gt { action } => {
                handle_gt_command(action)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_mmdd_from_folder() {
        assert_eq!(extract_mmdd_from_folder("0209切削_温度管理"), Some("0209".into()));
        assert_eq!(extract_mmdd_from_folder("品質管理_温度管理_0213"), Some("0213".into()));
        assert_eq!(extract_mmdd_from_folder("0212"), Some("0212".into()));
        assert_eq!(extract_mmdd_from_folder("温度管理"), None);
        assert_eq!(extract_mmdd_from_folder("1399_bad"), None); // month 13 invalid
    }

    #[test]
    fn test_extract_mmdd_from_results_uses_mode_date() {
        // 夜間工事: 大半が2/12、一部が日付またぎで2/13 → 施工日2/12が正
        let results: Vec<analyzer::AnalysisResult> = vec![
            analyzer::AnalysisResult { date: "2026-02-12 21:00:00".into(), ..Default::default() },
            analyzer::AnalysisResult { date: "2026-02-12 22:30:00".into(), ..Default::default() },
            analyzer::AnalysisResult { date: "2026-02-12 23:45:00".into(), ..Default::default() },
            analyzer::AnalysisResult { date: "2026-02-13 00:30:00".into(), ..Default::default() },
            analyzer::AnalysisResult { date: "2026-02-13 02:00:00".into(), ..Default::default() },
        ];
        assert_eq!(extract_mmdd_from_results(&results), Some("0212".into()));
    }
}
