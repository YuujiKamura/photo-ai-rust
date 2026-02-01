use clap::Parser;
use photo_ai_rust::{cli, config, error, station};
use photo_ai_rust::commands::{
    AnalyzeCommandArgs, handle_analyze_command,
    RunCommandArgs, handle_run_command,
    ExportCommandArgs, handle_export_command,
    ReviewCommandArgs, handle_review_command,
    NormalizeCommandArgs, handle_normalize_command,
    ConfigCommandArgs, handle_config_command,
    CacheCommandArgs, handle_cache_command,
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
            handle_config_command(ConfigCommandArgs {
                set_api_key,
                show,
                config,
            })?;
        }

        Commands::Station { input, output } => {
            println!("📍 photo-ai-rust - 測点入力\n");
            station::run_interactive_station(&input, output.as_deref())?;
        }

        Commands::Cache { clear, folder, info } => {
            handle_cache_command(CacheCommandArgs {
                clear,
                folder,
                info,
            });
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
