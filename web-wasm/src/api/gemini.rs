//! Gemini API連携

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};
use serde::{Deserialize, Serialize};
use photo_ai_common::AnalysisResult;

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

/// 写真を解析
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

    let mut result: AnalysisResult = serde_json::from_str(&text)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    result.file_name = file_name.to_string();

    Ok(result)
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
