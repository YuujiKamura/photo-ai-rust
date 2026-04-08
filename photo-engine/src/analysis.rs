use crate::agentapi;
use anyhow::{anyhow, Context, Result};
use photo_ai_common::{
    build_prompt_for_category, build_step1_prompt, parse_single_step_response,
    parse_step1_response, AnalysisResult, HierarchyMaster, RawImageData,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "JPG", "JPEG", "PNG"];
const CONFIDENCE_FINAL_ONLY: f64 = 0.67;
const CONFIDENCE_FALLBACK: f64 = 0.33;
const GROUP_GAP_SECS: i64 = 5 * 60;
const GROUP_FILE: &str = "photo-groups.json";

#[derive(Debug, Clone, Serialize)]
pub struct ScanImage {
    pub path: String,
    pub file_name: String,
    pub date: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Step1Output {
    pub images: Vec<ScanImage>,
    pub raw: Vec<RawImageData>,
}

#[derive(Debug, Serialize)]
pub struct SingleStepOutput {
    pub images: Vec<ScanImage>,
    pub results: Vec<AnalysisResult>,
}

#[derive(Debug, Serialize)]
pub struct PairEnsembleOutput {
    pub query: String,
    pub after_number: u32,
    pub confidence: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageMode {
    PayPerUse,
    Resident,
    TimeBasedQuota,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroupCore {
    pub role: String,
    pub machine_type: String,
    pub machine_id: String,
    #[serde(default)]
    pub has_board: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detected_text: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Deserialize)]
struct GroupItem {
    file: String,
    #[serde(flatten)]
    core: GroupCore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupRecord {
    #[serde(flatten)]
    pub core: GroupCore,
    pub group: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_at: Option<i64>,
}

pub type GroupRecords = HashMap<String, GroupRecord>;

pub fn tag_groups(
    folder: &Path,
    batch_size: usize,
    vocabulary: Option<&[String]>,
    usage_mode: UsageMode,
) -> Result<GroupRecords> {
    let mut records = load_group_records(folder);
    let images = collect_images_flat(folder);
    let capture_times = collect_capture_times(&images);

    if images.is_empty() {
        return Ok(records);
    }

    let pending: Vec<_> = images
        .iter()
        .filter(|img| {
            let name = img.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
            !records.contains_key(name.as_ref())
        })
        .cloned()
        .collect();

    if !pending.is_empty() {
        for batch in pending.chunks(batch_size) {
            let results = classify_group_batch(batch, vocabulary, usage_mode)?;
            for (fname, item) in results {
                records.insert(
                    fname,
                    GroupRecord {
                        core: item.core,
                        group: 0,
                        captured_at: None,
                    },
                );
            }
        }
    }

    apply_capture_times(&mut records, &capture_times);
    assign_groups(&mut records);
    save_group_records(folder, &records)?;
    Ok(records)
}

pub fn scan_folder(folder: &Path) -> Result<Vec<ScanImage>> {
    let mut images = Vec::new();

    for entry in std::fs::read_dir(folder)
        .with_context(|| format!("フォルダを読めません: {}", folder.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        if !IMAGE_EXTENSIONS.iter().any(|candidate| *candidate == ext) {
            continue;
        }

        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        let date = extract_date(&path);

        images.push(ScanImage {
            path: path.display().to_string(),
            file_name,
            date,
        });
    }

    images.sort_by(|a, b| a.file_name.cmp(&b.file_name));

    if images.is_empty() {
        return Err(anyhow!("画像が見つかりません: {}", folder.display()));
    }

    Ok(images)
}

pub fn analyze_step1(folder: &Path) -> Result<Step1Output> {
    let images = scan_folder(folder)?;
    let image_paths = to_path_bufs(&images);
    let image_meta = to_image_meta(&images);
    let prompt = build_step1_prompt(&image_meta);
    let response = agentapi::analyze(&prompt, &image_paths)?;
    let raw = parse_step1_response(&response).context("Step1 JSONパース失敗")?;
    Ok(Step1Output { images, raw })
}

pub fn analyze_single_step(
    folder: &Path,
    master_path: &Path,
    work_type: Option<&str>,
    variety: Option<&str>,
    photo_type: Option<&str>,
) -> Result<SingleStepOutput> {
    let images = scan_folder(folder)?;
    let image_paths = to_path_bufs(&images);
    let image_meta = to_image_meta(&images);
    let master = HierarchyMaster::from_csv(master_path)
        .map_err(|e| anyhow!("マスタ読み込み失敗: {e}"))?;
    let prompt = build_prompt_for_category(&image_meta, &master, work_type, variety, photo_type);
    let response = agentapi::analyze(&prompt, &image_paths)?;
    let mut results = parse_single_step_response(&response).context("1ステップ解析 JSONパース失敗")?;
    fill_image_metadata(&mut results, &images);
    sanitize_classification(&mut results, &master);
    Ok(SingleStepOutput { images, results })
}

pub fn pair_ensemble(
    before_sheet: &Path,
    after_sheet: &Path,
    query: &str,
    before_max: u32,
    after_max: u32,
) -> Result<PairEnsembleOutput> {
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

    let files = vec![before_sheet.to_path_buf(), after_sheet.to_path_buf()];
    let response = agentapi::analyze(&prompt, &files)?;
    let (after_number, confidence) = parse_ensemble_response(&response, after_max)?;

    Ok(PairEnsembleOutput {
        query: query.to_string(),
        after_number,
        confidence,
    })
}

fn to_image_meta(images: &[ScanImage]) -> Vec<(&str, Option<&str>)> {
    images
        .iter()
        .map(|img| (img.file_name.as_str(), img.date.as_deref()))
        .collect()
}

fn to_path_bufs(images: &[ScanImage]) -> Vec<PathBuf> {
    images.iter().map(|img| PathBuf::from(&img.path)).collect()
}

fn fill_image_metadata(results: &mut [AnalysisResult], images: &[ScanImage]) {
    let info_map: HashMap<&str, &ScanImage> = images
        .iter()
        .map(|img| (img.file_name.as_str(), img))
        .collect();

    for result in results {
        if let Some(img_info) = info_map.get(result.file_name.as_str()) {
            result.file_path = img_info.path.clone();
            result.date = img_info.date.clone().unwrap_or_default();
        }
    }
}

fn extract_date(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut bufreader = BufReader::new(file);
    let exif_data = exif::Reader::new().read_from_container(&mut bufreader).ok()?;
    let field = exif_data
        .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
        .or_else(|| exif_data.get_field(exif::Tag::DateTime, exif::In::PRIMARY))?;
    Some(field.display_value().to_string())
}

fn classify_group_batch(
    images: &[PathBuf],
    vocabulary: Option<&[String]>,
    _usage_mode: UsageMode,
) -> Result<Vec<(String, GroupItem)>> {
    let names: Vec<String> = images
        .iter()
        .enumerate()
        .map(|(idx, p)| {
            p.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| format!("unknown_{idx}"))
        })
        .collect();
    let names_ref: Vec<&str> = names.iter().map(|s| s.as_str()).collect();

    let prompt = group_prompt(&names_ref, vocabulary);
    let raw = agentapi::analyze(&prompt, images)?;
    let json_val = extract_json_array(&raw)
        .with_context(|| format!("No JSON array in: {raw}"))?;
    let items: Vec<GroupItem> =
        serde_json::from_value(json_val).context("Failed to parse group JSON")?;

    Ok(items
        .into_iter()
        .map(|g| {
            let file = g.file.clone();
            (file, g)
        })
        .collect())
}

fn extract_json_array(s: &str) -> Option<serde_json::Value> {
    let start = s.find('[')?;
    let end = s.rfind(']')? + 1;
    let candidate = &s[start..end];
    let val: serde_json::Value = serde_json::from_str(candidate).ok()?;
    if val.is_array() { Some(val) } else { None }
}

fn group_prompt(filenames: &[&str], vocabulary: Option<&[String]>) -> String {
    let list = filenames.join(", ");
    let mut prompt = GROUP_PROMPT_TEMPLATE.replace("{list}", &list);
    if let Some(vocab) = vocabulary {
        if !vocab.is_empty() {
            prompt.push_str(&format!(
                "\n工事現場で使われる用語リスト（該当するものがあればこの用語を使え。なければ自由に記述せよ）:\n{}",
                vocab.join(", ")
            ));
        }
    }
    prompt
}

fn is_image(p: &Path) -> bool {
    matches!(
        p.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("jpg" | "jpeg" | "png" | "heic")
    )
}

fn collect_images_flat(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_dir() && is_image(&p) {
            out.push(p);
        }
    }
    out.sort();
    out
}

fn load_group_records(base: &Path) -> GroupRecords {
    let path = base.join(GROUP_FILE);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_group_records(base: &Path, records: &GroupRecords) -> Result<()> {
    let path = base.join(GROUP_FILE);
    let json =
        serde_json::to_string_pretty(records).context("Failed to serialize group records")?;
    std::fs::write(&path, json)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

fn collect_capture_times(images: &[PathBuf]) -> HashMap<String, i64> {
    let mut out = HashMap::new();
    for p in images {
        let fname = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if fname.is_empty() {
            continue;
        }
        let ts = std::fs::metadata(p)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64);
        if let Some(v) = ts {
            out.insert(fname, v);
        }
    }
    out
}

fn apply_capture_times(records: &mut GroupRecords, capture_times: &HashMap<String, i64>) {
    for (fname, rec) in records.iter_mut() {
        normalize_machine_id(rec);
        if rec.captured_at.is_none() {
            if let Some(ts) = capture_times.get(fname) {
                rec.captured_at = Some(*ts);
            }
        }
    }
    propagate_attachment_by_time(records);
}

fn assign_groups(records: &mut GroupRecords) {
    let mut by_id: HashMap<String, Vec<String>> = HashMap::new();
    for (fname, rec) in records.iter() {
        by_id.entry(rec.core.machine_id.clone()).or_default().push(fname.clone());
    }

    let mut segment_heads: Vec<(i64, String, u32)> = Vec::new();
    let mut fname_to_tmp_group: HashMap<String, u32> = HashMap::new();
    let mut next_tmp_group = 1u32;

    for (machine_id, mut files) in by_id {
        files.sort_by(|a, b| {
            let ra = &records[a];
            let rb = &records[b];
            ra.captured_at
                .unwrap_or(i64::MAX)
                .cmp(&rb.captured_at.unwrap_or(i64::MAX))
                .then(a.cmp(b))
        });
        if files.is_empty() {
            continue;
        }

        let mut current_group = next_tmp_group;
        next_tmp_group += 1;
        let first_ts = records[&files[0]].captured_at.unwrap_or(i64::MAX);
        segment_heads.push((first_ts, machine_id.clone(), current_group));
        fname_to_tmp_group.insert(files[0].clone(), current_group);

        for pair in files.windows(2) {
            let prev = &records[&pair[0]];
            let curr = &records[&pair[1]];
            let prev_ts = prev.captured_at.unwrap_or(i64::MAX);
            let curr_ts = curr.captured_at.unwrap_or(i64::MAX);
            let gap = if prev_ts == i64::MAX || curr_ts == i64::MAX {
                0
            } else {
                (curr_ts - prev_ts).abs()
            };
            let prev_attach = has_attachment_hint(prev);
            let curr_attach = has_attachment_hint(curr);

            if gap > GROUP_GAP_SECS || prev_attach != curr_attach {
                current_group = next_tmp_group;
                next_tmp_group += 1;
                segment_heads.push((curr_ts, machine_id.clone(), current_group));
            }
            fname_to_tmp_group.insert(pair[1].clone(), current_group);
        }
    }

    segment_heads.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut compact_map: HashMap<u32, u32> = HashMap::new();
    for (idx, (_, _, tmp)) in segment_heads.iter().enumerate() {
        compact_map.insert(*tmp, (idx + 1) as u32);
    }

    for (fname, rec) in records.iter_mut() {
        if let Some(tmp) = fname_to_tmp_group.get(fname) {
            rec.group = *compact_map.get(tmp).unwrap_or(tmp);
        } else {
            rec.group = 0;
        }
    }
}

fn has_attachment_hint(rec: &GroupRecord) -> bool {
    rec.core.machine_id.contains("取付") || rec.core.detected_text.contains("取付")
}

fn extract_no(text: &str) -> Option<String> {
    for marker in ["No.", "No ", "NO.", "NO "] {
        if let Some(pos) = text.find(marker) {
            let rest = &text[pos + marker.len()..];
            let digits: String = rest
                .chars()
                .skip_while(|c| !c.is_ascii_digit())
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if !digits.is_empty() {
                return Some(format!("No.{}", digits));
            }
        }
    }
    None
}

fn normalize_machine_id(rec: &mut GroupRecord) {
    let merged = format!("{} {}", rec.core.detected_text, rec.core.description);
    if merged.contains("取付") {
        if let Some(no) = extract_no(&merged).or_else(|| extract_no(&rec.core.machine_id)) {
            rec.core.machine_id = format!("取付道路 {}", no);
        }
    }
}

fn propagate_attachment_by_time(records: &mut GroupRecords) {
    let mut by_no: HashMap<String, Vec<String>> = HashMap::new();
    for (fname, rec) in records.iter() {
        if let Some(no) = extract_no(&rec.core.machine_id)
            .or_else(|| extract_no(&rec.core.detected_text))
            .or_else(|| extract_no(&rec.core.description))
        {
            by_no.entry(no).or_default().push(fname.clone());
        }
    }

    for (no, mut files) in by_no {
        files.sort_by(|a, b| {
            let ra = &records[a];
            let rb = &records[b];
            ra.captured_at
                .unwrap_or(i64::MAX)
                .cmp(&rb.captured_at.unwrap_or(i64::MAX))
                .then(a.cmp(b))
        });
        if files.is_empty() {
            continue;
        }

        let mut chunk: Vec<String> = vec![files[0].clone()];
        for pair in files.windows(2) {
            let prev = &records[&pair[0]];
            let curr = &records[&pair[1]];
            let prev_ts = prev.captured_at.unwrap_or(i64::MAX);
            let curr_ts = curr.captured_at.unwrap_or(i64::MAX);
            let gap = if prev_ts == i64::MAX || curr_ts == i64::MAX {
                0
            } else {
                (curr_ts - prev_ts).abs()
            };
            if gap > GROUP_GAP_SECS {
                apply_attach_to_chunk(records, &chunk, &no);
                chunk.clear();
            }
            chunk.push(pair[1].clone());
        }
        if !chunk.is_empty() {
            apply_attach_to_chunk(records, &chunk, &no);
        }
    }
}

fn apply_attach_to_chunk(records: &mut GroupRecords, chunk: &[String], no: &str) {
    let has_attach = chunk
        .iter()
        .any(|fname| records.get(fname).map(has_attachment_hint).unwrap_or(false));
    if !has_attach {
        return;
    }
    for fname in chunk {
        if let Some(rec) = records.get_mut(fname) {
            rec.core.machine_id = format!("取付道路 {}", no);
        }
    }
}

fn parse_ensemble_response(response: &str, after_max: u32) -> Result<(u32, f64)> {
    let re = Regex::new(r"(?i)A\s*(\d+)").unwrap();
    let final_re = Regex::new(r"(?i)Final\s*:\s*A\s*(\d+)").unwrap();
    let scan_re = Regex::new(r"(?i)Scan\s*\d\s*:\s*A\s*(\d+)").unwrap();

    let final_num = final_re
        .captures(response)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<u32>().ok())
        .filter(|&n| n >= 1 && n <= after_max);

    let scans: Vec<u32> = scan_re
        .captures_iter(response)
        .filter_map(|c| c.get(1))
        .filter_map(|m| m.as_str().parse::<u32>().ok())
        .filter(|&n| n >= 1 && n <= after_max)
        .collect();

    if let Some(num) = final_num {
        let agree_count = scans.iter().filter(|&&s| s == num).count();
        let confidence = if scans.is_empty() {
            CONFIDENCE_FINAL_ONLY
        } else {
            agree_count as f64 / scans.len() as f64
        };
        return Ok((num, confidence));
    }

    if !scans.is_empty() {
        let (winner, count) = majority_vote(&scans);
        let confidence = count as f64 / scans.len() as f64;
        return Ok((winner, confidence));
    }

    if let Some(caps) = re.captures(response) {
        if let Some(m) = caps.get(1) {
            if let Ok(n) = m.as_str().parse::<u32>() {
                if n >= 1 && n <= after_max {
                    return Ok((n, CONFIDENCE_FALLBACK));
                }
            }
        }
    }

    Err(anyhow!("ペアリング回答パース失敗: {}", response.chars().take(300).collect::<String>()))
}

fn majority_vote(nums: &[u32]) -> (u32, usize) {
    let mut counts: HashMap<u32, usize> = HashMap::new();
    for &n in nums {
        *counts.entry(n).or_insert(0) += 1;
    }
    counts.into_iter().max_by_key(|(_, c)| *c).unwrap_or((0, 0))
}

fn sanitize_classification(results: &mut [AnalysisResult], master: &HierarchyMaster) {
    for result in results.iter_mut() {
        if !result.remarks.is_empty() {
            let mut candidates: Vec<_> = master
                .rows()
                .iter()
                .filter(|row| row.remarks == result.remarks)
                .collect();

            if !candidates.is_empty() {
                if !result.photo_category.is_empty() {
                    let filtered: Vec<_> = candidates
                        .iter()
                        .copied()
                        .filter(|row| row.photo_type == result.photo_category)
                        .collect();
                    if !filtered.is_empty() {
                        candidates = filtered;
                    }
                }
                if !result.work_type.is_empty() {
                    let filtered: Vec<_> = candidates
                        .iter()
                        .copied()
                        .filter(|row| row.work_type == result.work_type)
                        .collect();
                    if !filtered.is_empty() {
                        candidates = filtered;
                    }
                }
                if !result.variety.is_empty() {
                    let filtered: Vec<_> = candidates
                        .iter()
                        .copied()
                        .filter(|row| row.variety == result.variety)
                        .collect();
                    if !filtered.is_empty() {
                        candidates = filtered;
                    }
                }
                if !result.subphase.is_empty() {
                    let filtered: Vec<_> = candidates
                        .iter()
                        .copied()
                        .filter(|row| row.subphase == result.subphase)
                        .collect();
                    if !filtered.is_empty() {
                        candidates = filtered;
                    }
                }

                if let Some(row) = candidates.first() {
                    result.photo_category = row.photo_type.clone();
                    result.work_type = row.work_type.clone();
                    result.variety = row.variety.clone();
                    result.subphase = row.subphase.clone();
                }
            }
        }

        if result.work_type == "舗装工" && result.variety == "未舗装部舗装工" {
            result.variety = "舗装打換え工".to_string();
        }

        if !result.photo_category.is_empty() && !result.work_type.is_empty() {
            let has_work = master.rows().iter().any(|row| {
                row.photo_type == result.photo_category && row.work_type == result.work_type
            });
            if !has_work {
                result.work_type.clear();
                result.variety.clear();
                result.subphase.clear();
                result.remarks.clear();
                continue;
            }
        }

        if !result.work_type.is_empty() {
            let work_types = master.get_work_types();
            if !work_types.contains(&result.work_type.as_str()) {
                result.work_type.clear();
                result.variety.clear();
                result.subphase.clear();
                result.remarks.clear();
                continue;
            }
        }

        if !result.work_type.is_empty() && !result.variety.is_empty() {
            let has_variety = master.rows().iter().any(|row| {
                row.work_type == result.work_type
                    && row.variety == result.variety
                    && (result.photo_category.is_empty() || row.photo_type == result.photo_category)
            });
            if !has_variety {
                result.variety.clear();
                result.subphase.clear();
                result.remarks.clear();
            }
        } else {
            result.variety.clear();
            result.subphase.clear();
        }

        if !result.work_type.is_empty() && !result.variety.is_empty() && !result.subphase.is_empty() {
            let has_detail = master.rows().iter().any(|row| {
                row.work_type == result.work_type
                    && row.variety == result.variety
                    && row.subphase == result.subphase
                    && (result.photo_category.is_empty() || row.photo_type == result.photo_category)
            });
            if !has_detail {
                result.subphase.clear();
                result.remarks.clear();
            }
        } else if !result.work_type.is_empty() {
            result.subphase.clear();
            let has_remarks_in_master = !result.remarks.is_empty()
                && master.rows().iter().any(|row| {
                    row.work_type == result.work_type
                        && row.variety.is_empty()
                        && row.remarks == result.remarks
                });
            if !has_remarks_in_master {
                result.remarks.clear();
            }
        } else {
            result.subphase.clear();
        }

        if !result.remarks.is_empty() {
            let has_remarks = master.rows().iter().any(|row| {
                row.remarks == result.remarks
                    && row.work_type == result.work_type
                    && row.variety == result.variety
                    && row.subphase == result.subphase
                    && (result.photo_category.is_empty() || row.photo_type == result.photo_category)
            });
            if !has_remarks {
                result.remarks.clear();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pair_response_prefers_final() {
        let response = "Scan1: A03\nScan2: A05\nScan3: A03\nFinal: A03";
        let (num, confidence) = parse_ensemble_response(response, 10).unwrap();
        assert_eq!(num, 3);
        assert!(confidence > 0.6);
    }

    #[test]
    fn pair_response_falls_back_to_majority() {
        let response = "Scan1: A04\nScan2: A04\nScan3: A02";
        let (num, confidence) = parse_ensemble_response(response, 10).unwrap();
        assert_eq!(num, 4);
        assert!(confidence > 0.6);
    }
}

const GROUP_PROMPT_TEMPLATE: &str = r#"工事写真を分類・グループ分けせよ。同一対象の写真をグループにまとめろ。Output ONLY JSON array: [{{"file":"filename","role":"?","machine_type":"?","machine_id":"?","has_board":false,"detected_text":"","description":""}}, ...]
ファイル: {list}
用語定義(重要):
- 「計画高」は表層出来形の管理値。表層工の出来形管理に使う。
- 「切削高」は路面切削工の管理値。表層工の計画高とは別物。
- 「計画高(実施)」が読める場合は、路面切削ではなく表層出来形の根拠を優先する。
判定ルール(重要):
- グループ内に黒板アップ/出来形管理用紙アップがあり、「計画高(実施)」または「計画高」が手書きで確認できる場合:
  そのグループ全体を表層出来形として扱うこと。
- 逆に「切削高」のみで「計画高」が無い場合は切削出来形として扱う。
- 「No.1」と「取付道路 No.1」は別測点であり、同じ番号でも別groupにすること。
- machine_id には測点を識別できる表記を入れること（例: 本線は「No.1」、取付は「取付道路 No.1」）。
role: 写真の役割（例: "機械全景", "特定自主検査証票", "排ガス対策型・低騒音型機械証票", "ナンバープレート", "始業前点検", "点検状況", "安全活動", "作業状況", "出来形管理", "本検査実施状況" など）
machine_type: 機械・対象の種類（例: タイヤローラー, マカダムローラー, アスファルトフィニッシャー, バックホウ）。機械でなければ活動名（例: 安全パトロール, 朝礼）
machine_id: 型式番号や識別情報。銘板・証票・黒板から読み取れ。同一対象の写真は同じ値にせよ。不明なら空文字。
has_board: 黒板が写っていればtrue
detected_text: 黒板・銘板・証票・出来形管理用紙に書かれたテキストを記録。温度管理黒板の重要ルール: 黒板に到着温度・敷均し温度・初期転圧前温度が縦に並んでいる場合、記入済みの最下段の値がこの写真の測定値である（上の値は前の写真で既に記録済み）。detected_textには全値を記録し、descriptionには最下段の記入済み温度を明記せよ。出来形管理用紙の場合は以下のカンマ区切り形式で記録せよ: 「出来形管理用紙 No.X, 計画高(設計) V1=数値 V2=数値 V3=数値 V4=数値 V5=数値, 計画高(実施) V1=数値 V2=数値 V3=数値 V4=数値 V5=数値, 切削高(設計) V1=数値 V2=数値 V3=数値 V4=数値 V5=数値, 切削高(実施) V1=数値 V2=数値 V3=数値 V4=数値 V5=数値, 左幅員 設計X.XX 実測X.XX, 右幅員 設計X.XX 実測X.XX」
description: 写真の内容を1文で記述"#;
