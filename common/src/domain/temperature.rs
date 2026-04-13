//! 温度管理の測定種別
//!
//! アスファルト舗装工事の温度管理は5種類の測定種別を持つ。
//! `src/temperature.rs` から昇格した定義を集約する。

/// 温度管理の測定種別
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TemperatureKind {
    /// 到着温度（合材到着時）
    Arrival,
    /// 敷均し温度
    Spreading,
    /// 初期締固め前温度（初期転圧前温度）
    InitialCompaction,
    /// 開放温度（解放温度）
    Opening,
    /// 舗装日外気温
    OutsideAir,
}

impl TemperatureKind {
    /// 正規ラベル（「測定」suffix 付き）
    ///
    /// マスタ上の remarks 表記と一致させる。
    pub const fn label(self) -> &'static str {
        match self {
            Self::Arrival => "到着温度測定",
            Self::Spreading => "敷均し温度測定",
            Self::InitialCompaction => "初期締固め前温度測定",
            Self::Opening => "開放温度測定",
            Self::OutsideAir => "舗装日外気温測定",
        }
    }

    /// 略称（「測定」suffix なし、黒板表記用）
    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Arrival => "到着温度",
            Self::Spreading => "敷均し温度",
            Self::InitialCompaction => "初期締固め前温度",
            Self::Opening => "開放温度",
            Self::OutsideAir => "舗装日外気温",
        }
    }

    /// テキストから温度種別を判定（focusTarget / detectedText 両対応）
    ///
    /// 表記ゆれ（初期転圧前/初期締固め前、開放/解放）も吸収する。
    pub fn from_text(text: &str) -> Option<Self> {
        if text.is_empty() {
            return None;
        }
        if text.contains("到着温度") {
            Some(Self::Arrival)
        } else if text.contains("敷均し温度") || text.contains("敷きならし") {
            Some(Self::Spreading)
        } else if text.contains("初期転圧前") || text.contains("初期締固め前") {
            Some(Self::InitialCompaction)
        } else if text.contains("開放温度") || text.contains("解放温度") {
            Some(Self::Opening)
        } else if text.contains("舗装日外気温") || text.contains("外気温") {
            Some(Self::OutsideAir)
        } else {
            None
        }
    }

    /// 有効な温度種別ラベル一覧（「測定」suffix 付き・なし両方）
    pub const fn all_labels() -> &'static [&'static str] {
        &[
            "舗装日外気温測定",
            "到着温度測定",
            "敷均し温度測定",
            "初期締固め前温度測定",
            "開放温度測定",
            "到着温度",
            "敷均し温度",
            "初期締固め前温度",
            "開放温度",
        ]
    }

    /// 5種別すべてを列挙する
    pub const fn all() -> &'static [TemperatureKind] {
        &[
            Self::Arrival,
            Self::Spreading,
            Self::InitialCompaction,
            Self::Opening,
            Self::OutsideAir,
        ]
    }

    /// 温度の妥当範囲（℃）
    ///
    /// アスファルト舗装の実務で観測されるおおよその範囲。
    /// OCR誤読（小数点抜け等）の検出に使用する。
    pub const fn valid_range(self) -> (f64, f64) {
        match self {
            Self::Arrival => (140.0, 185.0),
            Self::Spreading => (130.0, 175.0),
            Self::InitialCompaction => (120.0, 165.0),
            Self::Opening => (30.0, 70.0),
            Self::OutsideAir => (-10.0, 50.0),
        }
    }

    /// 温度値が妥当な範囲内か判定する
    pub fn is_valid_temperature(self, temp: f64) -> bool {
        let (min, max) = self.valid_range();
        temp >= min && temp <= max
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_has_measurement_suffix() {
        for kind in TemperatureKind::all() {
            assert!(kind.label().ends_with("測定"), "{:?}", kind);
        }
    }

    #[test]
    fn short_label_has_no_measurement_suffix() {
        for kind in TemperatureKind::all() {
            assert!(!kind.short_label().ends_with("測定"), "{:?}", kind);
        }
    }

    #[test]
    fn short_label_is_prefix_of_label() {
        for kind in TemperatureKind::all() {
            assert_eq!(
                format!("{}測定", kind.short_label()),
                kind.label(),
                "{:?}",
                kind
            );
        }
    }

    #[test]
    fn from_text_detects_canonical_labels() {
        assert_eq!(
            TemperatureKind::from_text("到着温度"),
            Some(TemperatureKind::Arrival)
        );
        assert_eq!(
            TemperatureKind::from_text("敷均し温度"),
            Some(TemperatureKind::Spreading)
        );
        assert_eq!(
            TemperatureKind::from_text("初期締固め前温度"),
            Some(TemperatureKind::InitialCompaction)
        );
        assert_eq!(
            TemperatureKind::from_text("開放温度"),
            Some(TemperatureKind::Opening)
        );
        assert_eq!(
            TemperatureKind::from_text("舗装日外気温"),
            Some(TemperatureKind::OutsideAir)
        );
    }

    #[test]
    fn from_text_handles_spelling_variants() {
        // 初期転圧前 (旧表記) → InitialCompaction
        assert_eq!(
            TemperatureKind::from_text("初期転圧前温度"),
            Some(TemperatureKind::InitialCompaction)
        );
        // 解放温度 (誤字) → Opening
        assert_eq!(
            TemperatureKind::from_text("解放温度 45℃"),
            Some(TemperatureKind::Opening)
        );
        // 外気温単独 → OutsideAir
        assert_eq!(
            TemperatureKind::from_text("外気温 12℃"),
            Some(TemperatureKind::OutsideAir)
        );
        // 敷きならし (ひらがな表記)
        assert_eq!(
            TemperatureKind::from_text("敷きならし直後"),
            Some(TemperatureKind::Spreading)
        );
    }

    #[test]
    fn from_text_returns_none_for_unrelated() {
        assert_eq!(TemperatureKind::from_text(""), None);
        assert_eq!(TemperatureKind::from_text("舗設状況"), None);
        assert_eq!(TemperatureKind::from_text("密度試験"), None);
    }

    #[test]
    fn valid_range_arrival_covers_typical_values() {
        let k = TemperatureKind::Arrival;
        assert!(k.is_valid_temperature(160.0));
        assert!(k.is_valid_temperature(140.0)); // 境界
        assert!(k.is_valid_temperature(185.0)); // 境界
        assert!(!k.is_valid_temperature(139.9));
        assert!(!k.is_valid_temperature(185.1));
    }

    #[test]
    fn valid_range_opening_rejects_misread_ocr() {
        // 開放温度 32.6℃ → "126℃" と誤読された場合のような値を弾く
        let k = TemperatureKind::Opening;
        assert!(k.is_valid_temperature(32.6));
        assert!(k.is_valid_temperature(45.0));
        assert!(!k.is_valid_temperature(126.0)); // OCR誤読パターン
        assert!(!k.is_valid_temperature(0.0));
    }

    #[test]
    fn valid_range_outside_air_allows_below_zero() {
        let k = TemperatureKind::OutsideAir;
        assert!(k.is_valid_temperature(-5.0));
        assert!(k.is_valid_temperature(30.0));
        assert!(!k.is_valid_temperature(-20.0));
        assert!(!k.is_valid_temperature(60.0));
    }

    #[test]
    fn valid_range_initial_compaction_between_spreading_and_opening() {
        let (min_ic, max_ic) = TemperatureKind::InitialCompaction.valid_range();
        let (_min_sp, max_sp) = TemperatureKind::Spreading.valid_range();
        let (min_op, _max_op) = TemperatureKind::Opening.valid_range();
        // 温度は Spreading > InitialCompaction > Opening の順に下がる
        assert!(max_ic < max_sp);
        assert!(min_ic > min_op);
    }

    #[test]
    fn all_returns_five_kinds() {
        assert_eq!(TemperatureKind::all().len(), 5);
    }

    #[test]
    fn all_labels_contains_both_suffix_styles() {
        let labels = TemperatureKind::all_labels();
        assert!(labels.contains(&"到着温度測定"));
        assert!(labels.contains(&"到着温度"));
        assert_eq!(labels.len(), 9); // 5 with 測定 + 4 without (OutsideAir 含まず短縮形)
    }
}
