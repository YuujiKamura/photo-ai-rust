//! CSVパース関連ユーティリティ
//!
//! 工種階層マスタCSVの行パースとフィールド抽出を担当する。

use super::HierarchyRow;

/// CSV文字列からHierarchyRow配列をパース
///
/// ヘッダー行をスキップし、7カラム以上の行のみパースする。
pub(crate) fn parse_rows_from_csv(content: &str) -> Vec<HierarchyRow> {
    let mut rows = Vec::new();

    for line in content.lines().skip(1) {
        let fields: Vec<&str> = parse_csv_line(line);
        if fields.len() < 7 {
            continue;
        }

        rows.push(HierarchyRow {
            photo_division: fields[0].to_string(),
            photo_type: fields[1].to_string(),
            work_type: fields[2].to_string(),
            variety: fields[3].to_string(),
            subphase: fields[4].to_string(),
            remarks: fields[5].to_string(),
            search_patterns: fields[6].to_string(),
        });
    }

    rows
}

/// CSV行をパース（ダブルクォート対応）
fn parse_csv_line(line: &str) -> Vec<&str> {
    let mut fields = Vec::new();
    let mut in_quotes = false;
    let mut field_start = 0;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c == '"' {
            in_quotes = !in_quotes;
        } else if c == ',' && !in_quotes {
            let field = &line[field_start..byte_index(line, i)];
            fields.push(trim_quotes(field));
            field_start = byte_index(line, i + 1);
        }
        i += 1;
    }

    if field_start <= line.len() {
        let field = &line[field_start..];
        fields.push(trim_quotes(field));
    }

    fields
}

fn byte_index(s: &str, char_index: usize) -> usize {
    s.char_indices()
        .nth(char_index)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

fn trim_quotes(s: &str) -> &str {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        &s[1..s.len() - 1]
    } else {
        s
    }
}
