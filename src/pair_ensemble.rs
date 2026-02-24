//! AIアンサンブルペアリングモジュール
//!
//! コンタクトシートを使った1コール1問×3走査のアンサンブルでペアリングを行う。

use crate::contactsheet::ContactSheet;
use crate::error::{PhotoAiError, Result};
use regex::Regex;
use std::collections::HashMap;

// --- confidence値 ---
const CONFIDENCE_FINAL_ONLY: f64 = 0.67;
const CONFIDENCE_FALLBACK: f64 = 0.33;

/// 1つのBefore画像について、3回走査のアンサンブルでAfter番号を決定
pub fn ensemble_pair_query(
    before_sheet: &ContactSheet,
    after_sheet: &ContactSheet,
    query: &str,
    before_max: u32,
    after_max: u32,
    verbose: bool,
) -> Result<(u32, f64)> {
    let prompt = format!(
        "傷んだ舗装をなおしている道路工事の着手前竣工写真のペアリングである。\n\n\
         Image 1 is a numbered grid of BEFORE-construction road photos (B01-B{before_max:02}).\n\
         Image 2 is a numbered grid of AFTER-construction road photos (A01-A{after_max:02}).\n\n\
         Which AFTER number (A01-A{after_max:02}) shows the SAME road location as {query}?\n\
         Match by: vanishing point direction, building silhouettes, road width, surrounding structures.\n\n\
         Do 3 independent scans:\n\
         Scan 1: Go A01 to A{after_max:02} in order, pick the best match.\n\
         Scan 2: Go A{after_max:02} to A01 in reverse, pick the best match.\n\
         Scan 3: Go A01 to A{after_max:02} again, pick the best match.\n\n\
         Output format:\n\
         Scan1: A??\n\
         Scan2: A??\n\
         Scan3: A??\n\
         Final: A?? (majority vote)"
    );

    let files = vec![
        before_sheet.image_path.clone(),
        after_sheet.image_path.clone(),
    ];
    let options = cli_ai_analyzer::AnalyzeOptions::default();
    let response = cli_ai_analyzer::analyze(&prompt, &files, options)
        .map_err(|e| PhotoAiError::ApiCall(format!("ペアリングAIコールエラー: {}", e)))?;

    if verbose {
        eprintln!("  [{}] Response: {}", query, &response[..response.len().min(200)]);
    }

    parse_ensemble_response(&response, after_max)
}

/// アンサンブル回答をパース
///
/// "Final: A07" を最優先で取得。なければScan1-3から多数決。
pub(crate) fn parse_ensemble_response(response: &str, after_max: u32) -> Result<(u32, f64)> {
    let re = Regex::new(r"(?i)A\s*(\d+)").unwrap();

    // Final行を探す
    let final_re = Regex::new(r"(?i)Final\s*:\s*A\s*(\d+)").unwrap();
    let final_num = final_re
        .captures(response)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<u32>().ok())
        .filter(|&n| n >= 1 && n <= after_max);

    // Scan1-3を抽出
    let scan_re = Regex::new(r"(?i)Scan\s*\d\s*:\s*A\s*(\d+)").unwrap();
    let scans: Vec<u32> = scan_re
        .captures_iter(response)
        .filter_map(|c| c.get(1))
        .filter_map(|m| m.as_str().parse::<u32>().ok())
        .filter(|&n| n >= 1 && n <= after_max)
        .collect();

    if let Some(num) = final_num {
        // Finalがある場合: Scan結果との一致度でconfidence算出
        let agree_count = scans.iter().filter(|&&s| s == num).count();
        let confidence = if scans.is_empty() {
            CONFIDENCE_FINAL_ONLY // Finalのみ、Scanなし
        } else {
            agree_count as f64 / scans.len() as f64
        };
        return Ok((num, confidence));
    }

    // Finalがない場合: Scan結果から多数決
    if !scans.is_empty() {
        let (winner, count) = majority_vote(&scans);
        let confidence = count as f64 / scans.len() as f64;
        return Ok((winner, confidence));
    }

    // それでもダメならレスポンス全体から最初のA番号を探す
    if let Some(caps) = re.captures(response) {
        if let Some(m) = caps.get(1) {
            if let Ok(n) = m.as_str().parse::<u32>() {
                if n >= 1 && n <= after_max {
                    return Ok((n, CONFIDENCE_FALLBACK));
                }
            }
        }
    }

    Err(PhotoAiError::ApiParse(format!(
        "ペアリング回答パース失敗: {}",
        &response[..response.len().min(300)]
    )))
}

/// 多数決: 最も多い値とその出現回数を返す
fn majority_vote(nums: &[u32]) -> (u32, usize) {
    let mut counts: HashMap<u32, usize> = HashMap::new();
    for &n in nums {
        *counts.entry(n).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .unwrap_or((0, 0))
}
