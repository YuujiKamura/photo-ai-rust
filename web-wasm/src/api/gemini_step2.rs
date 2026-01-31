//! Gemini API連携（Step2/2段階解析）

use wasm_bindgen::prelude::*;

use crate::api::gemini::{
    analyze_step1, call_gemini_api, Content, GeminiRequest, GenerationConfig, Part,
};
use photo_ai_common::{
    RawImageData, Step2Result, AnalysisResult, HierarchyMaster,
    build_step2_prompt, parse_step2_response,
    detect_work_types, merge_results, ImageMeta,
};

/// Step2実行（マスタ照合）
///
/// RawImageDataとマスタを受け取り、Step2Resultを返す
/// 画像は不要（テキストのみでAI照合）
pub async fn analyze_step2(
    api_key: &str,
    raw_data: &[RawImageData],
    master: &HierarchyMaster,
) -> Result<Vec<Step2Result>, JsValue> {
    if raw_data.is_empty() {
        return Ok(vec![]);
    }

    let prompt = build_step2_prompt(raw_data, master);

    // リクエスト作成（テキストのみ、画像なし）
    let request = GeminiRequest {
        contents: vec![Content {
            parts: vec![Part::Text { text: prompt }],
        }],
        generation_config: GenerationConfig {
            temperature: 0.1,
            response_mime_type: "application/json".to_string(),
        },
    };

    let response_text = call_gemini_api(api_key, &request).await?;

    parse_step2_response(&response_text)
        .map_err(|e| JsValue::from_str(&format!("Step2 parse error: {}", e)))
}

/// 2段階解析（マスタあり）
///
/// Step1実行 -> 工種自動判定 -> マスタ絞込み -> Step2実行 -> 結果マージ
pub async fn analyze_with_master(
    api_key: &str,
    images: Vec<(String, Option<String>, String)>,  // (file_name, date, data_url)
    master: &HierarchyMaster,
    on_progress: impl Fn(usize, usize, &str),
) -> Result<Vec<AnalysisResult>, JsValue> {
    let total = 3; // Step1, マスタ絞込み, Step2

    // Step1: 画像認識
    on_progress(1, total, "Step1: 画像認識中...");
    let raw_data = analyze_step1(api_key, &images).await?;

    // 工種自動判定
    on_progress(2, total, "工種を自動判定中...");
    let detected_work_types = detect_work_types(&raw_data);

    // マスタ絞込み
    let filtered_master = master.filter_by_work_types(&detected_work_types);

    // Step2: マスタ照合
    on_progress(3, total, "Step2: マスタ照合中...");
    let step2_results = analyze_step2(api_key, &raw_data, &filtered_master).await?;

    // ImageMeta作成（WASMではfile_pathは空文字でOK）
    let image_metas: Vec<ImageMeta> = images
        .iter()
        .map(|(file_name, date, _)| ImageMeta {
            file_name: file_name.clone(),
            file_path: String::new(),
            date: date.clone().unwrap_or_default(),
        })
        .collect();

    // 結果マージ
    let results = merge_results(&raw_data, &step2_results, &image_metas);

    Ok(results)
}
