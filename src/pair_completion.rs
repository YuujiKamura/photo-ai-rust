//! 着手前・竣工写真 自動ペアリングモジュール（コンタクトシート+アンサンブル方式）
//!
//! Before/After写真をそれぞれ番号付きコンタクトシートにまとめ、
//! 1コール1問×アンサンブル（順逆走査3回）で90%精度のペアリングを実現する。

use crate::commands::{PairCompletionCommandArgs, PairReplaceCommandArgs};
use crate::contactsheet::generate_contact_sheet;
use crate::error::{PhotoAiError, Result};
use crate::export::pair_pdf::PairEntry;
use crate::pair_ensemble::ensemble_pair_query;
use crate::pair_extraction::{extract_images_from_pdf, ExtractedPage};
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
/// - PairEntry形式（フルパス入り）: そのまま返す
/// - pairing_manual形式（ファイル名のみ）: folder内のPフォルダから画像パスを解決
pub fn load_pairs_from_json(json_path: &Path, folder: Option<&Path>) -> Result<Vec<PairEntry>> {
    let content = std::fs::read_to_string(json_path)
        .map_err(|e| PhotoAiError::Config(format!("JSON読み込み失敗: {}", e)))?;

    // PairEntry形式を試す
    if let Ok(pairs) = serde_json::from_str::<Vec<PairEntry>>(&content) {
        return Ok(pairs);
    }

    // pairing_manual形式
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

        // Pフォルダ内からbefore/after画像を探す
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

/// Pフォルダ群の中から指定ファイル名に一致するファイルを探す
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

// --- ペアリング設定定数 ---
const BEFORE_SHEET_COLS: u32 = 5;
const AFTER_SHEET_COLS: u32 = 7;

// --- confidence閾値 ---
const CONFIDENCE_UNANIMOUS: f64 = 1.0;
const CONFIDENCE_MAJORITY: f64 = 0.67;

// --- データ型 ---

#[derive(Serialize, Deserialize, Debug)]
pub struct PairResult {
    pub before_page: u32,
    pub before_station: String,
    pub after_file: String,
    pub confidence: f64,
}

/// temp ディレクトリの自動削除ガード
struct TempDirGuard(PathBuf);
impl Drop for TempDirGuard {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

// --- メインハンドラ ---

pub async fn handle_pair_completion(args: PairCompletionCommandArgs) -> Result<()> {
    let verbose = args.cli_args.verbose;
    println!("photo-ai-rust - 着手前/竣工 ペアリング（コンタクトシート+アンサンブル方式）\n");

    // Step 1: 入力準備
    println!("[1/4] PDF画像抽出中...");
    let before_pages = extract_images_from_pdf(&args.before)?;
    println!("  着手前: {}ページ", before_pages.len());

    if before_pages.is_empty() {
        return Err(PhotoAiError::PdfExtraction(
            "PDFから画像を抽出できませんでした".into(),
        ));
    }

    // TempDirGuard: スコープ終了時にtempディレクトリを自動削除（空チェック後）
    let _temp_guard = TempDirGuard(before_pages[0].image_path.parent().unwrap().to_path_buf());

    let after_files = scan_after_folder(&args.after)?;
    println!("  竣工写真: {}枚", after_files.len());

    if after_files.is_empty() {
        return Err(PhotoAiError::NoImagesFound(
            args.after.display().to_string(),
        ));
    }

    // Step 2: コンタクトシート生成
    println!("[2/4] コンタクトシート生成中...");
    let temp_dir = before_pages[0]
        .image_path
        .parent()
        .unwrap_or(Path::new("."));

    let before_image_paths: Vec<PathBuf> =
        before_pages.iter().map(|p| p.image_path.clone()).collect();

    let before_sheet = generate_contact_sheet(
        &before_image_paths,
        "B",
        BEFORE_SHEET_COLS,
        &temp_dir.join("contact_before.jpg"),
    )?;

    let after_sheet = generate_contact_sheet(
        &after_files,
        "A",
        AFTER_SHEET_COLS,
        &temp_dir.join("contact_after.jpg"),
    )?;

    // コンタクトシートを竣工写真フォルダに恒久保存（解析の検証・記録用）
    let save_before = args.after.join("contact_before.jpg");
    let save_after = args.after.join("contact_after.jpg");
    std::fs::copy(&before_sheet.image_path, &save_before)?;
    std::fs::copy(&after_sheet.image_path, &save_after)?;
    println!("  保存: {}", save_before.display());
    println!("  保存: {}", save_after.display());

    // Step 3: アンサンブルペアリング（1コール1問×3走査）
    println!("[3/4] AIペアリング中（アンサンブル）...");
    let before_max = before_pages.len();
    let after_max = after_files.len();

    let mut results = Vec::new();
    for (i, page) in before_pages.iter().enumerate() {
        let b_num = i as u32 + 1;
        let query = format!("B{:02}", b_num);

        let (after_num, confidence) = ensemble_pair_query(
            &before_sheet,
            &after_sheet,
            &query,
            before_max as u32,
            after_max as u32,
            verbose,
        )?;

        // A番号 → ファイル名に逆引き
        let after_file = after_sheet
            .mapping
            .iter()
            .find(|(num, _)| *num == after_num)
            .map(|(_, name)| name.clone())
            .unwrap_or_else(|| format!("A{:02}", after_num));

        if verbose {
            println!(
                "  {} -> A{:02} ({}) confidence={:.2}",
                query, after_num, after_file, confidence
            );
        } else {
            let mark = if confidence >= CONFIDENCE_UNANIMOUS { "o" } else { "~" };
            println!("  {} {} A{:02}", query, mark, after_num);
        }

        results.push(PairResult {
            before_page: page.page_num,
            before_station: page.station_text.clone(),
            after_file,
            confidence,
        });
    }

    // Step 4: 結果を保存
    println!("[4/4] 結果を保存中...");
    let output_path = args
        .output
        .clone()
        .unwrap_or_else(|| PathBuf::from("pairing.json"));
    let json = serde_json::to_string_pretty(&results)?;
    std::fs::write(&output_path, &json)?;
    println!("  保存: {} ({}件のペア)", output_path.display(), results.len());

    // サマリー表示
    let unanimous = results.iter().filter(|r| r.confidence >= CONFIDENCE_UNANIMOUS).count();
    let majority = results
        .iter()
        .filter(|r| r.confidence >= CONFIDENCE_MAJORITY && r.confidence < CONFIDENCE_UNANIMOUS)
        .count();
    let split = results.iter().filter(|r| r.confidence < CONFIDENCE_MAJORITY).count();
    println!("\n結果サマリー:");
    println!("  全会一致 (3/3): {}件", unanimous);
    println!("  多数決 (2/3):   {}件", majority);
    println!("  不一致 (<2/3):  {}件 <- 要確認", split);

    // --build: フルパスJSON生成 + PDF生成
    if args.build {
        let project_name = args.project_name.as_deref().unwrap_or("工事");
        println!("\n--- フルパスJSON + PDF生成 ---");

        let output_dir = args
            .after
            .parent()
            .unwrap_or(Path::new("."))
            .join("写真帳まとめ");
        std::fs::create_dir_all(&output_dir)?;

        let pairs = build_pair_json(&results, &before_pages, &args.after, &output_dir)?;

        if pairs.is_empty() {
            eprintln!("警告: 有効なペアがありません");
        } else {
            // フルパスJSON保存
            let json_path = output_dir.join("pair_entries.json");
            let json = serde_json::to_string_pretty(&pairs)?;
            std::fs::write(&json_path, &json)?;
            println!("\nペアJSON: {} ({}件)", json_path.display(), pairs.len());

            // PDF生成
            let pdf_path = output_dir.join(format!("着手前竣工_{}.pdf", project_name));
            crate::export::pair_pdf::generate_pair_pdf(&pairs, project_name, &pdf_path)?;
            println!("PDF生成完了: {}", pdf_path.display());
        }
    }

    println!("\nペアリング完了");

    Ok(())
}

// --- --build: フルパスJSON生成 ---

/// ペアリング結果からフルパスのPairEntryリストを生成し、before画像を恒久保存
fn build_pair_json(
    results: &[PairResult],
    before_pages: &[ExtractedPage],
    after_folder: &Path,
    output_dir: &Path,
) -> Result<Vec<PairEntry>> {
    let before_dir = output_dir.join("before_extracted");
    std::fs::create_dir_all(&before_dir)?;

    let mut pairs = Vec::new();
    for (i, pair) in results.iter().enumerate() {
        if pair.confidence < CONFIDENCE_MAJORITY || pair.after_file.is_empty() {
            continue;
        }

        // before画像を恒久保存
        let before_path = if let Some(page) = before_pages
            .iter()
            .find(|p| p.page_num == pair.before_page)
        {
            let dest = before_dir.join(format!("before_{:02}.jpg", i + 1));
            std::fs::copy(&page.image_path, &dest)?;
            dest
        } else {
            eprintln!("警告: 着手前画像が見つかりません (page {})", pair.before_page);
            continue;
        };

        // after画像パス解決
        let after_path = after_folder.join(&pair.after_file);
        if !after_path.exists() {
            eprintln!("警告: 竣工写真が見つかりません: {}", pair.after_file);
            continue;
        }

        let station_name = pair.before_station
            .lines()
            .next()
            .unwrap_or(&pair.before_station)
            .trim()
            .to_string();

        pairs.push(PairEntry {
            station_name,
            before_path,
            after_path,
        });

        println!("  P{:02} {} -> {}", i + 1,
            pair.before_station.lines().next().unwrap_or("?").trim(),
            pair.after_file);
    }

    Ok(pairs)
}

// --- 竣工写真スキャン ---

fn scan_after_folder(folder: &Path) -> Result<Vec<PathBuf>> {
    if !folder.exists() || !folder.is_dir() {
        return Err(PhotoAiError::FolderNotFound(folder.display().to_string()));
    }

    let mut files: Vec<PathBuf> = std::fs::read_dir(folder)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            // Pフォルダ内の画像は除外（サブディレクトリのファイルは含まない）
            if !path.is_file() {
                return false;
            }
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| matches!(ext.to_lowercase().as_str(), "jpg" | "jpeg" | "png"))
                .unwrap_or(false)
        })
        .collect();

    files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(files)
}

// --- ペアリングフォルダスキャン ---

/// ペアリングフォルダをスキャンして PairEntry リストを構築
pub fn scan_pair_folders(folder: &Path) -> Result<Vec<PairEntry>> {
    let mut pairs = Vec::new();

    let entries: Vec<_> = std::fs::read_dir(folder)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .collect();

    // P番号の数値部分を抽出してソート
    let mut dir_entries: Vec<(u32, std::fs::DirEntry)> = Vec::new();
    for entry in entries {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // P{NN}_{測点名} パターンのみ対象
        if !name_str.starts_with('P') {
            continue;
        }
        let underscore_pos = match name_str.find('_') {
            Some(pos) => pos,
            None => continue,
        };
        let prefix = &name_str[1..underscore_pos];
        if prefix.is_empty() || !prefix.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let num: u32 = prefix.parse().unwrap_or(0);
        dir_entries.push((num, entry));
    }
    dir_entries.sort_by_key(|(num, _)| *num);

    for (_, entry) in dir_entries {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let underscore_pos = name_str.find('_').unwrap();

        let station_name = &name_str[underscore_pos + 1..];
        let dir = entry.path();

        // before/after写真を探す
        let (before, after) = find_before_after(&dir)?;

        match (before, after) {
            (Some(b), Some(a)) => {
                pairs.push(PairEntry {
                    station_name: station_name.to_string(),
                    before_path: b,
                    after_path: a,
                });
            }
            (None, Some(_)) => eprintln!("警告: {} - 着手前写真が見つかりません", name_str),
            (Some(_), None) => eprintln!("警告: {} - 竣工写真が見つかりません", name_str),
            (None, None) => eprintln!("警告: {} - 写真が見つかりません", name_str),
        }
    }

    Ok(pairs)
}

/// フォルダ内から着手前/竣工の写真ファイルを検出
///
/// 判定優先順位:
/// 1. ファイル名に "before"/"after" を含む
/// 2. 拡張子が大文字(.JPG) → 着手前、小文字(.jpg) → 竣工
fn find_before_after(dir: &Path) -> Result<(Option<PathBuf>, Option<PathBuf>)> {
    let image_extensions = ["jpg", "jpeg", "png", "bmp"];
    let mut images = Vec::new();

    for entry in std::fs::read_dir(dir)?.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() { continue; }
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        if image_extensions.contains(&ext.as_str()) {
            images.push(path);
        }
    }

    let mut before = None;
    let mut after = None;

    // 1. ファイル名による判定
    for path in &images {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        if stem.contains("before") {
            before = Some(path.clone());
        } else if stem.contains("after") {
            after = Some(path.clone());
        }
    }

    // 2. 拡張子の大文字/小文字で判定（フォールバック）
    if before.is_none() || after.is_none() {
        for path in &images {
            let ext_raw = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let is_upper = ext_raw.chars().any(|c| c.is_alphabetic())
                && ext_raw.chars().all(|c| !c.is_lowercase());
            if is_upper && before.is_none() {
                before = Some(path.clone());
            } else if !is_upper && after.is_none() {
                after = Some(path.clone());
            }
        }
    }

    Ok((before, after))
}

// --- pair-replace ---

/// `--pairs` 引数をパースしてペア番号リストを返す
///
/// "1-3" → [1, 2, 3], "1,3,5" → [1, 3, 5], "2" → [2]
fn parse_pair_numbers(s: &str) -> Result<Vec<u32>> {
    let s = s.trim();
    if s.is_empty() {
        return Err(PhotoAiError::Config("--pairs が空です".into()));
    }

    let mut numbers = Vec::new();

    if s.contains('-') && !s.contains(',') {
        // 範囲指定: "1-3"
        let parts: Vec<&str> = s.splitn(2, '-').collect();
        if parts.len() != 2 {
            return Err(PhotoAiError::Config(format!("--pairs の範囲指定が不正です: {}", s)));
        }
        let start: u32 = parts[0].trim().parse()
            .map_err(|_| PhotoAiError::Config(format!("--pairs の数値が不正です: {}", parts[0])))?;
        let end: u32 = parts[1].trim().parse()
            .map_err(|_| PhotoAiError::Config(format!("--pairs の数値が不正です: {}", parts[1])))?;
        if start == 0 || end == 0 || start > end {
            return Err(PhotoAiError::Config(format!("--pairs の範囲が不正です: {}-{}", start, end)));
        }
        for n in start..=end {
            numbers.push(n);
        }
    } else if s.contains(',') {
        // カンマ区切り: "1,3,5"
        for part in s.split(',') {
            let n: u32 = part.trim().parse()
                .map_err(|_| PhotoAiError::Config(format!("--pairs の数値が不正です: {}", part.trim())))?;
            if n == 0 {
                return Err(PhotoAiError::Config("--pairs のペア番号は1以上で指定してください".into()));
            }
            numbers.push(n);
        }
    } else {
        // 単一番号: "2"
        let n: u32 = s.parse()
            .map_err(|_| PhotoAiError::Config(format!("--pairs の数値が不正です: {}", s)))?;
        if n == 0 {
            return Err(PhotoAiError::Config("--pairs のペア番号は1以上で指定してください".into()));
        }
        numbers.push(n);
    }

    Ok(numbers)
}

/// 画像ファイルをスキャンしてソート済みリストを返す
fn scan_image_files(folder: &Path) -> Result<Vec<PathBuf>> {
    if !folder.exists() || !folder.is_dir() {
        return Err(PhotoAiError::FolderNotFound(folder.display().to_string()));
    }

    let mut files: Vec<PathBuf> = std::fs::read_dir(folder)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| matches!(ext.to_lowercase().as_str(), "jpg" | "jpeg" | "png"))
                    .unwrap_or(false)
        })
        .collect();

    files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(files)
}

/// P{NN}_* にマッチするフォルダを探す
fn find_pair_folder(parent: &Path, pair_num: u32) -> Result<PathBuf> {
    let prefix = format!("P{:02}_", pair_num);
    for entry in std::fs::read_dir(parent)?.filter_map(|e| e.ok()) {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with(&prefix) {
            return Ok(entry.path());
        }
    }
    Err(PhotoAiError::FolderNotFound(format!(
        "P{:02}_* フォルダが見つかりません: {}",
        pair_num,
        parent.display()
    )))
}

/// pair-replace コマンドのハンドラ
pub fn handle_pair_replace(args: PairReplaceCommandArgs) -> Result<()> {
    println!("photo-ai pair-replace - after写真差し替え\n");

    // 1. ペア番号パース
    let pairs = parse_pair_numbers(&args.pairs)?;
    println!("差し替え対象: {:?}", pairs);

    // 2. 新しい写真をスキャン
    let new_files = scan_image_files(&args.new_after)?;
    println!("新しい写真: {}枚", new_files.len());

    // 3. 枚数一致チェック
    if pairs.len() != new_files.len() {
        return Err(PhotoAiError::Config(format!(
            "ペア番号の数({})と新しい写真の枚数({})が一致しません",
            pairs.len(),
            new_files.len()
        )));
    }

    // 4. 各ペアのafter写真を差し替え
    for (pair_num, new_file) in pairs.iter().zip(new_files.iter()) {
        let pair_dir = find_pair_folder(&args.folder, *pair_num)?;
        let pair_name = pair_dir.file_name().unwrap_or_default().to_string_lossy().to_string();

        // 既存のafter写真を特定
        let (_before, after) = find_before_after(&pair_dir)?;

        // 古いafter写真を削除
        if let Some(ref old_after) = after {
            std::fs::remove_file(old_after)?;
            println!(
                "  {} : 削除 {}",
                pair_name,
                old_after.file_name().unwrap_or_default().to_string_lossy()
            );
        }

        // 新しい写真をPフォルダにコピー
        let new_name = new_file.file_name().unwrap_or_default();
        let dest = pair_dir.join(new_name);
        std::fs::copy(new_file, &dest)?;
        println!(
            "  {} : コピー {} -> {}",
            pair_name,
            new_name.to_string_lossy(),
            dest.display()
        );
    }

    // 5. PDF再生成
    println!("\nPDF再生成中...");
    let pair_entries = scan_pair_folders(&args.folder)?;
    if pair_entries.is_empty() {
        return Err(PhotoAiError::Config("ペアフォルダが見つかりません".into()));
    }

    let output_dir = args.output.unwrap_or_else(|| {
        args.folder
            .parent()
            .unwrap_or(Path::new("."))
            .join("写真帳まとめ")
    });
    let pdf_path = output_dir.join(format!("着手前竣工_{}.pdf", args.project_name));
    crate::export::pair_pdf::generate_pair_pdf(&pair_entries, &args.project_name, &pdf_path)?;

    println!("\n完了: {}", pdf_path.display());
    Ok(())
}
