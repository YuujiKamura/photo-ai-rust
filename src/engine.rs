use crate::error::{PhotoAiError, Result};
use crate::grouping::{CarrierConfig, GroupRecords};
use photo_ai_common::{AnalysisResult, RawImageData};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Deserialize)]
struct EngineResponse<T> {
    ok: bool,
    data: Option<T>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PairEnsembleData {
    after_number: u32,
    confidence: f64,
}

#[derive(Debug, Deserialize)]
struct Step1Data {
    raw: Vec<RawImageData>,
}

#[derive(Debug, Deserialize)]
struct SingleStepData {
    results: Vec<AnalysisResult>,
}

pub fn run_tag_groups(
    folder: &Path,
    batch_size: usize,
    vocabulary: Option<&[String]>,
    _carrier: CarrierConfig,
) -> Result<GroupRecords> {
    let mut args = vec![
        "tag-groups".to_string(),
        "--folder".to_string(),
        folder.display().to_string(),
        "--batch-size".to_string(),
        batch_size.to_string(),
    ];

    if let Some(vocabulary) = vocabulary {
        if !vocabulary.is_empty() {
            args.push("--vocabulary".to_string());
            args.push(vocabulary.join(","));
        }
    }

    run_engine_command(&args)
}

pub fn run_step1(folder: &Path) -> Result<Vec<RawImageData>> {
    let args = vec![
        "step1".to_string(),
        "--folder".to_string(),
        folder.display().to_string(),
    ];
    let data: Step1Data = run_engine_command(&args)?;
    Ok(data.raw)
}

pub fn run_single_step(
    folder: &Path,
    master: &Path,
    work_type: Option<&str>,
    variety: Option<&str>,
    photo_type: Option<&str>,
) -> Result<Vec<AnalysisResult>> {
    let mut args = vec![
        "single-step".to_string(),
        "--folder".to_string(),
        folder.display().to_string(),
        "--master".to_string(),
        master.display().to_string(),
    ];

    if let Some(work_type) = work_type {
        args.push("--work-type".to_string());
        args.push(work_type.to_string());
    }
    if let Some(variety) = variety {
        args.push("--variety".to_string());
        args.push(variety.to_string());
    }
    if let Some(photo_type) = photo_type {
        args.push("--photo-type".to_string());
        args.push(photo_type.to_string());
    }

    let data: SingleStepData = run_engine_command(&args)?;
    Ok(data.results)
}

pub fn run_pair_ensemble(
    before_sheet: &Path,
    after_sheet: &Path,
    query: &str,
    before_max: u32,
    after_max: u32,
) -> Result<(u32, f64)> {
    let args = vec![
        "pair-ensemble".to_string(),
        "--before-sheet".to_string(),
        before_sheet.display().to_string(),
        "--after-sheet".to_string(),
        after_sheet.display().to_string(),
        "--query".to_string(),
        query.to_string(),
        "--before-max".to_string(),
        before_max.to_string(),
        "--after-max".to_string(),
        after_max.to_string(),
    ];

    let data: PairEnsembleData = run_engine_command(&args)?;
    Ok((data.after_number, data.confidence))
}

fn run_engine_command<T: DeserializeOwned>(args: &[String]) -> Result<T> {
    let binary = resolve_analysis_engine_binary();
    let output = Command::new(&binary)
        .args(args)
        .output()
        .map_err(|e| {
            PhotoAiError::CliExecution(format!(
                "photo-analysis-engine 実行失敗 ({}): {}",
                binary.display(),
                e
            ))
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let payload = if stdout.trim().is_empty() {
        stderr.trim()
    } else {
        stdout.trim()
    };

    let response: EngineResponse<T> = serde_json::from_str(payload).map_err(|e| {
        PhotoAiError::ApiParse(format!(
            "photo-analysis-engine 応答パース失敗: {e}; payload={}",
            payload.chars().take(400).collect::<String>()
        ))
    })?;

    if !output.status.success() || !response.ok {
        return Err(PhotoAiError::CliExecution(
            response
                .error
                .unwrap_or_else(|| format!("photo-analysis-engine failed: {}", stderr.trim())),
        ));
    }

    response.data.ok_or_else(|| {
        PhotoAiError::ApiParse("photo-analysis-engine の data が空でした".to_string())
    })
}

fn resolve_analysis_engine_binary() -> PathBuf {
    let exe_name = if cfg!(windows) {
        "photo-analysis-engine.exe"
    } else {
        "photo-analysis-engine"
    };

    let current_exe = std::env::current_exe().ok();
    let mut candidates = Vec::new();

    if let Some(ref exe) = current_exe {
        if let Some(base_dir) = exe.parent() {
            candidates.push(base_dir.join(exe_name));
            if let Some(parent_dir) = base_dir.parent() {
                candidates.push(parent_dir.join(exe_name));
                candidates.push(parent_dir.join("debug").join(exe_name));
                candidates.push(parent_dir.join("release").join(exe_name));
                if let Some(grandparent_dir) = parent_dir.parent() {
                    candidates.push(grandparent_dir.join("debug").join(exe_name));
                    candidates.push(grandparent_dir.join("release").join(exe_name));
                }
            }
        }
    }

    candidates.push(PathBuf::from(exe_name));

    candidates
        .into_iter()
        .find(|path| path.exists())
        .unwrap_or_else(|| PathBuf::from(exe_name))
}

