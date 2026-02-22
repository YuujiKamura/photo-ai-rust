use clap::Parser;
use photo_ai_rust::{cli, config, error};
use photo_ai_rust::commands::CommonCliArgs;
use cli::Cli;
use config::Config;
use error::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load()?;
    let common_cli_args = CommonCliArgs {
        verbose: cli.verbose,
    };
    cli.command.execute(&common_cli_args, config).await
}
