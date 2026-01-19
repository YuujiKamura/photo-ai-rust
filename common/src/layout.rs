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

/// 写真アスペクト比（4:3）
pub const PHOTO_ASPECT_RATIO: f32 = 4.0 / 3.0;

/// 写真高さ: 4:3比率から計算（mm）
pub const PHOTO_HEIGHT_MM: f32 = PHOTO_WIDTH_MM / PHOTO_ASPECT_RATIO;  // 92.625mm

/// 写真高さ（旧: ページ分割基準、互換性のため残す）
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

/// Excel列幅変換係数
pub const PT_PER_EXCEL_COL: f32 = 5.3;
pub const PX_PER_EXCEL_COL: f32 = 7.1;
pub const EXCEL_COL_OFFSET_PX: f32 = 5.0;

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

/// ブロック高さ（pt）
pub const BLOCK_HEIGHT_3UP_PT: f32 = PHOTO_HEIGHT_PT_3UP + GAP_PT; // 271.25pt
pub const BLOCK_HEIGHT_2UP_PT: f32 = PHOTO_HEIGHT_PT_2UP * 1.5;    // 2upは別計算

// ============================================
// Excel用レイアウト定数
// ============================================

/// 全体スケール
pub const EXCEL_SCALE: f32 = 1.1;

/// 行の設計（写真高さ242.9ptを10行 + 余白1行）
pub const PHOTO_ROWS: u8 = 10;
pub const GAP_ROWS: u8 = 1;
pub const ROWS_PER_BLOCK_3UP: u8 = PHOTO_ROWS + GAP_ROWS; // 11行/ブロック
pub const ROWS_PER_BLOCK_2UP: u8 = PHOTO_ROWS + GAP_ROWS; // 2upも同様

/// 行高さ (pt) = 列幅から4:3比率で導出
pub const ROW_HEIGHT_PT: f32 = 27.0;

/// 列幅 (Excel単位)
pub const PHOTO_COL_WIDTH: f32 = 56.1;  // アスペクト比に合わせて調整
pub const LABEL_COL_WIDTH: f32 = 11.0;  // 10 * SCALE
pub const VALUE_COL_WIDTH: f32 = 28.6;  // 26 * SCALE
pub const INFO_COL_WIDTH: f32 = 39.6;   // LABEL + VALUE

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
    FieldDefinition { key: "date", label: "日時", row_span: 1 },
    FieldDefinition { key: "photoCategory", label: "区分", row_span: 1 },
    FieldDefinition { key: "workType", label: "工種", row_span: 1 },
    FieldDefinition { key: "variety", label: "種別", row_span: 1 },
    FieldDefinition { key: "subphase", label: "作業段階", row_span: 1 },
    FieldDefinition { key: "station", label: "測点", row_span: 1 },
    FieldDefinition { key: "remarks", label: "備考", row_span: 1 },
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
// Excelレイアウト設定構造体
// ============================================

/// Excelレイアウト設定
#[derive(Debug, Clone)]
pub struct ExcelLayout {
    /// 1ブロックあたりの行数
    pub rows_per_block: u8,
    /// 写真部分の行数
    pub photo_rows: u8,
    /// 行高さ (pt)
    pub row_height_pt: f32,
    /// 列A幅（画像列）
    pub col_a_width: f32,
    /// 列B幅（ラベル列）
    pub col_b_width: f32,
    /// 列C幅（値列）
    pub col_c_width: f32,
    /// 写真幅 (pt)
    pub photo_width_pt: f32,
    /// 写真高さ (pt)
    pub photo_height_pt: f32,
    /// 情報パネル幅 (pt)
    pub info_width_pt: f32,
}

impl ExcelLayout {
    /// 3枚/ページ用レイアウト
    pub fn three_up() -> Self {
        Self {
            rows_per_block: ROWS_PER_BLOCK_3UP,
            photo_rows: PHOTO_ROWS,
            row_height_pt: ROW_HEIGHT_PT,
            col_a_width: PHOTO_COL_WIDTH,
            col_b_width: LABEL_COL_WIDTH,
            col_c_width: VALUE_COL_WIDTH,
            photo_width_pt: PHOTO_WIDTH_PT,
            photo_height_pt: PHOTO_HEIGHT_PT_3UP,
            info_width_pt: INFO_WIDTH_PT,
        }
    }

    /// 2枚/ページ用レイアウト
    pub fn two_up() -> Self {
        Self {
            rows_per_block: ROWS_PER_BLOCK_2UP,
            photo_rows: PHOTO_ROWS,
            row_height_pt: ROW_HEIGHT_PT,
            col_a_width: PHOTO_COL_WIDTH,
            col_b_width: LABEL_COL_WIDTH,
            col_c_width: VALUE_COL_WIDTH,
            photo_width_pt: PHOTO_WIDTH_PT,
            photo_height_pt: PHOTO_HEIGHT_PT_2UP,
            info_width_pt: INFO_WIDTH_PT,
        }
    }

    /// 指定枚数でレイアウト取得
    pub fn for_photos_per_page(n: u8) -> Self {
        match n {
            2 => Self::two_up(),
            _ => Self::three_up(),
        }
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

/// pt → Excel列幅 変換
#[inline]
pub fn pt_to_excel_col(pt: f32) -> f32 {
    (pt / PT_PER_EXCEL_COL).round()
}

/// Excel列幅 → pt 変換
#[inline]
pub fn excel_col_to_pt(units: f32) -> f32 {
    (units * PT_PER_EXCEL_COL).round()
}

/// px → Excel幅 変換
#[inline]
pub fn px_to_excel_width(px: f32) -> f32 {
    ((px - EXCEL_COL_OFFSET_PX) / PX_PER_EXCEL_COL).round()
}

/// Excel幅 → px 変換
#[inline]
pub fn excel_width_to_px(units: f32) -> f32 {
    (units * PX_PER_EXCEL_COL + EXCEL_COL_OFFSET_PX).round()
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

    #[test]
    fn test_excel_layout() {
        let layout = ExcelLayout::three_up();
        assert_eq!(layout.rows_per_block, 11);
        assert_eq!(layout.photo_rows, 10);
        assert!((layout.row_height_pt - 27.0).abs() < 0.01);
        assert!((layout.col_a_width - 56.1).abs() < 0.01);
        assert!((layout.col_b_width - 11.0).abs() < 0.01);
        assert!((layout.col_c_width - 28.6).abs() < 0.01);
    }

    #[test]
    fn test_excel_conversion() {
        // Excel列幅変換
        let pt = 53.0;
        let col = pt_to_excel_col(pt);
        assert_eq!(col, 10.0);

        let back = excel_col_to_pt(col);
        assert_eq!(back, 53.0);
    }

    #[test]
    fn test_layout_fields() {
        // フィールド数の確認
        assert_eq!(LAYOUT_FIELDS.len(), 8);

        // 最初と最後のフィールド確認
        assert_eq!(LAYOUT_FIELDS[0].key, "date");
        assert_eq!(LAYOUT_FIELDS[0].label, "日時");
        assert_eq!(LAYOUT_FIELDS[7].key, "measurements");
        assert_eq!(LAYOUT_FIELDS[7].row_span, 3);
    }
}
