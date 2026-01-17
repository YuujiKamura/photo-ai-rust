use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "photo-ai")]
#[command(about = "工事写真AI解析・写真台帳生成ツール", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// 詳細ログを出力
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 写真フォルダを解析してJSONを出力
    Analyze {
        /// 写真フォルダのパス
        #[arg(required = true)]
        folder: PathBuf,

        /// 出力JSONファイル
        #[arg(short, long, default_value = "result.json")]
        output: PathBuf,

        /// バッチサイズ（一度に解析する枚数）
        #[arg(short, long, default_value = "5")]
        batch_size: usize,

        /// 工種マスタJSONファイル
        #[arg(short, long)]
        master: Option<PathBuf>,
    },

    /// 解析結果からPDF/Excelを生成
    Export {
        /// 入力JSONファイル
        #[arg(required = true)]
        input: PathBuf,

        /// 出力形式 (pdf/excel/both)
        #[arg(short, long, default_value = "both")]
        format: ExportFormat,

        /// 出力ファイル/ディレクトリ
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// ページあたりの写真数 (2/3)
        #[arg(short, long, default_value = "3")]
        photos_per_page: u8,

        /// ドキュメントタイトル
        #[arg(short, long, default_value = "工事写真帳")]
        title: String,
    },

    /// 解析からPDF/Excel出力まで一括実行
    Run {
        /// 写真フォルダのパス
        #[arg(required = true)]
        folder: PathBuf,

        /// 出力ファイル
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// 出力形式 (pdf/excel/both)
        #[arg(short, long, default_value = "pdf")]
        format: ExportFormat,

        /// バッチサイズ
        #[arg(short, long, default_value = "5")]
        batch_size: usize,

        /// 工種マスタJSONファイル
        #[arg(short, long)]
        master: Option<PathBuf>,
    },

    /// 設定を表示/編集
    Config {
        /// APIキーを設定
        #[arg(long)]
        set_api_key: Option<String>,

        /// 設定を表示
        #[arg(long)]
        show: bool,
    },
}

#[derive(Clone, Debug, Default)]
pub enum ExportFormat {
    Pdf,
    Excel,
    #[default]
    Both,
}

impl std::str::FromStr for ExportFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pdf" => Ok(ExportFormat::Pdf),
            "excel" | "xlsx" => Ok(ExportFormat::Excel),
            "both" => Ok(ExportFormat::Both),
            _ => Err(format!("Unknown format: {}. Use pdf, excel, or both", s)),
        }
    }
}
