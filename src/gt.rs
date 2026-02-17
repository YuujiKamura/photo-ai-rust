//! Ground Truth管理モジュール
//!
//! PDFまたはJSONからGTを取り込み、パイプライン出力と比較する。

use crate::error::Result;
use photo_ai_common::types::AnalysisResult;
use std::collections::HashMap;
use std::path::Path;

/// 比較対象フィールド（compare_accuracy.py と同等）
const COMPARE_FIELDS: &[(&str, fn(&AnalysisResult) -> &str)] = &[
    ("photoCategory", |r| &r.photo_category),
    ("remarks", |r| &r.remarks),
    ("station", |r| &r.station),
    ("workType", |r| &r.work_type),
    ("variety", |r| &r.variety),
    ("subphase", |r| &r.subphase),
    ("measurements", |r| &r.measurements),
    ("focusTarget", |r| &r.focus_target),
];

/// ファイル比較の差分
#[derive(Debug)]
pub struct FieldDiff {
    pub field: String,
    pub expected: String,
    pub got: String,
}

/// 1枚の写真の比較結果
#[derive(Debug)]
pub struct PhotoComparison {
    pub file_name: String,
    pub status: ComparisonStatus,
    pub diffs: Vec<FieldDiff>,
}

#[derive(Debug, PartialEq)]
pub enum ComparisonStatus {
    Ok,
    Diff,
    Missing,
}

/// フォルダ単位の比較結果
#[derive(Debug)]
pub struct FolderResult {
    pub folder: String,
    pub total: usize,
    pub ok: usize,
    pub diff: usize,
    pub missing: usize,
    pub details: Vec<PhotoComparison>,
}

/// 全体の比較レポート
#[derive(Debug)]
pub struct ComparisonReport {
    pub total_photos: usize,
    pub total_ok: usize,
    pub total_diff: usize,
    pub total_missing: usize,
    pub field_errors: HashMap<String, usize>,
    pub field_totals: HashMap<String, usize>,
    pub folders: Vec<FolderResult>,
}

/// ソースファイルから解析結果を読み込む（PDF or JSON）
pub fn import_from_source(path: &Path) -> Result<Vec<AnalysisResult>> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext.to_lowercase().as_str() {
        "pdf" => {
            let results = photo_ai_common::export::pdf_embed::read_analysis_from_pdf(path)?;
            Ok(results)
        }
        "json" => {
            let content = std::fs::read_to_string(path)?;
            let results: Vec<AnalysisResult> = serde_json::from_str(&content)?;
            Ok(results)
        }
        _ => Err(crate::error::PhotoAiError::Config(
            format!("未対応のファイル形式: .{}（PDF or JSONのみ）", ext),
        )),
    }
}

/// 解析結果をGT JSONとして保存
pub fn save_as_gt(results: &[AnalysisResult], output_path: &Path) -> Result<()> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(results)?;
    std::fs::write(output_path, json)?;
    Ok(())
}

/// 値の正規化（空白・改行の統一）
fn normalize_value(v: &str) -> String {
    v.trim().replace("\r\n", "\n").replace('\r', "\n")
}

/// 2つのJSONファイルを比較
fn compare_files(gt: &[AnalysisResult], pipe: &[AnalysisResult]) -> Vec<PhotoComparison> {
    let pipe_map: HashMap<&str, &AnalysisResult> = pipe.iter()
        .map(|r| (r.file_name.as_str(), r))
        .collect();

    gt.iter().map(|gt_item| {
        match pipe_map.get(gt_item.file_name.as_str()) {
            None => PhotoComparison {
                file_name: gt_item.file_name.clone(),
                status: ComparisonStatus::Missing,
                diffs: vec![],
            },
            Some(pipe_item) => {
                let diffs: Vec<FieldDiff> = COMPARE_FIELDS.iter()
                    .filter_map(|(name, accessor)| {
                        let gt_val = normalize_value(accessor(gt_item));
                        let pipe_val = normalize_value(accessor(pipe_item));
                        if gt_val != pipe_val {
                            Some(FieldDiff {
                                field: name.to_string(),
                                expected: gt_val,
                                got: pipe_val,
                            })
                        } else {
                            None
                        }
                    })
                    .collect();
                let status = if diffs.is_empty() { ComparisonStatus::Ok } else { ComparisonStatus::Diff };
                PhotoComparison {
                    file_name: gt_item.file_name.clone(),
                    status,
                    diffs,
                }
            }
        }
    }).collect()
}

/// GTディレクトリとパイプライン出力ディレクトリを比較
pub fn compare_folders(gt_dir: &Path, pipeline_dir: &Path) -> Result<ComparisonReport> {
    let mut gt_files: Vec<_> = std::fs::read_dir(gt_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().to_string_lossy().ends_with("__result.json"))
        .collect();
    gt_files.sort_by_key(|e| e.file_name());

    if gt_files.is_empty() {
        return Err(crate::error::PhotoAiError::FileNotFound(
            format!("GTファイルが見つかりません: {}", gt_dir.display()),
        ));
    }

    let mut report = ComparisonReport {
        total_photos: 0,
        total_ok: 0,
        total_diff: 0,
        total_missing: 0,
        field_errors: HashMap::new(),
        field_totals: HashMap::new(),
        folders: vec![],
    };

    for gt_entry in &gt_files {
        let gt_path = gt_entry.path();
        let name = gt_entry.file_name().to_string_lossy().to_string();
        let folder_label = name.replace("__result.json", "");
        let pipe_path = pipeline_dir.join(&name);

        if !pipe_path.exists() {
            eprintln!("SKIP {}: パイプライン出力なし", folder_label);
            continue;
        }

        let gt_content = std::fs::read_to_string(&gt_path)?;
        let gt_results: Vec<AnalysisResult> = serde_json::from_str(&gt_content)?;
        let pipe_content = std::fs::read_to_string(&pipe_path)?;
        let pipe_results: Vec<AnalysisResult> = serde_json::from_str(&pipe_content)?;

        let comparisons = compare_files(&gt_results, &pipe_results);
        let ok = comparisons.iter().filter(|c| c.status == ComparisonStatus::Ok).count();
        let diff = comparisons.iter().filter(|c| c.status == ComparisonStatus::Diff).count();
        let missing = comparisons.iter().filter(|c| c.status == ComparisonStatus::Missing).count();
        let count = comparisons.len();

        report.total_photos += count;
        report.total_ok += ok;
        report.total_diff += diff;
        report.total_missing += missing;

        for c in &comparisons {
            for d in &c.diffs {
                *report.field_errors.entry(d.field.clone()).or_insert(0) += 1;
            }
            for (field, _) in COMPARE_FIELDS {
                *report.field_totals.entry(field.to_string()).or_insert(0) += 1;
            }
        }

        let details: Vec<PhotoComparison> = comparisons.into_iter()
            .filter(|c| c.status != ComparisonStatus::Ok)
            .collect();

        report.folders.push(FolderResult {
            folder: folder_label,
            total: count,
            ok,
            diff,
            missing,
            details,
        });
    }

    Ok(report)
}

/// レポートを表形式で出力
pub fn print_report(report: &ComparisonReport) {
    println!("{}", "=".repeat(80));
    println!("PIPELINE ACCURACY REPORT");
    println!("{}", "=".repeat(80));
    println!();

    // Per-folder summary
    println!("{:<50} {:>4} {:>4} {:>5} {:>6}", "Folder", "OK", "Diff", "Total", "Acc");
    println!("{}", "-".repeat(70));
    for fr in &report.folders {
        let acc = if fr.total > 0 { fr.ok as f64 / fr.total as f64 * 100.0 } else { 0.0 };
        let marker = if fr.diff > 0 { " *" } else { "" };
        println!("{:<50} {:>4} {:>4} {:>5} {:>5.1}%{}", fr.folder, fr.ok, fr.diff, fr.total, acc, marker);
    }
    println!("{}", "-".repeat(70));
    let overall_acc = if report.total_photos > 0 {
        report.total_ok as f64 / report.total_photos as f64 * 100.0
    } else {
        0.0
    };
    println!("{:<50} {:>4} {:>4} {:>5} {:>5.1}%", "TOTAL", report.total_ok, report.total_diff, report.total_photos, overall_acc);
    println!();

    // Field-level accuracy
    println!("FIELD-LEVEL ACCURACY:");
    println!("  {:<20} {:>6} {:>6} {:>7}", "Field", "Errors", "Total", "Acc");
    println!("  {}", "-".repeat(42));
    for (field, _) in COMPARE_FIELDS {
        let errs = report.field_errors.get(*field).copied().unwrap_or(0);
        let total = report.field_totals.get(*field).copied().unwrap_or(0);
        let acc = if total > 0 { (total - errs) as f64 / total as f64 * 100.0 } else { 0.0 };
        println!("  {:<20} {:>6} {:>6} {:>6.1}%", field, errs, total, acc);
    }
    println!();

    // Detailed diffs
    let has_details = report.folders.iter().any(|fr| !fr.details.is_empty());
    if has_details {
        println!("DETAILED DIFFS:");
        println!("{}", "-".repeat(80));
        for fr in &report.folders {
            if fr.details.is_empty() { continue; }
            println!("\n[{}]", fr.folder);
            for r in &fr.details {
                if r.status == ComparisonStatus::Missing {
                    println!("  {}: MISSING in pipeline output", r.file_name);
                } else {
                    println!("  {}:", r.file_name);
                    for d in &r.diffs {
                        let exp = if d.expected.is_empty() { "(empty)" } else { &d.expected };
                        let got = if d.got.is_empty() { "(empty)" } else { &d.got };
                        let exp_trunc: String = exp.chars().take(60).collect();
                        let got_trunc: String = got.chars().take(60).collect();
                        println!("    {}: expected={}", d.field, exp_trunc);
                        println!("    {}  got    ={}", " ".repeat(d.field.len()), got_trunc);
                    }
                }
            }
        }
    }
}
