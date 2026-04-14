//! エクスポート関連の共有型定義
//!
//! CLIクレート削除に伴い cli.rs から移動。ライブラリ・bin クレート双方で使用。

/// エクスポート形式
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
