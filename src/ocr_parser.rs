//! 黒板OCRテキストの解析と正規化
//!
//! photo-taggerが返すdetected_textからキー:値ペアを抽出し、
//! 測点表記を正規化する。

/// 既知の黒板キー一覧
const KNOWN_KEYS: &[&str] = &["工事名", "場所", "工種", "測点", "車番", "車両番号"];

/// detected_textを正規化: 既知キー+コロンの前に改行を挿入
///
/// photo-taggerがスペース区切り/カンマ区切りで返す場合がある:
/// "工事名：AAA 場所：BBB 表層工 初期転圧状況"
/// → "工事名：AAA\n場所：BBB 表層工 初期転圧状況"
///
/// その後、extract_kv_from_textがカンマ展開と場所値のパースを行う
fn normalize_detected_text(text: &str) -> String {
    // リテラル "\n"（2文字）を実際の改行に変換
    let mut s = text.replace("\\n", "\n");

    // 既知キー+全角/半角コロンのパターンの前に改行を挿入
    for &key in KNOWN_KEYS {
        for colon in &["：", ":"] {
            let pattern = format!("{}{}", key, colon);
            let mut idx = 0;
            while let Some(pos) = s[idx..].find(&pattern) {
                let abs_pos = idx + pos;
                if abs_pos > 0 {
                    let prev_byte = s.as_bytes().get(abs_pos.saturating_sub(1));
                    if prev_byte != Some(&b'\n') {
                        s.insert(abs_pos, '\n');
                        idx = abs_pos + 1 + pattern.len();
                        continue;
                    }
                }
                idx = abs_pos + pattern.len();
            }
        }
    }
    s
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
pub(crate) fn extract_kv_from_text(text: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();

    // 前処理: 既知キー（全角/半角コロン付き）の前に改行を挿入して正規化
    // "工事名：AAA 場所：BBB 表層工 初期転圧状況" →
    // "工事名：AAA\n場所：BBB\n表層工\n初期転圧状況"
    let normalized = normalize_detected_text(text);

    // 改行で分割し、各行をさらにカンマ区切りで展開
    let mut lines: Vec<String> = Vec::new();
    for segment in normalized.split('\n') {
        let segment = segment.trim();
        if segment.is_empty() { continue; }
        if segment.contains(", ") {
            // カンマ区切り行を展開
            for part in segment.split(", ") {
                let p = part.trim();
                if !p.is_empty() { lines.push(p.to_string()); }
            }
        } else {
            lines.push(segment.to_string());
        }
    }
    for line in &lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // 全角コロン `：` または半角コロン `:` で分割を試みる
        if let Some((k, v)) = line.split_once('：').or_else(|| line.split_once(':')) {
            let k = k.trim();
            let v = v.trim();
            if !k.is_empty() {
                // 場所/測点: 値中のスペース後に日本語キーワードが続く場合を分離
                // "No.6 R 表層工 初期転圧状況" → 場所:"No.6 R", keywords: "表層工","初期転圧状況"
                if k == "場所" || k == "測点" {
                    let tokens: Vec<&str> = v.split_whitespace().collect();
                    let mut station_parts = Vec::new();
                    let mut found_jp = false;
                    for t in &tokens {
                        if !found_jp && !t.chars().any(|c| c > '\u{3000}') {
                            station_parts.push(*t);
                        } else {
                            found_jp = true;
                            result.push((String::new(), t.to_string()));
                        }
                    }
                    result.push((k.to_string(), station_parts.join(" ")));
                } else {
                    result.push((k.to_string(), v.to_string()));
                }
                continue;
            }
        }
        // 既知キーでスペース分割を試みる（"場所 No. 4 L" 等）
        // キー直後が「・」等の接続文字の場合は複合ラベル（"工種・種別"）なのでスキップ
        let mut matched = false;
        for &key in KNOWN_KEYS {
            if line.starts_with(key) && line.len() > key.len() {
                let after = line[key.len()..].chars().next();
                match after {
                    Some(' ') | Some('\u{3000}') => {
                        let rest = line[key.len()..].trim_start();
                        result.push((key.to_string(), rest.to_string()));
                        matched = true;
                        break;
                    }
                    _ => {} // "工種・種別" 等 → マッチしない
                }
            }
        }
        if !matched {
            // キーなし行はキーワードとして ("", value) で返す
            result.push((String::new(), line.to_string()));
        }
    }
    result
}

/// 測点表記を正規化する（L→左車線、R→右車線）
pub(crate) fn normalize_station(station: &str) -> String {
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
    fn test_extract_kv_from_text_literal_backslash_n() {
        // リテラル "\n" (2文字: バックスラッシュ + n) を含むケース
        let text = r"工事名：市道 南千反畑第1号線舗装補修工事\n場所：No.4 L\n路面切削工\n切削・積込状況";
        let kvs = extract_kv_from_text(text);
        assert_eq!(kvs.len(), 4);
        assert_eq!(kvs[0], ("工事名".to_string(), "市道 南千反畑第1号線舗装補修工事".to_string()));
        assert_eq!(kvs[1], ("場所".to_string(), "No.4 L".to_string()));
        assert_eq!(kvs[2], ("".to_string(), "路面切削工".to_string()));
        assert_eq!(kvs[3], ("".to_string(), "切削・積込状況".to_string()));
    }

    #[test]
    fn test_extract_kv_from_text_halfwidth_colon() {
        let text = "工事名:市道 南千反畑第1号線舗装補修工事\n場所:No.4 L";
        let kvs = extract_kv_from_text(text);
        assert_eq!(kvs.len(), 2);
        assert_eq!(kvs[0], ("工事名".to_string(), "市道 南千反畑第1号線舗装補修工事".to_string()));
        assert_eq!(kvs[1], ("場所".to_string(), "No.4 L".to_string()));
    }

    #[test]
    fn test_extract_kv_from_text_mixed_format() {
        // 全角コロン + スペース区切り + キーなし行の混在
        let text = "工事名：テスト工事\n工種 舗装工\n表層工\n舗設状況";
        let kvs = extract_kv_from_text(text);
        assert_eq!(kvs.len(), 4);
        assert_eq!(kvs[0], ("工事名".to_string(), "テスト工事".to_string()));
        assert_eq!(kvs[1], ("工種".to_string(), "舗装工".to_string()));
        assert_eq!(kvs[2], ("".to_string(), "表層工".to_string()));
        assert_eq!(kvs[3], ("".to_string(), "舗設状況".to_string()));
    }

    #[test]
    fn test_extract_kv_compound_label_not_matched() {
        // "工種・種別 舗装工" は複合ラベルなのでキーマッチしない → キーワードとして返す
        let text = "工種・種別 舗装工, 細別・規格 再生密粒度アスコン20mm, 解放温度 36.1 ℃";
        let kvs = extract_kv_from_text(text);
        // 全てキーなし行
        assert!(kvs.iter().all(|(k, _)| k.is_empty()), "compound labels should not match KNOWN_KEYS: {:?}", kvs);
    }
}
