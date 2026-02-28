//! PDF export core utilities shared by CLI/WASM.
//!
//! GAS版 pdfCore.ts のレイアウト計算を完全移植
//!
//! ## GAS準拠のレイアウト計算
//! ```typescript
//! // GAS pdfCore.ts 148-154行目
//! const usableHeight = A4_HEIGHT - MARGIN * 2;
//! const photoRowHeight = usableHeight / photosPerPage;
//! const photoHeight = photoRowHeight - GAP * 2;
//! const usableWidth = A4_WIDTH - MARGIN * 2;
//! const photoWidth = usableWidth * IMAGE_RATIO;  // 0.65
//! const infoWidth = usableWidth * INFO_RATIO;    // 0.35
//!
//! // GAS pdfCore.ts 168行目
//! const rowY = A4_HEIGHT - MARGIN - (i + 1) * photoRowHeight + GAP;
//!
//! // GAS pdfCore.ts 236行目
//! const infoX = MARGIN + photoWidth;  // ギャップなし
//! ```

use crate::layout::{
    A4_WIDTH_PT, A4_HEIGHT_PT, MARGIN_PT, GAP_PT,
    IMAGE_RATIO, INFO_RATIO, FieldKey, LAYOUT_FIELDS,
};
use crate::types::AnalysisResult;

/// PDF描画で使用するレイアウト計算結果（pt単位）
/// GAS版 pdfCore.ts の計算式を完全移植
#[derive(Debug, Clone)]
pub struct PdfMetrics {
    /// A4幅（pt）: 595.35
    pub page_width_pt: f32,
    /// A4高さ（pt）: 842.0
    pub page_height_pt: f32,
    /// マージン（pt）: 28.35
    pub margin_pt: f32,
    /// ギャップ（pt）: 5.0
    pub gap_pt: f32,
    /// 利用可能幅（pt）: A4_WIDTH - MARGIN * 2
    pub usable_width_pt: f32,
    /// 利用可能高さ（pt）: A4_HEIGHT - MARGIN * 2
    pub usable_height_pt: f32,
    /// 写真幅（pt）: usableWidth * 0.65
    pub photo_width_pt: f32,
    /// 情報欄幅（pt）: usableWidth * 0.35
    pub info_width_pt: f32,
    /// 1ページあたりの写真数
    pub photos_per_page: usize,
    /// 写真行高さ（pt）: usableHeight / photosPerPage
    pub photo_row_height_pt: f32,
    /// 写真高さ（pt）: photoRowHeight - GAP * 2
    pub photo_height_pt: f32,
}

impl PdfMetrics {
    /// GAS準拠のレイアウト計算
    /// pdfCore.ts 148-154行目を完全移植
    pub fn new(photos_per_page: u8) -> Self {
        let photos_per_page = photos_per_page.clamp(2, 3) as usize;

        // GAS準拠定数
        let page_width_pt = A4_WIDTH_PT;    // 595.35
        let page_height_pt = A4_HEIGHT_PT;  // 842.0
        let margin_pt = MARGIN_PT;          // 28.35
        let gap_pt = GAP_PT;                // 5.0

        // GAS pdfCore.ts 148-154行目の計算式
        let usable_height_pt = page_height_pt - margin_pt * 2.0;
        let photo_row_height_pt = usable_height_pt / photos_per_page as f32;
        let photo_height_pt = photo_row_height_pt - gap_pt * 2.0;
        let usable_width_pt = page_width_pt - margin_pt * 2.0;
        let photo_width_pt = usable_width_pt * IMAGE_RATIO;   // 0.65
        let info_width_pt = usable_width_pt * INFO_RATIO;     // 0.35

        Self {
            page_width_pt,
            page_height_pt,
            margin_pt,
            gap_pt,
            usable_width_pt,
            usable_height_pt,
            photo_width_pt,
            info_width_pt,
            photos_per_page,
            photo_row_height_pt,
            photo_height_pt,
        }
    }

    /// 写真スロットのY座標（pt）
    /// GAS pdfCore.ts 168行目を完全移植
    /// `const rowY = A4_HEIGHT - MARGIN - (i + 1) * photoRowHeight + GAP;`
    pub fn row_y_pt(&self, slot: usize) -> f32 {
        self.page_height_pt
            - self.margin_pt
            - ((slot + 1) as f32 * self.photo_row_height_pt)
            + self.gap_pt
    }

    /// 情報欄のX座標（pt）
    /// GAS pdfCore.ts 236行目を完全移植
    /// `const infoX = MARGIN + photoWidth;` (ギャップなし)
    pub fn info_x_pt(&self) -> f32 {
        self.margin_pt + self.photo_width_pt
    }

    /// 旧APIとの互換性のため（PdfLayoutから生成）
    pub fn from_layout(layout: &crate::layout::PdfLayout) -> Self {
        Self::new(layout.photos_per_page)
    }
}

/// PDFの情報欄に表示する1行
#[derive(Debug, Clone)]
pub struct PdfInfoField {
    pub key: crate::layout::FieldKey,
    pub label: String,
    pub value: String,
    pub row_span: u8,
}

/// 測定値フィールドを表示するか判定（プレゼンテーション層の責務）
///
/// 実際の測定データがあれば常に表示。
/// データがない場合は、品質管理・出来形管理写真のみ空欄プレースホルダーを表示。
/// 施工状況写真・安全管理写真・着手前完成写真・その他は非表示。
fn should_show_measurements(result: &AnalysisResult) -> bool {
    use crate::types::PhotoData;
    let v = result.measurements();
    if !v.is_empty() && v != "-" {
        return true;
    }
    matches!(result.photo_category.as_str(), "品質管理写真" | "出来形管理写真")
}

/// 測定値ラベルを非表示にするか判定
///
/// 使用材料写真: ラベル「測定値」を非表示にし、値の描画幅を拡大する。
fn should_hide_measurement_label(result: &AnalysisResult) -> bool {
    result.photo_category == "使用材料写真"
}

/// 情報欄フィールドを構築（GAS準拠: 8フィールド）
/// 日時 → 区分 → 工種 → 種別 → 細別 → 測点 → 備考 → 測定値
///
/// 測定値の表示制御はphoto_categoryに基づく:
/// - 品質管理/出来形管理写真: 常に表示（データなしでもプレースホルダー）
/// - 使用材料写真: ラベル非表示（値の描画幅を拡大）
/// - その他カテゴリ: データがある場合のみ表示
pub fn build_pdf_info_fields(result: &AnalysisResult) -> Vec<PdfInfoField> {
    use crate::types::PhotoData;
    let show_meas = should_show_measurements(result);
    let hide_meas_label = should_hide_measurement_label(result);

    LAYOUT_FIELDS
        .iter()
        .map(|field| {
            let is_meas = field.key == FieldKey::Measurements;
            let value = if is_meas && !show_meas {
                String::new()
            } else if field.key == FieldKey::Date {
                format_date(&result.date)
            } else {
                result.get_field_value(field.key).to_string()
            };
            let label = if is_meas && (!show_meas || hide_meas_label) {
                String::new()
            } else {
                result.get_label_for_field(field.key).to_string()
            };
            PdfInfoField {
                key: field.key,
                label,
                value,
                row_span: field.row_span,
            }
        })
        .collect()
}

/// 日時フォーマット変換
/// "2025-12-26 13:47:52" → "2025/12/26 13:47"
fn format_date(date: &str) -> String {
    if date.is_empty() {
        return "-".to_string();
    }
    // "YYYY-MM-DD HH:MM:SS" → "YYYY/MM/DD HH:MM"
    let formatted = date.replace('-', "/");
    // 秒を削除（最後の:SS部分）
    if formatted.len() > 16 {
        formatted[..16].to_string()
    } else {
        formatted
    }
}

/// GAS準拠テキスト描画設定（pdfCore.ts 261-306行目）
#[derive(Debug, Clone)]
pub struct TextDrawConfig {
    /// 基本フォントサイズ: 12pt
    pub base_font_size: f32,
    /// 行の高さ: 14pt
    pub line_height: f32,
    /// ラベル幅: 50pt
    pub label_width: f32,
    /// ラベルX位置: infoX + 5
    pub label_x_offset: f32,
    /// 値X位置: infoX + 55
    pub value_x_offset: f32,
    /// 開始Y位置オフセット: rowY + photoHeight - 20
    pub start_y_offset: f32,
    /// 最小フォントサイズ: 8pt
    pub min_font_size: f32,
    /// 最大行数: 2
    pub max_lines: usize,
}

impl Default for TextDrawConfig {
    fn default() -> Self {
        Self {
            base_font_size: 12.0,
            line_height: 14.0,
            label_width: 50.0,
            label_x_offset: 5.0,
            value_x_offset: 55.0,
            start_y_offset: 20.0,
            min_font_size: 8.0,
            max_lines: 2,
        }
    }
}

impl TextDrawConfig {
    /// 値の最大幅を計算（GAS準拠）
    /// `const valueMaxWidth = infoWidth - labelWidth - 10;`
    pub fn value_max_width(&self, info_width_pt: f32) -> f32 {
        info_width_pt - self.label_width - 10.0
    }

    /// 開始Y座標を計算（GAS準拠）
    /// `let currentY = rowY + photoHeight - 20;`
    pub fn start_y(&self, row_y_pt: f32, photo_height_pt: f32) -> f32 {
        row_y_pt + photo_height_pt - self.start_y_offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gas_layout_calculation() {
        let core = PdfMetrics::new(3);

        // GAS準拠定数の確認
        assert!((core.page_width_pt - 595.35).abs() < 0.01);
        assert!((core.page_height_pt - 842.0).abs() < 0.01);
        assert!((core.margin_pt - 28.35).abs() < 0.01);
        assert!((core.gap_pt - 5.0).abs() < 0.01);

        // usableHeight = 842 - 28.35 * 2 = 785.3
        assert!((core.usable_height_pt - 785.3).abs() < 0.1);

        // usableWidth = 595.35 - 28.35 * 2 = 538.65
        assert!((core.usable_width_pt - 538.65).abs() < 0.1);

        // photoWidth = 538.65 * 0.65 = 350.12
        assert!((core.photo_width_pt - 350.12).abs() < 0.1);

        // infoWidth = 538.65 * 0.35 = 188.53
        assert!((core.info_width_pt - 188.53).abs() < 0.1);

        // photoRowHeight = 785.3 / 3 = 261.77
        assert!((core.photo_row_height_pt - 261.77).abs() < 0.1);

        // photoHeight = 261.77 - 5 * 2 = 251.77
        assert!((core.photo_height_pt - 251.77).abs() < 0.1);
    }

    #[test]
    fn test_row_y_calculation() {
        let core = PdfMetrics::new(3);

        // rowY = 842 - 28.35 - (i + 1) * 261.77 + 5
        // slot 0: 842 - 28.35 - 261.77 + 5 = 556.88
        let row0 = core.row_y_pt(0);
        assert!((row0 - 556.88).abs() < 0.1);

        // slot 1: 842 - 28.35 - 2 * 261.77 + 5 = 295.11
        let row1 = core.row_y_pt(1);
        assert!((row1 - 295.11).abs() < 0.1);

        // slot 2: 842 - 28.35 - 3 * 261.77 + 5 = 33.34
        let row2 = core.row_y_pt(2);
        assert!((row2 - 33.34).abs() < 0.1);
    }

    #[test]
    fn test_info_x_calculation() {
        let core = PdfMetrics::new(3);

        // infoX = 28.35 + 350.12 = 378.47
        let info_x = core.info_x_pt();
        assert!((info_x - 378.47).abs() < 0.1);
    }

    #[test]
    fn test_build_info_fields() {
        let result = AnalysisResult::default();
        let fields = build_pdf_info_fields(&result);

        // 8フィールド確認
        assert_eq!(fields.len(), 8);

        // フィールド順序確認（測定値はデフォルトcategoryでは非表示）
        assert_eq!(fields[0].label, "日時");
        assert_eq!(fields[1].label, "区分");
        assert_eq!(fields[2].label, "工種");
        assert_eq!(fields[3].label, "種別");
        assert_eq!(fields[4].label, "細別");
        assert_eq!(fields[5].label, "測点");
        assert_eq!(fields[6].label, "備考");
        assert!(fields[7].label.is_empty(), "default category: measurement label hidden");

        // デフォルト値確認（測定値は非表示なので空）
        for field in &fields[..7] {
            assert_eq!(field.value, "-");
        }
        assert!(fields[7].value.is_empty(), "default category: measurement value hidden");
    }

    #[test]
    fn test_text_draw_config() {
        let config = TextDrawConfig::default();

        // GAS準拠値の確認
        assert!((config.base_font_size - 12.0).abs() < 0.01);
        assert!((config.line_height - 14.0).abs() < 0.01);
        assert!((config.label_width - 50.0).abs() < 0.01);
        assert!((config.label_x_offset - 5.0).abs() < 0.01);
        assert!((config.value_x_offset - 55.0).abs() < 0.01);

        // valueMaxWidth計算確認
        // infoWidth = 188.53, valueMaxWidth = 188.53 - 50 - 10 = 128.53
        let value_max = config.value_max_width(188.53);
        assert!((value_max - 128.53).abs() < 0.1);
    }

    #[test]
    fn test_format_date() {
        assert_eq!(format_date(""), "-");
        assert_eq!(format_date("2025-12-26 13:47:52"), "2025/12/26 13:47");
        assert_eq!(format_date("2025-01-01"), "2025/01/01");
    }

    #[test]
    fn test_two_up_layout() {
        let core = PdfMetrics::new(2);

        // 2枚モードの確認
        assert_eq!(core.photos_per_page, 2);

        // photoRowHeight = 785.3 / 2 = 392.65
        assert!((core.photo_row_height_pt - 392.65).abs() < 0.1);

        // photoHeight = 392.65 - 5 * 2 = 382.65
        assert!((core.photo_height_pt - 382.65).abs() < 0.1);
    }

    #[test]
    fn test_should_show_measurements_with_data() {
        let result = AnalysisResult {
            photo_category: "施工状況写真".to_string(),
            measurements: "厚さ50mm".to_string(),
            ..Default::default()
        };
        assert!(should_show_measurements(&result));
    }

    #[test]
    fn test_should_hide_measurements_construction() {
        let result = AnalysisResult {
            photo_category: "施工状況写真".to_string(),
            ..Default::default()
        };
        assert!(!should_show_measurements(&result));
    }

    #[test]
    fn test_should_show_measurements_quality() {
        let result = AnalysisResult {
            photo_category: "品質管理写真".to_string(),
            ..Default::default()
        };
        assert!(should_show_measurements(&result));
    }

    #[test]
    fn test_should_show_measurements_dekigata() {
        let result = AnalysisResult {
            photo_category: "出来形管理写真".to_string(),
            ..Default::default()
        };
        assert!(should_show_measurements(&result));
    }

    #[test]
    fn test_should_hide_measurements_safety() {
        let result = AnalysisResult {
            photo_category: "安全管理写真".to_string(),
            ..Default::default()
        };
        assert!(!should_show_measurements(&result));
    }

    #[test]
    fn test_hide_measurement_label_material() {
        let result = AnalysisResult {
            photo_category: "使用材料写真".to_string(),
            ..Default::default()
        };
        assert!(should_hide_measurement_label(&result));
    }

    #[test]
    fn test_build_info_fields_construction_no_measurements() {
        let result = AnalysisResult {
            photo_category: "施工状況写真".to_string(),
            ..Default::default()
        };
        let fields = build_pdf_info_fields(&result);
        let meas = fields.iter().find(|f| f.key == FieldKey::Measurements).unwrap();
        assert!(meas.label.is_empty(), "施工状況写真: measurement label should be empty");
        assert!(meas.value.is_empty(), "施工状況写真: measurement value should be empty");
    }

    #[test]
    fn test_build_info_fields_quality_placeholder() {
        let result = AnalysisResult {
            photo_category: "品質管理写真".to_string(),
            ..Default::default()
        };
        let fields = build_pdf_info_fields(&result);
        let meas = fields.iter().find(|f| f.key == FieldKey::Measurements).unwrap();
        assert_eq!(meas.label, "測定値", "品質管理写真: measurement label should be shown");
        assert_eq!(meas.value, "-", "品質管理写真: measurement value should be placeholder");
    }

    #[test]
    fn test_build_info_fields_material_hide_label() {
        let result = AnalysisResult {
            photo_category: "使用材料写真".to_string(),
            measurements: "セメント 50kg".to_string(),
            ..Default::default()
        };
        let fields = build_pdf_info_fields(&result);
        let meas = fields.iter().find(|f| f.key == FieldKey::Measurements).unwrap();
        assert!(meas.label.is_empty(), "使用材料写真: measurement label should be hidden");
        assert_eq!(meas.value, "セメント 50kg", "使用材料写真: measurement value should be shown");
    }
}
