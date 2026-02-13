//! 解析関連のヘルパー関数とデータ構造
//!
//! AI写真解析のためのスキャン、解析、正規化処理を提供する。

use crate::{ai_provider::AiProvider, analyzer, error, scanner};
use crate::normalizer::{self, NormalizationOptions};
use photo_ai_common::{HierarchyMaster, LineTypeEntry};
use photo_tagger::GroupRecords;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::error::Result;

/// スキャンと解析の設定
pub struct ScanAnalysisConfig<'a> {
    pub folder: &'a Path,
    pub batch_size: usize,
    pub verbose: bool,
    pub master_config: &'a MasterConfig,
    pub photo_type: Option<&'a str>,
    pub use_cache: bool,
    pub provider: AiProvider,
    pub variety: Option<&'a String>,
    pub station: Option<&'a String>,
    pub recursive: bool,
    pub include_all: bool,
    pub step_prefix_scan: &'a str,
    pub step_prefix_analyze: &'a str,
    pub line_types: Option<&'a [LineTypeEntry]>,
}

/// AI解析のオプション
pub struct AnalysisOptions<'a> {
    pub folder: &'a Path,
    pub batch_size: usize,
    pub verbose: bool,
    pub master: Option<&'a Path>,
    pub photo_type: Option<&'a str>,
    pub use_cache: bool,
    pub provider: AiProvider,
    pub work_type: Option<&'a str>,
    pub variety: Option<&'a str>,
    pub step_prefix: &'a str,
}

/// マスタ選択結果
pub struct MasterConfig {
    pub master_path: Option<PathBuf>,
    pub effective_work_type: Option<String>,
    /// 全工種時のマージ用パス一覧（by_work_type/*.csv + メインCSV）
    pub all_paths: Option<Vec<PathBuf>>,
}

/// マスタ選択と検証を行う共通関数
pub fn prepare_analysis(
    master: Option<PathBuf>,
    work_type: Option<String>,
    variety: Option<&String>,
    resolve_master_path: impl FnOnce(Option<PathBuf>, bool) -> Option<crate::master_selector::MasterSelection>,
) -> Result<MasterConfig> {
    // マスタ選択（対話式または引数から）
    let has_master_arg = master.is_some();
    let selection = resolve_master_path(master, !has_master_arg && work_type.is_none());

    // work_type: CLI引数優先、なければ選択結果から
    let effective_work_type = work_type.or_else(|| selection.as_ref().and_then(|s| s.work_type.clone()));
    let all_paths = selection.as_ref().and_then(|s| s.all_paths.clone());
    let master_path = selection.map(|s| s.path);

    // 検証
    if variety.is_some() && effective_work_type.is_none() {
        return Err(error::PhotoAiError::InvalidMaster(
            "variety指定にはwork_typeが必要です".to_string(),
        ));
    }
    if effective_work_type.is_some() && master_path.is_none() {
        return Err(error::PhotoAiError::MasterLoad(
            "work_type指定にはマスタが必要です".to_string(),
        ));
    }

    Ok(MasterConfig {
        master_path,
        effective_work_type,
        all_paths,
    })
}

/// MasterConfigからHierarchyMasterを読み込む
///
/// all_pathsがある場合はマージ読み込み、なければ単一ファイル読み込み
fn load_master_from_config(config: &MasterConfig) -> Result<HierarchyMaster> {
    if let Some(ref all_paths) = config.all_paths {
        HierarchyMaster::from_csv_files(all_paths)
            .map_err(|e| error::PhotoAiError::MasterLoad(e.to_string()))
    } else if let Some(ref path) = config.master_path {
        HierarchyMaster::from_csv(path)
            .map_err(|e| error::PhotoAiError::MasterLoad(e.to_string()))
    } else {
        // マスタパスなし: デフォルトマスタをフォールバック
        let default = PathBuf::from("master/construction_hierarchy.csv");
        if default.exists() {
            HierarchyMaster::from_csv(&default)
                .map_err(|e| error::PhotoAiError::MasterLoad(e.to_string()))
        } else {
            Err(error::PhotoAiError::MasterLoad("マスタファイルが見つかりません".to_string()))
        }
    }
}

/// スキャンから正規化までを行う共通関数
pub async fn scan_and_analyze(config: &ScanAnalysisConfig<'_>) -> Result<Vec<analyzer::AnalysisResult>> {
    // 1. 画像スキャン
    let images = scan_images(
        config.folder,
        config.recursive,
        config.include_all,
        config.step_prefix_scan,
    )?;

    // 2. photo-tagger（無条件実行）
    println!("{} photo-tagger実行中...", config.step_prefix_analyze);
    let group_records = photo_tagger::run_grouping(config.folder, config.batch_size)
        .map_err(|e| error::PhotoAiError::ApiCall(format!("photo-tagger: {}", e)))?;

    if group_records.is_empty() {
        return Err(error::PhotoAiError::NoImagesFound(
            format!("photo-taggerの結果が空: {}", config.folder.display())
        ));
    }

    // 3. マスタ照合
    let master = load_master_from_config(config.master_config)?;

    let grouped_images: Vec<_> = images.iter()
        .filter(|img| group_records.contains_key(&img.file_name))
        .cloned()
        .collect();
    let skipped = images.len() - grouped_images.len();
    if skipped > 0 {
        println!("  {} 枚をスキップ（グループ未登録）", skipped);
    }

    let folder_name = config.folder.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let mut results = convert_groups_to_results(&grouped_images, &group_records, &master, folder_name, config.line_types);
    println!("✔ マスタ照合完了（{}枚）\n", results.len());

    // 4. 正規化
    normalize_results_with_station(
        &mut results,
        config.station.map(|s| s.as_str()),
        config.verbose,
    );

    Ok(results)
}

/// 画像スキャンのみを実行
///
/// # Arguments
/// * `folder` - スキャン対象フォルダ
/// * `recursive` - 再帰スキャンするか
/// * `include_all` - 全ファイルを含めるか（falseの場合はJPEG/PNGのみ）
/// * `step_prefix` - ステップ表示用プレフィックス（例: "[1/3]"）
///
/// # Returns
/// スキャンされた画像情報のベクタ
pub fn scan_images(
    folder: &Path,
    recursive: bool,
    include_all: bool,
    step_prefix: &str,
) -> Result<Vec<scanner::ImageInfo>> {
    println!("{} 写真をスキャン中...{}", step_prefix, if recursive { " (再帰)" } else { "" });
    let images = scanner::scan_folder_full(folder, recursive, !include_all)?;
    println!("✔ {}枚の写真を検出\n", images.len());

    if images.is_empty() {
        return Err(error::PhotoAiError::NoImagesFound(
            folder.display().to_string()
        ));
    }

    Ok(images)
}

/// 測点適用と正規化を実行
///
/// # Arguments
/// * `results` - 解析結果（変更される）
/// * `station` - 適用する測点（Noneの場合はスキップ）
/// * `verbose` - 詳細出力するか
pub fn normalize_results_with_station(
    results: &mut Vec<analyzer::AnalysisResult>,
    station: Option<&str>,
    verbose: bool,
) {
    // 測点一括適用
    if let Some(st) = station {
        println!("  測点を一括適用: {}", st);
        apply_station(results, st);
    }

    // 正規化（3枚セット内で黒板アップの値に統一）
    let norm_options = NormalizationOptions::default();
    let norm_result = normalizer::normalize_results(results, &norm_options);
    if !norm_result.corrections.is_empty() {
        if verbose {
            println!("  計測値統一: {}件", norm_result.stats.measurement_corrections);
            for c in &norm_result.corrections {
                println!("    {} → {} ({})", c.file_name, c.corrected, c.reason);
            }
        }
        normalizer::apply_corrections(results, &norm_result.corrections);
    }
}

/// AI解析を実行（常に1ステップ解析）
pub async fn run_analysis(
    images: &[scanner::ImageInfo],
    options: &AnalysisOptions<'_>,
) -> Result<Vec<analyzer::AnalysisResult>> {
    // マスタパスを決定
    let master_path_buf = resolve_master_path(options.master, options.work_type)?;

    // 工種指定なし + by_work_typeディレクトリあり → マージ読み込み
    let hierarchy = if options.work_type.is_none() {
        if let Some(all_paths) = crate::master_selector::collect_all_master_paths() {
            HierarchyMaster::from_csv_files(&all_paths)
                .map_err(|e| error::PhotoAiError::MasterLoad(e.to_string()))?
        } else {
            HierarchyMaster::from_csv(&master_path_buf)
                .map_err(|e| error::PhotoAiError::MasterLoad(e.to_string()))?
        }
    } else {
        HierarchyMaster::from_csv(&master_path_buf)
            .map_err(|e| error::PhotoAiError::MasterLoad(e.to_string()))?
    };

    // photo_type指定時はそちらでフィルタ、なければwork_typeでフィルタ
    let master = if let Some(pt) = options.photo_type {
        println!("{} 1ステップ解析中 (写真種類: {})...", options.step_prefix, pt);
        let filtered = hierarchy.filter_by_photo_type(pt);
        println!("  マスタ読み込み: {}件 (写真種類: {})", filtered.rows().len(), pt);
        filtered
    } else if let Some(wt) = options.work_type {
        println!("{} 1ステップ解析中 (工種: {})...", options.step_prefix, wt);
        let filtered = hierarchy.filter_by_work_types(&[wt.to_string()]);
        println!("  マスタ読み込み: {}件 (工種: {})", filtered.rows().len(), wt);
        filtered
    } else {
        println!("{} 1ステップ解析中 (全工種)...", options.step_prefix);
        println!("  マスタ読み込み: {}件", hierarchy.rows().len());
        hierarchy
    };

    analyzer::analyze_images_single_step(
        images,
        &master,
        options.work_type,
        options.variety,
        options.photo_type,
        options.batch_size,
        options.verbose,
        options.provider,
    ).await
}

/// マスタファイルのパスを解決する
///
/// 優先順位:
/// 1. 明示的に指定されたマスタパス
/// 2. 工種指定時: 工種別マスタ (master/by_work_type/{work_type}.csv)
/// 3. デフォルトマスタ (master/construction_hierarchy.csv)
pub fn resolve_master_path(master: Option<&Path>, work_type: Option<&str>) -> Result<PathBuf> {
    if let Some(mp) = master {
        return Ok(mp.to_path_buf());
    }

    // 工種指定時は工種別マスタを優先
    if let Some(wt) = work_type {
        let by_work_type = PathBuf::from("master/by_work_type").join(format!("{}.csv", wt));
        if by_work_type.exists() {
            return Ok(by_work_type);
        }
    }

    // デフォルトマスタ
    let default = PathBuf::from("master/construction_hierarchy.csv");
    if default.exists() {
        return Ok(default);
    }

    Err(error::PhotoAiError::MasterLoad("マスタファイルが見つかりません".to_string()))
}

/// 後方互換: 工種指定必須版
pub fn resolve_master_for_work_type(master: Option<&Path>, work_type: &str) -> Result<PathBuf> {
    resolve_master_path(master, Some(work_type))
}

/// 全景を先頭にするためのロール優先度
fn role_priority(role: &str) -> u8 {
    if role.contains("全景") { 0 }
    else if role.contains("証票") { 1 }
    else if role.contains("ナンバー") { 2 }
    else { 3 }
}

/// 既知の黒板キー一覧
const KNOWN_KEYS: &[&str] = &["工事名", "場所", "工種", "測点", "車番", "車両番号"];

/// 2つの文字列のトークン重複スコアを計算
///
/// 日本語2文字トークン（bigram）で分割し、一致数を返す。
/// 例: "乳剤端部塗布状況" と "端部乳剤塗布状況" → "乳剤","端部","塗布","状況" が共通 → 4
fn token_overlap_score(a: &str, b: &str) -> usize {
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    // 完全一致は最高スコア
    if a == b {
        return 100;
    }
    // 部分文字列一致
    if b.contains(a) || a.contains(b) {
        return 50;
    }
    // 2文字bigramで重複カウント
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    if a_chars.len() < 2 || b_chars.len() < 2 {
        return 0;
    }
    let a_bigrams: std::collections::HashSet<(char, char)> = a_chars.windows(2)
        .map(|w| (w[0], w[1]))
        .collect();
    let b_bigrams: std::collections::HashSet<(char, char)> = b_chars.windows(2)
        .map(|w| (w[0], w[1]))
        .collect();
    a_bigrams.intersection(&b_bigrams).count()
}

/// detected_textからキー:値ペアを抽出
///
/// photo-taggerのdetected_textは改行区切り・全角コロンまたはスペース区切り:
/// ```text
/// 工事名：市道 南千反畑第1号線舗装補修工事\n場所：No.4 L\n路面切削工\n切削・積込状況
/// 工事名 市道 南千反畑第1号線舗装補修工事\n場所 No. 4 L\n表層工\n乳剤散布状況
/// ```
///
/// キーなし行（"路面切削工"、"切削・積込状況"）は ("", value) として返す
fn extract_kv_from_text(text: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for line in text.split('\n') {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // 全角コロン `：` または半角コロン `:` で分割を試みる
        if let Some((k, v)) = line.split_once('：').or_else(|| line.split_once(':')) {
            let k = k.trim();
            let v = v.trim();
            if !k.is_empty() {
                result.push((k.to_string(), v.to_string()));
                continue;
            }
        }
        // 既知キーでスペース分割を試みる（"場所 No. 4 L" 等）
        let mut matched = false;
        for &key in KNOWN_KEYS {
            if line.starts_with(key) && line.len() > key.len() {
                let rest = line[key.len()..].trim_start();
                result.push((key.to_string(), rest.to_string()));
                matched = true;
                break;
            }
        }
        if !matched {
            // キーなし行はキーワードとして ("", value) で返す
            result.push((String::new(), line.to_string()));
        }
    }
    result
}

/// フォルダ内の全detected_textから工種階層を推定し、マスタ照合する
fn match_master_from_detected_texts(
    master: &HierarchyMaster,
    detected_texts: &[&str],
    folder_name: &str,
) -> Option<photo_ai_common::hierarchy::HierarchyRow> {
    // 全detected_textからキー:値を集約
    let mut work_type: Option<String> = None;
    let mut variety_hint: Option<String> = None;
    let mut keywords: Vec<String> = Vec::new();

    for text in detected_texts {
        for (key, value) in extract_kv_from_text(text) {
            match key.as_str() {
                "工種" => { work_type = Some(value); }
                "工事名" | "車番" | "車両番号" => {} // 照合に不要
                "場所" | "測点" => {} // 測点は別管理
                "" => {
                    // キーなし行: 値全体をキーワードに（"路面切削工"、"切削・積込状況" 等）
                    if !value.is_empty() {
                        keywords.push(value);
                    }
                }
                _ => {
                    // キー自体もキーワードに（"処分状況" など）
                    keywords.push(key);
                    if !value.is_empty() {
                        keywords.push(value);
                    }
                }
            }
        }
    }

    // キーワードの中にマスタのvarietyと完全一致するものがあればvariety_hintとして使う
    // （黒板に「路面切削工」「表層工」等が直接書かれているケース）
    if variety_hint.is_none() {
        for kw in &keywords {
            if master.rows().iter().any(|r| r.variety == *kw || r.subphase == *kw) {
                variety_hint = Some(kw.clone());
                break;
            }
        }
    }

    // variety_hintとして使ったキーワードはremarks照合のノイズになるため除外
    if let Some(ref vh) = variety_hint {
        keywords.retain(|kw| kw != vh);
    }

    // フォルダ名のトークンもキーワードに追加（ただし汎用的すぎる語は除外）
    const GENERIC_FOLDER_NAMES: &[&str] = &[
        "施工状況", "品質管理", "出来形管理", "安全管理", "使用材料", "完成写真", "着手前",
    ];
    for token in folder_name.split(&['_', '　', ' ', '・'][..]) {
        let t = token.trim();
        if !t.is_empty() && !GENERIC_FOLDER_NAMES.contains(&t) {
            keywords.push(t.to_string());
        }
    }

    // 工種・種別でマスタをフィルタ
    let candidates: Vec<_> = master.rows().iter()
        .filter(|r| {
            // work_type フィルタ
            if let Some(wt) = &work_type {
                if r.work_type != *wt && !r.work_type.is_empty() {
                    return false;
                }
            }
            // variety_hint フィルタ: varietyまたはsubphaseに一致する行を優先
            if let Some(vh) = &variety_hint {
                // varietyかsubphaseにhintが含まれる行のみ通す
                r.variety == *vh || r.subphase == *vh || r.variety.contains(vh.as_str()) || vh.contains(&r.variety)
            } else {
                true
            }
        })
        .collect();

    // 1. 検索パターン列でマッチ
    let mut best: Option<&photo_ai_common::hierarchy::HierarchyRow> = None;
    let mut best_score: usize = 0;

    for row in &candidates {
        // 検索パターンがあればそれで照合
        if !row.search_patterns.is_empty() {
            let patterns: Vec<&str> = row.search_patterns.split('|').collect();
            let score = keywords.iter()
                .filter(|kw| patterns.iter().any(|p| kw.contains(p) || p.contains(kw.as_str())))
                .count();
            if score > best_score {
                best_score = score;
                best = Some(row);
            }
        }
    }

    if best.is_some() {
        return best.cloned();
    }

    // 2. remarks列にキーワード部分一致（トークンベース：語順違いに対応）
    for row in &candidates {
        if row.remarks.is_empty() { continue; }
        let score = keywords.iter()
            .map(|kw| token_overlap_score(kw, &row.remarks))
            .sum::<usize>();
        if score > best_score {
            best_score = score;
            best = Some(row);
        }
    }

    best.cloned()
}

/// photo-groups.json のレコードを AnalysisResult に変換し、マスタ照合する
/// グループ番号→全景優先でソート
fn convert_groups_to_results(
    images: &[scanner::ImageInfo],
    groups: &GroupRecords,
    master: &HierarchyMaster,
    folder_name: &str,
    line_types: Option<&[LineTypeEntry]>,
) -> Vec<analyzer::AnalysisResult> {
    // フォルダ名でのフォールバック照合（detected_textが空の写真用）
    let folder_fallback_row = {
        let empty: Vec<&str> = Vec::new();
        match_master_from_detected_texts(master, &empty, folder_name)
    };

    let mut results: Vec<(u32, u8, analyzer::AnalysisResult)> = images.iter().filter_map(|img| {
        let rec = groups.get(&img.file_name)?;
        let mut result = analyzer::AnalysisResult {
            file_name: img.file_name.clone(),
            file_path: img.path.display().to_string(),
            date: img.date.as_deref().unwrap_or("").to_string(),
            ..Default::default()
        };

        result.has_board = rec.has_board;
        result.detected_text = rec.detected_text.clone();
        result.description = rec.description.clone();
        result.focus_target = rec.role.clone();

        // 写真ごとのdetected_textからキー:値を抽出
        let kvs = extract_kv_from_text(&rec.detected_text);

        // 測点: この写真の黒板OCRから「場所」を取得
        let photo_station = kvs.iter()
            .find(|(k, _)| k == "場所" || k == "測点")
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        result.station = normalize_station(&photo_station);

        // 安全管理系: taggerのmachine_typeから直接判定（黒板なし写真に対応）
        let safety_remarks = safety_remarks_from_machine_type(&rec.machine_type);
        if let Some(remarks) = safety_remarks {
            result.photo_category = "安全管理写真".to_string();
            result.remarks = remarks;
        } else {
            // 写真ごとにマスタ照合（混在フォルダ対応）
            // machine_typeもキーワードに追加して照合精度を向上
            let photo_matched_row = if !rec.detected_text.is_empty() {
                let combined = if !rec.machine_type.is_empty() {
                    format!("{}\n{}", rec.detected_text, rec.machine_type)
                } else {
                    rec.detected_text.clone()
                };
                let texts = vec![combined.as_str()];
                match_master_from_detected_texts(master, &texts, folder_name)
            } else {
                folder_fallback_row.clone()
            };

            // マスタ照合結果を適用
            if let Some(row) = &photo_matched_row {
                result.photo_category = row.photo_type.clone();
                result.work_type = row.work_type.clone();
                result.variety = row.variety.clone();
                result.subphase = row.subphase.clone();
                result.remarks = row.remarks.clone();
            } else {
                // マスタ照合失敗: 工種キーワードだけ埋める
                let extracted_wt = kvs.iter()
                    .find(|(k, _)| k == "工種")
                    .map(|(_, v)| v.clone());
                if let Some(wt) = extracted_wt {
                    result.work_type = wt;
                }
                result.remarks = folder_name.replace('_', " ");
            }
        }

        result.reasoning = format!("photo-groups.json: {} / {}", rec.machine_type, rec.machine_id);

        Some((rec.group, role_priority(&rec.role), result))
    }).collect();

    // 日付→ファイル名の時系列ソート（グループ順ではなく撮影順）
    results.sort_by(|a, b| {
        a.2.date.cmp(&b.2.date).then(a.2.file_name.cmp(&b.2.file_name))
    });
    let mut final_results: Vec<analyzer::AnalysisResult> = results.into_iter().map(|(_, _, r)| r).collect();

    // ポスト処理: フォルダ内の文脈で種別を補正
    // 路面切削工が存在する場合、舗装打換え工/表層工 → 切削オーバーレイ工/表層工
    let has_road_cutting = final_results.iter().any(|r| r.variety == "路面切削工");
    if has_road_cutting {
        for r in &mut final_results {
            if r.variety == "舗装打換え工" && r.subphase == "表層工" {
                r.variety = "切削オーバーレイ工".to_string();
            }
        }
    }

    // 区画線工の線種判定: line_typesが指定されている場合のみ
    if let Some(lt) = line_types {
        if !lt.is_empty() {
            for r in &mut final_results {
                if r.work_type == "区画線工" && !r.file_path.is_empty() {
                    if let Some(detected) = detect_line_type(&r.file_path, lt) {
                        r.station = detected;
                    }
                }
            }
        }
    }

    final_results
}

/// 測点表記を正規化する（L→左車線、R→右車線）
fn normalize_station(station: &str) -> String {
    if station.is_empty() {
        return String::new();
    }
    // "No. 4" → "No.4" (normalize extra space after dot)
    let mut s = station.to_string();
    while s.contains(". ") {
        s = s.replace(". ", ".");
    }
    // Trailing " L" → " 左車線", " R" → " 右車線"
    if s.ends_with(" L") {
        s.truncate(s.len() - 2);
        s.push_str(" 左車線");
    } else if s.ends_with(" R") {
        s.truncate(s.len() - 2);
        s.push_str(" 右車線");
    }
    s
}

/// taggerのmachine_typeから安全管理系の写真を判定し、remarksを返す
///
/// 朝礼・KY活動など黒板のない写真はdetected_textが空のためマスタ照合できない。
/// taggerのグループ名（machine_type）から直接判定する。
fn safety_remarks_from_machine_type(machine_type: &str) -> Option<String> {
    const SAFETY_MAPPINGS: &[(&str, &str)] = &[
        ("朝礼", "安全朝礼実施状況"),
        ("KY", "KY活動状況"),
        ("新規入場者教育", "新規入場者教育状況"),
    ];
    SAFETY_MAPPINGS.iter()
        .find(|(pattern, _)| machine_type.contains(pattern))
        .map(|(_, remarks)| remarks.to_string())
}

/// 区画線工の線種判定プロンプトを生成する
///
/// line_typesから選択肢リストを構築し、Gemini CLIに渡すプロンプトを返す
fn build_line_type_prompt(line_types: &[LineTypeEntry]) -> String {
    let choices: Vec<String> = line_types
        .iter()
        .enumerate()
        .map(|(i, lt)| {
            let label = (b'A' + i as u8) as char;
            format!("({}){}", label, lt.name)
        })
        .collect();
    let choices_str = choices.join(" ");

    format!(
        "夜間道路工事の写真。作業員が仮ラインテープを路面に貼っている。\
        以下の手順で答えろ。\
        Step1:作業員の手元に見えるテープの線は直線か曲線か角度がついているか？\
        Step2:テープ全体でどんな図形を作ろうとしているか？（直線/平行な帯/ひし形/矢印/文字）\
        Step3:以下から該当する線種を1つ選べ：{} \
        各Stepの回答を1行ずつ出力。判別不能なら「判別不能」。",
        choices_str
    )
}

/// AIレスポンスから線種名を抽出する
///
/// レスポンス例: "A 中央線", "(B) 停止線", "横断歩道線" など
fn extract_line_type_from_response(response: &str, line_types: &[LineTypeEntry]) -> Option<String> {
    let response = response.trim();
    if response.is_empty() || response.contains("判別不能") {
        return None;
    }

    // 最終行を取得し、CoTの "Step3:" プレフィックスを除去
    let raw_last = response
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or(response)
        .trim();
    let last_line = raw_last
        .strip_prefix("Step3:")
        .or_else(|| raw_last.strip_prefix("Step 3:"))
        .unwrap_or(raw_last)
        .trim();

    // 記号(A,B,C...)で判定（最終行から）
    for (i, lt) in line_types.iter().enumerate() {
        let label = (b'A' + i as u8) as char;
        if last_line.starts_with(label)
            || last_line.starts_with(&format!("({})", label))
        {
            return Some(lt.name.clone());
        }
    }

    // 線種名の直接マッチ（最終行から優先、次に全文）
    for lt in line_types {
        if last_line.contains(&lt.name) {
            return Some(lt.name.clone());
        }
    }
    for lt in line_types {
        if response.contains(&lt.name) {
            return Some(lt.name.clone());
        }
    }

    None
}

/// 区画線工の写真に対して、Gemini CLIで線種を判定する
///
/// 戻り値: Some("横断歩道線") など。判定失敗時はNone。
pub fn detect_line_type(
    photo_path: &str,
    line_types: &[LineTypeEntry],
) -> Option<String> {
    if line_types.is_empty() {
        return None;
    }

    let prompt = build_line_type_prompt(line_types);

    // Gemini CLIのワークスペース外（H:ドライブ等）の場合に備え、一時ファイルにコピー
    let photo = Path::new(photo_path);
    let temp_dir = std::env::temp_dir().join(format!("photo-ai-linetype-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).ok();

    let ext = photo.extension().and_then(|e| e.to_str()).unwrap_or("jpg");
    let temp_photo = temp_dir.join(format!("photo.{}", ext));
    if let Err(e) = std::fs::copy(photo, &temp_photo) {
        eprintln!("  Warning: 写真コピー失敗 {}: {}", photo_path, e);
        let _ = std::fs::remove_dir_all(&temp_dir);
        return None;
    }

    let temp_photo_unix = temp_photo.display().to_string().replace('\\', "/");
    let stdin_content = format!("@{} {}", temp_photo_unix, prompt);
    // Gemini CLI呼び出し: stdin経由、-pフラグ不使用
    let result = run_gemini_cli_for_line_type(&stdin_content);

    // 一時ファイル削除
    let _ = std::fs::remove_dir_all(&temp_dir);

    match result {
        Ok(response) => extract_line_type_from_response(&response, line_types),
        Err(e) => {
            eprintln!("  Warning: 線種判定失敗: {}", e);
            None
        }
    }
}

/// Git for Windows の bash.exe パスを探す
fn find_git_bash() -> Option<PathBuf> {
    // 既知のパスを順に確認
    let candidates = [
        r"C:\Program Files\Git\usr\bin\bash.exe",
        r"C:\Program Files (x86)\Git\usr\bin\bash.exe",
    ];
    for path in &candidates {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    // where git からGitルートを推定
    let output = std::process::Command::new("where.exe")
        .arg("git")
        .output()
        .ok()?;
    let git_path = String::from_utf8_lossy(&output.stdout);
    for line in git_path.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        // git.exe → 親の親 = Gitルート → usr/bin/bash.exe
        let mut p = PathBuf::from(line);
        p.pop(); // bin or cmd
        p.pop(); // Git root
        let bash = p.join("usr").join("bin").join("bash.exe");
        if bash.exists() {
            return Some(bash);
        }
    }
    None
}

/// Gemini CLIを呼び出す（線種判定用の軽量版）
///
/// Git bash経由で一時ファイル→echo | geminiのシェルパイプを使う。
/// Rust native stdinパイプだと@fileの画像参照が正しく処理されないため。
fn run_gemini_cli_for_line_type(stdin_content: &str) -> std::result::Result<String, String> {
    println!("  🔍 区画線種判定中...");

    // Git bashを使う（Windows System32のbash.exeはWSLなのでNG）
    let bash = if cfg!(windows) {
        find_git_bash().ok_or_else(|| "Git bash not found".to_string())?
    } else {
        PathBuf::from("bash")
    };

    // stdin内容を一時ファイルに書き出し、bash内でcatしてパイプ
    let temp_stdin = std::env::temp_dir().join(format!("gemini-stdin-{}.txt", std::process::id()));
    std::fs::write(&temp_stdin, format!("{}\n", stdin_content))
        .map_err(|e| format!("一時ファイル書き込みエラー: {}", e))?;
    let temp_unix = temp_stdin.display().to_string().replace('\\', "/");

    let shell_cmd = format!(
        r#"cat '{}' | gemini -m gemini-2.5-pro --yolo -o text"#,
        temp_unix
    );

    let output = std::process::Command::new(&bash)
        .args(["-c", &shell_cmd])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Gemini CLI実行エラー: {}", e))?;

    let _ = std::fs::remove_file(&temp_stdin);

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // Gemini CLIはexit 0でもエラーの場合がある → stdoutが空ならstderrを確認
    if stdout.trim().is_empty() {
        if !stderr.is_empty() {
            return Err(format!("Gemini CLI error: {}", stderr.trim()));
        }
        return Err("Gemini CLI returned empty response".to_string());
    }

    Ok(stdout)
}

/// 測点を一括適用
/// - 安全管理写真・品質管理写真 → 日付（◯月◯日）
/// - 区画線工 → スキップ（線種ごとに撮影するため固定測点は不適切）
pub fn apply_station(results: &mut [analyzer::AnalysisResult], station: &str) {
    for result in results {
        match result.photo_category.as_str() {
            "安全管理写真" | "品質管理写真" => {
                result.station = date_to_month_day(&result.date);
            }
            _ if result.work_type == "区画線工" => {
                // 区画線工は線種ごとに撮影するため測点を一律適用しない
                // 以前のnormalize -Sで誤設定された値もクリア
                if result.station == station {
                    result.station.clear();
                }
            }
            _ => {
                result.station = station.to_string();
            }
        }
    }
}

/// "2026-02-09 21:23:53" → "2月9日"
fn date_to_month_day(date_str: &str) -> String {
    let parts: Vec<&str> = date_str.split(&['-', ' '][..]).collect();
    if parts.len() >= 3 {
        let month: u32 = parts[1].parse().unwrap_or(0);
        let day: u32 = parts[2].parse().unwrap_or(0);
        if month > 0 && day > 0 {
            return format!("{}月{}日", month, day);
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_station() {
        assert_eq!(normalize_station("No.4 L"), "No.4 左車線");
        assert_eq!(normalize_station("No.0 R"), "No.0 右車線");
        assert_eq!(normalize_station("No. 4 L"), "No.4 左車線");
        assert_eq!(normalize_station("ダイヤマーク"), "ダイヤマーク");
        assert_eq!(normalize_station(""), "");
    }

    #[test]
    fn test_extract_kv_from_text_newline_fullwidth_colon() {
        let text = "工事名：市道 南千反畑第1号線舗装補修工事\n場所：No.4 L\n路面切削工\n切削・積込状況";
        let kvs = extract_kv_from_text(text);
        assert_eq!(kvs.len(), 4);
        assert_eq!(kvs[0], ("工事名".to_string(), "市道 南千反畑第1号線舗装補修工事".to_string()));
        assert_eq!(kvs[1], ("場所".to_string(), "No.4 L".to_string()));
        assert_eq!(kvs[2], ("".to_string(), "路面切削工".to_string()));
        assert_eq!(kvs[3], ("".to_string(), "切削・積込状況".to_string()));
    }

    #[test]
    fn test_extract_kv_from_text_space_separator() {
        let text = "工事名 市道 南千反畑第1号線舗装補修工事\n場所 No. 4 L\n表層工\n乳剤散布状況";
        let kvs = extract_kv_from_text(text);
        assert_eq!(kvs.len(), 4);
        assert_eq!(kvs[0], ("工事名".to_string(), "市道 南千反畑第1号線舗装補修工事".to_string()));
        assert_eq!(kvs[1], ("場所".to_string(), "No. 4 L".to_string()));
        assert_eq!(kvs[2], ("".to_string(), "表層工".to_string()));
        assert_eq!(kvs[3], ("".to_string(), "乳剤散布状況".to_string()));
    }

    #[test]
    fn test_extract_kv_from_text_empty() {
        let kvs = extract_kv_from_text("");
        assert!(kvs.is_empty());
    }

    #[test]
    fn test_date_to_month_day() {
        assert_eq!(date_to_month_day("2026-02-09 21:23:53"), "2月9日");
        assert_eq!(date_to_month_day("2026-12-25 00:00:00"), "12月25日");
        assert_eq!(date_to_month_day(""), "");
    }

    #[test]
    fn test_safety_remarks_from_machine_type() {
        assert_eq!(
            safety_remarks_from_machine_type("朝礼"),
            Some("安全朝礼実施状況".to_string())
        );
        assert_eq!(
            safety_remarks_from_machine_type("KY活動"),
            Some("KY活動状況".to_string())
        );
        assert_eq!(
            safety_remarks_from_machine_type("新規入場者教育"),
            Some("新規入場者教育状況".to_string())
        );
        // 施工機械はNone
        assert_eq!(safety_remarks_from_machine_type("路面切削機"), None);
        assert_eq!(safety_remarks_from_machine_type("ダンプトラック"), None);
        assert_eq!(safety_remarks_from_machine_type(""), None);
    }

    #[test]
    fn test_apply_station_skips_safety_and_marking() {
        let mut results = vec![
            analyzer::AnalysisResult {
                photo_category: "施工状況写真".to_string(),
                date: "2026-02-09 23:34:47".to_string(),
                ..Default::default()
            },
            analyzer::AnalysisResult {
                photo_category: "安全管理写真".to_string(),
                date: "2026-02-09 21:23:53".to_string(),
                ..Default::default()
            },
            analyzer::AnalysisResult {
                photo_category: "品質管理写真".to_string(),
                date: "2026-02-10 01:15:00".to_string(),
                ..Default::default()
            },
            analyzer::AnalysisResult {
                photo_category: "施工状況写真".to_string(),
                work_type: "区画線工".to_string(),
                station: "".to_string(),
                date: "2026-02-10 03:44:26".to_string(),
                ..Default::default()
            },
            // 以前の-Sで誤設定された区画線工
            analyzer::AnalysisResult {
                photo_category: "施工状況写真".to_string(),
                work_type: "区画線工".to_string(),
                station: "No.4 左車線".to_string(),
                date: "2026-02-10 03:52:16".to_string(),
                ..Default::default()
            },
        ];
        apply_station(&mut results, "No.4 左車線");
        assert_eq!(results[0].station, "No.4 左車線");
        assert_eq!(results[1].station, "2月9日");
        assert_eq!(results[2].station, "2月10日");
        assert_eq!(results[3].station, ""); // 区画線工はスキップ
        assert_eq!(results[4].station, ""); // 区画線工: 以前の-S値をクリア
    }

    fn sample_line_types() -> Vec<LineTypeEntry> {
        vec![
            LineTypeEntry { name: "中央線".to_string(), length_m: 230.0 },
            LineTypeEntry { name: "停止線".to_string(), length_m: 43.0 },
            LineTypeEntry { name: "車線分離線".to_string(), length_m: 30.0 },
            LineTypeEntry { name: "横断歩道線".to_string(), length_m: 100.0 },
            LineTypeEntry { name: "停車禁止枠線".to_string(), length_m: 27.0 },
            LineTypeEntry { name: "停車禁止標示".to_string(), length_m: 29.0 },
            LineTypeEntry { name: "右左折禁止標示".to_string(), length_m: 38.0 },
        ]
    }

    #[test]
    fn test_build_line_type_prompt() {
        let lt = sample_line_types();
        let prompt = build_line_type_prompt(&lt);
        assert!(prompt.contains("(A)中央線230m"));
        assert!(prompt.contains("(B)停止線43m"));
        assert!(prompt.contains("(G)右左折禁止標示38m"));
        assert!(prompt.contains("画像をよく見て答えろ"));
        assert!(prompt.contains("記号と線種名のみ回答せよ"));
    }

    #[test]
    fn test_extract_line_type_from_response_letter() {
        let lt = sample_line_types();
        assert_eq!(
            extract_line_type_from_response("A 中央線", &lt),
            Some("中央線".to_string())
        );
        assert_eq!(
            extract_line_type_from_response("(B) 停止線", &lt),
            Some("停止線".to_string())
        );
        assert_eq!(
            extract_line_type_from_response("D 横断歩道線", &lt),
            Some("横断歩道線".to_string())
        );
        assert_eq!(
            extract_line_type_from_response("G", &lt),
            Some("右左折禁止標示".to_string())
        );
    }

    #[test]
    fn test_extract_line_type_from_response_name_match() {
        let lt = sample_line_types();
        assert_eq!(
            extract_line_type_from_response("この写真は横断歩道線です", &lt),
            Some("横断歩道線".to_string())
        );
        assert_eq!(
            extract_line_type_from_response("停車禁止枠線と判断します", &lt),
            Some("停車禁止枠線".to_string())
        );
    }

    #[test]
    fn test_extract_line_type_from_response_unknown() {
        let lt = sample_line_types();
        assert_eq!(extract_line_type_from_response("判別不能", &lt), None);
        assert_eq!(extract_line_type_from_response("", &lt), None);
        assert_eq!(extract_line_type_from_response("  ", &lt), None);
    }
}
