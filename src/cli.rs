use clap::{Parser, Subcommand};
use crate::ai_provider::AiProvider;
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

    /// AIプロバイダ (claude/codex)
    #[arg(long, default_value = "claude", global = true)]
    pub ai_provider: AiProvider,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 写真フォルダを解析してJSONを出力
    Analyze {
        /// 写真フォルダのパス
        #[arg(required = true)]
        folder: PathBuf,

        /// 出力JSONファイル（デフォルト: 入力フォルダ/result.json）
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// バッチサイズ（一度に解析する枚数）
        #[arg(short, long, default_value = "5")]
        batch_size: usize,

        /// 工種マスタJSONファイル
        #[arg(short, long)]
        master: Option<PathBuf>,

        /// 工種を指定（1ステップ解析モード）
        #[arg(short = 'w', long)]
        work_type: Option<String>,

        /// 種別を指定
        #[arg(long)]
        variety: Option<String>,

        /// 測点を一括指定
        #[arg(short = 's', long)]
        station: Option<String>,

        /// キャッシュを使用（再解析をスキップ）
        #[arg(long)]
        use_cache: bool,

        /// サブフォルダも再帰的にスキャン
        #[arg(short = 'r', long)]
        recursive: bool,

        /// 除外フォルダも含める（デフォルトは「非使用」等を除外）
        #[arg(long)]
        include_all: bool,
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

        /// 工種を指定（1ステップ解析モード）
        #[arg(short = 'w', long)]
        work_type: Option<String>,

        /// 種別を指定
        #[arg(long)]
        variety: Option<String>,

        /// 測点を一括指定
        #[arg(short = 's', long)]
        station: Option<String>,

        /// PDF画像品質 (high/medium/low)
        #[arg(long, default_value = "medium")]
        pdf_quality: PdfQuality,

        /// キャッシュを使用（再解析をスキップ）
        #[arg(long)]
        use_cache: bool,

        /// サブフォルダも再帰的にスキャン
        #[arg(short = 'r', long)]
        recursive: bool,

        /// 除外フォルダも含める（デフォルトは「非使用」等を除外）
        #[arg(long)]
        include_all: bool,
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

    /// 対話的に測点を入力
    Station {
        /// 解析結果JSONファイル
        #[arg(required = true)]
        input: PathBuf,

        /// 出力先（省略時は上書き）
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// キャッシュ管理
    Cache {
        /// キャッシュを削除
        #[arg(long)]
        clear: bool,

        /// 対象フォルダ（省略時はカレント）
        #[arg(short, long)]
        folder: Option<PathBuf>,

        /// キャッシュ情報を表示
        #[arg(long)]
        info: bool,
    },

    /// 解析結果を正規化（測点・工種の統一）
    Normalize {
        /// 入力JSONファイル
        #[arg(required = true)]
        input: PathBuf,

        /// 出力ファイル（省略時は上書き）
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// ドライラン（変更を適用せずプレビュー）
        #[arg(long)]
        dry_run: bool,

        /// 統一の閾値（0.0-1.0、デフォルト0.6）
        #[arg(long, default_value = "0.6")]
        threshold: f64,

        /// 測点の正規化を無効化
        #[arg(long)]
        no_station: bool,

        /// 工種・種別の統一を無効化
        #[arg(long)]
        no_work_type: bool,

        /// 計測値保護を無効化（温度・寸法を含むレコードも変更対象にする）
        #[arg(long)]
        no_protect_measurements: bool,
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
