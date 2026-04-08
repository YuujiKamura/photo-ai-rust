use std::path::{PathBuf};
use clap::Parser;
use photo_ai_rust::engine::run_tag_groups;
use photo_ai_rust::grouping::{GroupRecords, UsageMode};
use serde::Serialize;

#[derive(Parser)]
struct Args {
    /// 写真フォルダパス
    #[arg(short, long)]
    folder: String,

    /// バッチサイズ
    #[arg(short, long, default_value_t = 10)]
    batch_size: usize,

    /// 語彙リスト (カンマ区切り)
    #[arg(short, long)]
    vocabulary: Option<String>,

    /// 使用モード (pay_per_use, time_based_quota, or resident)
    #[arg(short, long, default_value = "time_based_quota")]
    usage_mode: String,
}

#[derive(Serialize)]
struct TagResult {
    pub folder: String,
    pub count: usize,
    pub records: GroupRecords,
}

#[derive(Serialize)]
struct EngineResponse<T: Serialize> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn main() {
    let args = Args::parse();
    match run(args) {
        Ok(res) => {
            let resp = EngineResponse { ok: true, data: Some(res), error: None };
            println!("{}", serde_json::to_string(&resp).unwrap());
        }
        Err(e) => {
            let resp: EngineResponse<()> = EngineResponse { ok: false, data: None, error: Some(e.to_string()) };
            eprintln!("{}", serde_json::to_string(&resp).unwrap());
            std::process::exit(1);
        }
    }
}

fn run(args: Args) -> anyhow::Result<TagResult> {
    let folder_path = PathBuf::from(&args.folder);
    if !folder_path.exists() {
        anyhow::bail!("Folder not found: {}", args.folder);
    }

    let vocab: Option<Vec<String>> = args.vocabulary.map(|v| {
        v.split(',').map(|s| s.trim().to_string()).collect()
    });

    let usage_mode = match args.usage_mode.as_str() {
        "pay_per_use" => UsageMode::PayPerUse,
        "resident" => UsageMode::Resident,
        _ => UsageMode::TimeBasedQuota,
    };

    let vocab_ref = vocab.as_ref().map(|v| v.as_slice());

    let records = run_tag_groups(&folder_path, args.batch_size, vocab_ref, usage_mode)?;

    Ok(TagResult {
        folder: args.folder,
        count: records.len(),
        records,
    })
}
