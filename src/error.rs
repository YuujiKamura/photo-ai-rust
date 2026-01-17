use thiserror::Error;

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum PhotoAiError {
    #[error("設定エラー: {0}")]
    Config(String),

    #[error("APIキーが設定されていません。`photo-ai config --set-api-key YOUR_KEY` で設定してください")]
    MissingApiKey,

    #[error("ファイルが見つかりません: {0}")]
    FileNotFound(String),

    #[error("フォルダが見つかりません: {0}")]
    FolderNotFound(String),

    #[error("画像読み込みエラー: {0}")]
    ImageLoad(String),

    #[error("API呼び出しエラー: {0}")]
    ApiCall(String),

    #[error("APIレスポンスのパースに失敗: {0}")]
    ApiParse(String),

    #[error("JSON解析エラー: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("IOエラー: {0}")]
    Io(#[from] std::io::Error),

    #[error("PDF生成エラー: {0}")]
    PdfGeneration(String),

    #[error("Excel生成エラー: {0}")]
    ExcelGeneration(String),

    #[error("マスタファイルが不正: {0}")]
    InvalidMaster(String),

    #[error("画像が見つかりません: {0}")]
    NoImagesFound(String),

    #[error("CLI実行エラー: {0}")]
    CliExecution(String),
}

pub type Result<T> = std::result::Result<T, PhotoAiError>;
