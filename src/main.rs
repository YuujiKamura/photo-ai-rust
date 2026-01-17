mod cli;
mod config;
mod error;
mod scanner;
mod analyzer;
mod matcher;
mod export;

use clap::Parser;
use cli::{Cli, Commands};
use config::Config;
use error::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load()?;

    match cli.command {
        Commands::Analyze { folder, output, batch_size, master } => {
            println!("📸 photo-ai-rust - 写真解析\n");

            // 1. 画像スキャン
            println!("- 写真をスキャン中...");
            let images = scanner::scan_folder(&folder)?;
            println!("✔ {}枚の写真を検出", images.len());

            if images.is_empty() {
                return Err(error::PhotoAiError::NoImagesFound(
                    folder.display().to_string()
                ));
            }

            // 2. Claude CLI解析
            println!("- AI解析中...");
            let raw_results = analyzer::analyze_images(&images, batch_size, cli.verbose).await?;
            println!("✔ 解析完了");

            // 3. マスタ照合
            if let Some(master_path) = master {
                println!("- マスタ照合中...");
                let _matched = matcher::match_with_master(&raw_results, &master_path)?;
                println!("✔ マスタ照合完了");
            }

            // 4. 結果保存
            println!("- 結果を保存中...");
            let json = serde_json::to_string_pretty(&raw_results)?;
            std::fs::write(&output, json)?;
            println!("✔ 結果を保存: {}", output.display());

            println!("\n✅ 解析完了");
        }

        Commands::Export { input, format, output, photos_per_page, title } => {
            println!("📄 photo-ai-rust - エクスポート\n");

            let content = std::fs::read_to_string(&input)?;
            let results: Vec<analyzer::AnalysisResult> = serde_json::from_str(&content)?;

            let output_dir = output.unwrap_or_else(|| std::path::PathBuf::from("."));

            export::export_results(&results, &format, &output_dir, photos_per_page, &title)?;

            println!("\n✅ エクスポート完了");
        }

        Commands::Run { folder, output, format, batch_size, master } => {
            println!("🚀 photo-ai-rust - 一括処理\n");

            // Analyze
            let images = scanner::scan_folder(&folder)?;
            let raw_results = analyzer::analyze_images(&images, batch_size, cli.verbose).await?;

            // Match with master if provided
            let results = if let Some(master_path) = master {
                matcher::match_with_master(&raw_results, &master_path)?
            } else {
                raw_results
            };

            // Export
            let output_dir = output.unwrap_or_else(|| folder.clone());
            export::export_results(&results, &format, &output_dir, 3, "工事写真帳")?;

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
    }

    Ok(())
}
