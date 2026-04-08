use std::path::PathBuf;
use clap::Parser;
use photo_ai_rust::export::{pdf, pair_pdf};
use photo_ai_rust::cli::PdfQuality;
use photo_ai_common::AnalysisResult;
use serde::Serialize;

#[derive(Parser)]
struct Args {
    /// 入力JSONパス
    #[arg(short, long)]
    input: String,

    /// 出力PDFパス
    #[arg(short, long)]
    output: String,

    /// ページあたりの写真数 (2 or 3)
    #[arg(short, long, default_value_t = 3)]
    photos_per_page: u8,

    /// 画質 (high/medium/low)
    #[arg(short, long, default_value = "medium")]
    quality: String,

    /// 動作モード: "photo-pdf" (デフォルト) or "pair-pdf"
    #[arg(long, default_value = "photo-pdf")]
    mode: String,

    /// pair-pdfモード用プロジェクト名
    #[arg(long, default_value = "")]
    project_name: String,
}

#[derive(Serialize)]
struct PdfResult {
    pub output_path: String,
    pub count: usize,
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

fn run(args: Args) -> anyhow::Result<PdfResult> {
    let output_path = PathBuf::from(&args.output);
    let json_data = std::fs::read_to_string(&args.input)?;

    if args.mode == "pair-pdf" {
        let pairs: Vec<pair_pdf::PairEntry> = serde_json::from_str(&json_data)?;
        let count = pairs.len();
        pair_pdf::generate_pair_pdf(&pairs, &args.project_name, &output_path)?;
        return Ok(PdfResult { output_path: args.output, count });
    }

    // デフォルト: photo-pdf
    let photos: Vec<AnalysisResult> = serde_json::from_str(&json_data)?;
    let quality = match args.quality.as_str() {
        "high" => PdfQuality::High,
        "low" => PdfQuality::Low,
        _ => PdfQuality::Medium,
    };
    let count = photos.len();
    pdf::generate_pdf(&photos, &output_path, args.photos_per_page, "写真台帳", quality)?;
    Ok(PdfResult { output_path: args.output, count })
}
