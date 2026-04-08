use anyhow::Result;
use clap::{Parser, Subcommand};
use photo_engine::analysis::{self, UsageMode};
use photo_engine::types::{to_json_string, EngineResponse};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "photo-analysis-engine")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    TagGroups {
        #[arg(short, long)]
        folder: PathBuf,
        #[arg(short, long, default_value_t = 10)]
        batch_size: usize,
        #[arg(short, long)]
        vocabulary: Option<String>,
        #[arg(short, long, default_value = "time_based_quota")]
        usage_mode: String,
    },
    Step1 {
        #[arg(short, long)]
        folder: PathBuf,
    },
    SingleStep {
        #[arg(short, long)]
        folder: PathBuf,
        #[arg(short, long)]
        master: PathBuf,
        #[arg(long)]
        work_type: Option<String>,
        #[arg(long)]
        variety: Option<String>,
        #[arg(long)]
        photo_type: Option<String>,
    },
    PairEnsemble {
        #[arg(long)]
        before_sheet: PathBuf,
        #[arg(long)]
        after_sheet: PathBuf,
        #[arg(long)]
        query: String,
        #[arg(long)]
        before_max: u32,
        #[arg(long)]
        after_max: u32,
    },
}

fn main() {
    let cli = Cli::parse();
    let (output, ok) = match run(cli) {
        Ok(json) => (json, true),
        Err(err) => {
            let resp: EngineResponse<()> = EngineResponse::failure(err.to_string());
            (to_json_string(&resp), false)
        }
    };

    println!("{output}");
    if !ok {
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<String> {
    match cli.command {
        Command::TagGroups {
            folder,
            batch_size,
            vocabulary,
            usage_mode,
        } => {
            let vocab = vocabulary.map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            });
            let usage_mode = match usage_mode.as_str() {
                "pay_per_use" => UsageMode::PayPerUse,
                "resident" => UsageMode::Resident,
                _ => UsageMode::TimeBasedQuota,
            };
            emit(analysis::tag_groups(
                &folder,
                batch_size,
                vocab.as_deref(),
                usage_mode,
            )?)
        }
        Command::Step1 { folder } => emit(analysis::analyze_step1(&folder)?),
        Command::SingleStep {
            folder,
            master,
            work_type,
            variety,
            photo_type,
        } => emit(analysis::analyze_single_step(
            &folder,
            &master,
            work_type.as_deref(),
            variety.as_deref(),
            photo_type.as_deref(),
        )?),
        Command::PairEnsemble {
            before_sheet,
            after_sheet,
            query,
            before_max,
            after_max,
        } => emit(analysis::pair_ensemble(
            &before_sheet,
            &after_sheet,
            &query,
            before_max,
            after_max,
        )?),
    }
}

fn emit<T: Serialize>(data: T) -> Result<String> {
    let resp = EngineResponse::success(data);
    Ok(to_json_string(&resp))
}
