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

/// 対話式でマスタを選択
pub fn select_master_interactive() -> Option<PathBuf> {
    let masters = list_available_masters();

    if masters.is_empty() {
        println!("⚠ master/by_work_type/ にマスタファイルがありません");
        println!("  デフォルトマスタ (master/construction_hierarchy.csv) を使用します");
        let default = PathBuf::from("master/construction_hierarchy.csv");
        if default.exists() {
            return Some(default);
        }
        return None;
    }

    println!("\n📋 工種マスタを選択してください:\n");
    println!("  0) 全工種 (construction_hierarchy.csv)");

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

    // 空入力はデフォルト
    if input.is_empty() {
        println!("→ 全工種マスタを使用");
        return Some(PathBuf::from("master/construction_hierarchy.csv"));
    }

    match input.parse::<usize>() {
        Ok(0) => {
            println!("→ 全工種マスタを使用");
            Some(PathBuf::from("master/construction_hierarchy.csv"))
        }
        Ok(n) if n >= 1 && n <= masters.len() => {
            let (name, path) = &masters[n - 1];
            println!("→ {} を使用", name);
            Some(path.clone())
        }
        _ => {
            println!("⚠ 無効な入力です。全工種マスタを使用します");
            Some(PathBuf::from("master/construction_hierarchy.csv"))
        }
    }
}

/// CSVの行数を取得（ヘッダー除く）
fn count_csv_rows(path: &PathBuf) -> usize {
    std::fs::read_to_string(path)
        .map(|content| content.lines().count().saturating_sub(1))
        .unwrap_or(0)
}
