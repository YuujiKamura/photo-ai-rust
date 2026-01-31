//! APIレスポンスパーサー
//!
//! Claude CLIなどのAPIレスポンスからJSONを抽出し、
//! Step1/Step2の結果をパースする

use crate::error::{Error, Result};
use crate::types::{AnalysisResult, RawImageData};

/// APIレスポンスからJSON部分を抽出
///
/// 抽出優先順位:
/// 1. ```json ... ``` ブロック
/// 2. 生の [...] 配列
/// 3. エラー
///
/// # Arguments
/// * `response` - APIレスポンス文字列
///
/// # Returns
/// * `Ok(&str)` - 抽出されたJSON文字列
/// * `Err` - JSONが見つからない場合
///
/// # Examples
/// ```
/// use photo_ai_common::extract_json;
///
/// // JSON配列形式
/// let response = "[{\"key\": \"value\"}]";
/// let json = extract_json(response).unwrap();
/// assert!(json.contains("key"));
/// ```
pub fn extract_json(response: &str) -> Result<&str> {
    // ```json ... ``` ブロックを探す
    if let Some(start_marker) = response.find("```json") {
        let start = start_marker + 7; // "```json" の長さ
        if let Some(end_offset) = response[start..].find("```") {
            let end = start + end_offset;
            return Ok(response[start..end].trim());
        }
    }

    // 生の [...] を探す
    if let Some(start) = response.find('[') {
        if let Some(end) = response.rfind(']') {
            if end >= start {
                return Ok(&response[start..=end]);
            }
        }
    }

    Err(Error::Parse("JSONが見つかりません".into()))
}

/// Step1レスポンスをパース
///
/// 画像認識の結果（RawImageData配列）をパースする
///
/// # Arguments
/// * `response` - Step1のAPIレスポンス
///
/// # Returns
/// * `Ok(Vec<RawImageData>)` - パース成功
/// * `Err` - JSONが見つからないかパース失敗
pub fn parse_step1_response(response: &str) -> Result<Vec<RawImageData>> {
    let json_str = extract_json(response)?;
    let raw: Vec<RawImageData> = serde_json::from_str(json_str.trim())
        .map_err(|e| Error::Parse(format!("Step1 JSONパースエラー: {}", e)))?;
    Ok(raw)
}

/// 1ステップ解析レスポンスをパース
///
/// 工種指定時の1回のAI呼び出しで得られる結果をパースする
/// レスポンスは直接AnalysisResult形式
///
/// # Arguments
/// * `response` - 1ステップ解析のAPIレスポンス
///
/// # Returns
/// * `Ok(Vec<AnalysisResult>)` - パース成功
/// * `Err` - JSONが見つからないかパース失敗
pub fn parse_single_step_response(response: &str) -> Result<Vec<AnalysisResult>> {
    let json_str = extract_json(response)?;
    let results: Vec<AnalysisResult> = serde_json::from_str(json_str.trim())
        .map_err(|e| Error::Parse(format!("1ステップ解析 JSONパースエラー: {}", e)))?;
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    // =============================================
    // extract_json テスト
    // =============================================

    #[test]
    fn test_extract_json_with_block() {
        let response = r#"Here is the analysis:
```json
[
  {"fileName": "test.jpg", "hasBoard": true}
]
```
Some additional text."#;

        let json = extract_json(response).unwrap();
        assert!(json.contains("fileName"));
        assert!(json.contains("test.jpg"));
    }

    #[test]
    fn test_extract_json_raw() {
        let response = r#"[{"fileName": "photo1.jpg", "hasBoard": false}]"#;

        let json = extract_json(response).unwrap();
        assert_eq!(json, r#"[{"fileName": "photo1.jpg", "hasBoard": false}]"#);
    }

    #[test]
    fn test_extract_json_with_surrounding_text() {
        let response = r#"Here is the result: [{"key": "value"}] and some more text."#;

        let json = extract_json(response).unwrap();
        assert_eq!(json, r#"[{"key": "value"}]"#);
    }

    #[test]
    fn test_extract_json_error() {
        let response = "No JSON here, just plain text.";

        let result = extract_json(response);
        assert!(result.is_err());
        if let Err(Error::Parse(msg)) = result {
            assert!(msg.contains("JSONが見つかりません"));
        } else {
            panic!("Expected Parse error");
        }
    }

    #[test]
    fn test_extract_json_empty_response() {
        let response = "";

        let result = extract_json(response);
        assert!(result.is_err());
    }

    // =============================================
    // parse_step1_response テスト
    // =============================================

    #[test]
    fn test_parse_step1_response() {
        let response = r#"Here is the analysis:
```json
[
  {
    "fileName": "test.jpg",
    "hasBoard": true,
    "detectedText": "温度 160.4℃",
    "measurements": "160.4℃",
    "sceneDescription": "アスファルト舗装工事",
    "photoCategory": "品質管理"
  }
]
```
"#;

        let result = parse_step1_response(response).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_name, "test.jpg");
        assert!(result[0].has_board);
        assert_eq!(result[0].detected_text, "温度 160.4℃");
        assert_eq!(result[0].measurements, "160.4℃");
        assert_eq!(result[0].scene_description, "アスファルト舗装工事");
        assert_eq!(result[0].photo_category, "品質管理");
    }

    #[test]
    fn test_parse_step1_response_raw_json() {
        let response = r#"[{"fileName": "photo1.jpg", "hasBoard": false, "sceneDescription": "道路工事"}]"#;

        let result = parse_step1_response(response).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_name, "photo1.jpg");
        assert!(!result[0].has_board);
        assert_eq!(result[0].scene_description, "道路工事");
        assert_eq!(result[0].detected_text, ""); // デフォルト値
    }

    #[test]
    fn test_parse_step1_response_multiple() {
        let response = r#"```json
[
  {"fileName": "img1.jpg", "hasBoard": true, "photoCategory": "到着温度"},
  {"fileName": "img2.jpg", "hasBoard": false, "photoCategory": "舗設状況"}
]
```"#;

        let result = parse_step1_response(response).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].file_name, "img1.jpg");
        assert_eq!(result[1].file_name, "img2.jpg");
    }

    #[test]
    fn test_parse_step1_response_error() {
        let response = "No JSON here";

        let result = parse_step1_response(response);
        assert!(result.is_err());
    }

    // =============================================
    // エッジケーステスト
    // =============================================

    #[test]
    fn test_extract_json_nested_brackets() {
        let response = r#"[{"data": [1, 2, 3], "nested": {"key": "value"}}]"#;

        let json = extract_json(response).unwrap();
        assert!(json.contains("data"));
        assert!(json.contains("nested"));
    }

    #[test]
    fn test_extract_json_with_newlines_in_block() {
        let response = "```json\n[\n  {\n    \"key\": \"value\"\n  }\n]\n```";

        let json = extract_json(response).unwrap();
        assert!(json.contains("key"));
    }
}
