//! 対話式測点入力モジュール
//!
//! ## 変更履歴
//! - 2026-01-18: 初期作成（Epic #21, Task #24-29）

use crate::analyzer::AnalysisResult;
use crate::error::{PhotoAiError, Result};
use dialoguer::Input;
use std::collections::HashSet;
use std::path::Path;

/// 測点が空の写真を抽出
pub fn extract_empty_station_photos(results: &[AnalysisResult]) -> Vec<usize> {
    results
        .iter()
        .enumerate()
        .filter(|(_, r)| r.station.trim().is_empty())
        .map(|(i, _)| i)
        .collect()
}

/// 既存の測点リストを収集（重複除去）
pub fn collect_existing_stations(results: &[AnalysisResult]) -> Vec<String> {
    let mut seen = HashSet::new();
    results
        .iter()
        .filter(|r| !r.station.trim().is_empty())
        .filter_map(|r| {
            let s = r.station.trim().to_string();
            if seen.insert(s.clone()) {
                Some(s)
            } else {
                None
            }
        })
        .collect()
}

/// 対話アクション
pub enum StationAction {
    /// 測点を入力
    Input(String),
    /// この写真をスキップ
    Skip,
    /// 残り全部スキップ
    SkipAll,
    /// 前と同じ測点を適用
    Repeat,
    /// 残り全部に前と同じ測点を適用
    RepeatAll,
    /// 保存して終了
    Quit,
}

/// 対話式で測点を入力
pub fn run_interactive_station(
    input_path: &Path,
    output_path: Option<&Path>,
) -> Result<()> {
    // JSONファイル読み込み
    let content = std::fs::read_to_string(input_path)?;
    let mut results: Vec<AnalysisResult> = serde_json::from_str(&content)?;

    // 測点が空の写真を抽出
    let empty_indices = extract_empty_station_photos(&results);

    if empty_indices.is_empty() {
        println!("✓ すべての写真に測点が設定されています");
        return Ok(());
    }

    println!("📍 測点が未設定の写真: {}枚", empty_indices.len());
    println!("---");
    println!("操作: [Enter]入力 [s]スキップ [S]残り全スキップ [r]前と同じ [R]残り全部同じ [q]終了");
    println!("---\n");

    // 既存測点を候補リストに
    let existing_stations = collect_existing_stations(&results);
    let mut prev_station: Option<String> = None;
    let mut skip_all = false;
    let mut repeat_all = false;

    for (count, &idx) in empty_indices.iter().enumerate() {
        if skip_all {
            continue;
        }

        let result = &results[idx];
        println!(
            "[{}/{}] {} ({})",
            count + 1,
            empty_indices.len(),
            result.file_name,
            result.photo_category
        );

        if repeat_all {
            if let Some(ref station) = prev_station {
                results[idx].station = station.clone();
                println!("  → {} (自動適用)\n", station);
                continue;
            }
        }

        // 操作選択
        let action = prompt_station_action(&existing_stations, prev_station.as_deref())?;

        match action {
            StationAction::Input(station) => {
                results[idx].station = station.clone();
                prev_station = Some(station.clone());
                println!("  → {}\n", station);
            }
            StationAction::Skip => {
                println!("  → スキップ\n");
            }
            StationAction::SkipAll => {
                println!("  → 残り全部スキップ\n");
                skip_all = true;
            }
            StationAction::Repeat => {
                if let Some(ref station) = prev_station {
                    results[idx].station = station.clone();
                    println!("  → {} (前と同じ)\n", station);
                } else {
                    println!("  → 前の測点がありません、スキップ\n");
                }
            }
            StationAction::RepeatAll => {
                if let Some(ref station) = prev_station {
                    results[idx].station = station.clone();
                    println!("  → {} (残り全部に適用)\n", station);
                    repeat_all = true;
                } else {
                    println!("  → 前の測点がありません、スキップ\n");
                }
            }
            StationAction::Quit => {
                println!("保存して終了します...");
                break;
            }
        }
    }

    // 保存
    let output = output_path.unwrap_or(input_path);
    let json = serde_json::to_string_pretty(&results)?;
    std::fs::write(output, json)?;

    println!("\n✓ 保存しました: {}", output.display());

    Ok(())
}

/// 測点入力プロンプト
fn prompt_station_action(
    candidates: &[String],
    prev: Option<&str>,
) -> Result<StationAction> {
    let prompt = if prev.is_some() {
        "測点 (s:スキップ S:全スキップ r:前と同じ R:全部同じ q:終了)"
    } else {
        "測点 (s:スキップ S:全スキップ q:終了)"
    };

    // 候補がある場合は選択肢を表示
    if !candidates.is_empty() {
        println!("  候補: {}", candidates.join(", "));
    }

    let input: String = Input::new()
        .with_prompt(prompt)
        .allow_empty(true)
        .interact_text()
        .map_err(|e| PhotoAiError::CliExecution(e.to_string()))?;

    let trimmed = input.trim();

    match trimmed {
        "" | "s" => Ok(StationAction::Skip),
        "S" => Ok(StationAction::SkipAll),
        "r" if prev.is_some() => Ok(StationAction::Repeat),
        "R" if prev.is_some() => Ok(StationAction::RepeatAll),
        "q" | "Q" => Ok(StationAction::Quit),
        _ => Ok(StationAction::Input(trimmed.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_empty_stations() {
        let results = vec![
            AnalysisResult { station: "No.10".into(), ..Default::default() },
            AnalysisResult { station: "".into(), ..Default::default() },
            AnalysisResult { station: "  ".into(), ..Default::default() },
            AnalysisResult { station: "No.20".into(), ..Default::default() },
        ];
        let empty = extract_empty_station_photos(&results);
        assert_eq!(empty, vec![1, 2]);
    }

    #[test]
    fn test_collect_existing_stations() {
        let results = vec![
            AnalysisResult { station: "No.10".into(), ..Default::default() },
            AnalysisResult { station: "No.20".into(), ..Default::default() },
            AnalysisResult { station: "No.10".into(), ..Default::default() },
            AnalysisResult { station: "".into(), ..Default::default() },
        ];
        let stations = collect_existing_stations(&results);
        assert_eq!(stations.len(), 2);
        assert!(stations.contains(&"No.10".to_string()));
        assert!(stations.contains(&"No.20".to_string()));
    }
}
