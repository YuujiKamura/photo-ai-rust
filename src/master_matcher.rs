//! 工種マスタ照合ロジック
//!
//! detected_text（黒板OCR）から抽出したキーワードを
//! 工種階層マスタの検索パターン・備考と照合する。

use photo_ai_common::HierarchyMaster;
use crate::ocr_parser::extract_kv_from_text;

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

/// フォルダ内の全detected_textから工種階層を推定し、マスタ照合する
///
/// `focus_target` が指定された場合、Phase 2のbigramスコアにboostを加算し、
/// taggerのrole（写真の視覚的内容）を考慮した照合を行う。
pub(crate) fn match_master_from_detected_texts(
    master: &HierarchyMaster,
    detected_texts: &[&str],
    folder_name: &str,
    focus_target: Option<&str>,
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
    // focus_targetが指定されている場合、remarksとのoverlapをboostとして加算
    let ft = focus_target.unwrap_or("");
    for row in &candidates {
        if row.remarks.is_empty() { continue; }
        let kw_score: usize = keywords.iter()
            .map(|kw| token_overlap_score(kw, &row.remarks))
            .sum();
        let ft_boost = if !ft.is_empty() {
            token_overlap_score(ft, &row.remarks) * 3
        } else {
            0
        };
        let score = kw_score + ft_boost;
        if score > best_score {
            best_score = score;
            best = Some(row);
        }
    }

    best.cloned()
}

/// taggerのmachine_typeから安全管理系の写真を判定し、remarksを返す
///
/// 朝礼・KY活動など黒板のない写真はdetected_textが空のためマスタ照合できない。
/// taggerのグループ名（machine_type）から直接判定する。
pub(crate) fn safety_remarks_from_machine_type(machine_type: &str) -> Option<String> {
    const SAFETY_MAPPINGS: &[(&str, &str)] = &[
        ("朝礼", "安全朝礼実施状況"),
        ("安全ミーティング", "安全朝礼実施状況"),
        ("KY", "KY活動状況"),
        ("新規入場者教育", "新規入場者教育状況"),
    ];
    SAFETY_MAPPINGS.iter()
        .find(|(pattern, _)| machine_type.contains(pattern))
        .map(|(_, remarks)| remarks.to_string())
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
}
