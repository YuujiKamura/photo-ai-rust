use std::path::{Path};
use clap::Parser;
use photo_ai_common::export::excel::generate_excel_buffer;
use photo_ai_common::export::ImageData;
use photo_ai_common::AnalysisResult;
use serde::Serialize;
use std::io::Write;

#[derive(Parser)]
struct Args {
    /// 入力JSONパス
    #[arg(short, long)]
    input: String,

    /// 出力Excelパス
    #[arg(short, long)]
    output: String,

    /// ページあたりの写真数 (2 or 3)
    #[arg(short, long, default_value_t = 3)]
    photos_per_page: u8,

    /// タイトル
    #[arg(short, long, default_value = "写真台帳")]
    title: String,
}

#[derive(Serialize)]
struct ExcelResult {
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

fn run(args: Args) -> anyhow::Result<ExcelResult> {
    // 1. JSON読み込み
    let json_data = std::fs::read_to_string(&args.input)?;
    let photos: Vec<AnalysisResult> = serde_json::from_str(&json_data)?;

    // 2. 画像ローダーの定義
    let image_loader = |path: &str| -> Option<ImageData> {
        let p = Path::new(path);
        if !p.exists() {
            return None;
        }
        let data = std::fs::read(p).ok()?;
        let ext = p.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        Some(ImageData {
            data,
            extension: ext,
        })
    };

    // 3. Excelバッファ生成
    let buffer = generate_excel_buffer(&photos, args.photos_per_page, image_loader)
        .map_err(|e| anyhow::anyhow!("Excel生成エラー: {}", e))?;

    // 4. ファイル保存
    let mut file = std::fs::File::create(&args.output)?;
    file.write_all(&buffer)?;

    Ok(ExcelResult {
        output_path: args.output,
        count: photos.len(),
    })
}
