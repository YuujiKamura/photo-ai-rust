//! 計測値の検出と保護
//!
//! 温度・寸法などの計測値を含むレコードを検出し、
//! 正規化処理から保護する。

use regex::Regex;

/// 計測値の種類
#[derive(Debug, Clone, PartialEq)]
pub enum MeasurementType {
    /// 温度（℃, 度）
    Temperature(f64),
    /// 寸法（mm, cm, m）
    Dimension(f64, String),
    /// 密度（%）
    Density(f64),
    /// その他の数値
    Other(String),
}

/// テキストに計測値が含まれているか判定
pub fn contains_measurement(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }

    lazy_static::lazy_static! {
        // 温度パターン
        static ref TEMP_RE: Regex = Regex::new(r"(\d+\.?\d*)\s*[℃度]").unwrap();
        // 寸法パターン
        static ref DIM_RE: Regex = Regex::new(r"[t=]?\s*(\d+\.?\d*)\s*(mm|cm|m)\b").unwrap();
        // 密度パターン
        static ref DENSITY_RE: Regex = Regex::new(r"(\d+\.?\d*)\s*%").unwrap();
        // 一般的な数値+単位パターン
        static ref GENERAL_RE: Regex = Regex::new(r"\d+\.?\d*\s*(kg|g|L|kN|MPa)").unwrap();
    }

    TEMP_RE.is_match(text)
        || DIM_RE.is_match(text)
        || DENSITY_RE.is_match(text)
        || GENERAL_RE.is_match(text)
}

/// テキストから計測値を抽出
pub fn extract_measurements(text: &str) -> Vec<MeasurementType> {
    let mut measurements = Vec::new();

    lazy_static::lazy_static! {
        static ref TEMP_RE: Regex = Regex::new(r"(\d+\.?\d*)\s*[℃度]").unwrap();
        static ref DIM_RE: Regex = Regex::new(r"[t=]?\s*(\d+\.?\d*)\s*(mm|cm|m)\b").unwrap();
        static ref DENSITY_RE: Regex = Regex::new(r"(\d+\.?\d*)\s*%").unwrap();
    }

    // 温度
    for cap in TEMP_RE.captures_iter(text) {
        if let Ok(value) = cap[1].parse::<f64>() {
            measurements.push(MeasurementType::Temperature(value));
        }
    }

    // 寸法
    for cap in DIM_RE.captures_iter(text) {
        if let Ok(value) = cap[1].parse::<f64>() {
            measurements.push(MeasurementType::Dimension(value, cap[2].to_string()));
        }
    }

    // 密度
    for cap in DENSITY_RE.captures_iter(text) {
        if let Ok(value) = cap[1].parse::<f64>() {
            measurements.push(MeasurementType::Density(value));
        }
    }

    measurements
}

/// 温度値を抽出（℃）
pub fn extract_temperature(text: &str) -> Option<f64> {
    lazy_static::lazy_static! {
        static ref TEMP_RE: Regex = Regex::new(r"(\d+\.?\d*)\s*[℃度]").unwrap();
    }

    TEMP_RE
        .captures(text)
        .and_then(|cap| cap[1].parse::<f64>().ok())
}

/// 寸法値を抽出（mm単位に正規化）
pub fn extract_dimension_mm(text: &str) -> Option<f64> {
    lazy_static::lazy_static! {
        static ref DIM_RE: Regex = Regex::new(r"[t=]?\s*(\d+\.?\d*)\s*(mm|cm|m)\b").unwrap();
    }

    DIM_RE.captures(text).and_then(|cap| {
        let value: f64 = cap[1].parse().ok()?;
        let unit = &cap[2];
        Some(match unit {
            "m" => value * 1000.0,
            "cm" => value * 10.0,
            _ => value, // mm
        })
    })
}

/// 温度写真かどうか判定（温度関連のキーワード）
pub fn is_temperature_photo(text: &str) -> bool {
    lazy_static::lazy_static! {
        static ref TEMP_KEYWORDS: Regex = Regex::new(
            r"(?i)(到着温度|敷均し温度|初期締固め|温度測定|温度計|出荷時|舗設温度)"
        ).unwrap();
    }

    TEMP_KEYWORDS.is_match(text) || extract_temperature(text).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains_measurement_temperature() {
        assert!(contains_measurement("出荷時156℃"));
        assert!(contains_measurement("到着温度 160.4度"));
        assert!(contains_measurement("温度：158℃"));
    }

    #[test]
    fn test_contains_measurement_dimension() {
        assert!(contains_measurement("t=50mm"));
        assert!(contains_measurement("厚さ 5cm"));
        assert!(contains_measurement("幅 2.5m"));
    }

    #[test]
    fn test_contains_measurement_density() {
        assert!(contains_measurement("締固め度 98.5%"));
        assert!(contains_measurement("密度 96%"));
    }

    #[test]
    fn test_contains_measurement_false() {
        assert!(!contains_measurement(""));
        assert!(!contains_measurement("舗設状況"));
        assert!(!contains_measurement("No.10+50"));
    }

    #[test]
    fn test_extract_temperature() {
        assert_eq!(extract_temperature("出荷時156℃"), Some(156.0));
        assert_eq!(extract_temperature("温度 160.4度"), Some(160.4));
        assert_eq!(extract_temperature("測定なし"), None);
    }

    #[test]
    fn test_extract_dimension_mm() {
        assert_eq!(extract_dimension_mm("t=50mm"), Some(50.0));
        assert_eq!(extract_dimension_mm("厚さ 5cm"), Some(50.0));
        assert_eq!(extract_dimension_mm("幅 2.5m"), Some(2500.0));
    }

    #[test]
    fn test_extract_measurements() {
        let text = "出荷時156℃、t=50mm";
        let measurements = extract_measurements(text);
        assert_eq!(measurements.len(), 2);
        assert!(matches!(measurements[0], MeasurementType::Temperature(156.0)));
        assert!(matches!(measurements[1], MeasurementType::Dimension(50.0, _)));
    }

    #[test]
    fn test_is_temperature_photo() {
        assert!(is_temperature_photo("到着温度"));
        assert!(is_temperature_photo("敷均し温度測定"));
        assert!(is_temperature_photo("出荷時 156℃"));
        assert!(!is_temperature_photo("舗設状況"));
    }
}
