//! コマンドハンドラモジュール
//!
//! CLIコマンドの実行ロジックを提供します。

use crate::ai_provider::AiProvider;
use crate::analysis::{apply_station, ScanAnalysisConfig, prepare_analysis, scan_and_analyze};
use crate::cli::{ExportFormat, PdfQuality};
use crate::error::{self, Result};
use crate::normalizer::{self, NormalizationOptions};
use crate::{analyzer, export, master_selector, matcher};
use ai_code_review::{CodeReviewer, Backend as ReviewBackend};
use std::path::{Path, PathBuf};

// MasterConfig を再エクスポート
pub use crate::analysis::MasterConfig;

/// 共通CLI引数
#[derive(Clone)]
pub struct CommonCliArgs {
    pub verbose: bool,
    pub provider: AiProvider,
}

/// Analyzeコマンドの引数
pub struct AnalyzeCommandArgs {
    pub folder: PathBuf,
    pub output: Option<PathBuf>,
    pub batch_size: usize,
    pub master: Option<PathBuf>,
    pub work_type: Option<String>,
    pub variety: Option<String>,
    pub station: Option<String>,
    pub use_cache: bool,
    pub recursive: bool,
    pub include_all: bool,
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
    pub variety: Option<String>,
    pub station: Option<String>,
    pub pdf_quality: PdfQuality,
    pub use_cache: bool,
    pub recursive: bool,
    pub include_all: bool,
    pub cli_args: CommonCliArgs,
}

/// Exportコマンドの引数
pub struct ExportCommandArgs {
    pub input: PathBuf,
    pub format: ExportFormat,
    pub output: Option<PathBuf>,
    pub photos_per_page: u8,
    pub title: String,
    pub pdf_quality: PdfQuality,
    pub preset: Option<String>,
    pub alias: Option<PathBuf>,
}

/// Reviewコマンドの引数
pub struct ReviewCommandArgs {
    pub path: PathBuf,
    pub watch: bool,
    pub model: Option<String>,
    pub cli_args: CommonCliArgs,
}

/// Normalizeコマンドの引数
pub struct NormalizeCommandArgs {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub station: Option<String>,
    pub dry_run: bool,
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
    variety: Option<String>,
    station: Option<String>,
    use_cache: bool,
    recursive: bool,
    include_all: bool,
    verbose: bool,
    provider: AiProvider,
}

fn resolve_master_path(master: Option<PathBuf>, interactive: bool) -> Option<master_selector::MasterSelection> {
    if let Some(path) = master {
        // パスからwork_typeを推定（by_work_type/xxx.csv → xxx）
        let work_type = path.file_stem()
            .and_then(|s| s.to_str())
            .filter(|s| *s != "construction_hierarchy")
            .map(|s| s.to_string());
        return Some(master_selector::MasterSelection { path, work_type });
    }

    // 対話式選択
    if interactive {
        return master_selector::select_master_interactive();
    }

    // デフォルトマスタ
    let default_path = PathBuf::from("master").join("construction_hierarchy.csv");
    if default_path.exists() {
        Some(master_selector::MasterSelection { path: default_path, work_type: None })
    } else {
        None
    }
}

/// 共通の解析処理を実行（ヘッダー表示→prepare_analysis→scan_and_analyze）
async fn run_common_analysis(
    params: &CommonAnalysisParams,
    header: &str,
    step_prefix_scan: &str,
    step_prefix_analyze: &str,
) -> Result<Vec<analyzer::AnalysisResult>> {
    println!("{}\n", header);

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
        use_cache: params.use_cache,
        provider: params.provider,
        variety: params.variety.as_ref(),
        station: params.station.as_ref(),
        recursive: params.recursive,
        include_all: params.include_all,
        step_prefix_scan,
        step_prefix_analyze,
    };

    scan_and_analyze(&scan_config).await
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
        results = matcher::apply_aliases(
            &results,
            args.preset.as_deref(),
            args.alias.as_ref().map(|p| p.as_path()),
        )?;
        println!("✔ エイリアス変換完了");
    }

    let output_dir = args.output.unwrap_or_else(|| PathBuf::from("."));

    export::export_results(&results, &args.format, &output_dir, args.photos_per_page, &args.title, args.pdf_quality)?;

    println!("\n✅ エクスポート完了");
    Ok(())
}

/// Analyzeコマンドを処理
pub async fn handle_analyze_command(args: AnalyzeCommandArgs) -> Result<()> {
    // 共通解析処理
    let params = CommonAnalysisParams {
        folder: args.folder.clone(),
        batch_size: args.batch_size,
        master: args.master,
        work_type: args.work_type,
        variety: args.variety,
        station: args.station,
        use_cache: args.use_cache,
        recursive: args.recursive,
        include_all: args.include_all,
        verbose: args.cli_args.verbose,
        provider: args.cli_args.provider,
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
    // 共通解析処理
    let params = CommonAnalysisParams {
        folder: args.folder.clone(),
        batch_size: args.batch_size,
        master: args.master,
        work_type: args.work_type,
        variety: args.variety,
        station: args.station,
        use_cache: args.use_cache,
        recursive: args.recursive,
        include_all: args.include_all,
        verbose: args.cli_args.verbose,
        provider: args.cli_args.provider,
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
    export::export_results(&results, &args.format, &output_paths.export_path, 3, "工事写真帳", args.pdf_quality)?;

    println!("\n✅ 完了");
    Ok(())
}

/// Reviewコマンドを処理
pub fn handle_review_command(args: ReviewCommandArgs) -> Result<()> {
    println!("🔍 photo-ai-rust - コードレビュー\n");

    // AIプロバイダからレビューバックエンドへ変換
    let backend = match args.cli_args.provider {
        AiProvider::Claude => ReviewBackend::Claude,
        AiProvider::Codex => ReviewBackend::Codex,
        AiProvider::Gemini => ReviewBackend::Gemini,
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
    let options = NormalizationOptions::default();

    // 正規化実行
    let result = normalizer::normalize_results(&results, &options);

    // 統計表示
    println!("\n📊 正規化結果:");
    println!("  総レコード数: {}", result.stats.total_records);
    println!("  修正対象: {}件", result.stats.corrected_records);
    println!("  - 計測値修正: {}件", result.stats.measurement_corrections);

    // 修正内容を表示
    if !result.corrections.is_empty() {
        println!("\n📝 修正内容:");
        for correction in &result.corrections {
            println!(
                "  {} [{}]: {} → {}",
                correction.file_name,
                correction.field,
                correction.original,
                correction.corrected
            );
        }
    }

    // ドライランでなければ適用
    let has_changes = args.station.is_some() || !result.corrections.is_empty();
    if !args.dry_run && has_changes {
        normalizer::apply_corrections(&mut results, &result.corrections);

        let output_path = args.output.unwrap_or(args.input);
        let json = serde_json::to_string_pretty(&results)?;
        std::fs::write(&output_path, json)?;
        println!("\n✔ 保存: {}", output_path.display());
    } else if args.dry_run {
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
