//! レポートレンダリング port
//!
//! 解析結果を PDF/Excel/XML 等のバイト列に変換する adapter を抽象化する。
//! feature フラグに依存しないため、Application 層はこの trait だけに依存できる。
//!
//! 既存の `crate::export::Exporter` は内部実装として残し、段階的にこの trait を
//! 採用していく（blanket impl や bridge 関数は将来追加予定）。

use crate::types::AnalysisResult;
use std::io;

/// 出力形式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutputFormat {
    /// PDF 写真台帳
    Pdf,
    /// Excel 写真台帳
    Excel,
    /// GASPhotoAIManager 互換 XML
    PhotoXml,
}

impl OutputFormat {
    /// 拡張子を返す（先頭ドットなし）
    pub const fn file_extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Excel => "xlsx",
            Self::PhotoXml => "xml",
        }
    }

    /// 表示用の短縮名
    pub const fn short_name(self) -> &'static str {
        match self {
            Self::Pdf => "PDF",
            Self::Excel => "Excel",
            Self::PhotoXml => "XML",
        }
    }

    /// 拡張子（`pdf` / `xlsx` / `xml`。大小問わず）から判定する
    pub fn from_extension(s: &str) -> Option<Self> {
        match s.trim_start_matches('.').to_ascii_lowercase().as_str() {
            "pdf" => Some(Self::Pdf),
            "xlsx" | "xls" => Some(Self::Excel),
            "xml" => Some(Self::PhotoXml),
            _ => None,
        }
    }

    /// 全形式を列挙
    pub const fn all() -> &'static [OutputFormat] {
        &[Self::Pdf, Self::Excel, Self::PhotoXml]
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.short_name())
    }
}

/// 画像データ（バイト配列）
///
/// レンダラが画像を埋め込む際に参照する。`crate::export::ImageData` と同じ構造だが、
/// port 層が feature フラグに依存しないよう独立した型として定義する。
#[derive(Debug, Clone)]
pub struct ImageData {
    pub data: Vec<u8>,
    pub extension: String,
}

/// レンダリングエラー
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("render failed ({format}): {detail}")]
    Failed { format: String, detail: String },

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

/// 画像ローダの型エイリアス
///
/// ファイルパス文字列から `ImageData` を解決する関数。
/// PDF/Excel レンダラは写真埋め込みのためにこれを必要とする。
pub type ImageLoader<'a> = dyn Fn(&str) -> Option<ImageData> + 'a;

/// レポートレンダラ port
///
/// 解析結果配列を受け取り、指定フォーマットのバイト列を返す。
pub trait ReportRenderer {
    /// 生成するフォーマット
    fn output_format(&self) -> OutputFormat;

    /// 解析結果をバイト列にレンダリングする
    fn render(
        &self,
        results: &[AnalysisResult],
        image_loader: &ImageLoader<'_>,
    ) -> Result<Vec<u8>, RenderError>;
}

// ============================================================================
// MockRenderer: テスト専用
// ============================================================================

/// 固定バイト列を返すテスト用レンダラ
///
/// 呼び出し回数・渡された結果件数を記録する（内部可変性を使わず、呼出側が検証する場合は
/// 返り値のサイズやパターンで判定できる）。
pub struct MockRenderer {
    format: OutputFormat,
    output: Vec<u8>,
}

impl MockRenderer {
    pub fn new(format: OutputFormat, output: Vec<u8>) -> Self {
        Self { format, output }
    }

    /// 「件数分の `A` を返すだけ」のデバッグ用レンダラ
    pub fn count_echo(format: OutputFormat) -> Self {
        Self {
            format,
            output: Vec::new(),
        }
    }

    fn is_count_echo(&self) -> bool {
        self.output.is_empty()
    }
}

impl ReportRenderer for MockRenderer {
    fn output_format(&self) -> OutputFormat {
        self.format
    }

    fn render(
        &self,
        results: &[AnalysisResult],
        _image_loader: &ImageLoader<'_>,
    ) -> Result<Vec<u8>, RenderError> {
        if self.is_count_echo() {
            return Ok(vec![b'A'; results.len()]);
        }
        Ok(self.output.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === OutputFormat ===

    #[test]
    fn file_extension_pdf_is_pdf() {
        assert_eq!(OutputFormat::Pdf.file_extension(), "pdf");
    }

    #[test]
    fn file_extension_excel_is_xlsx() {
        assert_eq!(OutputFormat::Excel.file_extension(), "xlsx");
    }

    #[test]
    fn from_extension_accepts_all_formats() {
        assert_eq!(OutputFormat::from_extension("pdf"), Some(OutputFormat::Pdf));
        assert_eq!(OutputFormat::from_extension(".pdf"), Some(OutputFormat::Pdf));
        assert_eq!(OutputFormat::from_extension("PDF"), Some(OutputFormat::Pdf));
        assert_eq!(
            OutputFormat::from_extension("xlsx"),
            Some(OutputFormat::Excel)
        );
        assert_eq!(
            OutputFormat::from_extension("xls"),
            Some(OutputFormat::Excel)
        );
        assert_eq!(
            OutputFormat::from_extension("xml"),
            Some(OutputFormat::PhotoXml)
        );
    }

    #[test]
    fn from_extension_returns_none_for_unknown() {
        assert_eq!(OutputFormat::from_extension("csv"), None);
        assert_eq!(OutputFormat::from_extension(""), None);
    }

    #[test]
    fn file_extension_roundtrips_through_from_extension() {
        for fmt in OutputFormat::all() {
            let ext = fmt.file_extension();
            assert_eq!(OutputFormat::from_extension(ext), Some(*fmt));
        }
    }

    #[test]
    fn display_uses_short_name() {
        assert_eq!(format!("{}", OutputFormat::Pdf), "PDF");
        assert_eq!(format!("{}", OutputFormat::Excel), "Excel");
        assert_eq!(format!("{}", OutputFormat::PhotoXml), "XML");
    }

    // === MockRenderer ===

    #[test]
    fn mock_returns_fixed_output() {
        let r = MockRenderer::new(OutputFormat::Pdf, vec![1, 2, 3, 4]);
        let loader = |_: &str| None;
        let out = r.render(&[], &loader).unwrap();
        assert_eq!(out, vec![1, 2, 3, 4]);
    }

    #[test]
    fn count_echo_returns_one_byte_per_result() {
        let r = MockRenderer::count_echo(OutputFormat::Excel);
        let results = vec![AnalysisResult::default(); 3];
        let loader = |_: &str| None;
        let out = r.render(&results, &loader).unwrap();
        assert_eq!(out, b"AAA");
    }

    #[test]
    fn mock_reports_its_format() {
        let r = MockRenderer::new(OutputFormat::PhotoXml, vec![]);
        assert_eq!(r.output_format(), OutputFormat::PhotoXml);
    }

    // === RenderError Display ===

    #[test]
    fn render_error_failed_display_contains_format_and_detail() {
        let e = RenderError::Failed {
            format: "PDF".to_string(),
            detail: "font missing".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("PDF"));
        assert!(s.contains("font missing"));
    }
}
