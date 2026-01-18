//! Claude CLI連携モジュール
//!
//! 2段階解析処理:
//! - Step1 (Vision): 画像から生データを抽出（OCR、数値、シーン説明）
//! - Step2 (Text): 階層マスタとの照合で分類

use crate::error::{PhotoAiError, Result};
use crate::scanner::ImageInfo;
use super::types::AnalysisResult;
use photo_ai_common::HierarchyMaster;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

/// Step1の出力: 画像から抽出した生データ
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RawImageData {
    pub file_name: String,
    pub has_board: bool,
    pub detected_text: String,
    pub measurements: String,
    pub scene_description: String,
    pub photo_category: String,
}

/// 写真区分（フォトカテゴリ）
const PHOTO_CATEGORIES: &[&str] = &[
    // 品質管理 - 温度測定
    "到着温度", "敷均し温度", "初期締固め前温度", "開放温度",
    "アスファルト混合物温度測定",
    // 品質管理 - 密度測定
    "現場密度測定",
    // 施工状況
    "転圧状況", "敷均し状況", "舗設状況", "初期転圧状況", "2次転圧状況",
    "乳剤散布状況", "端部乳剤塗布状況", "養生砂散布状況", "清掃状況",
    "掘削状況", "積込状況", "取壊し状況", "据付状況", "設置状況",
    // 着手前・完成
    "着手前", "完了", "竣工", "施工完了", "既済部分",
    // 出来形管理
    "不陸整正出来形", "路盤厚出来形", "表層厚出来形", "幅員出来形",
    // 安全管理
    "朝礼実施状況", "朝礼・KYミーティング実施状況", "朝礼状況",
    "KY活動状況", "危険予知活動状況", "KYミーティング実施状況",
    "新規入場者教育状況", "新規入場者教育実施状況",
    "保安施設設置状況", "点灯確認状況", "安全巡視状況",
    "安全訓練実施状況", "避難訓練実施状況",
    // その他
    "その他"
];

/// Step1プロンプト（画像認識）
fn build_step1_prompt(images: &[ImageInfo]) -> String {
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

    let categories = PHOTO_CATEGORIES.join(", ");

    format!(
        r#"あなたは工事写真帳を作成する現場監督です。複数の写真を同時に解析し、一貫性のある分類を行ってください。

## 写真区分（フォトカテゴリ）
以下から最も適切なものを選択：
{categories}

## 出力形式（厳密にこのJSON配列形式で出力）
```json
[
  {{
    "fileName": "ファイル名",
    "hasBoard": true/false,
    "detectedText": "黒板・看板から読み取った全テキスト",
    "measurements": "数値データ（温度、寸法、密度等）単位付き",
    "sceneDescription": "写真に写っているものの客観的な説明",
    "photoCategory": "写真区分から選択"
  }}
]
```

## 注意
- 黒板のテキストは正確にOCR
- 数値は単位も含めて正確に（例: "160.4℃", "厚さ50mm"）
- 同じ場所・同じ作業の写真は一貫した分類を
- 推測せず、見えるものだけを記載
- JSON配列のみ出力。説明文は不要

対象写真:
{photo_list}"#
    )
}

/// Step2プロンプト（マスタ照合）
fn build_step2_prompt(raw_data: &[RawImageData], master: &HierarchyMaster) -> String {
    let hierarchy_json = master.to_hierarchy_json();
    let hierarchy_str = serde_json::to_string(&hierarchy_json).unwrap_or_default();

    let raw_data_str = raw_data
        .iter()
        .map(|d| {
            format!(
                r#"
ファイル: {}
黒板: {}
OCRテキスト: {}
数値: {}
シーン: {}
写真区分: {}"#,
                d.file_name,
                if d.has_board { "あり" } else { "なし" },
                if d.detected_text.is_empty() { "なし" } else { &d.detected_text },
                if d.measurements.is_empty() { "なし" } else { &d.measurements },
                d.scene_description,
                d.photo_category
            )
        })
        .collect::<Vec<_>>()
        .join("\n---\n");

    format!(
        r#"あなたは工事写真の分類専門家です。
以下の画像解析結果を、工種マスタに基づいて正確に分類してください。

## 工種マスタ（階層構造）
{hierarchy_str}

## 画像解析結果
{raw_data_str}

## 出力ルール
1. workType, variety, detail は必ずマスタに存在する値を選択
2. マスタにない値は絶対に使用しない
3. 該当なしの場合は空文字""

## 出力形式（JSON配列）
```json
[
  {{
    "fileName": "ファイル名",
    "workType": "工種（マスタから選択）",
    "variety": "種別（マスタから選択）",
    "detail": "細別（マスタから選択）",
    "remarks": "備考",
    "station": "測点（黒板から読み取れた場合）",
    "description": "写真の説明",
    "reasoning": "分類理由"
  }}
]
```

出力はJSON配列のみ。説明不要。"#
    )
}

/// Step1: 画像認識を実行
pub async fn analyze_batch_step1(images: &[ImageInfo], verbose: bool) -> Result<Vec<RawImageData>> {
    // 画像をtemp-imagesにコピー
    let temp_dir = get_temp_dir()?;
    let local_paths = copy_to_temp(images, &temp_dir)?;

    // プロンプト構築
    let image_list = local_paths
        .iter()
        .map(|p| p.display().to_string().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join(", ");

    let step1_prompt = build_step1_prompt(images);

    // プロンプト構築（改行をスペースに置換してcmd経由で渡す）
    let raw_prompt = format!(
        "Read the following image files and analyze them: {}\n\n{}",
        image_list, step1_prompt
    );
    let full_prompt = raw_prompt.replace('\n', " ").replace('"', "\\\"");

    if verbose {
        println!("  [Step1] プロンプト長: {} chars", full_prompt.len());
    }

    // Claude CLI呼び出し
    let response = run_claude_cli(&full_prompt, verbose)?;

    if verbose {
        println!("  [Step1] レスポンス長: {} chars", response.len());
    }

    // JSONパース
    parse_step1_response(&response)
}

/// Step2: マスタ照合を実行
pub async fn analyze_batch_step2(
    raw_data: &[RawImageData],
    master: &HierarchyMaster,
    verbose: bool,
) -> Result<Vec<Step2Result>> {
    let step2_prompt = build_step2_prompt(raw_data, master);
    let full_prompt = step2_prompt.replace('\n', " ").replace('"', "\\\"");

    if verbose {
        println!("  [Step2] プロンプト長: {} chars", full_prompt.len());
    }

    // Claude CLI呼び出し（画像なし）
    let response = run_claude_cli(&full_prompt, verbose)?;

    if verbose {
        println!("  [Step2] レスポンス長: {} chars", response.len());
    }

    // JSONパース
    parse_step2_response(&response)
}

/// Step1とStep2の結果をマージ
pub fn merge_results(
    raw_data: &[RawImageData],
    step2_results: &[Step2Result],
    images: &[ImageInfo],
) -> Vec<AnalysisResult> {
    // file_nameからImageInfoを取得するためのマップ
    let info_map: std::collections::HashMap<&str, &ImageInfo> = images
        .iter()
        .map(|img| (img.file_name.as_str(), img))
        .collect();

    // file_nameからStep2結果を取得するためのマップ
    let step2_map: std::collections::HashMap<&str, &Step2Result> = step2_results
        .iter()
        .map(|r| (r.file_name.as_str(), r))
        .collect();

    raw_data
        .iter()
        .map(|raw| {
            let img_info = info_map.get(raw.file_name.as_str());
            let step2 = step2_map.get(raw.file_name.as_str());

            let file_path = img_info
                .map(|i| i.path.display().to_string())
                .unwrap_or_default();
            let date = img_info
                .and_then(|i| i.date.clone())
                .unwrap_or_default();

            AnalysisResult {
                file_name: raw.file_name.clone(),
                file_path,
                date,
                has_board: raw.has_board,
                detected_text: raw.detected_text.clone(),
                measurements: raw.measurements.clone(),
                description: step2.map(|s| s.description.clone()).unwrap_or_else(|| raw.scene_description.clone()),
                photo_category: raw.photo_category.clone(),
                work_type: step2.map(|s| s.work_type.clone()).unwrap_or_default(),
                variety: step2.map(|s| s.variety.clone()).unwrap_or_default(),
                detail: step2.map(|s| s.detail.clone()).unwrap_or_default(),
                station: step2.map(|s| s.station.clone()).unwrap_or_default(),
                remarks: step2.map(|s| s.remarks.clone()).unwrap_or_default(),
                reasoning: step2.map(|s| s.reasoning.clone()).unwrap_or_default(),
            }
        })
        .collect()
}

/// 2段階解析を実行（後方互換性のため維持）
pub async fn analyze_batch(images: &[ImageInfo], verbose: bool) -> Result<Vec<AnalysisResult>> {
    // Step1のみ実行（マスタなし）
    let raw_data = analyze_batch_step1(images, verbose).await?;

    // マスタなしの場合はStep1結果をそのまま変換
    let info_map: std::collections::HashMap<&str, &ImageInfo> = images
        .iter()
        .map(|img| (img.file_name.as_str(), img))
        .collect();

    let results = raw_data
        .iter()
        .map(|raw| {
            let img_info = info_map.get(raw.file_name.as_str());
            let file_path = img_info
                .map(|i| i.path.display().to_string())
                .unwrap_or_default();
            let date = img_info
                .and_then(|i| i.date.clone())
                .unwrap_or_default();

            AnalysisResult {
                file_name: raw.file_name.clone(),
                file_path,
                date,
                has_board: raw.has_board,
                detected_text: raw.detected_text.clone(),
                measurements: raw.measurements.clone(),
                description: raw.scene_description.clone(),
                photo_category: raw.photo_category.clone(),
                ..Default::default()
            }
        })
        .collect();

    Ok(results)
}

/// 2段階解析を実行（マスタあり）
pub async fn analyze_batch_with_master(
    images: &[ImageInfo],
    master: &HierarchyMaster,
    verbose: bool,
) -> Result<Vec<AnalysisResult>> {
    // Step1: 画像認識
    if verbose {
        println!("  Step1: 画像認識開始...");
    }
    let raw_data = analyze_batch_step1(images, verbose).await?;
    if verbose {
        println!("  Step1: 完了 ({}件)", raw_data.len());
    }

    // Step1結果から工種を自動判定してマスタをフィルタ
    let detected_types = detect_work_types(&raw_data);
    let filtered_master = if detected_types.is_empty() {
        if verbose {
            println!("  工種判定: 該当なし → 全マスタ使用 ({}件)", master.rows().len());
        }
        master.clone()
    } else {
        let filtered = master.filter_by_work_types(&detected_types);
        if verbose {
            println!("  工種判定: {:?} → マスタ絞込み ({}件 → {}件)",
                detected_types, master.rows().len(), filtered.rows().len());
        }
        filtered
    };

    // Step2: マスタ照合（フィルタ済みマスタを使用）
    if verbose {
        println!("  Step2: マスタ照合開始...");
    }
    let step2_results = analyze_batch_step2(&raw_data, &filtered_master, verbose).await?;
    if verbose {
        println!("  Step2: 完了 ({}件)", step2_results.len());
    }

    // 結果マージ
    let results = merge_results(&raw_data, &step2_results, images);
    Ok(results)
}

/// Step1結果から工種を自動判定
pub fn detect_work_types(raw_data: &[RawImageData]) -> Vec<String> {
    use std::collections::HashSet;
    let mut types = HashSet::new();

    for r in raw_data {
        let cat = r.photo_category.as_str();
        let text = r.detected_text.as_str();
        let scene = r.scene_description.as_str();

        // 舗装工の判定
        if cat.contains("温度") || cat.contains("転圧") || cat.contains("舗設")
            || cat.contains("敷均し") || cat.contains("乳剤") || cat.contains("路盤")
            || text.contains("舗装") || text.contains("表層") || text.contains("基層")
            || scene.contains("アスファルト") || scene.contains("フィニッシャー")
            || scene.contains("ローラー")
        {
            types.insert("舗装工".to_string());
        }

        // 区画線工の判定
        if cat.contains("区画線") || text.contains("区画線") || text.contains("ライン")
            || scene.contains("白線") || scene.contains("区画線")
        {
            types.insert("区画線工".to_string());
        }

        // 構造物撤去工の判定
        if cat.contains("取壊し") || text.contains("撤去") || text.contains("取壊")
            || scene.contains("解体") || scene.contains("撤去")
        {
            types.insert("構造物撤去工".to_string());
        }

        // 道路土工の判定
        if cat.contains("掘削") || cat.contains("路床") || text.contains("掘削")
            || scene.contains("掘削") || scene.contains("バックホウ")
        {
            types.insert("道路土工".to_string());
        }

        // 排水構造物工の判定
        if text.contains("側溝") || text.contains("集水") || text.contains("人孔")
            || scene.contains("側溝") || scene.contains("マンホール")
        {
            types.insert("排水構造物工".to_string());
        }

        // 人孔改良工の判定
        if text.contains("人孔改良") || text.contains("マンホール蓋")
        {
            types.insert("人孔改良工".to_string());
        }
    }

    types.into_iter().collect()
}

/// Step2の結果
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Step2Result {
    pub file_name: String,
    #[serde(default)]
    pub work_type: String,
    #[serde(default)]
    pub variety: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub remarks: String,
    #[serde(default)]
    pub station: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub reasoning: String,
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

fn run_claude_cli(prompt: &str, verbose: bool) -> Result<String> {
    // Claude CLI呼び出し（Windowsではcmd /c経由）
    #[cfg(windows)]
    let output = Command::new("cmd")
        .args(["/c", "claude", "-p", prompt, "--output-format", "text"])
        .output()
        .map_err(|e| PhotoAiError::ApiCall(format!("Claude CLI実行エラー: {}", e)))?;

    #[cfg(not(windows))]
    let output = Command::new("claude")
        .args(["-p", prompt, "--output-format", "text"])
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
        let preview: String = response.chars().take(500).collect();
        println!("  レスポンス: {}", preview);
    }

    Ok(response)
}

fn parse_step1_response(response: &str) -> Result<Vec<RawImageData>> {
    let json_str = extract_json(response)?;
    let raw: Vec<RawImageData> = serde_json::from_str(json_str.trim())
        .map_err(|e| PhotoAiError::ApiParse(format!("Step1 JSONパースエラー: {}", e)))?;
    Ok(raw)
}

fn parse_step2_response(response: &str) -> Result<Vec<Step2Result>> {
    let json_str = extract_json(response)?;
    let results: Vec<Step2Result> = serde_json::from_str(json_str.trim())
        .map_err(|e| PhotoAiError::ApiParse(format!("Step2 JSONパースエラー: {}", e)))?;
    Ok(results)
}

fn extract_json(response: &str) -> Result<&str> {
    // JSONブロックを抽出
    if let Some(caps) = response.find("```json") {
        let start = caps + 7;
        let end = response[start..].find("```").map(|e| start + e).unwrap_or(response.len());
        return Ok(&response[start..end]);
    }

    if let Some(start) = response.find('[') {
        let end = response.rfind(']').map(|e| e + 1).unwrap_or(response.len());
        return Ok(&response[start..end]);
    }

    Err(PhotoAiError::ApiParse("JSONが見つかりません".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_step1_response_with_json_block() {
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
        assert_eq!(result[0].photo_category, "品質管理");
    }

    #[test]
    fn test_parse_step1_response_raw_json() {
        let response = r#"[{"fileName": "photo1.jpg", "hasBoard": false, "sceneDescription": "道路工事"}]"#;
        let result = parse_step1_response(response).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_name, "photo1.jpg");
        assert!(!result[0].has_board);
    }

    #[test]
    fn test_parse_step2_response() {
        let response = r#"```json
[
  {
    "fileName": "test.jpg",
    "workType": "舗装工",
    "variety": "舗装打換え工",
    "detail": "表層工",
    "station": "No.10",
    "description": "舗設状況"
  }
]
```"#;
        let result = parse_step2_response(response).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].work_type, "舗装工");
        assert_eq!(result[0].variety, "舗装打換え工");
        assert_eq!(result[0].detail, "表層工");
    }

    #[test]
    fn test_build_step1_prompt() {
        let images = vec![ImageInfo {
            path: PathBuf::from("test.jpg"),
            file_name: "test.jpg".to_string(),
            date: Some("2025-01-18".to_string()),
        }];
        let prompt = build_step1_prompt(&images);
        assert!(prompt.contains("test.jpg"));
        assert!(prompt.contains("到着温度"));
        assert!(prompt.contains("JSON配列のみ出力"));
    }
}
