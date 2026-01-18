use photo_ai_common::parse_step1_response;
use serde_json::json;

const GEMINI_API_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash-exp:generateContent";

#[tokio::test]
async fn gemini_step1_integration() {
    let api_key = match std::env::var("GEMINI_API_KEY") {
        Ok(key) if !key.trim().is_empty() => key,
        _ => {
            eprintln!("GEMINI_API_KEY not set; skipping integration test");
            return;
        }
    };

    let prompt = r#"Return ONLY a JSON array exactly in this format:
[
  {
    "fileName": "integration-test.jpg",
    "hasBoard": false,
    "detectedText": "",
    "measurements": "",
    "sceneDescription": "integration test",
    "photoCategory": "その他"
  }
]
"#;

    let body = json!({
        "contents": [
            { "parts": [ { "text": prompt } ] }
        ],
        "generationConfig": {
            "temperature": 0.1,
            "responseMimeType": "application/json"
        }
    });

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}?key={}", GEMINI_API_URL, api_key))
        .json(&body)
        .send()
        .await
        .expect("request failed");

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        panic!("gemini api failed with status {}: {}", status, text);
    }

    let payload: serde_json::Value = response.json().await.expect("invalid json response");
    let text = payload["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .expect("response text missing");

    let results = parse_step1_response(text).expect("failed to parse step1 response");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].file_name, "integration-test.jpg");
}
