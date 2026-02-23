//! マスタ照合モジュール
//!
//! ## 照合アルゴリズム
//! 1. OCRテキストからキーワード抽出（工種/種別ヒント含む）
//! 2. Phase 1: マスタの検索パターン(正規表現)でマッチ → スコア1000
//! 3. Phase 2: マスタの備考とbigramトークン重複でスコアリング
//!    - focus_target boost: taggerのroleとの一致度 × 3倍
//! 4. 最高スコアの候補を選定

use photo_ai_common::HierarchyMaster;
use photo_ai_common::hierarchy::HierarchyRow;
#[allow(unused_imports)]
use crate::domain::*;
use crate::ocr_parser::extract_kv_from_text;

/// スコアリング定数
const EXACT_MATCH_SCORE: usize = 100;
const PARTIAL_MATCH_SCORE: usize = 50;
const FOCUS_TARGET_BOOST_MULTIPLIER: usize = 3;
const SEARCH_PATTERN_PRIORITY_SCORE: usize = 1000;

/// OCRから抽出した工種名をマスタの工種名に正規化する
///
/// 黒板OCRが「舗装補修工事」を読み取ると「舗装補修工」になるが、
/// マスタには「舗装工」しかないためフィルタが外れてしまう。
/// OCR由来の揺れをマスタに合わせる。
fn normalize_work_type(work_type: &str) -> String {
    // 「〜補修工」→「〜工」に正規化（舗装補修工→舗装工 等）
    if let Some(prefix) = work_type.strip_suffix("補修工") {
        return format!("{}工", prefix);
    }
    work_type.to_string()
}

/// フォルダ名からマスタの種別（variety）を推定する
///
/// フォルダ名に種別の核心部分（「工」を除いた部分）が含まれていれば、その種別を返す。
/// 含まれていない場合は、マスタの種別core（「工」除去）がフォルダ名を含むかもチェック。
/// 最も長いマッチを優先する。
fn extract_variety_from_folder(master: &HierarchyMaster, folder_name: &str) -> Option<String> {
    let varieties: std::collections::HashSet<&str> = master.rows().iter()
        .filter(|r| !r.variety.is_empty())
        .map(|r| r.variety.as_str())
        .collect();

    let mut best: Option<(&str, usize)> = None;
    for v in &varieties {
        let core = v.strip_suffix('工').unwrap_or(v);
        // 双方向の包含チェック
        let overlap = if folder_name.contains(core) {
            core.chars().count()
        } else if core.contains(folder_name) {
            folder_name.chars().count()
        } else {
            0
        };
        if overlap >= 2
            && (best.is_none() || overlap > best.unwrap().1) {
                best = Some((v, overlap));
            }
    }
    best.map(|(v, _)| v.to_string())
}

/// 全景を先頭にするためのロール優先度
pub(crate) fn role_priority(role: &str) -> u8 {
    if role.contains("全景") { 0 }
    else if role.contains("証票") { 1 }
    else if role.contains("ナンバー") { 2 }
    else { 3 }
}

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
        return EXACT_MATCH_SCORE;
    }
    // 部分文字列一致
    if b.contains(a) || a.contains(b) {
        return PARTIAL_MATCH_SCORE;
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

/// OCRテキストからマッチ用キーワードを抽出する
///
/// 返り値: (keywords, work_type_hint, variety_hint)
/// - keywords: 照合に使うキーワード群
/// - work_type_hint: OCRから抽出した工種ヒント
/// - variety_hint: OCRから抽出した種別ヒント
fn extract_match_keywords(
    master: &HierarchyMaster,
    detected_texts: &[&str],
    folder_name: &str,
) -> (Vec<String>, Option<String>, Option<String>) {
    let mut work_type: Option<String> = None;
    let mut variety_hint: Option<String> = None;
    let mut keywords: Vec<String> = Vec::new();

    for text in detected_texts {
        for (key, value) in extract_kv_from_text(text) {
            match key.as_str() {
                "工種" => { if !value.is_empty() { work_type = Some(normalize_work_type(&value)); } }
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

    // フォルダ名からvariety_hintを部分一致で抽出（OCRにvariety名がない場合のフォールバック）
    // 例: フォルダ「切削出来形」→ 「切削」が「路面切削工」に含まれる → variety_hint = "路面切削工"
    if variety_hint.is_none() && !folder_name.is_empty() {
        variety_hint = extract_variety_from_folder(master, folder_name);
    }

    (keywords, work_type, variety_hint)
}

/// マスタ候補にスコアを付ける
///
/// Phase 1: 検索パターン列でマッチ → SEARCH_PATTERN_PRIORITY_SCORE
/// Phase 2: 備考とbigramトークン重複 + focus_target boost
fn score_candidates(
    master: &HierarchyMaster,
    keywords: &[String],
    work_type_hint: Option<&str>,
    variety_hint: Option<&str>,
    focus_target: Option<&str>,
) -> Vec<(HierarchyRow, usize)> {
    // 工種・種別でマスタをフィルタ
    let candidates: Vec<_> = master.rows().iter()
        .filter(|r| {
            // work_type フィルタ
            if let Some(wt) = work_type_hint {
                if r.work_type != wt && !r.work_type.is_empty() {
                    return false;
                }
            }
            // variety_hint フィルタ: varietyまたはsubphaseに一致する行を優先
            if let Some(vh) = variety_hint {
                r.variety == vh || r.subphase == vh || r.variety.contains(vh) || vh.contains(&r.variety)
            } else {
                true
            }
        })
        .collect();

    let mut scored: Vec<(HierarchyRow, usize)> = Vec::new();

    // Phase 1: 検索パターン列でマッチ
    let ft = focus_target.unwrap_or("");
    let mut phase1_matches: Vec<(&HierarchyRow, usize)> = Vec::new();
    for row in &candidates {
        if !row.search_patterns.is_empty() {
            let patterns: Vec<&str> = row.search_patterns.split('|').collect();
            let mut score = keywords.iter()
                .filter(|kw| patterns.iter().any(|p| kw.contains(p) || p.contains(kw.as_str())))
                .count();
            // focus_targetも検索パターンに照合（AIが正しい答えを出しているなら直接使う）
            if !ft.is_empty() && patterns.iter().any(|p| ft.contains(p) || p.contains(ft)) {
                score += 1;
            }
            if score > 0 {
                phase1_matches.push((row, score));
            }
        }
    }

    if !phase1_matches.is_empty() {
        let max_score = phase1_matches.iter().map(|(_, s)| *s).max().unwrap();
        let best_matches: Vec<_> = phase1_matches.into_iter()
            .filter(|(_, s)| *s == max_score)
            .collect();

        if best_matches.len() == 1 {
            scored.push((best_matches[0].0.clone(), SEARCH_PATTERN_PRIORITY_SCORE));
            return scored;
        }

        // 同スコアの複数候補 → focus_targetでタイブレイク
        if !ft.is_empty() {
            let best = best_matches.iter()
                .max_by_key(|(row, _)| token_overlap_score(ft, &row.remarks))
                .unwrap();
            scored.push((best.0.clone(), SEARCH_PATTERN_PRIORITY_SCORE));
            return scored;
        }

        // focus_targetなし → 最初の候補
        scored.push((best_matches[0].0.clone(), SEARCH_PATTERN_PRIORITY_SCORE));
        return scored;
    }

    // Phase 2: remarks列にキーワード部分一致（トークンベース：語順違いに対応）
    for row in &candidates {
        if row.remarks.is_empty() { continue; }
        let kw_score: usize = keywords.iter()
            .map(|kw| token_overlap_score(kw, &row.remarks))
            .sum();
        let ft_boost = if !ft.is_empty() {
            token_overlap_score(ft, &row.remarks) * FOCUS_TARGET_BOOST_MULTIPLIER
        } else {
            0
        };
        let score = kw_score + ft_boost;
        if score > 0 {
            scored.push(((*row).clone(), score));
        }
    }

    scored
}

/// フォルダ内の全detected_textから工種階層を推定し、マスタ照合する
///
/// `focus_target` が指定された場合、Phase 2のbigramスコアにboostを加算し、
/// taggerのrole（写真の視覚的内容）を考慮した照合を行う。
pub(crate) fn match_master_from_detected_texts(
    master: &HierarchyMaster,
    detected_texts: &[&str],
    folder_name: &str,
    focus_target: Option<&str>,
) -> Option<HierarchyRow> {
    let (keywords, work_type_hint, variety_hint) = extract_match_keywords(master, detected_texts, folder_name);

    let scored = score_candidates(
        master,
        &keywords,
        work_type_hint.as_deref(),
        variety_hint.as_deref(),
        focus_target,
    );

    scored.into_iter()
        .max_by_key(|(_, score)| *score)
        .map(|(row, _)| row)
}

/// フォルダ名がマスタに直接マッチするか確認する
///
/// Returns true if:
/// 1. フォルダ名がマスタの備考列に完全一致する
/// 2. フォルダ名がマスタの検索パターンにマッチする
pub(crate) fn folder_has_master_entry(master: &HierarchyMaster, folder_name: &str) -> bool {
    if folder_name.is_empty() {
        return false;
    }
    // Check 1: 備考列の完全一致
    if master.rows().iter().any(|r| r.remarks == folder_name) {
        return true;
    }
    // Check 2: 検索パターンでマッチ
    for row in master.rows() {
        if row.search_patterns.is_empty() {
            continue;
        }
        for pat in row.search_patterns.split('|') {
            let pat = pat.trim();
            if !pat.is_empty() && folder_name.contains(pat) {
                return true;
            }
        }
    }
    false
}

/// "2026-02-09 21:23:53" → "2月9日"
pub(crate) fn date_to_month_day(date_str: &str) -> String {
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
    fn test_date_to_month_day() {
        assert_eq!(date_to_month_day("2026-02-09 21:23:53"), "2月9日");
        assert_eq!(date_to_month_day("2026-12-25 00:00:00"), "12月25日");
        assert_eq!(date_to_month_day(""), "");
    }

    #[test]
    fn test_normalize_work_type() {
        // 舗装補修工 → 舗装工
        assert_eq!(normalize_work_type("舗装補修工"), "舗装工");
        // そのまま通過するケース
        assert_eq!(normalize_work_type("舗装工"), "舗装工");
        assert_eq!(normalize_work_type("区画線工"), "区画線工");
        assert_eq!(normalize_work_type("構造物撤去工"), "構造物撤去工");
        // 空文字
        assert_eq!(normalize_work_type(""), "");
        // 他の「〜補修工」パターン
        assert_eq!(normalize_work_type("道路補修工"), "道路工");
    }

    #[test]
    fn test_work_type_normalization_in_matching() {
        // OCRが「舗装補修工」を読み取った場合でもマスタの「舗装工」にマッチすること
        let csv = "\
費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,施工状況写真,舗装工,舗装打換え工,表層工,舗設状況,
直接工事費,施工状況写真,舗装工,路面切削工,路面切削,路面切削状況,
";
        let master = HierarchyMaster::from_csv_str(csv).unwrap();
        // 黒板に「工種:舗装補修工」と書かれている
        let text = "工種：舗装補修工\n表層工\n舗設状況";
        let texts = vec![text];

        let result = match_master_from_detected_texts(&master, &texts, "", None);
        assert!(result.is_some(), "should match despite work_type being 舗装補修工");
        let row = result.unwrap();
        assert_eq!(row.work_type, "舗装工");
        assert_eq!(row.remarks, "舗設状況");
    }

    #[test]
    fn test_focus_target_boost_road_cutting() {
        // R0010387: 黒板OCR「切削・積込状況」が「切削殻積込状況」にマッチしてしまう問題
        // focusTarget「路面切削状況」でboostすると「路面切削状況」が勝つべき
        let csv = "\
費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,施工状況写真,舗装工,路面切削工,路面切削,路面切削状況,
直接工事費,施工状況写真,舗装工,路面切削工,路面切削,切削殻積込状況,
";
        let master = HierarchyMaster::from_csv_str(csv).unwrap();
        let text = "工事名：テスト工事\n路面切削工\n切削・積込状況";
        let texts = vec![text];

        // focusTargetなし: 「切削殻積込状況」が勝つ（bigram score 4 > 2）
        let result = match_master_from_detected_texts(&master, &texts, "", None);
        assert_eq!(result.as_ref().unwrap().remarks, "切削殻積込状況");

        // focusTarget「路面切削状況」あり: boostにより「路面切削状況」が勝つ
        let result = match_master_from_detected_texts(&master, &texts, "", Some("路面切削状況"));
        assert_eq!(result.as_ref().unwrap().remarks, "路面切削状況");
    }

    #[test]
    fn test_phase1_tiebreak_with_focus_target() {
        // 黒板に複数温度が併記されている場合、検索パターンが全てマッチするが
        // focus_targetで正しい温度を選ぶ
        let csv = "\
費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,品質管理写真,舗装工,舗装打換え工,表層工,到着温度,到着温度|出荷温度|合材到着
直接工事費,品質管理写真,舗装工,舗装打換え工,表層工,敷均し温度,敷均し温度|敷きならし|舗設温度
直接工事費,品質管理写真,舗装工,舗装打換え工,表層工,初期締固め前温度,初期締固|初転圧前|初期転圧温度|初期転圧前
";
        let master = HierarchyMaster::from_csv_str(csv).unwrap();
        // 黒板に全温度が併記: 「到着温度 159.7, 敷均し温度 154.8, 初期転圧前温度 148.1」
        let text = "表層工\n到着温度 159.7\n敷均し温度 154.8\n初期転圧前温度 148.1";
        let texts = vec![text];

        // ft=敷均し温度 → 敷均し温度が勝つ
        let result = match_master_from_detected_texts(&master, &texts, "", Some("敷均し温度"));
        assert_eq!(result.as_ref().unwrap().remarks, "敷均し温度");

        // ft=初期転圧前温度 → 初期締固め前温度が勝つ（bigram: 初期, 前温, 温度 が共通）
        let result = match_master_from_detected_texts(&master, &texts, "", Some("初期転圧前温度"));
        assert_eq!(result.as_ref().unwrap().remarks, "初期締固め前温度");

        // ft=到着温度 → 到着温度が勝つ
        let result = match_master_from_detected_texts(&master, &texts, "", Some("到着温度"));
        assert_eq!(result.as_ref().unwrap().remarks, "到着温度");
    }

    #[test]
    fn test_dekigata_folder_切削出来形_matches_路面切削工() {
        // フォルダ名「切削出来形」の写真が「路面切削工出来形測定」にマッチすべき
        // （「不陸整正出来形・管理値」に誤マッチしない）
        let csv = "\
費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,出来形管理写真,舗装工,舗装打換え工,上層路盤工,不陸整正出来形,路盤出来形|出来形検測|路盤|基準高下がり|基準高
直接工事費,出来形管理写真,舗装工,舗装打換え工,上層路盤工,不陸整正出来形・管理値,
直接工事費,出来形管理写真,舗装工,路面切削工,路面切削,路面切削工出来形測定,切削出来形|切削工出来形
";
        let master = HierarchyMaster::from_csv_str(csv).unwrap();

        // detected_text: 出来形管理用紙（黒板に路面切削工と書いてないケース）
        let text = "出来形管理用紙 No.9";
        let texts = vec![text];

        // フォルダ名「切削出来形」→ 検索パターン「切削出来形」で路面切削工出来形測定がマッチ
        let result = match_master_from_detected_texts(&master, &texts, "切削出来形", None);
        assert!(result.is_some(), "should match");
        assert_eq!(result.unwrap().remarks, "路面切削工出来形測定");
    }

    #[test]
    fn test_dekigata_folder_不陸整正_still_matches() {
        // フォルダ名が「不陸整正出来形」の場合は不陸整正にマッチすべき
        let csv = "\
費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,出来形管理写真,舗装工,舗装打換え工,上層路盤工,不陸整正出来形,路盤出来形|出来形検測|路盤|基準高下がり|基準高
直接工事費,出来形管理写真,舗装工,舗装打換え工,上層路盤工,不陸整正出来形・管理値,
直接工事費,出来形管理写真,舗装工,路面切削工,路面切削,路面切削工出来形測定,切削出来形|切削工出来形
";
        let master = HierarchyMaster::from_csv_str(csv).unwrap();

        let text = "出来形管理用紙 No.3\n基準高";
        let texts = vec![text];

        // フォルダ名「不陸整正」→ 検索パターン「基準高」で不陸整正出来形がマッチ
        let result = match_master_from_detected_texts(&master, &texts, "不陸整正", None);
        assert!(result.is_some(), "should match");
        assert_eq!(result.unwrap().remarks, "不陸整正出来形");
    }

    #[test]
    fn test_extract_variety_from_folder() {
        let csv = "\
費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,施工状況写真,舗装工,舗装打換え工,表層工,舗設状況,
直接工事費,施工状況写真,舗装工,路面切削工,路面切削,路面切削状況,
直接工事費,施工状況写真,舗装工,切削オーバーレイ工,表層工,舗設状況,
";
        let master = HierarchyMaster::from_csv_str(csv).unwrap();

        // 完全一致ケース: "舗装打換え" → "舗装打換え工"
        let result = extract_variety_from_folder(&master, "舗装打換え");
        assert_eq!(result.as_deref(), Some("舗装打換え工"));

        // 部分一致: "路面切削" → "路面切削工"（coreが完全にフォルダ名に含まれる）
        let result = extract_variety_from_folder(&master, "路面切削出来形");
        assert_eq!(result.as_deref(), Some("路面切削工"));

        // マッチなし
        let result = extract_variety_from_folder(&master, "温度管理");
        assert!(result.is_none());
    }

    #[test]
    fn test_focus_target_no_effect_when_empty() {
        // focusTargetが空の場合、従来通りの動作
        let csv = "\
費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,施工状況写真,舗装工,舗装打換え工,表層工,舗設状況,
直接工事費,施工状況写真,舗装工,舗装打換え工,表層工,初期転圧状況,
";
        let master = HierarchyMaster::from_csv_str(csv).unwrap();
        let text = "表層工\n舗設状況";
        let texts = vec![text];

        let result_none = match_master_from_detected_texts(&master, &texts, "", None);
        let result_empty = match_master_from_detected_texts(&master, &texts, "", Some(""));
        assert_eq!(result_none.as_ref().unwrap().remarks, result_empty.as_ref().unwrap().remarks);
        assert_eq!(result_none.unwrap().remarks, "舗設状況");
    }

    #[test]
    fn test_focus_target_matches_search_pattern_directly() {
        // Issue #97: コアー写真のdetected_textにコアーキーワードがない場合
        // focus_targetがsearch_patternに直接マッチして正しい行を選ぶべき
        let csv = "\
費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,出来形管理写真,舗装工,切削オーバーレイ工,表層工,コアー採取前,コアー採取前|コア採取前
直接工事費,出来形管理写真,舗装工,切削オーバーレイ工,表層工,コアー採取状況,コアー採取状況|コア採取状況|コアー採集
直接工事費,出来形管理写真,舗装工,切削オーバーレイ工,表層工,コアー厚さ測定,コアー厚さ測定|コア厚測定|舗装厚測定|コア厚さ
直接工事費,出来形管理写真,舗装工,切削オーバーレイ工,表層工,コアー復築前,コアー復築前|コア復築前
直接工事費,出来形管理写真,舗装工,切削オーバーレイ工,表層工,コアー復築状況,コアー復築状況|コア復築状況|コアー復築
直接工事費,出来形管理写真,舗装工,切削オーバーレイ工,表層工,コアー復築完了,コアー復築完了|コア復築完了
";
        let master = HierarchyMaster::from_csv_str(csv).unwrap();

        // R0010577: detected_textにコアーキーワードなし、focus_targetに正解あり
        let text = "市道 南千反畑町第1号線舗装補修工事, 舗装工 表層工, No.8 R";
        let texts = vec![text];
        let result = match_master_from_detected_texts(&master, &texts, "コアNo.8", Some("コアー厚さ測定"));
        assert!(result.is_some(), "focus_target should match コアー厚さ測定 search pattern");
        assert_eq!(result.unwrap().remarks, "コアー厚さ測定");

        // R0010578: detected_text="55mm"のみ、focus_target="舗装厚測定・接写"
        let text2 = "55mm";
        let texts2 = vec![text2];
        let result2 = match_master_from_detected_texts(&master, &texts2, "コアNo.8", Some("舗装厚測定・接写"));
        assert!(result2.is_some(), "focus_target '舗装厚測定・接写' should match search pattern '舗装厚測定'");
        assert_eq!(result2.unwrap().remarks, "コアー厚さ測定");

        // R0010582: コアー採取前
        let text3 = "工事名：市道 南千反畑町第1号線舗装補修工事, 場所：No.1 R, 表層工 コアー 採取前";
        let texts3 = vec![text3];
        let result3 = match_master_from_detected_texts(&master, &texts3, "コアNo.1", Some("コアー採取前"));
        assert!(result3.is_some());
        assert_eq!(result3.unwrap().remarks, "コアー採取前");

        // R0010589: コアー復築前
        let text4 = "工事名 市道 南千反畑第1号線舗装補修工事, 場所 No. 1R, 表層工 コアー 復築前";
        let texts4 = vec![text4];
        let result4 = match_master_from_detected_texts(&master, &texts4, "コアNo.1", Some("コアー復築前"));
        assert!(result4.is_some());
        assert_eq!(result4.unwrap().remarks, "コアー復築前");
    }

    #[test]
    fn test_empty_work_type_in_ocr_does_not_filter_out_all_rows() {
        // スマホ写真の黒板OCR: "工種:" が空 → work_type_hint=Noneとして扱う
        let csv = "\
費目,写真区分,工種,種別,細別,備考,検索パターン
直接工事費,施工状況写真,舗装工,路面切削工,路面切削,切削完了,切削完了
直接工事費,施工状況写真,舗装工,路面切削工,路面切削,端部舗装版撤去状況,端部舗装版撤去
直接工事費,施工状況写真,舗装工,路面切削工,路面切削,不純物の除去確認,不純物.*除去|除去確認
";
        let master = HierarchyMaster::from_csv_str(csv).unwrap();

        // 「工種:」空のOCR
        let text = "工事名:テスト工事, 工種:, 測点:No.12, 切削完了";
        let texts = vec![text];
        let result = match_master_from_detected_texts(&master, &texts, "施工状況", Some("切削完了"));
        assert!(result.is_some(), "should match 切削完了 despite empty 工種:");
        assert_eq!(result.unwrap().remarks, "切削完了");

        // 端部舗装版撤去
        let text2 = "工事名:テスト工事, 工種:, 測点:No.12, 端部舗装版撤去状況";
        let texts2 = vec![text2];
        let result2 = match_master_from_detected_texts(&master, &texts2, "施工状況", Some("端部舗装版撤去状況"));
        assert!(result2.is_some(), "should match 端部舗装版撤去状況");
        assert_eq!(result2.unwrap().remarks, "端部舗装版撤去状況");
    }
}
