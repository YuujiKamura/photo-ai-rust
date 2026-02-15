//! コマンドハンドラモジュール
//!
//! CLIコマンドの実行ロジックを提供します。

use crate::ai_provider::AiProvider;
use crate::analysis::{apply_station, ScanAnalysisConfig, prepare_analysis, scan_and_analyze};
use crate::cli::{Commands, ExportFormat, PdfQuality};
use crate::config::Config;
use crate::error::{self, Result};
use crate::export_history::{self, ExportHistoryEntry};
use crate::normalizer::{self, NormalizationOptions};
use crate::{analyzer, export, master_selector};
use ai_code_review::{CodeReviewer, Backend as ReviewBackend};
use photo_ai_common::{LineTypeEntry, LineTypesConfig};
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
    pub photo_type: Option<String>,
    pub variety: Option<String>,
    pub station: Option<String>,
    pub use_cache: bool,
    pub recursive: bool,
    pub include_all: bool,
    pub line_types: Option<Vec<LineTypeEntry>>,
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
    provider: AiProvider,
    line_types: Option<Vec<LineTypeEntry>>,
}

fn resolve_master_path(master: Option<PathBuf>, interactive: bool) -> Option<master_selector::MasterSelection> {
    if let Some(path) = master {
        // パスからwork_typeを推定（by_work_type/xxx.csv → xxx）
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if stem == "construction_hierarchy" {
            // 共通マスタ指定 → 全工種マージ読み込み
            let all_paths = master_selector::collect_all_master_paths();
            return Some(master_selector::MasterSelection { path, work_type: None, all_paths });
        }
        let work_type = Some(stem.to_string());
        return Some(master_selector::MasterSelection { path, work_type, all_paths: None });
    }

    // 対話式選択
    if interactive {
        return master_selector::select_master_interactive();
    }

    // デフォルトマスタ（全工種 → マージ読み込み）
    let default_path = PathBuf::from("master").join("construction_hierarchy.csv");
    if default_path.exists() {
        let all_paths = master_selector::collect_all_master_paths();
        Some(master_selector::MasterSelection { path: default_path, work_type: None, all_paths })
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
        photo_type: params.photo_type.as_deref(),
        use_cache: params.use_cache,
        provider: params.provider,
        variety: params.variety.as_ref(),
        station: params.station.as_ref(),
        recursive: params.recursive,
        include_all: params.include_all,
        step_prefix_scan,
        step_prefix_analyze,
        line_types: params.line_types.as_deref(),
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

    // タイトルがデフォルトの場合、結果データまたは入力パスから自動導出
    let title = if args.title == "工事写真帳" {
        derive_export_title(&results, args.input.parent().unwrap_or(Path::new(".")))
    } else {
        args.title.clone()
    };

    // エイリアス変換を適用
    if args.preset.is_some() || args.alias.is_some() {
        println!("- エイリアス変換中...");
        results = normalizer::apply_aliases(
            &results,
            args.preset.as_deref(),
            args.alias.as_ref().map(|p| p.as_path()),
        )?;
        println!("✔ エイリアス変換完了");
    }

    let output_dir = args.output.unwrap_or_else(|| {
        args.input.parent().unwrap_or(Path::new(".")).to_path_buf()
    });

    export::export_results(&results, &args.format, &output_dir, args.photos_per_page, &title, args.pdf_quality)?;

    // 履歴記録
    let input_abs = std::fs::canonicalize(&args.input)
        .unwrap_or_else(|_| args.input.clone());
    let output_abs = std::fs::canonicalize(&output_dir)
        .unwrap_or_else(|_| output_dir.clone());
    if let Err(e) = export_history::record(ExportHistoryEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        input: input_abs.to_string_lossy().to_string(),
        format: args.format.to_string(),
        output: output_abs.to_string_lossy().to_string(),
        photos_per_page: args.photos_per_page,
        title: title.clone(),
        pdf_quality: args.pdf_quality.to_string(),
        preset: args.preset.clone(),
        alias: args.alias.as_ref().map(|p| p.to_string_lossy().to_string()),
    }) {
        eprintln!("⚠ 履歴記録に失敗: {e}");
    }

    println!("\n✅ エクスポート完了");
    Ok(())
}

/// ReExportAllコマンドを処理
pub fn handle_re_export_all_command(
    format: Option<ExportFormat>,
    pdf_quality: Option<PdfQuality>,
    dry_run: bool,
) -> Result<()> {
    let entries = export_history::list();
    if entries.is_empty() {
        println!("履歴がありません");
        return Ok(());
    }

    if dry_run {
        println!("📋 エクスポート履歴 ({}件)\n", entries.len());
        for (i, entry) in entries.iter().enumerate() {
            let exists = Path::new(&entry.input).exists();
            let mark = if exists { "✔" } else { "✘" };
            println!(
                "  {}. [{}] {} (format={}, quality={})",
                i + 1,
                mark,
                entry.input,
                entry.format,
                entry.pdf_quality,
            );
            println!(
                "     → {} (title={}, {}枚/page)",
                entry.output, entry.title, entry.photos_per_page,
            );
        }
        return Ok(());
    }

    println!("🔄 全履歴を再エクスポート ({}件)\n", entries.len());
    let mut success = 0usize;
    let mut skipped = 0usize;

    for entry in &entries {
        let input_path = PathBuf::from(&entry.input);
        if !input_path.exists() {
            eprintln!("⚠ スキップ (入力なし): {}", entry.input);
            skipped += 1;
            continue;
        }

        let entry_format = format
            .clone()
            .unwrap_or_else(|| entry.format.parse().unwrap_or_default());
        let entry_quality = pdf_quality
            .unwrap_or_else(|| entry.pdf_quality.parse().unwrap_or_default());

        println!("- {} ...", entry.input);
        let result = handle_export_command(ExportCommandArgs {
            input: input_path,
            format: entry_format,
            output: Some(PathBuf::from(&entry.output)),
            photos_per_page: entry.photos_per_page,
            title: entry.title.clone(),
            pdf_quality: entry_quality,
            preset: entry.preset.clone(),
            alias: entry.alias.as_ref().map(PathBuf::from),
        });

        match result {
            Ok(()) => success += 1,
            Err(e) => {
                eprintln!("⚠ 失敗: {}: {e}", entry.input);
                skipped += 1;
            }
        }
    }

    println!("\n✅ 再エクスポート完了: 成功={}, スキップ={}", success, skipped);
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
        photo_type: args.photo_type,
        variety: args.variety,
        station: args.station,
        use_cache: args.use_cache,
        recursive: args.recursive,
        include_all: args.include_all,
        verbose: args.cli_args.verbose,
        provider: args.cli_args.provider,
        line_types: args.line_types,
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
        photo_type: args.photo_type,
        variety: args.variety,
        station: args.station,
        use_cache: args.use_cache,
        recursive: args.recursive,
        include_all: args.include_all,
        verbose: args.cli_args.verbose,
        provider: args.cli_args.provider,
        line_types: args.line_types,
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
    let title = derive_export_title(&results, &args.folder);
    export::export_results(&results, &args.format, &output_paths.export_path, 3, &title, args.pdf_quality)?;

    // 履歴記録
    let input_abs = std::fs::canonicalize(&json_path)
        .unwrap_or_else(|_| json_path.clone());
    let output_abs = std::fs::canonicalize(&output_paths.export_path)
        .unwrap_or_else(|_| output_paths.export_path.clone());
    if let Err(e) = export_history::record(ExportHistoryEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        input: input_abs.to_string_lossy().to_string(),
        format: args.format.to_string(),
        output: output_abs.to_string_lossy().to_string(),
        photos_per_page: 3,
        title: title.clone(),
        pdf_quality: args.pdf_quality.to_string(),
        preset: None,
        alias: None,
    }) {
        eprintln!("⚠ 履歴記録に失敗: {e}");
    }

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
    let options = NormalizationOptions {
        dekigata_lane: args.lane,
        dekigata_remarks: args.dekigata_remarks,
        ..Default::default()
    };

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

    // 区画線工の線種判定（差分実行）
    let mut line_type_changes = 0;
    if let Some(ref lt) = args.line_types {
        if !lt.is_empty() {
            let line_type_names: Vec<&str> = lt.iter().map(|e| e.name.as_str()).collect();
            for r in &mut results {
                if r.work_type == "区画線工" && !r.file_path.is_empty() {
                    // スキップ: stationが既に線種リストのnameに含まれる場合
                    if !r.station.is_empty() && line_type_names.contains(&r.station.as_str()) {
                        continue;
                    }
                    if let Some(detected) = crate::line_type_detector::detect_line_type(&r.file_path, lt) {
                        println!("  線種判定: {} → {}", r.file_name, detected);
                        r.station = detected;
                        line_type_changes += 1;
                    }
                }
            }
            println!("線種判定: {}件更新", line_type_changes);
        }
    }

    // ドライランでなければ適用
    if !args.dry_run {
        let has_corrections = args.station.is_some() || !result.corrections.is_empty() || line_type_changes > 0;
        if has_corrections {
            normalizer::apply_corrections(&mut results, &result.corrections);
        }

        // 日付ソートは常に適用
        results.sort_by(|a, b| a.date.cmp(&b.date).then(a.file_name.cmp(&b.file_name)));

        let output_path = args.output.unwrap_or(args.input);
        let json = serde_json::to_string_pretty(&results)?;
        std::fs::write(&output_path, json)?;
        println!("\n✔ 保存: {}", output_path.display());
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

/// Collectコマンドを処理
pub fn handle_collect_command(dest: &Path, format: &ExportFormat, dry_run: bool) -> Result<()> {
    let entries = export_history::list();
    if entries.is_empty() {
        println!("履歴がありません");
        return Ok(());
    }

    // 対象拡張子を決定
    let extensions: Vec<&str> = match format {
        ExportFormat::Pdf => vec!["pdf"],
        ExportFormat::Excel => vec!["xlsx"],
        ExportFormat::Both => vec!["pdf", "xlsx"],
        ExportFormat::PhotoXml => vec!["pdf", "xlsx", "xml"],
    };

    // 衝突回避用カウンタ
    let mut name_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut copy_items: Vec<(PathBuf, PathBuf)> = Vec::new();

    for entry in &entries {
        let output_dir = Path::new(&entry.output);
        for ext in &extensions {
            let src = output_dir.join(format!("{}.{}", entry.title, ext));
            if !src.exists() {
                continue;
            }

            let base_name = derive_collect_name(entry, ext);
            let count = name_counts.entry(base_name.clone()).or_insert(0);
            *count += 1;
            let dest_name = if *count == 1 {
                base_name
            } else {
                // 拡張子を分離してサフィックスを挿入
                let stem = Path::new(&base_name).file_stem().unwrap_or_default().to_string_lossy();
                format!("{}_{}.{}", stem, count, ext)
            };
            let dest_path = dest.join(&dest_name);
            copy_items.push((src, dest_path));
        }
    }

    if copy_items.is_empty() {
        println!("コピー対象のファイルがありません");
        return Ok(());
    }

    if dry_run {
        println!("📋 収集予定 ({}件)\n", copy_items.len());
        for (src, dst) in &copy_items {
            println!("  {} → {}", src.display(), dst.file_name().unwrap_or_default().to_string_lossy());
        }
        return Ok(());
    }

    // dest ディレクトリを作成
    std::fs::create_dir_all(dest)?;

    let mut success = 0usize;
    let mut failed = 0usize;
    for (src, dst) in &copy_items {
        match std::fs::copy(src, dst) {
            Ok(_) => {
                println!("  ✔ {}", dst.file_name().unwrap_or_default().to_string_lossy());
                success += 1;
            }
            Err(e) => {
                eprintln!("  ✘ {} : {}", src.display(), e);
                failed += 1;
            }
        }
    }

    println!("\n✅ 収集完了: 成功={}, 失敗={}", success, failed);
    Ok(())
}

/// エクスポート履歴エントリからコピー先ファイル名を導出
///
/// entry.output のパスから末尾2コンポーネントを取得し "_" で結合
/// 例: `.../0209切削/施工状況` → `0209切削_施工状況.pdf`
fn derive_collect_name(entry: &ExportHistoryEntry, ext: &str) -> String {
    let output_path = Path::new(&entry.output);
    let components: Vec<&str> = output_path
        .components()
        .filter_map(|c| {
            match c {
                std::path::Component::Normal(s) => s.to_str(),
                _ => None,
            }
        })
        .collect();

    let prefix = if components.len() >= 2 {
        format!("{}_{}", components[components.len() - 2], components[components.len() - 1])
    } else if components.len() == 1 {
        components[0].to_string()
    } else {
        "unknown".to_string()
    };

    format!("{}.{}", prefix, ext)
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

/// フォルダ名からエクスポートタイトルを生成
fn derive_export_title(_results: &[analyzer::AnalysisResult], folder: &Path) -> String {
    folder.file_name()
        .and_then(|n| n.to_str())
        .map(|n| format!("写真帳_{}", n))
        .unwrap_or_else(|| "工事写真帳".to_string())
}

impl Commands {
    /// コマンドを実行する
    pub async fn execute(self, cli_args: &CommonCliArgs, config: Config) -> Result<()> {
        match self {
            Commands::Analyze { folder, output, batch_size, master, work_type, photo_type, variety, station, use_cache, recursive, include_all, line_types } => {
                let line_types = load_line_types(line_types.as_ref())?;
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
                    cli_args: cli_args.clone(),
                }).await?;
            }

            Commands::Export { input, format, output, photos_per_page, title, pdf_quality, preset, alias } => {
                handle_export_command(ExportCommandArgs {
                    input,
                    format,
                    output,
                    photos_per_page,
                    title,
                    pdf_quality,
                    preset,
                    alias,
                })?;
            }

            Commands::Run { folder, output, format, batch_size, master, work_type, photo_type, variety, station, pdf_quality, use_cache, recursive, include_all, line_types } => {
                let line_types = load_line_types(line_types.as_ref())?;
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

            Commands::ReExportAll { format, pdf_quality, dry_run } => {
                handle_re_export_all_command(format, pdf_quality, dry_run)?;
            }

            Commands::Review { path, watch, model } => {
                handle_review_command(ReviewCommandArgs {
                    path,
                    watch,
                    model,
                    cli_args: cli_args.clone(),
                })?;
            }

            Commands::Collect { dest, format, dry_run } => {
                handle_collect_command(&dest, &format, dry_run)?;
            }
        }

        Ok(())
    }
}
