//! 測点（Station）
//!
//! 工事写真の station フィールドは文字列だが、意味的に4つのケースに分かれる:
//! 1. No.測点（道路工事の施工位置、左右車線の別あり）
//! 2. 取付道路 No.測点（枝道）
//! 3. 日付（安全管理/品質管理/温度管理写真、ダンプ台数付加あり）
//! 4. 線種名やフリーテキスト（区画線工等）
//!
//! これらを enum で区別し、ダンプ台数の付加・レーン判定・access_road フラグを
//! 型で扱えるようにする。
//!
//! ## 運用ルール
//!
//! - `parse` は文字列から自動判定する。線種名やフリーテキストは `Other` に入る。
//!   区画線工かどうかのコンテキスト判定は呼出元の責任（work_type を見る）。
//! - `to_display` で表示文字列に戻す（PDF/Excel 向け）。
//! - `with_dump_number` は `Date` にのみ効く（他の variant は変更なし）。

/// 測点
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Station {
    /// 未設定
    Empty,
    /// No.測点 (道路本線/取付道路)
    Post(PostStation),
    /// 日付 (+ オプションでダンプ台数)
    Date(DateStation),
    /// それ以外のフリーテキスト（線種名、作業名等）
    Other(String),
}

/// 車線
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lane {
    Left,
    Right,
}

impl Lane {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Left => "L",
            Self::Right => "R",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "L" | "l" | "左" => Some(Self::Left),
            "R" | "r" | "右" => Some(Self::Right),
            _ => None,
        }
    }
}

/// No.測点
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PostStation {
    /// 正規化ラベル（例: `"No.6"`）
    pub label: String,
    /// 車線（L/R）
    pub lane: Option<Lane>,
    /// 取付道路フラグ（枝道）
    pub access_road: bool,
}

/// 日付測点
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DateStation {
    pub month: u8,
    pub day: u8,
    pub dump_number: Option<u8>,
}

impl Station {
    /// 文字列から測点を解析する
    ///
    /// - 空/ダッシュ → `Empty`
    /// - `取付道路 No.N` → `Post { access_road: true }`
    /// - `No.N [L|R]` → `Post`
    /// - `X月Y日` または `X月Y日 N台目` → `Date`
    /// - その他 → `Other(s)`
    pub fn parse(s: &str) -> Self {
        let trimmed = s.trim();
        if trimmed.is_empty() || trimmed == "-" {
            return Self::Empty;
        }

        // 取付道路
        if let Some(rest) = trimmed.strip_prefix("取付道路") {
            let rest = rest.trim_start();
            if let Some(label) = extract_post_label(rest) {
                return Self::Post(PostStation {
                    label,
                    lane: None,
                    access_road: true,
                });
            }
        }

        // 日付（X月Y日 [N台目]）
        if let Some(ds) = parse_date_station(trimmed) {
            return Self::Date(ds);
        }

        // No.測点
        if let Some((label, lane)) = parse_post(trimmed) {
            return Self::Post(PostStation {
                label,
                lane,
                access_road: false,
            });
        }

        Self::Other(trimmed.to_string())
    }

    /// PDF/Excel 表示用の文字列に戻す
    pub fn to_display(&self) -> String {
        match self {
            Self::Empty => String::new(),
            Self::Post(p) => {
                let mut s = if p.access_road {
                    format!("取付道路 {}", p.label)
                } else {
                    p.label.clone()
                };
                if let Some(lane) = p.lane {
                    s.push(' ');
                    s.push_str(lane.as_str());
                }
                s
            }
            Self::Date(d) => {
                let base = format!("{}月{}日", d.month, d.day);
                if let Some(n) = d.dump_number {
                    format!("{} {}台目", base, n)
                } else {
                    base
                }
            }
            Self::Other(s) => s.clone(),
        }
    }

    /// ダンプ台数を付加する（`Date` のみ効果あり）
    pub fn with_dump_number(self, n: u8) -> Self {
        match self {
            Self::Date(d) => Self::Date(DateStation {
                dump_number: Some(n),
                ..d
            }),
            other => other,
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    pub fn is_date(&self) -> bool {
        matches!(self, Self::Date(_))
    }

    pub fn is_post(&self) -> bool {
        matches!(self, Self::Post(_))
    }

    /// ダンプ台数が付与されているか（`Date` variant 限定）
    pub fn has_dump_number(&self) -> bool {
        matches!(self, Self::Date(d) if d.dump_number.is_some())
    }

    /// ダンプ台数を取得する（`Date` variant 限定）
    pub fn dump_number(&self) -> Option<u8> {
        match self {
            Self::Date(d) => d.dump_number,
            _ => None,
        }
    }
}

fn extract_post_label(s: &str) -> Option<String> {
    // "No.1" / "NO.5" / "no.10" を受理
    let lower = s.trim_start_matches(|c: char| !c.is_ascii_alphabetic());
    let head_len = lower
        .chars()
        .take(3)
        .take_while(|c| c.is_ascii_alphabetic() || *c == '.')
        .count();
    if head_len < 3 {
        return None;
    }
    let (head, tail) = lower.split_at(head_len);
    if !head.eq_ignore_ascii_case("No.") {
        return None;
    }
    let digits: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    Some(format!("No.{}", digits))
}

/// `No.N [lane] [車線]` 形式のパース。regex に依存しない手書き実装。
fn parse_post(s: &str) -> Option<(String, Option<Lane>)> {
    let s = s.trim();
    // "No." prefix（大小問わず）
    let rest = {
        let mut chars = s.chars();
        let mut prefix = String::new();
        for _ in 0..3 {
            match chars.next() {
                Some(c) => prefix.push(c),
                None => return None,
            }
        }
        if !prefix.eq_ignore_ascii_case("No.") {
            return None;
        }
        chars.as_str()
    };

    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let after_digits = rest[digits.len()..].trim();

    // lane 判定 (L/R/左/右) + オプションで "車線"
    let (lane_part, trailer) = after_digits.split_once("車線").unwrap_or((after_digits, ""));
    if !trailer.trim().is_empty() {
        return None; // "車線" の後に余分な文字
    }
    let lane = match lane_part.trim() {
        "" => None,
        other => Some(Lane::from_str(other)?),
    };

    Some((format!("No.{}", digits), lane))
}

/// `X月Y日 [N台目]` 形式のパース。regex に依存しない手書き実装。
fn parse_date_station(s: &str) -> Option<DateStation> {
    let (month_str, rest) = s.split_once('月')?;
    let month: u8 = month_str.trim().parse().ok()?;
    if !(1..=12).contains(&month) {
        return None;
    }
    let (day_str, tail) = rest.split_once('日')?;
    let day: u8 = day_str.trim().parse().ok()?;
    if !(1..=31).contains(&day) {
        return None;
    }
    let tail = tail.trim();
    let dump_number = if tail.is_empty() {
        None
    } else {
        let (num_str, suffix) = tail.split_once("台目")?;
        if !suffix.trim().is_empty() {
            return None;
        }
        let n: u8 = num_str.trim().parse().ok()?;
        if n == 0 {
            return None;
        }
        Some(n)
    };
    Some(DateStation {
        month,
        day,
        dump_number,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // === parse: empty/dash ===

    #[test]
    fn parse_empty_string_returns_empty() {
        assert_eq!(Station::parse(""), Station::Empty);
        assert_eq!(Station::parse("  "), Station::Empty);
        assert_eq!(Station::parse("-"), Station::Empty);
    }

    // === parse: Post ===

    #[test]
    fn parse_post_without_lane() {
        let s = Station::parse("No.6");
        assert_eq!(
            s,
            Station::Post(PostStation {
                label: "No.6".to_string(),
                lane: None,
                access_road: false,
            })
        );
    }

    #[test]
    fn parse_post_with_right_lane() {
        let s = Station::parse("No.6 R");
        let Station::Post(p) = s else { panic!("expected Post") };
        assert_eq!(p.label, "No.6");
        assert_eq!(p.lane, Some(Lane::Right));
        assert!(!p.access_road);
    }

    #[test]
    fn parse_post_with_left_lane_variants() {
        for input in ["No.4 L", "No.4 l", "No.4 左"] {
            let Station::Post(p) = Station::parse(input) else {
                panic!("expected Post for {}", input);
            };
            assert_eq!(p.lane, Some(Lane::Left));
        }
    }

    #[test]
    fn parse_access_road() {
        let s = Station::parse("取付道路 No.1");
        assert_eq!(
            s,
            Station::Post(PostStation {
                label: "No.1".to_string(),
                lane: None,
                access_road: true,
            })
        );
    }

    // === parse: Date ===

    #[test]
    fn parse_date_without_dump() {
        let s = Station::parse("2月11日");
        assert_eq!(
            s,
            Station::Date(DateStation {
                month: 2,
                day: 11,
                dump_number: None,
            })
        );
    }

    #[test]
    fn parse_date_with_dump_number() {
        let s = Station::parse("2月9日 1台目");
        let Station::Date(d) = s else {
            panic!("expected Date")
        };
        assert_eq!(d.month, 2);
        assert_eq!(d.day, 9);
        assert_eq!(d.dump_number, Some(1));
    }

    #[test]
    fn parse_date_rejects_invalid_month_or_day() {
        // 13月 → Date ではなく Other
        assert!(matches!(Station::parse("13月5日"), Station::Other(_)));
        // 32日 → Date ではなく Other
        assert!(matches!(Station::parse("2月32日"), Station::Other(_)));
    }

    // === parse: Other ===

    #[test]
    fn parse_line_type_falls_to_other() {
        // 区画線工の線種名は Other として保持される（コンテキストで意味付けするのは呼出元）
        assert_eq!(
            Station::parse("ダイヤモンド標示"),
            Station::Other("ダイヤモンド標示".to_string())
        );
        assert_eq!(
            Station::parse("横断歩道線"),
            Station::Other("横断歩道線".to_string())
        );
    }

    // === to_display round-trip ===

    #[test]
    fn roundtrip_post_without_lane() {
        let s = Station::parse("No.6");
        assert_eq!(s.to_display(), "No.6");
    }

    #[test]
    fn roundtrip_post_with_lane() {
        let s = Station::parse("No.6 R");
        assert_eq!(s.to_display(), "No.6 R");
    }

    #[test]
    fn roundtrip_access_road() {
        let s = Station::parse("取付道路 No.1");
        assert_eq!(s.to_display(), "取付道路 No.1");
    }

    #[test]
    fn roundtrip_date_with_dump() {
        let s = Station::parse("2月11日 2台目");
        assert_eq!(s.to_display(), "2月11日 2台目");
    }

    #[test]
    fn empty_to_display_is_empty_string() {
        assert_eq!(Station::Empty.to_display(), "");
    }

    // === with_dump_number ===

    #[test]
    fn with_dump_number_adds_to_date() {
        let s = Station::parse("2月11日").with_dump_number(3);
        assert_eq!(s.to_display(), "2月11日 3台目");
    }

    #[test]
    fn with_dump_number_overrides_existing() {
        let s = Station::parse("2月11日 1台目").with_dump_number(5);
        assert_eq!(s.to_display(), "2月11日 5台目");
    }

    #[test]
    fn with_dump_number_no_effect_on_post() {
        let before = Station::parse("No.6 R");
        let after = before.clone().with_dump_number(3);
        assert_eq!(before, after);
    }

    #[test]
    fn with_dump_number_no_effect_on_empty() {
        assert_eq!(Station::Empty.with_dump_number(2), Station::Empty);
    }

    // === Predicates ===

    #[test]
    fn is_date_true_only_for_date() {
        assert!(Station::parse("2月11日").is_date());
        assert!(!Station::parse("No.6").is_date());
        assert!(!Station::Empty.is_date());
    }

    #[test]
    fn is_post_true_for_access_road_and_normal() {
        assert!(Station::parse("No.6").is_post());
        assert!(Station::parse("取付道路 No.1").is_post());
        assert!(!Station::parse("2月11日").is_post());
    }

    #[test]
    fn has_dump_number_true_only_for_date_with_dump() {
        assert!(Station::parse("2月11日 2台目").has_dump_number());
        assert!(!Station::parse("2月11日").has_dump_number());
        assert!(!Station::parse("No.6 R").has_dump_number());
        assert!(!Station::Empty.has_dump_number());
    }

    #[test]
    fn dump_number_extracts_value() {
        assert_eq!(Station::parse("2月11日 3台目").dump_number(), Some(3));
        assert_eq!(Station::parse("2月11日").dump_number(), None);
        assert_eq!(Station::parse("No.6").dump_number(), None);
    }
}
