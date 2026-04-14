// Package normalizer — temperature domain types.
// Ported from common/src/domain/temperature.rs (issue #165 Stream B1).
//
// TemperatureKind represents the five measurement categories used in asphalt
// paving temperature management.  All public API names use the prefix
// "Temperature*" to avoid collision with other streams in this package.
package normalizer

import "strings"

// TemperatureKind is the category of a temperature measurement.
type TemperatureKind int

const (
	// TemperatureArrival — 到着温度 (asphalt arrival at site).
	TemperatureArrival TemperatureKind = iota
	// TemperatureSpreading — 敷均し温度 (spreading).
	TemperatureSpreading
	// TemperatureInitialCompaction — 初期締固め前温度 (before initial compaction).
	TemperatureInitialCompaction
	// TemperatureOpening — 開放温度 (opening/release temperature).
	TemperatureOpening
	// TemperatureOutsideAir — 舗装日外気温 (ambient air temperature on paving day).
	TemperatureOutsideAir
)

// TemperatureLabel returns the canonical label with 「測定」suffix.
// Mirrors TemperatureKind::label in common/src/domain/temperature.rs.
func (k TemperatureKind) TemperatureLabel() string {
	switch k {
	case TemperatureArrival:
		return "到着温度測定"
	case TemperatureSpreading:
		return "敷均し温度測定"
	case TemperatureInitialCompaction:
		return "初期締固め前温度測定"
	case TemperatureOpening:
		return "開放温度測定"
	case TemperatureOutsideAir:
		return "舗装日外気温測定"
	default:
		return ""
	}
}

// TemperatureShortLabel returns the abbreviated label without 「測定」suffix.
// Mirrors TemperatureKind::short_label in common/src/domain/temperature.rs.
func (k TemperatureKind) TemperatureShortLabel() string {
	switch k {
	case TemperatureArrival:
		return "到着温度"
	case TemperatureSpreading:
		return "敷均し温度"
	case TemperatureInitialCompaction:
		return "初期締固め前温度"
	case TemperatureOpening:
		return "開放温度"
	case TemperatureOutsideAir:
		return "舗装日外気温"
	default:
		return ""
	}
}

// TemperatureKindFromText infers a TemperatureKind from arbitrary text.
// Returns (kind, true) when a match is found, (0, false) otherwise.
// Mirrors TemperatureKind::from_text in common/src/domain/temperature.rs including
// spelling-variant absorption (初期転圧前/初期締固め前, 開放/解放).
func TemperatureKindFromText(text string) (TemperatureKind, bool) {
	if text == "" {
		return 0, false
	}
	if strings.Contains(text, "到着温度") {
		return TemperatureArrival, true
	}
	if strings.Contains(text, "敷均し温度") || strings.Contains(text, "敷きならし") {
		return TemperatureSpreading, true
	}
	if strings.Contains(text, "初期転圧前") || strings.Contains(text, "初期締固め前") {
		return TemperatureInitialCompaction, true
	}
	if strings.Contains(text, "開放温度") || strings.Contains(text, "解放温度") {
		return TemperatureOpening, true
	}
	if strings.Contains(text, "舗装日外気温") || strings.Contains(text, "外気温") {
		return TemperatureOutsideAir, true
	}
	return 0, false
}

// TemperatureAllKinds returns the canonical slice of all five TemperatureKind values.
// Mirrors TemperatureKind::all in common/src/domain/temperature.rs.
func TemperatureAllKinds() []TemperatureKind {
	return []TemperatureKind{
		TemperatureArrival,
		TemperatureSpreading,
		TemperatureInitialCompaction,
		TemperatureOpening,
		TemperatureOutsideAir,
	}
}

// TemperatureAllLabels returns all recognised label strings (with and without 測定).
// Mirrors TemperatureKind::all_labels in common/src/domain/temperature.rs.
// The slice length is 9: 5 with 測定 suffix + 4 without (OutsideAir has no short form).
func TemperatureAllLabels() []string {
	return []string{
		"舗装日外気温測定",
		"到着温度測定",
		"敷均し温度測定",
		"初期締固め前温度測定",
		"開放温度測定",
		"到着温度",
		"敷均し温度",
		"初期締固め前温度",
		"開放温度",
	}
}

// TemperatureValidRange returns the plausible temperature range (min, max) in ℃
// for a given kind.
// Mirrors TemperatureKind::valid_range in common/src/domain/temperature.rs.
func TemperatureValidRange(k TemperatureKind) (float64, float64) {
	switch k {
	case TemperatureArrival:
		return 140.0, 185.0
	case TemperatureSpreading:
		return 130.0, 175.0
	case TemperatureInitialCompaction:
		return 120.0, 165.0
	case TemperatureOpening:
		return 30.0, 70.0
	case TemperatureOutsideAir:
		return -10.0, 50.0
	default:
		return 0, 0
	}
}

// TemperatureIsValid reports whether temp (℃) falls within the valid range for k.
// Mirrors TemperatureKind::is_valid_temperature in common/src/domain/temperature.rs.
func TemperatureIsValid(k TemperatureKind, temp float64) bool {
	min, max := TemperatureValidRange(k)
	return temp >= min && temp <= max
}
