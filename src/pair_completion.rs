//! 着手前・竣工写真 自動ペアリングモジュール（コンタクトシート+アンサンブル方式）
//!
//! Before/After写真をそれぞれ番号付きコンタクトシートにまとめ、
//! 1コール1問×アンサンブル（順逆走査3回）で90%精度のペアリングを実現する。

use crate::commands::{PairCompletionCommandArgs, PairReplaceCommandArgs};
#[cfg(feature = "pdf-gen")]
use crate::contactsheet::generate_contact_sheet;
use crate::error::{PhotoAiError, Result};
use crate::export::pair_pdf::PairEntry;
#[cfg(feature = "pdf-gen")]
use crate::pair_ensemble::ensemble_pair_query;
#[cfg(feature = "pdf-gen")]
use crate::pair_extraction::{extract_images_from_pdf, extract_pages_from_folder, ExtractedPage};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// pairing_manual.json のエントリ
#[derive(Deserialize)]
struct ManualPairEntry {
    #[allow(dead_code)]
    before_page: u32,
    before_station: String,
    #[serde(default)]
    before_file: String,
    after_file: String,
    confidence: f64,
}

/// JSONファイルからPairEntryリストを読み込む
pub fn load_pairs_from_json(json_path: &Path, folder: Option<&Path>) -> Result<Vec<PairEntry>> {
    let content = std::fs::read_to_string(json_path)
        .map_err(|e| PhotoAiError::Config(format!("JSON読み込み失敗: {}", e)))?;

    if let Ok(pairs) = serde_json::from_str::<Vec<PairEntry>>(&content) {
        return Ok(pairs);
    }

    let manual: Vec<ManualPairEntry> = serde_json::from_str(&content)
        .map_err(|e| PhotoAiError::Config(format!("JSONパース失敗: {}", e)))?;

    let folder = folder.ok_or_else(|| {
        PhotoAiError::Config(
            "pairing_manual形式のJSONにはfolder引数が必要です".to_string(),
        )
    })?;

    let mut pairs = Vec::new();
    for entry in &manual {
        if entry.confidence < 0.67 || entry.after_file.is_empty() {
            continue;
        }

        let before_path = match find_file_in_pfolders(folder, &entry.before_file) {
            Some(p) => p,
            None => {
                eprintln!("警告: before画像が見つかりません: {}", entry.before_file);
                continue;
            }
        };
        let after_path = match find_file_in_pfolders(folder, &entry.after_file) {
            Some(p) => p,
            None => {
                eprintln!("警告: after画像が見つかりません: {}", entry.after_file);
                continue;
            }
        };

        let station_name = entry.before_station
            .lines().next().unwrap_or(&entry.before_station)
            .trim().to_string();

        pairs.push(PairEntry { station_name, before_path, after_path });
    }

    Ok(pairs)
}

fn find_file_in_pfolders(folder: &Path, filename: &str) -> Option<PathBuf> {
    if filename.is_empty() {
        return None;
    }
    let entries = std::fs::read_dir(folder).ok()?;
    for entry in entries.filter_map(|e| e.ok()) {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with('P') {
            continue;
        }
        let candidate = entry.path().join(filename);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(feature = "pdf-gen")] const BEFORE_SHEET_COLS: u32 = 5;
#[cfg(feature = "pdf-gen")] const AFTER_SHEET_COLS: u32 = 7;
#[cfg(feature = "pdf-gen")] const CONFIDENCE_UNANIMOUS: f64 = 1.0;
#[cfg(feature = "pdf-gen")] const CONFIDENCE_MAJORITY: f64 = 0.67;

#[derive(Serialize, Deserialize, Debug)]
pub struct PairResult {
    pub before_page: u32,
    pub before_station: String,
    pub after_file: String,
    pub confidence: f64,
}

#[cfg(feature = "pdf-gen")]
struct TempDirGuard(PathBuf);
#[cfg(feature = "pdf-gen")]
impl Drop for TempDirGuard {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

#[cfg(feature = "pdf-gen")]
pub async fn handle_pair_completion(args: PairCompletionCommandArgs) -> Result<()> {
    let verbose = args.cli_args.verbose;
    println!("photo-ai-rust - 着手前/竣工 ペアリング（コンタクトシート+アンサンブル方式）\n");

    let (before_pages, _temp_guard) = if args.before.is_dir() {
        println!("[1/4] 画像フォルダスキャン中...");
        let pages = extract_pages_from_folder(&args.before)?;
        println!("  着手前: {}枚（フォルダ入力）", pages.len());
        if pages.is_empty() {
            return Err(PhotoAiError::NoImagesFound(args.before.display().to_string()));
        }
        (pages, None)
    } else {
        println!("[1/4] PDF画像抽出中...");
        let pages = extract_images_from_pdf(&args.before)?;
        println!("  着手前: {}ページ", pages.len());
        if pages.is_empty() {
            return Err(PhotoAiError::PdfExtraction("PDFから画像を抽出できませんでした".into()));
        }
        let guard = TempDirGuard(pages[0].image_path.parent().unwrap().to_path_buf());
        (pages, Some(guard))
    };

    let after_files = scan_after_folder(&args.after)?;
    println!("  竣工写真: {}枚", after_files.len());

    if after_files.is_empty() {
        return Err(PhotoAiError::NoImagesFound(args.after.display().to_string()));
    }

    println!("[2/4] コンタクトシート生成中...");
    let cs_temp_dir;
    let _cs_temp_guard;
    let temp_dir = if _temp_guard.is_some() {
        _cs_temp_guard = None;
        before_pages[0].image_path.parent().unwrap_or(Path::new("."))
    } else {
        cs_temp_dir = std::env::temp_dir().join(format!("photo-ai-cs-{}", std::process::id()));
        std::fs::create_dir_all(&cs_temp_dir)?;
        _cs_temp_guard = Some(TempDirGuard(cs_temp_dir.clone()));
        cs_temp_dir.as_path()
    };

    let before_image_paths: Vec<PathBuf> = before_pages.iter().map(|p| p.image_path.clone()).collect();
    let before_sheet = generate_contact_sheet(&before_image_paths, "B", BEFORE_SHEET_COLS, &temp_dir.join("contact_before.jpg"))?;
    let after_sheet = generate_contact_sheet(&after_files, "A", AFTER_SHEET_COLS, &temp_dir.join("contact_after.jpg"))?;

    let save_before = args.after.join("contact_before.jpg");
    let save_after = args.after.join("contact_after.jpg");
    std::fs::copy(&before_sheet.image_path, &save_before)?;
    std::fs::copy(&after_sheet.image_path, &save_after)?;
    println!("  保存: {}", save_before.display());
    println!("  保存: {}", save_after.display());

    println!("[3/4] AIペアリング中（アンサンブル）...");
    let before_max = before_pages.len();
    let after_max = after_files.len();

    let mut results = Vec::new();
    for (i, page) in before_pages.iter().enumerate() {
        let b_num = i as u32 + 1;
        let query = format!("B{:02}", b_num);

        let (after_num, confidence) = ensemble_pair_query(&before_sheet, &after_sheet, &query, before_max as u32, after_max as u32, verbose)?;

        let after_file = after_sheet.mapping.iter()
            .find(|&&(num, _)| num == after_num)
            .map(|(_, name)| name.clone())
            .unwrap_or_else(|| format!("A{:02}", after_num));

        if verbose {
            println!("  {} -> A{:02} ({}) confidence={:.2}", query, after_num, after_file, confidence);
        } else {
            let mark = if confidence >= CONFIDENCE_UNANIMOUS { "o" } else { "~" };
            println!("  {} {} A{:02}", query, mark, after_num);
        }

        results.push(PairResult {
            before_page: b_num,
            before_station: page.station_text.clone(),
            after_file,
            confidence,
        });
    }

    println!("\n[4/4] 結果を保存中...");
    let output_path = args.output.clone().unwrap_or_else(|| args.after.join("pairing.json"));
    let json = serde_json::to_string_pretty(&results)?;
    std::fs::write(&output_path, &json)?;
    println!("  保存: {} ({}件のペア)", output_path.display(), results.len());

    let unanimous = results.iter().filter(|r| r.confidence >= CONFIDENCE_UNANIMOUS).count();
    let majority = results.iter().filter(|r| r.confidence >= CONFIDENCE_MAJORITY && r.confidence < CONFIDENCE_UNANIMOUS).count();
    let split = results.iter().filter(|r| r.confidence < CONFIDENCE_MAJORITY).count();
    println!("\n結果サマリー:");
    println!("  全会一致 (3/3): {}件", unanimous);
    println!("  多数決 (2/3):   {}件", majority);
    println!("  不一致 (<2/3):  {}件 <- 要確認", split);

    if args.build {
        println!("\n--- Pフォルダ作成 ---");
        let output_dir = args.after.clone();
        let pairs = build_pair_folders(&results, &before_pages, &args.after, &output_dir)?;
        println!("\n{}件のPフォルダを作成: {}", pairs.len(), output_dir.display());
    }

    println!("\nペアリング完了");
    Ok(())
}

#[cfg(not(feature = "pdf-gen"))]
pub async fn handle_pair_completion(_args: PairCompletionCommandArgs) -> Result<()> {
    Err(PhotoAiError::CliExecution(
        "ペアリング機能には pdf-gen feature が必要です（photo-pdf-engine バイナリを使用してください）".into()
    ))
}

#[cfg(feature = "pdf-gen")]
fn build_pair_folders(
    results: &[PairResult],
    before_pages: &[ExtractedPage],
    after_folder: &Path,
    output_dir: &Path,
) -> Result<Vec<PairEntry>> {
    let mut pairs = Vec::new();
    let mut pair_num = 0u32;

    for pair in results.iter() {
        if pair.confidence < CONFIDENCE_MAJORITY || pair.after_file.is_empty() {
            continue;
        }
        pair_num += 1;

        let station_name = pair.before_station.lines().next().unwrap_or(&pair.before_station).trim().to_string();
        let safe_name = station_name.replace(['\\', '/', ':', '*', '?', '"', '<', '>', '|'], "_");
        let folder_name = format!("P{:02}_{}", pair_num, safe_name);
        let pair_dir = output_dir.join(&folder_name);
        std::fs::create_dir_all(&pair_dir)?;

        let before_path = if let Some(page) = before_pages.iter().find(|p| p.page_num == pair.before_page) {
            let src_name = page.image_path.file_name().unwrap_or_default();
            let dest = pair_dir.join(src_name);
            std::fs::copy(&page.image_path, &dest)?;
            dest
        } else {
            eprintln!("警告: 着手前画像が見つかりません (page {})", pair.before_page);
            continue;
        };

        let after_src = after_folder.join(&pair.after_file);
        if !after_src.exists() {
            eprintln!("警告: 竣工写真が見つかりません: {}", pair.after_file);
            continue;
        }
        let after_dest = pair_dir.join(&pair.after_file);
        std::fs::copy(&after_src, &after_dest)?;

        pairs.push(PairEntry { station_name, before_path, after_path: after_dest });
    }
    Ok(pairs)
}

#[cfg(feature = "pdf-gen")]
fn scan_after_folder(folder: &Path) -> Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder)?
        .filter_map(|entry| entry.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && matches!(p.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()).as_deref(), Some("jpg") | Some("jpeg")))
        .collect();
    files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(files)
}

fn scan_image_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && matches!(p.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()).as_deref(), Some("jpg") | Some("jpeg")))
        .collect();
    files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(files)
}

fn parse_pair_numbers(s: &str) -> Result<Vec<u32>> {
    let mut nums = Vec::new();
    for part in s.split(',') {
        if part.contains('-') {
            let mut range = part.split('-');
            let start = range.next().unwrap_or("").parse::<u32>().map_err(|_| PhotoAiError::Config(format!("不正な番号: {}", part)))?;
            let end = range.next().unwrap_or("").parse::<u32>().map_err(|_| PhotoAiError::Config(format!("不正な番号: {}", part)))?;
            for n in start..=end { nums.push(n); }
        } else {
            nums.push(part.parse::<u32>().map_err(|_| PhotoAiError::Config(format!("不正な番号: {}", part)))?);
        }
    }
    Ok(nums)
}

fn find_pair_folder(parent: &Path, pair_num: u32) -> Result<PathBuf> {
    let prefix = format!("P{:02}_", pair_num);
    for entry in std::fs::read_dir(parent)?.filter_map(|e| e.ok()) {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) { continue; }
        if entry.file_name().to_string_lossy().starts_with(&prefix) { return Ok(entry.path()); }
    }
    Err(PhotoAiError::FolderNotFound(format!("P{:02}_* フォルダが見つかりません", pair_num)))
}

fn find_before_after(dir: &Path) -> Result<(Option<PathBuf>, Option<PathBuf>)> {
    let mut before = None;
    let mut after = None;
    for entry in std::fs::read_dir(dir)?.filter_map(|e| e.ok()) {
        let path = entry.path();
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_lowercase();
        if stem.contains("before") { before = Some(path); }
        else if stem.contains("after") || path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()).as_deref() == Some("jpg") { after = Some(path); }
    }
    Ok((before, after))
}

pub fn scan_pair_folders(dir: &Path) -> Result<Vec<PairEntry>> {
    let mut entries = Vec::new();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.file_name().unwrap_or_default().to_string_lossy().starts_with('P'))
        .collect();
    dirs.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    for d in dirs {
        let (before, after) = find_before_after(&d)?;
        if let (Some(b), Some(a)) = (before, after) {
            let name = d.file_name().unwrap_or_default().to_string_lossy().to_string();
            let station = name.splitn(2, '_').nth(1).unwrap_or(&name).to_string();
            entries.push(PairEntry { station_name: station, before_path: b, after_path: a });
        }
    }
    Ok(entries)
}

pub fn handle_pair_replace(args: PairReplaceCommandArgs) -> Result<()> {
    println!("photo-ai pair-replace - after写真差し替え\n");
    let pairs = parse_pair_numbers(&args.pairs)?;
    let new_files = scan_image_files(&args.new_after)?;
    if pairs.len() != new_files.len() {
        return Err(PhotoAiError::Config(format!("件数不一致: {} vs {}", pairs.len(), new_files.len())));
    }
    for (pair_num, new_file) in pairs.iter().zip(new_files.iter()) {
        let pair_dir = find_pair_folder(&args.folder, *pair_num)?;
        let (_before, after) = find_before_after(&pair_dir)?;
        if let Some(ref old_after) = after { std::fs::remove_file(old_after)?; }
        let dest = pair_dir.join(new_file.file_name().unwrap_or_default());
        std::fs::copy(new_file, &dest)?;
        println!("  P{:02} : 差し替え完了", pair_num);
    }
    println!("\nPDF再生成中...");
    let pair_entries = scan_pair_folders(&args.folder)?;
    let output_dir = args.output.unwrap_or_else(|| args.folder.parent().unwrap_or(Path::new(".")).join("写真帳まとめ"));
    let pdf_path = output_dir.join(format!("着手前竣工_{}.pdf", args.project_name));
    crate::export::pair_pdf::generate_pair_pdf(&pair_entries, &args.project_name, &pdf_path)?;
    println!("\n完了: {}", pdf_path.display());
    Ok(())
}
