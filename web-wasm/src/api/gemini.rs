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

/// 写真を解析
pub async fn analyze_photo(
    api_key: &str,
    image_data: &str,  // Base64 data URL
    file_name: &str,
) -> Result<AnalysisResult, JsValue> {
    // Data URLからBase64部分を抽出
    let base64_data = image_data
        .split(',')
        .nth(1)
        .ok_or_else(|| JsValue::from_str("Invalid data URL"))?;

    // MIMEタイプを取得
    let mime_type = image_data
        .split(':')
        .nth(1)
        .and_then(|s| s.split(';').next())
        .unwrap_or("image/jpeg");

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
