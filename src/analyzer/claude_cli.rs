use crate::error::{PhotoAiError, Result};
use crate::scanner::ImageInfo;
use super::types::AnalysisResult;
use std::path::PathBuf;
use std::process::Command;

const STEP1_PROMPT: &str = r#"あなたは工事写真帳を作成する現場監督です。複数の写真を同時に解析し、一貫性のある分類を行ってください。

## 写真区分（フォトカテゴリ）
以下から最も適切なものを選択：
着工前, 完成, 施工状況, 安全管理, 使用材料, 品質管理, 出来形管理, 段階確認, 材料検収, その他

## 出力形式（厳密にこのJSON配列形式で出力）
```json
[
  {
    "fileName": "ファイル名",
    "hasBoard": true/false,
    "detectedText": "黒板・看板から読み取った全テキスト",
    "measurements": "数値データ（温度、寸法、密度等）単位付き",
    "sceneDescription": "写真に写っているものの客観的な説明",
    "photoCategory": "写真区分から選択"
  }
]
```

## 注意
- 黒板のテキストは正確にOCR
- 数値は単位も含めて正確に（例: "160.4℃", "厚さ50mm"）
- 同じ場所・同じ作業の写真は一貫した分類を
- 推測せず、見えるものだけを記載
- JSON配列のみ出力。説明文は不要"#;

pub async fn analyze_batch(images: &[ImageInfo], verbose: bool) -> Result<Vec<AnalysisResult>> {
    // 画像をtemp-imagesにコピー
    let temp_dir = get_temp_dir()?;
    let local_paths = copy_to_temp(images, &temp_dir)?;

    // プロンプト構築
    let image_list = local_paths
        .iter()
        .map(|p| p.display().to_string().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join(", ");

    let photo_list = images
        .iter()
        .map(|img| {
            format!(
                "- {} (撮影: {})",
                img.file_name,
                img.date.as_deref().unwrap_or("unknown")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // プロンプト構築（改行をスペースに置換してcmd経由で渡す）
    let raw_prompt = format!(
        "Read the following image files and analyze them: {}\n\n{}\n\n対象写真:\n{}",
        image_list, STEP1_PROMPT, photo_list
    );
    let full_prompt = raw_prompt.replace('\n', " ").replace('"', "\\\"");

    if verbose {
        println!("  プロンプト長: {} chars", full_prompt.len());
    }

    // Claude CLI呼び出し（Windowsではcmd /c経由）
    #[cfg(windows)]
    let output = Command::new("cmd")
        .args(["/c", "claude", "-p", &full_prompt, "--output-format", "text"])
        .output()
        .map_err(|e| PhotoAiError::ApiCall(format!("Claude CLI実行エラー: {}", e)))?;

    #[cfg(not(windows))]
    let output = Command::new("claude")
        .args(["-p", &full_prompt, "--output-format", "text"])
        .output()
        .map_err(|e| PhotoAiError::ApiCall(format!("Claude CLI実行エラー: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PhotoAiError::ApiCall(format!(
            "Claude CLI failed (code {:?}): {}",
            output.status.code(),
            stderr
        )));
    }

    let response = String::from_utf8_lossy(&output.stdout).to_string();

    if verbose {
        println!("  レスポンス長: {} chars", response.len());
        let preview: String = response.chars().take(500).collect();
        println!("  レスポンス: {}", preview);
    }

    // JSONパース
    parse_response(&response, images)
}

fn get_temp_dir() -> Result<PathBuf> {
    let temp_dir = std::env::current_dir()?.join("temp-images");
    std::fs::create_dir_all(&temp_dir)?;
    Ok(temp_dir)
}

fn copy_to_temp(images: &[ImageInfo], temp_dir: &std::path::Path) -> Result<Vec<PathBuf>> {
    let mut local_paths = Vec::new();

    for img in images {
        let dest = temp_dir.join(&img.file_name);
        std::fs::copy(&img.path, &dest)?;
        // 絶対パスに変換
        let abs_path = std::fs::canonicalize(&dest)?;
        local_paths.push(abs_path);
    }

    Ok(local_paths)
}

fn parse_response(response: &str, images: &[ImageInfo]) -> Result<Vec<AnalysisResult>> {
    // JSONブロックを抽出
    let json_str = if let Some(caps) = response.find("```json") {
        let start = caps + 7;
        let end = response[start..].find("```").map(|e| start + e).unwrap_or(response.len());
        &response[start..end]
    } else if let Some(start) = response.find('[') {
        let end = response.rfind(']').map(|e| e + 1).unwrap_or(response.len());
        &response[start..end]
    } else {
        return Err(PhotoAiError::ApiParse("JSONが見つかりません".into()));
    };

    // パース
    let raw: Vec<RawResult> = serde_json::from_str(json_str.trim())
        .map_err(|e| PhotoAiError::ApiParse(format!("JSONパースエラー: {}", e)))?;

    // file_nameからfile_pathを取得するためのマップ
    let path_map: std::collections::HashMap<&str, &std::path::Path> = images
        .iter()
        .map(|img| (img.file_name.as_str(), img.path.as_path()))
        .collect();

    // AnalysisResultに変換
    let results = raw
        .into_iter()
        .map(|r| {
            let file_path = path_map
                .get(r.file_name.as_str())
                .map(|p| p.display().to_string())
                .unwrap_or_default();

            AnalysisResult {
                file_name: r.file_name,
                file_path,
                has_board: r.has_board,
                detected_text: r.detected_text,
                measurements: r.measurements,
                description: r.scene_description,
                photo_category: r.photo_category,
                ..Default::default()
            }
        })
        .collect();

    Ok(results)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawResult {
    file_name: String,
    #[serde(default)]
    has_board: bool,
    #[serde(default)]
    detected_text: String,
    #[serde(default)]
    measurements: String,
    #[serde(default)]
    scene_description: String,
    #[serde(default)]
    photo_category: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_response_with_json_block() {
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
        let result = parse_response(response, &[]).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_name, "test.jpg");
        assert!(result[0].has_board);
        assert_eq!(result[0].detected_text, "温度 160.4℃");
        assert_eq!(result[0].photo_category, "品質管理");
    }

    #[test]
    fn test_parse_response_raw_json() {
        let response = r#"[{"fileName": "photo1.jpg", "hasBoard": false, "sceneDescription": "道路工事"}]"#;
        let result = parse_response(response, &[]).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_name, "photo1.jpg");
        assert!(!result[0].has_board);
    }

    #[test]
    fn test_parse_response_multiple_photos() {
        let response = r#"```json
[
  {"fileName": "a.jpg", "photoCategory": "施工状況"},
  {"fileName": "b.jpg", "photoCategory": "品質管理"},
  {"fileName": "c.jpg", "photoCategory": "安全管理"}
]
```"#;
        let result = parse_response(response, &[]).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].photo_category, "施工状況");
        assert_eq!(result[1].photo_category, "品質管理");
        assert_eq!(result[2].photo_category, "安全管理");
    }

    #[test]
    fn test_parse_response_no_json() {
        let response = "This is not valid JSON";
        let result = parse_response(response, &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_response_with_default_values() {
        let response = r#"[{"fileName": "minimal.jpg"}]"#;
        let result = parse_response(response, &[]).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_name, "minimal.jpg");
        assert!(!result[0].has_board);
        assert_eq!(result[0].detected_text, "");
        assert_eq!(result[0].measurements, "");
    }
}
