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
    #[command(after_help = "例:\n  photo-ai-rust analyze ./photos\n  photo-ai-rust analyze ./photos -m master/by_work_type/舗装工.csv\n  photo-ai-rust analyze ./photos -w 舗装工 -t 施工状況写真")]
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

        /// 工種マスタCSVファイル（複数指定可）
        #[arg(short, long)]
        master: Vec<PathBuf>,

        /// 工種を指定（1ステップ解析モード）
        #[arg(short = 'w', long)]
        work_type: Option<String>,

        /// 写真種類を指定（使用機械、安全管理写真など）
        #[arg(short = 't', long)]
        photo_type: Option<String>,

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

        /// 区画線工の線種リストJSONファイル
        #[arg(long)]
        line_types: Option<PathBuf>,

        /// フォルダルールJSONファイル
        #[arg(long)]
        folder_rules: Option<PathBuf>,

        /// API key従量課金モード（GEMINI_API_KEY環境変数が必要）
        #[arg(long)]
        pay_per_use: bool,
    },

    /// 解析結果からPDF/Excelを生成
    #[command(after_help = "例:\n  photo-ai-rust export result.json --format pdf\n  photo-ai-rust export result.json --format both -o ./output/")]
    Export {
        /// 入力JSONファイル
        #[arg(required = true)]
        input: PathBuf,

        /// 出力形式 (pdf/excel/xml/both)
        #[arg(short, long, default_value = "both")]
        format: ExportFormat,

        /// 出力ファイル/ディレクトリ
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// ページあたりの写真数 (2/3)
        #[arg(short, long, default_value = "3")]
        photos_per_page: u8,

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
    #[command(after_help = "例:\n  photo-ai-rust run ./photos --format pdf\n  photo-ai-rust run ./photos -m master/by_work_type/舗装工.csv --format both\n  photo-ai-rust run ./photos --use-cache --format pdf")]
    Run {
        /// 写真フォルダのパス
        #[arg(required = true)]
        folder: PathBuf,

        /// 出力ファイル
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// 出力形式 (pdf/excel/xml/both)
        #[arg(short, long, default_value = "pdf")]
        format: ExportFormat,

        /// バッチサイズ
        #[arg(short, long, default_value = "5")]
        batch_size: usize,

        /// 工種マスタCSVファイル（複数指定可）
        #[arg(short, long)]
        master: Vec<PathBuf>,

        /// 工種を指定（1ステップ解析モード）
        #[arg(short = 'w', long)]
        work_type: Option<String>,

        /// 写真種類を指定（使用機械、安全管理写真など）
        #[arg(short = 't', long)]
        photo_type: Option<String>,

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

        /// 区画線工の線種リストJSONファイル
        #[arg(long)]
        line_types: Option<PathBuf>,

        /// フォルダルールJSONファイル
        #[arg(long)]
        folder_rules: Option<PathBuf>,

        /// API key従量課金モード（GEMINI_API_KEY環境変数が必要）
        #[arg(long)]
        pay_per_use: bool,
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

    /// 解析結果を正規化（グループ単位で計測値を統一）
    Normalize {
        /// 入力JSONファイル
        #[arg(required = true)]
        input: PathBuf,

        /// 出力ファイル（省略時は上書き）
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// 測点を一括指定（例: "12月26日"）
        #[arg(short = 'S', long)]
        station: Option<String>,

        /// 出来形管理写真の車線指定 (left/right)
        #[arg(long)]
        lane: Option<LaneArg>,

        /// 出来形管理写真の備考テキスト
        #[arg(long)]
        dekigata_remarks: Option<String>,

        /// ドライラン（変更を適用せずプレビュー）
        #[arg(long)]
        dry_run: bool,

        /// 線種リストJSON（区画線工の線種を差分判定）
        #[arg(long, value_name = "FILE")]
        line_types: Option<PathBuf>,
    },

    /// ソースコードをAIでレビュー
    Review {
        /// レビュー対象のファイルまたはフォルダ
        #[arg(required = true)]
        path: PathBuf,

        /// ファイル監視モード（変更時に自動レビュー）
        #[arg(short, long)]
        watch: bool,

        /// AIモデル指定（プロバイダに依存）
        #[arg(short, long)]
        model: Option<String>,

        /// レビューバックエンド (gemini/claude/codex)
        #[arg(long, default_value = "gemini")]
        backend: ReviewBackendArg,
    },

    /// 解析結果を評価（GT比較・精度検証）
    Evaluate {
        /// パイプライン出力JSONファイル
        #[arg(required = true)]
        input: PathBuf,

        /// GTファイル（JSON）
        #[arg(long, required = true)]
        gt: PathBuf,

        /// 評価フィールド（カンマ区切り: remarks,station,measurements）
        #[arg(long)]
        fields: Option<String>,

        /// JSON形式で出力（CI連携用）
        #[arg(long)]
        json: bool,
    },

    /// 着手前写真と竣工写真をAIで自動ペアリング（コンタクトシート+アンサンブル方式）
    #[command(name = "pair-completion")]
    PairCompletion {
        /// 着手前写真PDF or 画像フォルダ
        #[arg(long, required = true)]
        before: PathBuf,

        /// 竣工写真フォルダ
        #[arg(long, required = true)]
        after: PathBuf,

        /// 出力JSONファイル（デフォルト: pairing.json）
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// 工事名（フォルダ作成・PDF生成に使用）
        #[arg(long)]
        project_name: Option<String>,

        /// ペアリング後にフォルダ作成+PDF生成まで実行
        #[arg(long)]
        build: bool,
    },

    /// 着手前竣工写真帳PDFを生成
    #[command(name = "pair-pdf")]
    PairPdf {
        /// 画像フォルダ（単独: Pフォルダスキャン、--json併用: 画像パス解決）
        folder: Option<PathBuf>,

        /// ペアリングJSON（PairEntry形式 or pairing_manual形式）
        #[arg(long)]
        json: Option<PathBuf>,

        /// 工事名
        #[arg(long, required = true)]
        project_name: String,

        /// 出力先ディレクトリ（デフォルト: フォルダの親/写真帳まとめ/）
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Pフォルダ内のafter写真を差し替えてPDFを再生成
    #[command(name = "pair-replace")]
    PairReplace {
        /// Pフォルダの親（竣工写真フォルダ）
        #[arg(required = true)]
        folder: PathBuf,

        /// 差し替えるペア番号（例: 1-3, 1,3,5, 2）
        #[arg(long, required = true)]
        pairs: String,

        /// 新しい竣工写真フォルダ（ソート順でpairsに対応）
        #[arg(long, required = true)]
        new_after: PathBuf,

        /// 工事名（PDF生成に必要）
        #[arg(long, required = true)]
        project_name: String,

        /// PDF出力先（デフォルト: folder親/写真帳まとめ/）
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// 前提条件を一括チェック
    Doctor,

    /// Ground Truth管理（取り込み・比較）
    Gt {
        #[command(subcommand)]
        action: GtAction,
    },
}

#[derive(Subcommand)]
pub enum GtAction {
    /// PDFまたはJSONからGTを取り込む
    Import {
        /// 入力ファイル（PDF or JSON）
        #[arg(required = true)]
        source: PathBuf,
        /// GT保存先ディレクトリ
        #[arg(short, long, default_value = "tests/ground_truth/accuracy_eval")]
        output_dir: PathBuf,
        /// フォルダ名（GT JSONのファイル名。省略時はソースのステム）
        #[arg(short, long)]
        name: Option<String>,
    },
    /// パイプライン出力とGTを比較
    Compare {
        /// GTディレクトリ
        #[arg(long, default_value = "tests/ground_truth/accuracy_eval")]
        gt_dir: PathBuf,
        /// パイプライン出力ディレクトリ
        #[arg(long, default_value = "tests/ground_truth/pipeline_output")]
        pipeline_dir: PathBuf,
    },
}

#[derive(Clone, Debug, Default)]
pub enum ExportFormat {
    Pdf,
    Excel,
    PhotoXml,
    #[default]
    Both,
}

impl std::str::FromStr for ExportFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pdf" => Ok(ExportFormat::Pdf),
            "excel" | "xlsx" => Ok(ExportFormat::Excel),
            "xml" | "photo-xml" | "photo.xml" => Ok(ExportFormat::PhotoXml),
            "both" => Ok(ExportFormat::Both),
            _ => Err(format!("Unknown format: {}. Use pdf, excel, xml, or both", s)),
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

/// 出来形管理写真の車線指定（CLIパース用）
#[derive(Clone, Copy, Debug)]
pub enum LaneArg {
    Left,
    Right,
    Both,
}

impl LaneArg {
    pub fn to_lane(self) -> crate::normalizer::Lane {
        match self {
            LaneArg::Left => crate::normalizer::Lane::Left,
            LaneArg::Right => crate::normalizer::Lane::Right,
            LaneArg::Both => crate::normalizer::Lane::Both,
        }
    }
}

impl std::str::FromStr for LaneArg {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "left" | "l" | "左" => Ok(LaneArg::Left),
            "right" | "r" | "右" => Ok(LaneArg::Right),
            "both" | "b" | "両" => Ok(LaneArg::Both),
            _ => Err(format!("Unknown lane: {}. Use left/right/both", s)),
        }
    }
}

/// レビューバックエンド（CLIパース用）
#[derive(Clone, Debug, Default)]
pub enum ReviewBackendArg {
    #[default]
    Gemini,
    Claude,
    Codex,
}

impl std::str::FromStr for ReviewBackendArg {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "gemini" => Ok(ReviewBackendArg::Gemini),
            "claude" => Ok(ReviewBackendArg::Claude),
            "codex" => Ok(ReviewBackendArg::Codex),
            _ => Err(format!("Unknown backend: {}. Use gemini/claude/codex", s)),
        }
    }
}
