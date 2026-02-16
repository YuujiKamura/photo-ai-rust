//! マスタファイル対話式選択モジュール

use std::path::PathBuf;
use std::io::{self, Write};

/// master/by_work_type/ から利用可能なマスタ一覧を取得
pub fn list_available_masters() -> Vec<(String, PathBuf)> {
    let master_dir = PathBuf::from("master/by_work_type");

    if !master_dir.exists() {
        return Vec::new();
    }

    let mut masters = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&master_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "csv").unwrap_or(false) {
                if let Some(name) = path.file_stem() {
                    masters.push((name.to_string_lossy().to_string(), path));
                }
            }
        }
    }

    // 名前でソート
    masters.sort_by(|a, b| a.0.cmp(&b.0));
    masters
}

/// マスタ選択結果
pub struct MasterSelection {
    pub path: PathBuf,
    pub work_type: Option<String>,  // 工種名（全工種の場合はNone）
    pub all_paths: Option<Vec<PathBuf>>,  // 全工種の場合: by_work_type/*.csv + メインCSV
}

/// 対話式でマスタを選択（工種名も返す）
pub fn select_master_interactive() -> Option<MasterSelection> {
    let masters = list_available_masters();

    if masters.is_empty() {
        println!("⚠ master/by_work_type/ にマスタファイルがありません");
        return None;
    }

    println!("\n📋 工種マスタを選択してください:\n");
    println!("  0) 全工種 (マージ読み込み)");

    for (i, (name, path)) in masters.iter().enumerate() {
        // 件数を取得
        let count = count_csv_rows(path);
        println!("  {}) {} ({}件)", i + 1, name, count);
    }

    println!();
    print!("番号を入力 [0-{}]: ", masters.len());
    io::stdout().flush().ok();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return None;
    }

    let input = input.trim();

    // 空入力はデフォルト（全工種）
    if input.is_empty() {
        println!("→ 全工種マスタを使用（マージ読み込み）");
        return make_all_master_selection();
    }

    match input.parse::<usize>() {
        Ok(0) => {
            println!("→ 全工種マスタを使用（マージ読み込み）");
            make_all_master_selection()
        }
        Ok(n) if n >= 1 && n <= masters.len() => {
            let (name, path) = &masters[n - 1];
            println!("→ {} を使用", name);
            Some(MasterSelection {
                path: path.clone(),
                work_type: Some(name.clone()),
                all_paths: None,
            })
        }
        _ => {
            println!("⚠ 無効な入力です。全工種マスタを使用します");
            make_all_master_selection()
        }
    }
}

/// 全工種マージのMasterSelectionを作成
fn make_all_master_selection() -> Option<MasterSelection> {
    let all_paths = collect_all_master_paths();
    match all_paths {
        Some(paths) => {
            let first = paths[0].clone();
            Some(MasterSelection {
                path: first,
                work_type: None,
                all_paths: Some(paths),
            })
        }
        None => None,
    }
}

/// 全工種用: by_work_type/*.csv のパス一覧を返す
pub fn collect_all_master_paths() -> Option<Vec<PathBuf>> {
    let mut paths = Vec::new();
    let by_work_type_dir = PathBuf::from("master/by_work_type");
    if by_work_type_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&by_work_type_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "csv").unwrap_or(false) {
                    paths.push(path);
                }
            }
        }
    }
    if paths.is_empty() {
        None
    } else {
        paths.sort();
        Some(paths)
    }
}

/// CSVの行数を取得（ヘッダー除く）
fn count_csv_rows(path: &PathBuf) -> usize {
    std::fs::read_to_string(path)
        .map(|content| content.lines().count().saturating_sub(1))
        .unwrap_or(0)
}
