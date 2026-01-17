//! レイアウト設定モジュール
//!
//! mm基準のレイアウト定義（Source of Truth）
//! React版 layoutConfig.ts と同一の設計思想

// ============================================
// mm基準レイアウト（Source of Truth）
// ============================================

/// A4サイズ（mm）
pub const A4_WIDTH_MM: f32 = 210.0;
pub const A4_HEIGHT_MM: f32 = 297.0;

/// 余白設定（mm）
pub const MARGIN_MM: f32 = 10.0;
pub const PHOTO_GAP_MM: f32 = 10.0;

/// プレビュー比率（これが正）
pub const IMAGE_RATIO: f32 = 0.65;
pub const INFO_RATIO: f32 = 0.35;

/// 利用可能幅から写真/情報幅を計算（mm）
pub const USABLE_WIDTH_MM: f32 = A4_WIDTH_MM - MARGIN_MM * 2.0;  // 190mm
pub const PHOTO_WIDTH_MM: f32 = USABLE_WIDTH_MM * IMAGE_RATIO;   // 123.5mm
pub const INFO_WIDTH_MM: f32 = USABLE_WIDTH_MM * INFO_RATIO;     // 66.5mm

/// 写真高さ: 3枚配置から計算（mm）
pub const PHOTO_HEIGHT_MM_3UP: f32 = (A4_HEIGHT_MM - MARGIN_MM * 2.0 - PHOTO_GAP_MM * 2.0) / 3.0;  // 85.67mm
pub const PHOTO_HEIGHT_MM_2UP: f32 = (A4_HEIGHT_MM - MARGIN_MM * 2.0 - PHOTO_GAP_MM) / 2.0;        // 128.5mm

// ============================================
// 変換係数
// ============================================

/// mm → pt変換 (1mm = 72/25.4 pt ≈ 2.835pt)
pub const MM_TO_PT: f32 = 72.0 / 25.4;

/// pt → px変換 (96dpi基準)
pub const PT_TO_PX: f32 = 96.0 / 72.0;
pub const PX_TO_PT: f32 = 72.0 / 96.0;

// ============================================
// 導出されるpt値
// ============================================

/// ページサイズ（pt）
pub const PAGE_WIDTH_PT: f32 = A4_WIDTH_MM * MM_TO_PT;   // 595.35pt
pub const PAGE_HEIGHT_PT: f32 = A4_HEIGHT_MM * MM_TO_PT; // 842.0pt

/// マージン（pt）
pub const MARGIN_PT: f32 = MARGIN_MM * MM_TO_PT;         // 28.35pt
pub const GAP_PT: f32 = PHOTO_GAP_MM * MM_TO_PT;         // 28.35pt

/// 写真サイズ（pt）
pub const PHOTO_WIDTH_PT: f32 = PHOTO_WIDTH_MM * MM_TO_PT;       // 350.1pt
pub const PHOTO_HEIGHT_PT_3UP: f32 = PHOTO_HEIGHT_MM_3UP * MM_TO_PT; // 242.9pt
pub const PHOTO_HEIGHT_PT_2UP: f32 = PHOTO_HEIGHT_MM_2UP * MM_TO_PT; // 364.3pt

/// 情報パネル幅（pt）
pub const INFO_WIDTH_PT: f32 = INFO_WIDTH_MM * MM_TO_PT;         // 188.5pt

// ============================================
// フィールド定義
// ============================================

/// 情報パネルに表示するフィールド
#[derive(Debug, Clone, Copy)]
pub struct FieldDefinition {
    pub key: &'static str,
    pub label: &'static str,
    pub row_span: u8,
}

/// レイアウトフィールド（React版 LAYOUT_FIELDS と同等）
pub const LAYOUT_FIELDS: &[FieldDefinition] = &[
    FieldDefinition { key: "workType", label: "工種", row_span: 1 },
    FieldDefinition { key: "variety", label: "種別", row_span: 1 },
    FieldDefinition { key: "detail", label: "細別", row_span: 1 },
    FieldDefinition { key: "station", label: "測点", row_span: 1 },
    FieldDefinition { key: "remarks", label: "備考", row_span: 2 },
    FieldDefinition { key: "measurements", label: "測定値", row_span: 3 },
];

// ============================================
// レイアウト設定構造体
// ============================================

/// PDFレイアウト設定
#[derive(Debug, Clone)]
pub struct PdfLayout {
    /// ページ幅（mm）
    pub page_width_mm: f32,
    /// ページ高さ（mm）
    pub page_height_mm: f32,
    /// マージン（mm）
    pub margin_mm: f32,
    /// 写真間ギャップ（mm）
    pub gap_mm: f32,
    /// 写真幅（mm）
    pub photo_width_mm: f32,
    /// 写真高さ（mm）
    pub photo_height_mm: f32,
    /// 情報パネル幅（mm）
    pub info_width_mm: f32,
    /// 1ページあたりの写真数
    pub photos_per_page: u8,
}

impl PdfLayout {
    /// 3枚/ページ用レイアウト
    pub fn three_up() -> Self {
        Self {
            page_width_mm: A4_WIDTH_MM,
            page_height_mm: A4_HEIGHT_MM,
            margin_mm: MARGIN_MM,
            gap_mm: PHOTO_GAP_MM,
            photo_width_mm: PHOTO_WIDTH_MM,
            photo_height_mm: PHOTO_HEIGHT_MM_3UP,
            info_width_mm: INFO_WIDTH_MM,
            photos_per_page: 3,
        }
    }

    /// 2枚/ページ用レイアウト
    pub fn two_up() -> Self {
        Self {
            page_width_mm: A4_WIDTH_MM,
            page_height_mm: A4_HEIGHT_MM,
            margin_mm: MARGIN_MM,
            gap_mm: PHOTO_GAP_MM,
            photo_width_mm: PHOTO_WIDTH_MM,
            photo_height_mm: PHOTO_HEIGHT_MM_2UP,
            info_width_mm: INFO_WIDTH_MM,
            photos_per_page: 2,
        }
    }

    /// 指定枚数でレイアウト取得
    pub fn for_photos_per_page(n: u8) -> Self {
        match n {
            2 => Self::two_up(),
            _ => Self::three_up(),
        }
    }

    /// ブロック高さ（写真 + ギャップ）mm
    pub fn block_height_mm(&self) -> f32 {
        self.photo_height_mm + self.gap_mm
    }

    /// 利用可能幅（mm）
    pub fn usable_width_mm(&self) -> f32 {
        self.page_width_mm - self.margin_mm * 2.0
    }

    /// 利用可能高さ（mm）
    pub fn usable_height_mm(&self) -> f32 {
        self.page_height_mm - self.margin_mm * 2.0
    }

    /// コンテンツ開始Y座標（mm、上から）
    pub fn content_start_y_mm(&self) -> f32 {
        self.page_height_mm - self.margin_mm
    }
}

// ============================================
// ヘルパー関数
// ============================================

/// mm → pt 変換
#[inline]
pub fn mm_to_pt(mm: f32) -> f32 {
    mm * MM_TO_PT
}

/// pt → mm 変換
#[inline]
pub fn pt_to_mm(pt: f32) -> f32 {
    pt / MM_TO_PT
}

/// px → pt 変換
#[inline]
pub fn px_to_pt(px: f32) -> f32 {
    px * PX_TO_PT
}

/// pt → px 変換
#[inline]
pub fn pt_to_px(pt: f32) -> f32 {
    pt * PT_TO_PX
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dimensions() {
        // 基本寸法の確認
        assert!((USABLE_WIDTH_MM - 190.0).abs() < 0.01);
        assert!((PHOTO_WIDTH_MM - 123.5).abs() < 0.01);
        assert!((INFO_WIDTH_MM - 66.5).abs() < 0.01);
        assert!((PHOTO_HEIGHT_MM_3UP - 85.67).abs() < 0.1);
    }

    #[test]
    fn test_ratios() {
        // 比率の確認
        let total = IMAGE_RATIO + INFO_RATIO;
        assert!((total - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_conversion() {
        // 変換係数の確認
        assert!((MM_TO_PT - 2.835).abs() < 0.01);
        assert!((mm_to_pt(10.0) - 28.35).abs() < 0.1);
    }

    #[test]
    fn test_layout_config() {
        let layout = PdfLayout::three_up();
        assert_eq!(layout.photos_per_page, 3);
        assert!((layout.photo_width_mm - 123.5).abs() < 0.01);

        let layout2 = PdfLayout::two_up();
        assert_eq!(layout2.photos_per_page, 2);
        assert!(layout2.photo_height_mm > layout.photo_height_mm);
    }
}
