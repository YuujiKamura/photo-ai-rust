//! JavaScript Bridge バインディング
//!
//! Rust WASM から JavaScript 関数を呼び出すためのバインディング定義。
//! PDF/Excel生成をJavaScript側に委譲する際に使用。

use serde::Serialize;
use wasm_bindgen::prelude::*;

// ============================================
// データ型定義
// ============================================

/// JavaScript側に渡す写真データ
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsPhotoEntry {
    pub file_name: String,
    pub file_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_data_url: Option<String>,
    pub date: String,
    pub work_type: String,
    pub variety: String,
    pub subphase: String,
    pub station: String,
    pub remarks: String,
    pub description: String,
    pub measurements: String,
    pub photo_category: String,
    pub has_board: bool,
    pub detected_text: String,
}

/// JavaScript側に渡すレイアウト設定
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsLayoutConfig {
    pub page_width_mm: f32,
    pub page_height_mm: f32,
    pub margin_mm: f32,
    pub gap_mm: f32,
    pub photo_width_mm: f32,
    pub photo_height_mm: f32,
    pub info_width_mm: f32,
    pub photos_per_page: u8,
}

// ============================================
// 変換トレイト実装
// ============================================

impl From<&photo_ai_common::AnalysisResult> for JsPhotoEntry {
    fn from(result: &photo_ai_common::AnalysisResult) -> Self {
        let image_data_url = if result.file_path.is_empty() {
            None
        } else {
            Some(result.file_path.clone())
        };

        Self {
            file_name: result.file_name.clone(),
            file_path: result.file_path.clone(),
            image_data_url,
            date: result.date.clone(),
            work_type: result.work_type.clone(),
            variety: result.variety.clone(),
            subphase: result.subphase.clone(),
            station: result.station.clone(),
            remarks: result.remarks.clone(),
            description: result.description.clone(),
            measurements: result.measurements.clone(),
            photo_category: result.photo_category.clone(),
            has_board: result.has_board,
            detected_text: result.detected_text.clone(),
        }
    }
}

impl From<&photo_ai_common::PdfLayout> for JsLayoutConfig {
    fn from(layout: &photo_ai_common::PdfLayout) -> Self {
        Self {
            page_width_mm: layout.page_width_mm,
            page_height_mm: layout.page_height_mm,
            margin_mm: layout.margin_mm,
            gap_mm: layout.gap_mm,
            photo_width_mm: layout.photo_width_mm,
            photo_height_mm: layout.photo_height_mm,
            info_width_mm: layout.info_width_mm,
            photos_per_page: layout.photos_per_page,
        }
    }
}

// ============================================
// JavaScript関数のextern宣言
// ============================================

#[wasm_bindgen(module = "/js/pdf-bridge.js")]
extern "C" {
    /// JavaScript側でPDFを生成
    ///
    /// # Arguments
    /// * `photos_json` - JsPhotoEntry配列のJSON文字列
    /// * `layout_json` - JsLayoutConfigのJSON文字列
    /// * `options_json` - 追加オプションのJSON文字列
    ///
    /// # Returns
    /// PDFのバイト配列（Uint8Array）
    #[wasm_bindgen(js_name = "generatePdf", catch)]
    pub async fn generate_pdf_js(
        photos_json: &str,
        layout_json: &str,
        options_json: &str,
    ) -> Result<JsValue, JsValue>;

    /// フォントをロード
    ///
    /// # Arguments
    /// * `font_url` - フォントファイルのURL
    #[wasm_bindgen(js_name = "loadFont", catch)]
    pub async fn load_font_js(font_url: &str) -> Result<(), JsValue>;
}

#[wasm_bindgen(module = "/js/excel-bridge.js")]
extern "C" {
    /// JavaScript側でExcelを生成
    ///
    /// # Arguments
    /// * `photos_json` - JsPhotoEntry配列のJSON文字列
    /// * `options_json` - 追加オプションのJSON文字列
    ///
    /// # Returns
    /// Excelのバイト配列（Uint8Array）
    #[wasm_bindgen(js_name = "generateExcel", catch)]
    pub async fn generate_excel_js(
        photos_json: &str,
        options_json: &str,
    ) -> Result<JsValue, JsValue>;
}

#[wasm_bindgen(module = "/js/download.js")]
extern "C" {
    /// PDFをダウンロード
    ///
    /// # Arguments
    /// * `data` - PDFのバイト配列
    /// * `filename` - ダウンロード時のファイル名
    #[wasm_bindgen(js_name = "downloadPdf")]
    pub fn download_pdf_js(data: &[u8], filename: &str);

    /// Excelをダウンロード
    ///
    /// # Arguments
    /// * `data` - Excelのバイト配列
    /// * `filename` - ダウンロード時のファイル名
    #[wasm_bindgen(js_name = "downloadExcel")]
    pub fn download_excel_js(data: &[u8], filename: &str);
}

// ============================================
// ヘルパー関数
// ============================================

/// AnalysisResult配列をJSON文字列に変換
pub fn photos_to_json(results: &[photo_ai_common::AnalysisResult]) -> Result<String, String> {
    let entries: Vec<JsPhotoEntry> = results.iter().map(JsPhotoEntry::from).collect();
    serde_json::to_string(&entries).map_err(|e| format!("JSON serialization failed: {}", e))
}

/// PdfLayoutをJSON文字列に変換
pub fn layout_to_json(layout: &photo_ai_common::PdfLayout) -> Result<String, String> {
    let config = JsLayoutConfig::from(layout);
    serde_json::to_string(&config).map_err(|e| format!("JSON serialization failed: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use photo_ai_common::{AnalysisResult, PdfLayout};

    #[test]
    fn test_js_photo_entry_from_analysis_result() {
        let result = AnalysisResult {
            file_name: "test.jpg".to_string(),
            file_path: "/path/to/test.jpg".to_string(),
            date: "2025-01-18".to_string(),
            work_type: "舗装工".to_string(),
            variety: "表層工".to_string(),
            subphase: "作業段階".to_string(),
            station: "No.10".to_string(),
            remarks: "備考".to_string(),
            description: "説明".to_string(),
            measurements: "厚さ50mm".to_string(),
            photo_category: "品質管理写真".to_string(),
            has_board: true,
            detected_text: "黒板テキスト".to_string(),
            reasoning: "分類理由".to_string(),
        };

        let entry = JsPhotoEntry::from(&result);

        assert_eq!(entry.file_name, "test.jpg");
        assert_eq!(entry.work_type, "舗装工");
        assert!(entry.has_board);
    }

    #[test]
    fn test_js_layout_config_from_pdf_layout() {
        let layout = PdfLayout::three_up();
        let config = JsLayoutConfig::from(&layout);

        assert_eq!(config.photos_per_page, 3);
        assert!((config.page_width_mm - 210.0).abs() < 0.01);
        assert!((config.page_height_mm - 297.0).abs() < 0.01);
    }

    #[test]
    fn test_photos_to_json() {
        let results = vec![
            AnalysisResult {
                file_name: "photo1.jpg".to_string(),
                work_type: "舗装工".to_string(),
                ..Default::default()
            },
            AnalysisResult {
                file_name: "photo2.jpg".to_string(),
                work_type: "区画線工".to_string(),
                ..Default::default()
            },
        ];

        let json = photos_to_json(&results).expect("JSON変換失敗");
        assert!(json.contains("photo1.jpg"));
        assert!(json.contains("photo2.jpg"));
        assert!(json.contains("舗装工"));
        assert!(json.contains("区画線工"));
    }

    #[test]
    fn test_layout_to_json() {
        let layout = PdfLayout::two_up();
        let json = layout_to_json(&layout).expect("JSON変換失敗");

        assert!(json.contains("pageWidthMm"));
        assert!(json.contains("photosPerPage"));
        assert!(json.contains("2")); // photos_per_page = 2
    }

    #[test]
    fn test_js_photo_entry_serialize() {
        let entry = JsPhotoEntry {
            file_name: "test.jpg".to_string(),
            file_path: "/path/test.jpg".to_string(),
            image_data_url: Some("data:image/jpeg;base64,aaa".to_string()),
            date: "2025-01-18".to_string(),
            work_type: "舗装工".to_string(),
            variety: "表層工".to_string(),
            subphase: "".to_string(),
            station: "No.10".to_string(),
            remarks: "".to_string(),
            description: "".to_string(),
            measurements: "".to_string(),
            photo_category: "施工状況写真".to_string(),
            has_board: false,
            detected_text: "".to_string(),
        };

        let json = serde_json::to_string(&entry).expect("シリアライズ失敗");

        // camelCase変換の確認
        assert!(json.contains("\"fileName\":"));
        assert!(json.contains("\"filePath\":"));
        assert!(json.contains("\"imageDataUrl\":"));
        assert!(json.contains("\"workType\":"));
        assert!(json.contains("\"photoCategory\":"));
        assert!(json.contains("\"hasBoard\":"));
        assert!(json.contains("\"detectedText\":"));
    }
}

#[cfg(all(target_arch = "wasm32", test))]
mod wasm_tests {
    use super::*;
    use photo_ai_common::AnalysisResult;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn wasm_photos_to_json_includes_image_data_url() {
        let results = vec![AnalysisResult {
            file_name: "photo1.jpg".to_string(),
            file_path: "data:image/jpeg;base64,aaaa".to_string(),
            ..Default::default()
        }];

        let json = photos_to_json(&results).expect("JSON conversion failed");
        assert!(json.contains("\"imageDataUrl\":"));
    }
}
