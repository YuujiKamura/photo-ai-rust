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

        /// キャッシュを使用（再解析をスキップ）
        #[arg(long)]
        use_cache: bool,
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

        /// PDF画像品質 (high/medium/low)
        #[arg(long, default_value = "medium")]
        pdf_quality: PdfQuality,

        /// エイリアスプリセット (pavement/marking/general)
        #[arg(long)]
        preset: Option<String>,

        /// カスタムエイリアスファイル（JSON）
        #[arg(long)]
        alias: Option<PathBuf>,
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

        /// PDF画像品質 (high/medium/low)
        #[arg(long, default_value = "medium")]
        pdf_quality: PdfQuality,

        /// キャッシュを使用（再解析をスキップ）
        #[arg(long)]
        use_cache: bool,
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

/// PDF画像品質設定
#[derive(Clone, Copy, Debug, Default)]
pub enum PdfQuality {
    /// 高品質: 1400px, 85%
    High,
    /// 中品質: 800px, 75%（デフォルト）
    #[default]
    Medium,
    /// 低品質: 500px, 60%
    Low,
}

impl PdfQuality {
    /// 最大ピクセル幅
    pub fn max_width(&self) -> u32 {
        match self {
            PdfQuality::High => 1400,
            PdfQuality::Medium => 800,
            PdfQuality::Low => 500,
        }
    }

    /// JPEG品質 (0-100)
    pub fn jpeg_quality(&self) -> u8 {
        match self {
            PdfQuality::High => 85,
            PdfQuality::Medium => 75,
            PdfQuality::Low => 60,
        }
    }
}

impl std::str::FromStr for PdfQuality {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "high" | "h" => Ok(PdfQuality::High),
            "medium" | "med" | "m" => Ok(PdfQuality::Medium),
            "low" | "l" => Ok(PdfQuality::Low),
            _ => Err(format!("Unknown quality: {}. Use high, medium, or low", s)),
        }
    }
}

impl std::fmt::Display for PdfQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PdfQuality::High => write!(f, "high"),
            PdfQuality::Medium => write!(f, "medium"),
            PdfQuality::Low => write!(f, "low"),
        }
    }
}
