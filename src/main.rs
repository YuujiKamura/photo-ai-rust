use clap::Parser;
use photo_ai_rust::{cli, config, error, scanner, analyzer, matcher, export, station};
use cli::{Cli, Commands};
use config::Config;
use error::Result;
use photo_ai_common::HierarchyMaster;
use std::path::Path;

/// AI解析を実行（マスタ有無・キャッシュ有無で分岐）
async fn run_analysis(
    images: &[scanner::ImageInfo],
    folder: &Path,
    batch_size: usize,
    verbose: bool,
    master: Option<&Path>,
    use_cache: bool,
    step_prefix: &str,
) -> Result<Vec<analyzer::AnalysisResult>> {
    if let Some(master_path) = master {
        println!("{} 2段階解析中 (Step1: 画像認識 → Step2: マスタ照合)...", step_prefix);
        let hierarchy = HierarchyMaster::from_csv(master_path)
            .map_err(|e| error::PhotoAiError::MasterLoad(e.to_string()))?;
        println!("  マスタ読み込み: {}件", hierarchy.rows().len());
        analyzer::analyze_images_with_master(images, &hierarchy, batch_size, verbose).await
    } else if use_cache {
        println!("{} AI解析中... (キャッシュ有効)", step_prefix);
        analyzer::analyze_images_with_cache(images, folder, batch_size, verbose).await
    } else {
        println!("{} AI解析中...", step_prefix);
        analyzer::analyze_images(images, batch_size, verbose).await
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load()?;

    match cli.command {
        Commands::Analyze { folder, output, batch_size, master, use_cache } => {
            println!("📸 photo-ai-rust - 写真解析\n");

            // 1. 画像スキャン
            println!("[1/3] 写真をスキャン中...");
            let images = scanner::scan_folder(&folder)?;
            println!("✔ {}枚の写真を検出\n", images.len());

            if images.is_empty() {
                return Err(error::PhotoAiError::NoImagesFound(
                    folder.display().to_string()
                ));
            }

            // 2. AI解析（マスタがある場合は2段階解析）
            let results = run_analysis(
                &images,
                &folder,
                batch_size,
                cli.verbose,
                master.as_deref(),
                use_cache,
                "[2/3]",
            ).await?;
            println!("✔ 解析完了\n");

            // 3. 結果保存
            println!("[3/3] 結果を保存中...");
            let json = serde_json::to_string_pretty(&results)?;
            std::fs::write(&output, json)?;
            println!("✔ 結果を保存: {}", output.display());

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

        Commands::Run { folder, output, format, batch_size, master, pdf_quality, use_cache } => {
            println!("🚀 photo-ai-rust - 一括処理\n");

            // 1. Scan
            println!("[1/4] 写真をスキャン中...");
            let images = scanner::scan_folder(&folder)?;
            println!("✔ {}枚の写真を検出\n", images.len());

            if images.is_empty() {
                return Err(error::PhotoAiError::NoImagesFound(
                    folder.display().to_string()
                ));
            }

            // 2. AI解析（マスタがある場合は2段階解析）
            let results = run_analysis(
                &images,
                &folder,
                batch_size,
                cli.verbose,
                master.as_deref(),
                use_cache,
                "[2/4]",
            ).await?;
            println!("✔ 解析完了\n");

            // 3. Export
            println!("[3/4] エクスポート中...");
            let output_dir = output.unwrap_or_else(|| folder.clone());
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
    }

    Ok(())
}
