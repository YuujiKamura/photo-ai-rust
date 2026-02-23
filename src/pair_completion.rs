//! 着手前・竣工写真 自動ペアリングモジュール

use crate::commands::PairCompletionCommandArgs;
use crate::error::{PhotoAiError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// --- データ型 ---

#[derive(Serialize, Deserialize, Debug)]
pub struct PairResult {
    pub before_page: u32,
    pub before_station: String,
    pub after_file: String,
    pub confidence: f64,
}

struct ExtractedPage {
    page_num: u32,
    image_path: PathBuf,
    station_text: String,
}

#[derive(Deserialize, Debug, Clone)]
struct VisualFeatures {
    #[serde(alias = "fileName")]
    #[allow(dead_code)]
    file_name: String,
    background: String,
    #[serde(alias = "laneMarkings")]
    lane_markings: String,
    #[serde(alias = "cameraAngle")]
    #[allow(dead_code)]
    camera_angle: String,
    #[serde(alias = "sceneSummary")]
    scene_summary: String,
}

#[derive(Deserialize, Debug)]
struct PairCompareResult {
    #[serde(alias = "bestMatch")]
    best_match: String,
    confidence: f64,
    #[allow(dead_code)]
    rankings: Vec<RankingEntry>,
}

#[derive(Deserialize, Debug)]
struct RankingEntry {
    #[serde(alias = "fileName")]
    #[allow(dead_code)]
    file_name: String,
    #[allow(dead_code)]
    score: f64,
}

// --- メインハンドラ ---

pub async fn handle_pair_completion(args: PairCompletionCommandArgs) -> Result<()> {
    let verbose = args.cli_args.verbose;
    println!("🔗 photo-ai-rust - 着手前/竣工 ペアリング\n");

    // Phase 0: 入力準備
    println!("[1/5] PDF画像抽出中...");
    let before_pages = extract_images_from_pdf(&args.before)?;
    println!("  着手前: {}ページ", before_pages.len());

    let after_files = scan_after_folder(&args.after)?;
    println!("  竣工写真: {}枚", after_files.len());

    if before_pages.is_empty() {
        return Err(PhotoAiError::PdfExtraction("PDFから画像を抽出できませんでした".into()));
    }
    if after_files.is_empty() {
        return Err(PhotoAiError::NoImagesFound(args.after.display().to_string()));
    }

    // Phase 1: 竣工写真の特徴抽出
    println!("[2/5] 竣工写真の特徴抽出中...");
    let after_features = extract_features_batch(&after_files, verbose)?;
    println!("  特徴抽出完了: {}枚", after_features.len());

    // Phase 2: 着手前写真の特徴抽出
    println!("[3/5] 着手前写真の特徴抽出中...");
    let before_image_paths: Vec<PathBuf> = before_pages.iter().map(|p| p.image_path.clone()).collect();
    let before_features = extract_features_batch(&before_image_paths, verbose)?;
    println!("  特徴抽出完了: {}枚", before_features.len());

    // Phase 3: AIペア比較（全候補画像+特徴テキストをGeminiに渡す）
    println!("[4/5] AIペア比較中...");
    let mut results = Vec::new();

    for (i, page) in before_pages.iter().enumerate() {
        let before_feat = before_features.get(i);

        // 着手前1枚 + 竣工全枚の画像と特徴をGeminiに渡す
        let pair_result = compare_pair_with_ai(
            &page.image_path,
            &after_files,
            before_feat,
            &after_features,
            verbose,
        )?;

        if verbose {
            println!("  Page {} → {} (confidence: {:.2})", page.page_num, pair_result.best_match, pair_result.confidence);
        }

        results.push(PairResult {
            before_page: page.page_num,
            before_station: page.station_text.clone(),
            after_file: pair_result.best_match,
            confidence: pair_result.confidence,
        });
    }

    // Phase 4: 順序制約による confidence 調整
    apply_order_constraint(&mut results, &after_files);

    // 出力
    println!("[5/5] 結果を保存中...");
    let output_path = args.output.unwrap_or_else(|| PathBuf::from("pairing.json"));
    let json = serde_json::to_string_pretty(&results)?;
    std::fs::write(&output_path, &json)?;
    println!("✔ 保存: {} ({}件のペア)", output_path.display(), results.len());

    // サマリー表示
    let high_conf = results.iter().filter(|r| r.confidence >= 0.8).count();
    let low_conf = results.iter().filter(|r| r.confidence < 0.5).count();
    println!("\n📊 結果サマリー:");
    println!("  高信頼度 (≥0.8): {}件", high_conf);
    println!("  低信頼度 (<0.5): {}件 ← 要確認", low_conf);
    println!("\n✅ ペアリング完了");

    // temp画像を削除
    cleanup_temp_images(&before_pages);

    Ok(())
}

// --- Phase 0: PDF画像抽出 ---

fn extract_images_from_pdf(pdf_path: &Path) -> Result<Vec<ExtractedPage>> {
    // PDFの存在確認
    if !pdf_path.exists() {
        return Err(PhotoAiError::FileNotFound(pdf_path.display().to_string()));
    }

    let doc = lopdf::Document::load(pdf_path)
        .map_err(|e| PhotoAiError::PdfExtraction(format!("PDF読み込み失敗: {}", e)))?;

    let temp_dir = std::env::temp_dir().join(format!("photo-ai-pair-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| PhotoAiError::PdfExtraction(format!("tempディレクトリ作成失敗: {}", e)))?;

    let pages = doc.get_pages();
    let mut extracted = Vec::new();

    // ページ番号順にソート
    let mut sorted_pages: Vec<_> = pages.into_iter().collect();
    sorted_pages.sort_by_key(|(num, _)| *num);

    for (page_num, page_id) in &sorted_pages {
        // 画像抽出を試みる
        if let Some(image_data) = extract_first_image_from_page(&doc, *page_id) {
            let image_path = temp_dir.join(format!("page_{:02}.jpg", page_num));
            if let Err(e) = std::fs::write(&image_path, &image_data) {
                eprintln!("  Warning: page {} 画像保存失敗: {}", page_num, e);
                continue;
            }

            // テキスト抽出（測点名）
            let station_text = extract_page_text(pdf_path, *page_num);

            extracted.push(ExtractedPage {
                page_num: *page_num,
                image_path,
                station_text,
            });
        }
    }

    Ok(extracted)
}

/// ページ内の最初のJPEG画像（DCTDecode）を抽出
fn extract_first_image_from_page(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> Option<Vec<u8>> {
    // ページオブジェクトからResourcesを取得
    let page_obj = doc.get_object(page_id).ok()?;
    let page_dict = page_obj.as_dict().ok()?;

    let resources = match page_dict.get(b"Resources") {
        Ok(r) => doc.dereference(r).ok().map(|(_, o)| o)?,
        Err(_) => return None,
    };
    let resources_dict = resources.as_dict().ok()?;

    let xobject = match resources_dict.get(b"XObject") {
        Ok(x) => doc.dereference(x).ok().map(|(_, o)| o)?,
        Err(_) => return None,
    };
    let xobject_dict = xobject.as_dict().ok()?;

    // XObject辞書内で最大の画像を探す
    let mut best_image: Option<Vec<u8>> = None;
    let mut best_size: usize = 0;

    for (_name, obj_ref) in xobject_dict.iter() {
        let Ok((_, obj)) = doc.dereference(obj_ref) else {
            continue;
        };
        let Ok(stream) = obj.as_stream() else {
            continue;
        };
        let dict = &stream.dict;

        // Subtypeが Image か確認
        if let Ok(subtype) = dict.get(b"Subtype") {
            let subtype_obj = doc
                .dereference(subtype)
                .map(|(_, o)| o)
                .unwrap_or(subtype);
            if let Ok(name) = subtype_obj.as_name_str() {
                if name != "Image" {
                    continue;
                }
            }
        }

        // Filter が DCTDecode (= JPEG) か確認
        let Ok(filter) = dict.get(b"Filter") else {
            continue;
        };
        let filter_obj = doc
            .dereference(filter)
            .map(|(_, o)| o)
            .unwrap_or(filter);
        let is_dct = match filter_obj.as_name_str() {
            Ok(name) => name == "DCTDecode",
            Err(_) => {
                if let Ok(arr) = filter_obj.as_array() {
                    arr.iter().any(|f| {
                        doc.dereference(f)
                            .ok()
                            .and_then(|(_, o)| o.as_name_str().ok().map(|n| n == "DCTDecode"))
                            .unwrap_or(false)
                    })
                } else {
                    false
                }
            }
        };

        if is_dct {
            let data = stream.content.clone();
            if data.len() > best_size {
                best_size = data.len();
                best_image = Some(data);
            }
        }
    }

    best_image
}

/// ページテキストを抽出（測点名など）
fn extract_page_text(pdf_path: &Path, page_num: u32) -> String {
    let options = pdf_analysis_embed::ExtractOptions::new().with_pages(vec![page_num]);
    pdf_analysis_embed::extract_text_with_options(pdf_path, &options)
        .unwrap_or_default()
        .trim()
        .to_string()
}

// --- Phase 0: 竣工写真スキャン ---

fn scan_after_folder(folder: &Path) -> Result<Vec<PathBuf>> {
    if !folder.exists() || !folder.is_dir() {
        return Err(PhotoAiError::FolderNotFound(folder.display().to_string()));
    }

    let mut files: Vec<PathBuf> = std::fs::read_dir(folder)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| matches!(ext.to_lowercase().as_str(), "jpg" | "jpeg" | "png"))
                .unwrap_or(false)
        })
        .collect();

    files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(files)
}

// --- Phase 1 & 2: 特徴抽出 ---

const FEATURE_BATCH_SIZE: usize = 5;

const FEATURE_PROMPT: &str = "\
You are matching before/after construction photos of the same road location. \
Extract visual features that are INVARIANT between before and after. \
For each image, output JSON array: \
[{\"fileName\":\"...\",\"background\":\"permanent structures: buildings, bridges, poles, signs\",\
\"laneMarkings\":\"crosswalks, arrows, diamonds, stop lines\",\
\"cameraAngle\":\"facing direction, vanishing point, road curvature\",\
\"sceneSummary\":\"50-word location description\"}] \
IGNORE: road surface condition, line color freshness, pavement color/texture. \
Output ONLY the JSON array.";

fn extract_features_batch(image_paths: &[PathBuf], _verbose: bool) -> Result<Vec<VisualFeatures>> {
    let mut all_features = Vec::new();

    for chunk in image_paths.chunks(FEATURE_BATCH_SIZE) {
        let options = cli_ai_analyzer::AnalyzeOptions::default();
        let response = cli_ai_analyzer::analyze(FEATURE_PROMPT, chunk, options)
            .map_err(|e| PhotoAiError::ApiCall(format!("特徴抽出エラー: {}", e)))?;
        let parsed = parse_features_response(&response)?;
        all_features.extend(parsed);
    }

    Ok(all_features)
}

fn parse_features_response(response: &str) -> Result<Vec<VisualFeatures>> {
    // JSON配列を抽出（レスポンスにmarkdownコードフェンスがある場合に対応）
    let json_str = extract_json_array(response);
    serde_json::from_str::<Vec<VisualFeatures>>(&json_str)
        .map_err(|e| PhotoAiError::ApiParse(format!("特徴抽出JSONパースエラー: {}\nレスポンス: {}", e, &response[..response.len().min(500)])))
}

/// レスポンスからJSON配列を抽出
fn extract_json_array(text: &str) -> String {
    // ```json ... ``` ブロックを探す
    if let Some(start) = text.find("```json") {
        let after = &text[start + 7..];
        if let Some(end) = after.find("```") {
            return after[..end].trim().to_string();
        }
    }
    if let Some(start) = text.find("```") {
        let after = &text[start + 3..];
        if let Some(end) = after.find("```") {
            return after[..end].trim().to_string();
        }
    }
    // [ で始まる部分を探す
    if let Some(start) = text.find('[') {
        if let Some(end) = text.rfind(']') {
            return text[start..=end].to_string();
        }
    }
    text.trim().to_string()
}

/// レスポンスからJSONオブジェクトを抽出
fn extract_json_object(text: &str) -> String {
    if let Some(start) = text.find("```json") {
        let after = &text[start + 7..];
        if let Some(end) = after.find("```") {
            return after[..end].trim().to_string();
        }
    }
    if let Some(start) = text.find("```") {
        let after = &text[start + 3..];
        if let Some(end) = after.find("```") {
            return after[..end].trim().to_string();
        }
    }
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            return text[start..=end].to_string();
        }
    }
    text.trim().to_string()
}

// --- Phase 3: AIペア比較（全候補画像+特徴テキスト） ---

fn compare_pair_with_ai(
    before_image: &Path,
    candidate_paths: &[PathBuf],
    before_feat: Option<&VisualFeatures>,
    after_features: &[VisualFeatures],
    _verbose: bool,
) -> Result<PairCompareResult> {
    // 着手前1枚 + 候補全枚
    let mut all_images = vec![before_image.to_path_buf()];
    all_images.extend_from_slice(candidate_paths);

    let candidate_names: Vec<String> = candidate_paths.iter()
        .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
        .collect();

    // 特徴テキストをプロンプトに埋め込む
    let before_feat_text = before_feat
        .map(|f| format!(
            "BEFORE features: background={}, laneMarkings={}, sceneSummary={}",
            f.background, f.lane_markings, f.scene_summary
        ))
        .unwrap_or_default();

    let after_feat_text: String = after_features.iter()
        .map(|f| format!(
            "  {}: background={}, laneMarkings={}, sceneSummary={}",
            f.file_name, f.background, f.lane_markings, f.scene_summary
        ))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "The first image is a BEFORE-construction photo. The remaining {} images are AFTER candidates: [{}].\n\
         {}\n\
         AFTER features:\n{}\n\n\
         Which after-photo was taken at the SAME LOCATION? \
         Use BOTH the images AND the feature text to decide. \
         Compare: (1) background structures (buildings, poles, signs), (2) lane markings, (3) camera angle and vanishing point. \
         IMPORTANT: Each before page should ideally match a DIFFERENT after photo. \
         Output JSON: {{\"bestMatch\":\"filename\",\"confidence\":0.0-1.0,\"rankings\":[{{\"fileName\":\"...\",\"score\":0.85}}]}} \
         Output ONLY the JSON object.",
        candidate_paths.len(),
        candidate_names.join(", "),
        before_feat_text,
        after_feat_text,
    );

    let options = cli_ai_analyzer::AnalyzeOptions::default();
    let response = cli_ai_analyzer::analyze(&prompt, &all_images, options)
        .map_err(|e| PhotoAiError::ApiCall(format!("ペア比較エラー: {}", e)))?;
    let json_str = extract_json_object(&response);

    serde_json::from_str::<PairCompareResult>(&json_str)
        .map_err(|e| PhotoAiError::ApiParse(format!(
            "ペア比較JSONパースエラー: {}\nレスポンス: {}", e, &response[..response.len().min(500)]
        )))
}

// --- Phase 4: 順序制約 ---

fn apply_order_constraint(results: &mut [PairResult], after_files: &[PathBuf]) {
    if results.len() < 3 {
        return; // 少数では順序制約は不要
    }

    // 竣工写真のファイル名→インデックスマップ
    let file_index: std::collections::HashMap<String, usize> = after_files.iter()
        .enumerate()
        .filter_map(|(i, p)| p.file_name().map(|n| (n.to_string_lossy().to_string(), i)))
        .collect();

    let indices: Vec<Option<usize>> = results.iter()
        .map(|r| file_index.get(&r.after_file).copied())
        .collect();

    // 単調増加チェック
    let increasing_violations = count_monotonicity_violations(&indices, true);
    let decreasing_violations = count_monotonicity_violations(&indices, false);

    // 違反が少ない方を採用
    let violations = if increasing_violations <= decreasing_violations {
        find_violating_positions(&indices, true)
    } else {
        find_violating_positions(&indices, false)
    };

    for pos in violations {
        if pos < results.len() {
            results[pos].confidence = (results[pos].confidence - 0.1).max(0.0);
        }
    }
}

fn count_monotonicity_violations(indices: &[Option<usize>], ascending: bool) -> usize {
    let valid: Vec<usize> = indices.iter().filter_map(|x| *x).collect();
    if valid.len() < 2 { return 0; }
    valid.windows(2).filter(|w| {
        if ascending { w[0] >= w[1] } else { w[0] <= w[1] }
    }).count()
}

fn find_violating_positions(indices: &[Option<usize>], ascending: bool) -> Vec<usize> {
    let mut violations = Vec::new();
    let mut prev: Option<usize> = None;
    for (pos, idx) in indices.iter().enumerate() {
        if let Some(cur) = idx {
            if let Some(p) = prev {
                let violated = if ascending { p >= *cur } else { p <= *cur };
                if violated {
                    violations.push(pos);
                }
            }
            prev = Some(*cur);
        }
    }
    violations
}

// --- ユーティリティ ---

fn cleanup_temp_images(pages: &[ExtractedPage]) {
    if let Some(first) = pages.first() {
        if let Some(parent) = first.image_path.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
    }
}
