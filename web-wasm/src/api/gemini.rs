//! Gemini API連携（2段階解析対応）
//!
//! Step1: 画像認識（build_step1_prompt, parse_step1_response）
//! Step2: マスタ照合（build_step2_prompt, parse_step2_response）

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};
use serde::{Deserialize, Serialize};
use photo_ai_common::{
    RawImageData, Step2Result, AnalysisResult, HierarchyMaster,
    build_step1_prompt, build_step2_prompt,
    parse_step1_response, parse_step2_response,
    detect_work_types, merge_results, ImageMeta,
    extract_json,
};

const GEMINI_API_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash-exp:generateContent";

/// Gemini APIリクエスト
#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<Content>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
}

#[derive(Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum Part {
    Text { text: String },
    InlineData { inline_data: InlineData },
}

#[derive(Serialize)]
struct InlineData {
    mime_type: String,
    data: String,
}

#[derive(Serialize)]
struct GenerationConfig {
    temperature: f32,
    #[serde(rename = "responseMimeType")]
    response_mime_type: String,
}

/// Gemini APIレスポンス
#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<Candidate>,
}

#[derive(Deserialize)]
struct Candidate {
    content: ResponseContent,
}

#[derive(Deserialize)]
struct ResponseContent {
    parts: Vec<ResponsePart>,
}

#[derive(Deserialize)]
struct ResponsePart {
    text: String,
}

/// Data URLからBase64データ部分を抽出
///
/// # Arguments
/// * `data_url` - "data:image/jpeg;base64,/9j/4AAQ..." 形式のData URL
///
/// # Returns
/// Base64エンコードされたデータ部分、または抽出失敗時はNone
pub fn extract_base64_from_data_url(data_url: &str) -> Option<&str> {
    data_url.split(',').nth(1)
}

/// Data URLからMIMEタイプを抽出
///
/// # Arguments
/// * `data_url` - "data:image/jpeg;base64,..." 形式のData URL
///
/// # Returns
/// MIMEタイプ（例: "image/jpeg"）、抽出失敗時は"image/jpeg"をデフォルトとして返す
pub fn extract_mime_type_from_data_url(data_url: &str) -> &str {
    data_url
        .split(':')
        .nth(1)
        .and_then(|s| s.split(';').next())
        .unwrap_or("image/jpeg")
}

/// Gemini API呼び出し（共通処理）
async fn call_gemini_api(api_key: &str, request: &GeminiRequest) -> Result<String, JsValue> {
    let url = format!("{}?key={}", GEMINI_API_URL, api_key);
    let body = serde_json::to_string(request)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let mut opts = RequestInit::new();
    opts.method("POST");
    opts.mode(RequestMode::Cors);
    opts.body(Some(&JsValue::from_str(&body)));

    let request = Request::new_with_str_and_init(&url, &opts)?;
    request.headers().set("Content-Type", "application/json")?;

    let window = web_sys::window().unwrap();
    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
    let resp: Response = resp_value.dyn_into()?;

    if !resp.ok() {
        return Err(JsValue::from_str(&format!("API error: {}", resp.status())));
    }

    let json = JsFuture::from(resp.json()?).await?;
    let response: GeminiResponse = serde_wasm_bindgen::from_value(json)?;

    response
        .candidates
        .first()
        .and_then(|c| c.content.parts.first())
        .map(|p| p.text.clone())
        .ok_or_else(|| JsValue::from_str("Empty response"))
}

/// Step1実行（画像認識）
///
/// 画像を受け取りGemini APIへ送信し、RawImageDataを返す
///
/// # Arguments
/// * `api_key` - Gemini API key
/// * `images` - (ファイル名, 日付Option, DataURL)のベクター
///
/// # Returns
/// Vec<RawImageData>
pub async fn analyze_step1(
    api_key: &str,
    images: &[(String, Option<String>, String)],  // (file_name, date, data_url)
) -> Result<Vec<RawImageData>, JsValue> {
    if images.is_empty() {
        return Ok(vec![]);
    }

    // プロンプト生成用のメタデータ
    let image_meta: Vec<(&str, Option<&str>)> = images
        .iter()
        .map(|(name, date, _)| (name.as_str(), date.as_deref()))
        .collect();

    let prompt = build_step1_prompt(&image_meta);

    // リクエスト作成（画像付き）
    let mut parts: Vec<Part> = vec![Part::Text { text: prompt }];

    for (_, _, data_url) in images {
        if let Some(base64_data) = extract_base64_from_data_url(data_url) {
            let mime_type = extract_mime_type_from_data_url(data_url);
            parts.push(Part::InlineData {
                inline_data: InlineData {
                    mime_type: mime_type.to_string(),
                    data: base64_data.to_string(),
                },
            });
        }
    }

    let request = GeminiRequest {
        contents: vec![Content { parts }],
        generation_config: GenerationConfig {
            temperature: 0.1,
            response_mime_type: "application/json".to_string(),
        },
    };

    let response_text = call_gemini_api(api_key, &request).await?;

    parse_step1_response(&response_text)
        .map_err(|e| JsValue::from_str(&format!("Step1 parse error: {}", e)))
}

/// Step2実行（マスタ照合）
///
/// RawImageDataとマスタを受け取り、Step2Resultを返す
/// 画像は不要（テキストのみでAI照合）
///
/// # Arguments
/// * `api_key` - Gemini API key
/// * `raw_data` - Step1の出力
/// * `master` - 階層マスタ
///
/// # Returns
/// Vec<Step2Result>
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
///
/// # Arguments
/// * `api_key` - Gemini API key
/// * `images` - (ファイル名, 日付Option, DataURL)のベクター
/// * `master` - 階層マスタ
/// * `on_progress` - 進捗コールバック (current, total, message)
///
/// # Returns
/// Vec<AnalysisResult>
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
            file_path: String::new(),  // WASMでは空文字
            date: date.clone().unwrap_or_default(),
        })
        .collect();

    // 結果マージ
    let results = merge_results(&raw_data, &step2_results, &image_metas);

    Ok(results)
}

/// 写真を解析（後方互換性のため維持）
pub async fn analyze_photo(
    api_key: &str,
    image_data: &str,  // Base64 data URL
    file_name: &str,
) -> Result<AnalysisResult, JsValue> {
    // Data URLからBase64部分を抽出
    let base64_data = extract_base64_from_data_url(image_data)
        .ok_or_else(|| JsValue::from_str("Invalid data URL"))?;

    // MIMEタイプを取得
    let mime_type = extract_mime_type_from_data_url(image_data);

    // プロンプト
    let prompt = r#"この工事写真を分析し、以下のJSON形式で情報を抽出してください。

{
  "workType": "工種（舗装工、道路土工など）",
  "variety": "種別（舗装打換え工など）",
  "detail": "細別（表層工、上層路盤工など）",
  "photoCategory": "写真区分（施工状況写真、品質管理写真、出来形管理写真、安全管理写真、使用材料写真など）",
  "station": "測点（黒板から読み取れる場合）",
  "remarks": "備考（補足情報）",
  "description": "写真説明（作業内容の簡潔な説明）",
  "hasBoard": true/false（黒板の有無）,
  "detectedText": "黒板から読み取ったテキスト",
  "measurements": "測定値・数値データ",
  "reasoning": "分類の理由"
}

黒板がある場合は、黒板の内容を優先して情報を抽出してください。"#;

    // リクエスト作成
    let request = GeminiRequest {
        contents: vec![Content {
            parts: vec![
                Part::Text { text: prompt.to_string() },
                Part::InlineData {
                    inline_data: InlineData {
                        mime_type: mime_type.to_string(),
                        data: base64_data.to_string(),
                    },
                },
            ],
        }],
        generation_config: GenerationConfig {
            temperature: 0.1,
            response_mime_type: "application/json".to_string(),
        },
    };

    // fetch API呼び出し
    let url = format!("{}?key={}", GEMINI_API_URL, api_key);
    let body = serde_json::to_string(&request)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let mut opts = RequestInit::new();
    opts.method("POST");
    opts.mode(RequestMode::Cors);
    opts.body(Some(&JsValue::from_str(&body)));

    let request = Request::new_with_str_and_init(&url, &opts)?;
    request.headers().set("Content-Type", "application/json")?;

    let window = web_sys::window().unwrap();
    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
    let resp: Response = resp_value.dyn_into()?;

    if !resp.ok() {
        return Err(JsValue::from_str(&format!("API error: {}", resp.status())));
    }

    let json = JsFuture::from(resp.json()?).await?;
    let response: GeminiResponse = serde_wasm_bindgen::from_value(json)?;

    // レスポンスをパース
    let text = response
        .candidates
        .first()
        .and_then(|c| c.content.parts.first())
        .map(|p| p.text.clone())
        .ok_or_else(|| JsValue::from_str("Empty response"))?;

    parse_analysis_result(&text, file_name)
        .map_err(|e| JsValue::from_str(&e))
}

fn parse_analysis_result(response_text: &str, fallback_file_name: &str) -> Result<AnalysisResult, String> {
    let json_str = extract_json(response_text).unwrap_or(response_text);
    let value: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| format!("JSON parse error: {}", e))?;

    let obj = if let Some(arr) = value.as_array() {
        arr.first().cloned().unwrap_or_else(|| serde_json::Value::Null)
    } else {
        value
    };

    let Some(map) = obj.as_object() else {
        return Err("JSON object not found".to_string());
    };

    Ok(AnalysisResult {
        file_name: get_string(map, "fileName").unwrap_or_else(|| fallback_file_name.to_string()),
        work_type: get_string(map, "workType").unwrap_or_default(),
        variety: get_string(map, "variety").unwrap_or_default(),
        detail: get_string(map, "detail").unwrap_or_default(),
        photo_category: get_string(map, "photoCategory").unwrap_or_default(),
        station: get_string(map, "station").unwrap_or_default(),
        remarks: get_string(map, "remarks").unwrap_or_default(),
        description: get_string(map, "description").unwrap_or_default(),
        has_board: get_bool(map, "hasBoard").unwrap_or(false),
        detected_text: get_string(map, "detectedText").unwrap_or_default(),
        measurements: get_string(map, "measurements").unwrap_or_default(),
        reasoning: get_string(map, "reasoning").unwrap_or_default(),
        ..Default::default()
    })
}

fn get_string(map: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<String> {
    let value = map.get(key)?;
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }
    if value.is_null() {
        return None;
    }
    Some(value.to_string())
}

fn get_bool(map: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<bool> {
    let value = map.get(key)?;
    if let Some(b) = value.as_bool() {
        return Some(b);
    }
    if let Some(s) = value.as_str() {
        return Some(matches!(s.to_lowercase().as_str(), "true" | "1" | "yes"));
    }
    None
}

/// バッチ解析（5枚単位）
pub async fn analyze_batch(
    api_key: &str,
    photos: Vec<(String, String, String)>,  // (id, data_url, file_name)
    on_progress: impl Fn(usize, usize),
) -> Vec<(String, Result<AnalysisResult, String>)> {
    let total = photos.len();
    let mut results = Vec::new();

    for (i, (id, data_url, file_name)) in photos.into_iter().enumerate() {
        on_progress(i + 1, total);

        let result = analyze_photo(api_key, &data_url, &file_name)
            .await
            .map_err(|e| format!("{:?}", e));

        results.push((id, result));
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    // =============================================
    // Data URL抽出テスト
    // =============================================

    #[test]
    fn test_extract_base64_from_data_url_jpeg() {
        let data_url = "data:image/jpeg;base64,/9j/4AAQSkZJRg==";
        let result = extract_base64_from_data_url(data_url);
        assert_eq!(result, Some("/9j/4AAQSkZJRg=="));
    }

    #[test]
    fn test_extract_base64_from_data_url_png() {
        let data_url = "data:image/png;base64,iVBORw0KGgo=";
        let result = extract_base64_from_data_url(data_url);
        assert_eq!(result, Some("iVBORw0KGgo="));
    }

    #[test]
    fn test_extract_base64_from_data_url_invalid() {
        let invalid_url = "not a data url";
        let result = extract_base64_from_data_url(invalid_url);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_base64_from_data_url_empty() {
        let empty_url = "";
        let result = extract_base64_from_data_url(empty_url);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_mime_type_jpeg() {
        let data_url = "data:image/jpeg;base64,/9j/4AAQ";
        let result = extract_mime_type_from_data_url(data_url);
        assert_eq!(result, "image/jpeg");
    }

    #[test]
    fn test_extract_mime_type_png() {
        let data_url = "data:image/png;base64,iVBORw0KGgo=";
        let result = extract_mime_type_from_data_url(data_url);
        assert_eq!(result, "image/png");
    }

    #[test]
    fn test_extract_mime_type_webp() {
        let data_url = "data:image/webp;base64,UklGR";
        let result = extract_mime_type_from_data_url(data_url);
        assert_eq!(result, "image/webp");
    }

    #[test]
    fn test_extract_mime_type_default() {
        // 不正なフォーマットの場合はデフォルト値を返す
        let invalid_url = "invalid";
        let result = extract_mime_type_from_data_url(invalid_url);
        assert_eq!(result, "image/jpeg");
    }

    // =============================================
    // Gemini リクエスト/レスポンス シリアライズテスト
    // =============================================

    #[test]
    fn test_gemini_request_serialize() {
        let request = GeminiRequest {
            contents: vec![Content {
                parts: vec![
                    Part::Text { text: "テストプロンプト".to_string() },
                ],
            }],
            generation_config: GenerationConfig {
                temperature: 0.1,
                response_mime_type: "application/json".to_string(),
            },
        };

        let json = serde_json::to_string(&request).expect("シリアライズ失敗");
        assert!(json.contains("\"contents\""));
        assert!(json.contains("\"generationConfig\""));
        assert!(json.contains("\"temperature\":0.1"));
        assert!(json.contains("\"responseMimeType\":\"application/json\""));
    }

    #[test]
    fn test_part_text_serialize() {
        let part = Part::Text { text: "Hello".to_string() };
        let json = serde_json::to_string(&part).expect("シリアライズ失敗");
        assert_eq!(json, r#"{"text":"Hello"}"#);
    }

    #[test]
    fn test_part_inline_data_serialize() {
        let part = Part::InlineData {
            inline_data: InlineData {
                mime_type: "image/jpeg".to_string(),
                data: "base64data".to_string(),
            },
        };
        let json = serde_json::to_string(&part).expect("シリアライズ失敗");
        assert!(json.contains("\"inline_data\""));
        assert!(json.contains("\"mime_type\":\"image/jpeg\""));
        assert!(json.contains("\"data\":\"base64data\""));
    }

    #[test]
    fn test_gemini_response_deserialize() {
        let json = r#"{
            "candidates": [{
                "content": {
                    "parts": [{
                        "text": "{\"workType\": \"舗装工\"}"
                    }]
                }
            }]
        }"#;

        let response: GeminiResponse = serde_json::from_str(json).expect("デシリアライズ失敗");
        assert_eq!(response.candidates.len(), 1);
        assert_eq!(response.candidates[0].content.parts.len(), 1);
        assert!(response.candidates[0].content.parts[0].text.contains("舗装工"));
    }

    #[test]
    fn test_generation_config_serialize() {
        let config = GenerationConfig {
            temperature: 0.5,
            response_mime_type: "text/plain".to_string(),
        };

        let json = serde_json::to_string(&config).expect("シリアライズ失敗");
        assert!(json.contains("\"temperature\":0.5"));
        assert!(json.contains("\"responseMimeType\":\"text/plain\""));
    }
}
