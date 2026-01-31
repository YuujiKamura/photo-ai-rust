//! PDF export core utilities shared by CLI/WASM.

use crate::layout::{mm_to_pt, PdfLayout, LAYOUT_FIELDS};
use crate::types::AnalysisResult;

/// PDF描画で使用するレイアウト計算結果（pt単位）
#[derive(Debug, Clone)]
pub struct PdfLayoutCore {
    pub page_width_pt: f32,
    pub page_height_pt: f32,
    pub margin_pt: f32,
    pub photo_width_pt: f32,
    pub photo_height_pt: f32,
    pub info_width_pt: f32,
    pub header_height_pt: f32,
    pub photo_info_gap_pt: f32,
    pub photos_per_page: usize,
    pub photo_row_height_pt: f32,
}

impl PdfLayoutCore {
    pub fn from_layout(layout: &PdfLayout) -> Self {
        let page_width_pt = mm_to_pt(layout.page_width_mm);
        let page_height_pt = mm_to_pt(layout.page_height_mm);
        let margin_pt = mm_to_pt(layout.margin_mm);
        let photo_width_pt = mm_to_pt(layout.photo_width_mm);
        let photo_height_pt = mm_to_pt(layout.photo_height_mm);
        let info_width_pt = mm_to_pt(layout.info_width_mm);
        let header_height_pt = 40.0;
        let photo_info_gap_pt = 5.0;
        let photos_per_page = layout.photos_per_page as usize;
        let photo_row_height_pt = photo_height_pt + photo_info_gap_pt * 2.0;

        Self {
            page_width_pt,
            page_height_pt,
            margin_pt,
            photo_width_pt,
            photo_height_pt,
            info_width_pt,
            header_height_pt,
            photo_info_gap_pt,
            photos_per_page,
            photo_row_height_pt,
        }
    }

    /// 写真スロットのY座標（pt）
    pub fn row_y_pt(&self, slot: usize) -> f32 {
        self.page_height_pt
            - self.margin_pt
            - self.header_height_pt
            - ((slot + 1) as f32 * self.photo_row_height_pt)
            + self.photo_info_gap_pt
    }

    /// 情報欄のX座標（pt）
    pub fn info_x_pt(&self) -> f32 {
        self.margin_pt + self.photo_width_pt + self.photo_info_gap_pt
    }
}

/// PDFの情報欄に表示する1行
#[derive(Debug, Clone)]
pub struct PdfInfoField {
    pub label: &'static str,
    pub value: String,
    pub row_span: u8,
}

/// 情報欄フィールドを構築
pub fn build_pdf_info_fields(result: &AnalysisResult) -> Vec<PdfInfoField> {
    LAYOUT_FIELDS
        .iter()
        .map(|field| {
            let value = match field.key {
                "date" => format_date(&result.date),
                "remarks" => {
                    // 備考はマスタから選択した値のみ
                    if result.remarks.is_empty() { "-".to_string() } else { result.remarks.clone() }
                }
                "measurements" => {
                    // 測定値は別行として表示
                    if result.measurements.is_empty() { "-".to_string() } else { result.measurements.clone() }
                }
                _ => {
                    let raw = get_field_value(result, field.key);
                    if raw.is_empty() { "-".to_string() } else { raw.to_string() }
                }
            };
            PdfInfoField {
                label: field.label,
                value,
                row_span: field.row_span,
            }
        })
        .collect()
}

/// 日時フォーマット変換: "2025-12-26 13:47:52" → "2025/12/26 13:47"
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

fn get_field_value<'a>(result: &'a AnalysisResult, key: &str) -> &'a str {
    match key {
        "date" => "", // 特殊処理（format_dateを使用）
        "photoCategory" => &result.photo_category,
        "workType" => &result.work_type,
        "variety" => &result.variety,
        "subphase" => &result.subphase,
        "station" => &result.station,
        // 備考: measurementsがあればそれを優先（温度等の具体的な値）
        "remarks" => {
            if !result.measurements.is_empty() {
                &result.measurements
            } else if !result.remarks.is_empty() {
                &result.remarks
            } else {
                ""
            }
        },
        "measurements" => &result.measurements,
        _ => "-",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_core_positions() {
        let layout = PdfLayout::three_up();
        let core = PdfLayoutCore::from_layout(&layout);

        assert!(core.page_width_pt > 0.0);
        assert!(core.photo_row_height_pt > 0.0);

        let first_row = core.row_y_pt(0);
        let second_row = core.row_y_pt(1);
        assert!(first_row > second_row);
    }

    #[test]
    fn test_build_info_fields_defaults() {
        let result = AnalysisResult::default();
        let fields = build_pdf_info_fields(&result);
        assert_eq!(fields.len(), LAYOUT_FIELDS.len());
        assert_eq!(fields[0].label, "日時");
        assert_eq!(fields[0].value, "-");
    }
}
