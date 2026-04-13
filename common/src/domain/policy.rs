//! ドメインポリシー（業務ルール判定）
//!
//! 工事写真の分類・正規化に関する業務ルールを集約する。
//! 各関数は純粋関数（副作用なし）であり、単体テストで仕様が固定されている。
//!
//! ## 収録しているルール
//!
//! - [`is_machinery_related`]: 機械関連写真の判定
//! - [`should_auto_date_station`]: 安全管理写真で日付測点を自動設定するか
//! - [`has_temperature_strong_keyword`]: detected_text に温度写真の強キーワードが含まれるか
//! - [`normalize_work_type_from_ocr`]: OCR揺れ（〜補修工→〜工）の正規化
//! - [`extract_tonnage_from_text`]: detected_text から積載量を抽出

use super::constants::*;

/// 機械関連の写真か判定する
///
/// 備考が「使用機械」または「重機始業前点検」の場合に `true`。
pub fn is_machinery_related(remarks: &str) -> bool {
    remarks == REMARKS_MACHINERY || remarks == REMARKS_MACHINERY_CHECK
}

/// 安全管理写真で日付測点を自動設定する対象か判定する
///
/// 黒板に場所情報がなく、備考が安全朝礼/KY/新規入場者教育/安全訓練の場合に `true`。
/// photo-tagger の `machine_type` 経由で分類される一部の安全管理写真に対し、
/// 撮影日を「X月Y日」として station に格納する運用をサポートする。
pub fn should_auto_date_station(remarks: &str) -> bool {
    matches!(
        remarks,
        REMARKS_SAFETY_MORNING
            | REMARKS_SAFETY_KY
            | REMARKS_SAFETY_NEW_ENTRY
            | REMARKS_SAFETY_TRAINING
    )
}

/// detected_text に温度写真の強キーワードが含まれているか判定する
///
/// これらのキーワードが含まれていれば、AI の分類結果を信頼して
/// 隣接写真からの remarks 伝搬をスキップする（normalizer の誤分類修正で使用）。
pub fn has_temperature_strong_keyword(detected_text: &str) -> bool {
    TEMPERATURE_STRONG_KEYWORDS
        .iter()
        .any(|kw| detected_text.contains(kw))
}

/// 温度写真の強キーワード（detected_text で高信頼マッチに使う）
pub const TEMPERATURE_STRONG_KEYWORDS: &[&str] = &[
    "到着温度",
    "敷均し温度",
    "初期転圧前温度",
    "初期締固め前温度",
    "開放温度",
    "解放温度",
    "舗装日外気温",
    "外気温",
];

/// OCR 由来の工種名をマスタ正規名に揃える
///
/// 黒板 OCR が「舗装補修工事」を読み取ると「舗装補修工」になるが、
/// マスタには「舗装工」しかないためフィルタが外れてしまう。
/// 「〜補修工」→「〜工」の語尾変換で揺れを吸収する。
pub fn normalize_work_type_from_ocr(work_type: &str) -> String {
    if let Some(prefix) = work_type.strip_suffix("補修工") {
        return format!("{}工", prefix);
    }
    work_type.to_string()
}

/// detected_text から積載量を抽出する
///
/// 「計量積載量」「積載量」「数量」のいずれかのキーに続く数値を取得し、
/// `積載量：N.Nｔ` 形式で返す。処分状況（車番調査）写真の measurements に使用。
pub fn extract_tonnage_from_text(text: &str) -> Option<String> {
    for key in ["計量積載量", "積載量", "数量"] {
        if let Some(pos) = text.find(key) {
            let tail = &text[pos + key.len()..];
            let tail = tail.trim_start_matches(&[':', '：', ' ', '　'][..]);
            let num: String = tail
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if !num.is_empty() {
                return Some(format!("積載量：{}ｔ", num));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // === is_machinery_related ===

    #[test]
    fn machinery_related_true_for_usage() {
        assert!(is_machinery_related("使用機械"));
    }

    #[test]
    fn machinery_related_true_for_pre_check() {
        assert!(is_machinery_related("重機始業前点検"));
    }

    #[test]
    fn machinery_related_false_for_other() {
        assert!(!is_machinery_related("表層工"));
        assert!(!is_machinery_related(""));
        assert!(!is_machinery_related("使用機械（路面切削機 ER552F）"));
    }

    // === should_auto_date_station ===

    #[test]
    fn auto_date_station_for_safety_morning() {
        assert!(should_auto_date_station("安全朝礼実施状況"));
    }

    #[test]
    fn auto_date_station_for_ky() {
        assert!(should_auto_date_station("KY活動状況"));
    }

    #[test]
    fn auto_date_station_for_new_entry_and_training() {
        assert!(should_auto_date_station("新規入場者教育状況"));
        assert!(should_auto_date_station("安全訓練実施状況"));
    }

    #[test]
    fn auto_date_station_false_for_patrol() {
        // 安全パトロールは station に日付を入れない運用
        assert!(!should_auto_date_station("安全パトロール実施状況"));
    }

    #[test]
    fn auto_date_station_false_for_non_safety() {
        assert!(!should_auto_date_station("表層工"));
        assert!(!should_auto_date_station(""));
    }

    // === has_temperature_strong_keyword ===

    #[test]
    fn temperature_strong_keyword_detects_arrival() {
        assert!(has_temperature_strong_keyword("到着温度 160℃"));
    }

    #[test]
    fn temperature_strong_keyword_detects_spreading() {
        assert!(has_temperature_strong_keyword("敷均し温度"));
    }

    #[test]
    fn temperature_strong_keyword_detects_opening_both_spellings() {
        assert!(has_temperature_strong_keyword("開放温度"));
        assert!(has_temperature_strong_keyword("解放温度"));
    }

    #[test]
    fn temperature_strong_keyword_detects_ambient() {
        assert!(has_temperature_strong_keyword("舗装日外気温"));
        assert!(has_temperature_strong_keyword("外気温"));
    }

    #[test]
    fn temperature_strong_keyword_false_for_unrelated() {
        assert!(!has_temperature_strong_keyword("舗設状況"));
        assert!(!has_temperature_strong_keyword(""));
    }

    // === normalize_work_type_from_ocr ===

    #[test]
    fn normalize_work_type_strips_hoshu_suffix() {
        assert_eq!(normalize_work_type_from_ocr("舗装補修工"), "舗装工");
        assert_eq!(normalize_work_type_from_ocr("道路補修工"), "道路工");
    }

    #[test]
    fn normalize_work_type_passes_through_canonical() {
        assert_eq!(normalize_work_type_from_ocr("舗装工"), "舗装工");
        assert_eq!(normalize_work_type_from_ocr("区画線工"), "区画線工");
        assert_eq!(normalize_work_type_from_ocr("構造物撤去工"), "構造物撤去工");
    }

    #[test]
    fn normalize_work_type_passes_through_empty() {
        assert_eq!(normalize_work_type_from_ocr(""), "");
    }

    // === extract_tonnage_from_text ===

    #[test]
    fn extract_tonnage_from_weighing_key() {
        assert_eq!(
            extract_tonnage_from_text("計量積載量：9.5"),
            Some("積載量：9.5ｔ".to_string())
        );
    }

    #[test]
    fn extract_tonnage_from_fullwidth_colon() {
        assert_eq!(
            extract_tonnage_from_text("積載量：10"),
            Some("積載量：10ｔ".to_string())
        );
    }

    #[test]
    fn extract_tonnage_from_quantity_key() {
        assert_eq!(
            extract_tonnage_from_text("車番 1234 数量 8.2"),
            Some("積載量：8.2ｔ".to_string())
        );
    }

    #[test]
    fn extract_tonnage_returns_none_without_number() {
        assert!(extract_tonnage_from_text("積載量：").is_none());
        assert!(extract_tonnage_from_text("処分状況").is_none());
        assert!(extract_tonnage_from_text("").is_none());
    }

    #[test]
    fn temperature_keyword_list_is_not_empty() {
        assert!(!TEMPERATURE_STRONG_KEYWORDS.is_empty());
    }
}
