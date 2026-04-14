//! 視覚解析 Oracle port
//!
//! AI 画像認識（Step1 画像→構造化データ）を抽象化する。
//! 実装は Gemini CLI / Claude CLI / Codex 等のアダプタや、
//! テスト用の MockOracle が来る。

use crate::types::RawImageData;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// Oracle に渡す画像参照
#[derive(Debug, Clone)]
pub struct ImageRef {
    pub file_name: String,
    pub path: PathBuf,
    pub date: Option<String>,
}

impl ImageRef {
    pub fn new(file_name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            file_name: file_name.into(),
            path: path.into(),
            date: None,
        }
    }

    pub fn with_date(mut self, date: impl Into<String>) -> Self {
        self.date = Some(date.into());
        self
    }
}

/// Oracle 呼び出しのエラー
#[derive(Debug, thiserror::Error)]
pub enum OracleError {
    #[error("oracle failed: {0}")]
    Failed(String),

    #[error("oracle unavailable")]
    Unavailable,

    #[error("response parse error: {0}")]
    ResponseParse(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// 視覚解析 Oracle port
///
/// 画像の集合とプロンプトを受け取り、Step1 構造化データ（`RawImageData`）の配列を返す。
/// 順序保証: 返り値の長さが `images` と一致し、同じインデックスが同じ画像に対応する。
#[async_trait]
pub trait VisionOracle: Send + Sync {
    /// 画像バッチを解析する
    async fn analyze_batch(
        &self,
        images: &[ImageRef],
        prompt: &str,
    ) -> Result<Vec<RawImageData>, OracleError>;

    /// Oracle の名前（ログ/メタデータ用）
    fn provider_name(&self) -> &str;
}

// ============================================================================
// MockOracle: テスト専用
// ============================================================================

/// テスト用 Oracle
///
/// ファイル名→ `RawImageData` のマップを事前に登録しておき、呼び出し時に返す。
/// 呼び出し回数と直近プロンプトを記録するので、ハンドオーバ検証にも使える。
pub struct MockOracle {
    responses: HashMap<String, RawImageData>,
    default_response: Option<RawImageData>,
    call_log: Mutex<MockCallLog>,
}

#[derive(Debug, Default)]
pub struct MockCallLog {
    pub call_count: usize,
    pub last_prompt: Option<String>,
    pub last_filenames: Vec<String>,
}

impl MockOracle {
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
            default_response: None,
            call_log: Mutex::new(MockCallLog::default()),
        }
    }

    /// ファイル名に対するレスポンスを登録する
    pub fn with_response(mut self, file_name: impl Into<String>, response: RawImageData) -> Self {
        self.responses.insert(file_name.into(), response);
        self
    }

    /// 未登録ファイル名に対するフォールバックを設定する
    pub fn with_default(mut self, response: RawImageData) -> Self {
        self.default_response = Some(response);
        self
    }

    /// 呼び出しログを取得する（テスト検証用）
    pub fn snapshot_log(&self) -> MockCallLog {
        let guard = self.call_log.lock().expect("mock call log poisoned");
        MockCallLog {
            call_count: guard.call_count,
            last_prompt: guard.last_prompt.clone(),
            last_filenames: guard.last_filenames.clone(),
        }
    }
}

impl Default for MockOracle {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VisionOracle for MockOracle {
    async fn analyze_batch(
        &self,
        images: &[ImageRef],
        prompt: &str,
    ) -> Result<Vec<RawImageData>, OracleError> {
        {
            let mut log = self.call_log.lock().expect("mock call log poisoned");
            log.call_count += 1;
            log.last_prompt = Some(prompt.to_string());
            log.last_filenames = images.iter().map(|i| i.file_name.clone()).collect();
        }

        let mut out = Vec::with_capacity(images.len());
        for img in images {
            if let Some(r) = self.responses.get(&img.file_name) {
                out.push(r.clone());
            } else if let Some(d) = &self.default_response {
                let mut cloned = d.clone();
                cloned.file_name = img.file_name.clone();
                out.push(cloned);
            } else {
                let mut empty = RawImageData::default();
                empty.file_name = img.file_name.clone();
                out.push(empty);
            }
        }
        Ok(out)
    }

    fn provider_name(&self) -> &str {
        "mock"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn make_raw(file: &str, cat: &str) -> RawImageData {
        RawImageData {
            file_name: file.to_string(),
            has_board: true,
            photo_category: cat.to_string(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn mock_returns_registered_response_for_each_image() {
        let oracle = MockOracle::new()
            .with_response("a.jpg", make_raw("a.jpg", "安全管理写真"))
            .with_response("b.jpg", make_raw("b.jpg", "品質管理写真"));

        let imgs = [
            ImageRef::new("a.jpg", "/tmp/a.jpg"),
            ImageRef::new("b.jpg", "/tmp/b.jpg"),
        ];
        let out = oracle.analyze_batch(&imgs, "prompt").await.unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].file_name, "a.jpg");
        assert_eq!(out[0].photo_category, "安全管理写真");
        assert_eq!(out[1].photo_category, "品質管理写真");
    }

    #[tokio::test]
    async fn mock_falls_back_to_default_for_unknown_images() {
        let default = make_raw("_", "施工状況写真");
        let oracle = MockOracle::new().with_default(default);
        let imgs = [ImageRef::new("unknown.jpg", "/tmp/u.jpg")];
        let out = oracle.analyze_batch(&imgs, "prompt").await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].file_name, "unknown.jpg"); // default の file_name は上書きされる
        assert_eq!(out[0].photo_category, "施工状況写真");
    }

    #[tokio::test]
    async fn mock_returns_empty_result_for_unknown_without_default() {
        let oracle = MockOracle::new();
        let imgs = [ImageRef::new("x.jpg", "/tmp/x.jpg")];
        let out = oracle.analyze_batch(&imgs, "").await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].file_name, "x.jpg");
        assert_eq!(out[0].photo_category, "");
    }

    #[tokio::test]
    async fn mock_preserves_order() {
        let oracle = MockOracle::new()
            .with_response("b.jpg", make_raw("b.jpg", "B"))
            .with_response("a.jpg", make_raw("a.jpg", "A"));
        let imgs = [
            ImageRef::new("a.jpg", "/tmp/a.jpg"),
            ImageRef::new("b.jpg", "/tmp/b.jpg"),
        ];
        let out = oracle.analyze_batch(&imgs, "").await.unwrap();
        assert_eq!(out[0].photo_category, "A");
        assert_eq!(out[1].photo_category, "B");
    }

    #[tokio::test]
    async fn mock_logs_calls() {
        let oracle = MockOracle::new();
        let imgs = [ImageRef::new("a.jpg", "/tmp/a.jpg")];
        oracle.analyze_batch(&imgs, "prompt-1").await.unwrap();
        oracle.analyze_batch(&imgs, "prompt-2").await.unwrap();
        let snap = oracle.snapshot_log();
        assert_eq!(snap.call_count, 2);
        assert_eq!(snap.last_prompt.as_deref(), Some("prompt-2"));
        assert_eq!(snap.last_filenames, vec!["a.jpg"]);
    }

    #[tokio::test]
    async fn oracle_is_object_safe() {
        // dyn VisionOracle が成立することを静的に確認
        let oracle: Arc<dyn VisionOracle> = Arc::new(MockOracle::new());
        let imgs = [ImageRef::new("a.jpg", "/tmp/a.jpg")];
        let out = oracle.analyze_batch(&imgs, "").await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(oracle.provider_name(), "mock");
    }

    #[test]
    fn image_ref_builder() {
        let r = ImageRef::new("a.jpg", "/tmp/a.jpg").with_date("2026-02-11 10:00:00");
        assert_eq!(r.file_name, "a.jpg");
        assert_eq!(r.date.as_deref(), Some("2026-02-11 10:00:00"));
    }
}
