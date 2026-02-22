//! マスタファイル対話式選択モジュール

use std::path::PathBuf;
use std::io::{self, Write};

/// マスタディレクトリ（master/by_work_type を含む親）を解決する。
///
/// フォールバックチェーン:
/// 1. `PHOTO_AI_MASTER_DIR` 環境変数
/// 2. 実行ファイルからの相対（`exe_dir/../../master`）
/// 3. `~/.config/photo-ai/master/`
/// 4. CWD相対（後方互換）
pub fn resolve_master_base_dir() -> Option<PathBuf> {
    let candidates = build_master_candidates();
    candidates.into_iter().find(|p| p.join("by_work_type").is_dir())
}

/// 検索候補パス一覧を構築する（エラーメッセージ表示用にも使う）
pub fn build_master_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    // 1. PHOTO_AI_MASTER_DIR 環境変数
    if let Ok(env_dir) = std::env::var("PHOTO_AI_MASTER_DIR") {
        let p = PathBuf::from(&env_dir);
        // 環境変数が by_work_type を直接指している場合は親を使う
        if p.file_name().map(|n| n == "by_work_type").unwrap_or(false) {
            if let Some(parent) = p.parent() {
                candidates.push(parent.to_path_buf());
            }
        } else {
            candidates.push(p);
        }
    }

    // 2. 実行ファイルからの相対（exe_dir/../../master）
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            // target/debug/ or target/release/ → repo root
            if let Some(repo_root) = exe_dir.parent().and_then(|p| p.parent()) {
                candidates.push(repo_root.join("master"));
            }
        }
    }

    // 3. ~/.config/photo-ai/master/
    if let Some(config_dir) = dirs::config_dir() {
        candidates.push(config_dir.join("photo-ai").join("master"));
    }

    // 4. CWD相対（後方互換）
    candidates.push(PathBuf::from("master"));

    candidates
}

/// master/by_work_type/ から利用可能なマスタ一覧を取得
pub fn list_available_masters() -> Vec<(String, PathBuf)> {
    let master_dir = match resolve_master_base_dir() {
        Some(base) => base.join("by_work_type"),
        None => return Vec::new(),
    };

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
        eprintln!("⚠ マスタファイルが見つかりません");
        eprintln!("  検索パス:");
        for candidate in build_master_candidates() {
            let status = if candidate.join("by_work_type").is_dir() { "OK" } else { "NG" };
            eprintln!("    [{}] {}/by_work_type/", status, candidate.display());
        }
        eprintln!("  対処: --master で直接指定 or PHOTO_AI_MASTER_DIR 環境変数を設定");
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
    let by_work_type_dir = match resolve_master_base_dir() {
        Some(base) => base.join("by_work_type"),
        None => return None,
    };
    if let Ok(entries) = std::fs::read_dir(&by_work_type_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "csv").unwrap_or(false) {
                paths.push(path);
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
