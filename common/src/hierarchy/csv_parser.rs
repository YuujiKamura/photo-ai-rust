//! CSVパース関連ユーティリティ
//!
//! 工種階層マスタCSVの行パースとフィールド抽出を担当する。

use super::HierarchyRow;

/// CSV文字列からHierarchyRow配列をパース
///
/// ヘッダー行をスキップし、7カラム以上の行のみパースする。
/// カラム数不足の行はeprintln!で警告を出してスキップする。
pub(crate) fn parse_rows_from_csv(content: &str) -> Vec<HierarchyRow> {
    let mut rows = Vec::new();

    for (line_index, line) in content.lines().enumerate() {
        // line_index 0 = ヘッダー行（スキップ）
        if line_index == 0 {
            continue;
        }
        let line_number = line_index + 1; // 1-based line number

        if line.trim().is_empty() {
            continue;
        }

        let fields: Vec<&str> = parse_csv_line(line);
        if fields.len() < 7 {
            eprintln!(
                "CSV parse warning: line {}: expected 7+ fields, got {} (skipped)",
                line_number,
                fields.len()
            );
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
///
/// イテレータベースでバイト位置を追跡し、O(N)で処理する。
fn parse_csv_line(line: &str) -> Vec<&str> {
    let mut fields = Vec::new();
    let mut in_quotes = false;
    let mut field_start = 0;

    for (byte_pos, c) in line.char_indices() {
        if c == '"' {
            in_quotes = !in_quotes;
        } else if c == ',' && !in_quotes {
            let field = &line[field_start..byte_pos];
            fields.push(trim_quotes(field));
            field_start = byte_pos + 1; // ',' is 1 byte in UTF-8
        }
    }

    // 最後のフィールド
    if field_start <= line.len() {
        let field = &line[field_start..];
        fields.push(trim_quotes(field));
    }

    fields
}

fn trim_quotes(s: &str) -> &str {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_csv_line_basic() {
        let fields = parse_csv_line("a,b,c");
        assert_eq!(fields, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_parse_csv_line_quoted() {
        let fields = parse_csv_line("\"hello\",\"world\"");
        assert_eq!(fields, vec!["hello", "world"]);
    }

    #[test]
    fn test_parse_csv_line_comma_in_quotes() {
        let fields = parse_csv_line("\"a,b\",c");
        assert_eq!(fields, vec!["a,b", "c"]);
    }

    #[test]
    fn test_parse_csv_line_japanese() {
        let fields = parse_csv_line("\"舗装工\",\"舗装打換え工\",\"表層工\"");
        assert_eq!(fields, vec!["舗装工", "舗装打換え工", "表層工"]);
    }

    #[test]
    fn test_parse_csv_line_empty_fields() {
        let fields = parse_csv_line(",,,");
        assert_eq!(fields, vec!["", "", "", ""]);
    }

    #[test]
    fn test_parse_rows_skips_header() {
        let csv = "header1,header2,h3,h4,h5,h6,h7\na,b,c,d,e,f,g\n";
        let rows = parse_rows_from_csv(csv);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].photo_division, "a");
    }

    #[test]
    fn test_parse_rows_warns_on_short_line() {
        let csv = "h1,h2,h3,h4,h5,h6,h7\nonly,three,fields\na,b,c,d,e,f,g\n";
        let rows = parse_rows_from_csv(csv);
        // Short line is skipped with warning, valid line is kept
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].photo_division, "a");
    }

    #[test]
    fn test_parse_rows_skips_empty_lines() {
        let csv = "h1,h2,h3,h4,h5,h6,h7\n\na,b,c,d,e,f,g\n  \n";
        let rows = parse_rows_from_csv(csv);
        assert_eq!(rows.len(), 1);
    }
}
