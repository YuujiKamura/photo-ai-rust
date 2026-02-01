use clap::Parser;
use photo_ai_rust::{cli, config, error, analyzer, station};
use photo_ai_rust::commands::{
    AnalyzeCommandArgs, handle_analyze_command,
    RunCommandArgs, handle_run_command,
    ExportCommandArgs, handle_export_command,
    ReviewCommandArgs, handle_review_command,
    NormalizeCommandArgs, handle_normalize_command,
};
use cli::{Cli, Commands};
use config::Config;
use error::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load()?;

    match cli.command {
        Commands::Analyze { folder, output, batch_size, master, work_type, variety, station, use_cache, recursive, include_all } => {
            handle_analyze_command(AnalyzeCommandArgs {
                folder,
                output,
                batch_size,
                master,
                work_type,
                variety,
                station,
                use_cache,
                recursive,
                include_all,
                verbose: cli.verbose,
                provider: cli.ai_provider,
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

        Commands::Run { folder, output, format, batch_size, master, work_type, variety, station, pdf_quality, use_cache, recursive, include_all } => {
            handle_run_command(RunCommandArgs {
                folder,
                output,
                format,
                batch_size,
                master,
                work_type,
                variety,
                station,
                pdf_quality,
                use_cache,
                recursive,
                include_all,
                verbose: cli.verbose,
                provider: cli.ai_provider,
            }).await?;
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

        Commands::Normalize { input, output, station, dry_run } => {
            handle_normalize_command(NormalizeCommandArgs {
                input,
                output,
                station,
                dry_run,
            })?;
        }

        Commands::Review { path, watch, model } => {
            handle_review_command(ReviewCommandArgs {
                path,
                watch,
                model,
                provider: cli.ai_provider,
            })?;
        }
    }

    Ok(())
}
