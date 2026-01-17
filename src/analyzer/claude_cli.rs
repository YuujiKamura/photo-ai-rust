use crate::error::{PhotoAiError, Result};
use crate::scanner::ImageInfo;
use super::types::AnalysisResult;
use std::path::PathBuf;
use std::process::Command;

const STEP1_PROMPT: &str = r#"Output ONLY a JSON array. No markdown, no explanation, no text before or after.

Required JSON format:
[{"fileName":"photo-1.jpg","hasBoard":false,"detectedText":"","measurements":"","sceneDescription":"description here","photoCategory":"その他"}]

Categories: 着工前/完成/施工状況/安全管理/使用材料/品質管理/出来形管理/段階確認/材料検収/その他

IMPORTANT: Output raw JSON array only. Do not use markdown code blocks."#;

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

    let full_prompt = format!(
        "Read the following image files and analyze them: {}\n\n{}\n\n対象写真:\n{}",
        image_list, STEP1_PROMPT, photo_list
    );

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

fn copy_to_temp(images: &[ImageInfo], temp_dir: &PathBuf) -> Result<Vec<PathBuf>> {
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

    // AnalysisResultに変換
    let results = raw
        .into_iter()
        .map(|r| AnalysisResult {
            file_name: r.file_name,
            has_board: r.has_board,
            detected_text: r.detected_text,
            measurements: r.measurements,
            description: r.scene_description,
            photo_category: r.photo_category,
            ..Default::default()
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
