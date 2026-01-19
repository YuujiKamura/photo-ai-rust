use clap::Parser;
use photo_ai_rust::{ai_provider::AiProvider, cli, config, error, scanner, analyzer, matcher, export, station, master_selector};
use cli::{Cli, Commands};
use config::Config;
use error::Result;
use photo_ai_common::HierarchyMaster;
use std::path::{Path, PathBuf};

/// AI解析を実行（1ステップ解析優先）
async fn run_analysis(
    images: &[scanner::ImageInfo],
    folder: &Path,
    batch_size: usize,
    verbose: bool,
    master: Option<&Path>,
    use_cache: bool,
    provider: AiProvider,
    work_type: Option<&str>,
    variety: Option<&str>,
    _station: Option<&str>,
    step_prefix: &str,
) -> Result<Vec<analyzer::AnalysisResult>> {
    // 工種指定時は1ステップ解析（推奨）
    if let Some(wt) = work_type {
        // マスタパスを決定
        let master_path_buf: PathBuf = if let Some(mp) = master {
            mp.to_path_buf()
        } else {
            // 工種別マスタを自動選択
            let by_work_type = PathBuf::from("master/by_work_type").join(format!("{}.csv", wt));
            if by_work_type.exists() {
                by_work_type
            } else {
                // デフォルトマスタ
                let default = PathBuf::from("master/construction_hierarchy.csv");
                if default.exists() {
                    default
                } else {
                    return Err(error::PhotoAiError::MasterLoad("マスタファイルが見つかりません".to_string()));
                }
            }
        };

        println!("{} 1ステップ解析中 (工種: {})...", step_prefix, wt);
        let hierarchy = HierarchyMaster::from_csv(&master_path_buf)
            .map_err(|e| error::PhotoAiError::MasterLoad(e.to_string()))?;

        // 指定工種でマスタをフィルタ
        let filtered = hierarchy.filter_by_work_types(&[wt.to_string()]);
        println!("  マスタ読み込み: {}件 (工種: {})", filtered.rows().len(), wt);

        return analyzer::analyze_images_single_step(
            images,
            &filtered,
            wt,
            variety,
            batch_size,
            verbose,
            provider,
        ).await;
    }

    // 工種未指定の場合はキャッシュまたは基本解析
    // ※2ステップ解析は廃止（API消費が多いため）
    if use_cache {
        println!("{} AI解析中... (キャッシュ有効)", step_prefix);
        println!("  ⚠ 工種未指定: --work-type で指定すると精度向上");
        analyzer::analyze_images_with_cache(images, folder, batch_size, verbose, provider).await
    } else {
        println!("{} AI解析中...", step_prefix);
        println!("  ⚠ 工種未指定: --work-type で指定すると精度向上");
        analyzer::analyze_images(images, batch_size, verbose, provider).await
    }
}

/// 測点を一括適用
fn apply_station(results: &mut [analyzer::AnalysisResult], station: &str) {
    for result in results {
        result.station = station.to_string();
    }
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load()?;

    match cli.command {
        Commands::Analyze { folder, output, batch_size, master, work_type, variety, station, use_cache, recursive, include_all } => {
            println!("📸 photo-ai-rust - 写真解析\n");

            // マスタ選択（対話式または引数から）
            let has_master_arg = master.is_some();
            let selection = resolve_master_path(master, !has_master_arg && work_type.is_none());

            // work_type: CLI引数優先、なければ選択結果から
            let effective_work_type = work_type.or_else(|| selection.as_ref().and_then(|s| s.work_type.clone()));
            let master_path = selection.map(|s| s.path);
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

            // 1. 画像スキャン
            println!("[1/3] 写真をスキャン中...{}", if recursive { " (再帰)" } else { "" });
            let images = scanner::scan_folder_full(&folder, recursive, !include_all)?;
            println!("✔ {}枚の写真を検出\n", images.len());

            if images.is_empty() {
                return Err(error::PhotoAiError::NoImagesFound(
                    folder.display().to_string()
                ));
            }

            // 2. AI解析（1ステップ解析）
            let mut results = run_analysis(
                &images,
                &folder,
                batch_size,
                cli.verbose,
                master_path.as_deref(),
                use_cache,
                cli.ai_provider,
                effective_work_type.as_deref(),
                variety.as_deref(),
                station.as_deref(),
                "[2/3]",
            ).await?;
            println!("✔ 解析完了\n");

            // 測点一括適用
            if let Some(ref st) = station {
                println!("  測点を一括適用: {}", st);
                apply_station(&mut results, st);
            }

            // 3. 結果保存
            println!("[3/3] 結果を保存中...");
            let output_path = output.unwrap_or_else(|| folder.join("result.json"));
            let json = serde_json::to_string_pretty(&results)?;
            std::fs::write(&output_path, json)?;
            println!("✔ 結果を保存: {}", output_path.display());

            println!("\n✅ 解析完了");
        }

        Commands::Export { input, format, output, photos_per_page, title, pdf_quality, preset, alias } => {
            println!("📄 photo-ai-rust - エクスポート\n");

            let content = std::fs::read_to_string(&input)?;
            let mut results: Vec<analyzer::AnalysisResult> = serde_json::from_str(&content)?;

            // JSONファイルの親ディレクトリを基準に相対パスを解決
            let base_dir = input.parent().unwrap_or(std::path::Path::new("."));
            for result in &mut results {
                if !result.file_path.is_empty() {
                    let path = std::path::Path::new(&result.file_path);
                    if path.is_relative() {
                        if let Ok(abs_path) = base_dir.join(path).canonicalize() {
                            result.file_path = abs_path.to_string_lossy().to_string();
                        }
                    }
                }
            }

            // エイリアス変換を適用
            if preset.is_some() || alias.is_some() {
                println!("- エイリアス変換中...");
                results = matcher::apply_aliases(
                    &results,
                    preset.as_deref(),
                    alias.as_deref(),
                )?;
                println!("✔ エイリアス変換完了");
            }

            let output_dir = output.unwrap_or_else(|| std::path::PathBuf::from("."));

            export::export_results(&results, &format, &output_dir, photos_per_page, &title, pdf_quality)?;

            println!("\n✅ エクスポート完了");
        }

        Commands::Run { folder, output, format, batch_size, master, work_type, variety, station, pdf_quality, use_cache, recursive, include_all } => {
            println!("🚀 photo-ai-rust - 一括処理\n");

            // マスタ選択（対話式または引数から）
            let has_master_arg = master.is_some();
            let selection = resolve_master_path(master, !has_master_arg && work_type.is_none());

            // work_type: CLI引数優先、なければ選択結果から
            let effective_work_type = work_type.or_else(|| selection.as_ref().and_then(|s| s.work_type.clone()));
            let master_path = selection.map(|s| s.path);
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

            // 1. Scan
            println!("[1/4] 写真をスキャン中...{}", if recursive { " (再帰)" } else { "" });
            let images = scanner::scan_folder_full(&folder, recursive, !include_all)?;
            println!("✔ {}枚の写真を検出\n", images.len());

            if images.is_empty() {
                return Err(error::PhotoAiError::NoImagesFound(
                    folder.display().to_string()
                ));
            }

            // 2. AI解析（1ステップ解析）
            let mut results = run_analysis(
                &images,
                &folder,
                batch_size,
                cli.verbose,
                master_path.as_deref(),
                use_cache,
                cli.ai_provider,
                effective_work_type.as_deref(),
                variety.as_deref(),
                station.as_deref(),
                "[2/4]",
            ).await?;
            println!("✔ 解析完了\n");

            // 測点一括適用
            if let Some(ref st) = station {
                println!("  測点を一括適用: {}", st);
                apply_station(&mut results, st);
            }

            // 3. 結果保存
            let output_dir = output.unwrap_or_else(|| folder.clone());
            println!("[3/4] 結果を保存中...");
            let json_path = output_dir.join("result.json");
            let json = serde_json::to_string_pretty(&results)?;
            std::fs::write(&json_path, &json)?;
            println!("✔ 結果を保存: {}", json_path.display());

            // 4. Export
            println!("[4/4] エクスポート中...");
            export::export_results(&results, &format, &output_dir, 3, "工事写真帳", pdf_quality)?;

            println!("\n✅ 完了");
        }

        Commands::Config { set_api_key, show } => {
            let mut config = config;

            if let Some(key) = set_api_key {
                config.set_api_key(key)?;
                println!("✔ APIキーを設定しました");
            }

            if show {
                println!("設定:");
                println!("  モデル: {}", config.model);
                println!("  最大画像サイズ: {}px", config.max_image_size);
                println!("  バッチサイズ: {}", config.default_batch_size);
                println!("  APIキー: {}", if config.api_key.is_some() { "設定済み" } else { "未設定" });
            }
        }

        Commands::Station { input, output } => {
            println!("📍 photo-ai-rust - 測点入力\n");
            station::run_interactive_station(&input, output.as_deref())?;
        }

        Commands::Cache { clear, folder, info } => {
            let target = folder.unwrap_or_else(|| std::path::PathBuf::from("."));
            let cache_path = analyzer::CacheFile::cache_path(&target);

            if info || !clear {
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

            if clear {
                match analyzer::CacheFile::clear(&target) {
                    Ok(true) => println!("✔ キャッシュを削除しました: {}", cache_path.display()),
                    Ok(false) => println!("キャッシュファイルが存在しません"),
                    Err(e) => println!("キャッシュ削除エラー: {}", e),
                }
            }
        }

        Commands::Normalize { input, output, dry_run } => {
            use photo_ai_rust::normalizer::{self, NormalizationOptions};

            println!("🔧 photo-ai-rust - 正規化\n");

            // JSONを読み込み
            let content = std::fs::read_to_string(&input)?;
            let mut results: Vec<analyzer::AnalysisResult> = serde_json::from_str(&content)?;
            println!("読み込み: {}件", results.len());

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
            if !dry_run && !result.corrections.is_empty() {
                normalizer::apply_corrections(&mut results, &result.corrections);

                let output_path = output.unwrap_or(input);
                let json = serde_json::to_string_pretty(&results)?;
                std::fs::write(&output_path, json)?;
                println!("\n✔ 保存: {}", output_path.display());
            } else if dry_run {
                println!("\n[ドライラン] 変更は適用されませんでした");
            }

            println!("\n✅ 正規化完了");
        }
    }

    Ok(())
}
