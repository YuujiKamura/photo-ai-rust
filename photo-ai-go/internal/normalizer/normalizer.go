// Package normalizer normalises measurement strings extracted from photo boards.
// Ported from src/normalizer/measurements.rs and src/temperature.rs
package normalizer

import (
	"regexp"
	"strconv"
	"strings"
)

// Options controls normalisation behaviour.
type Options struct {
	StripUnits bool
	Locale     string // e.g. "ja"
}

// Normalize returns a canonical form of the raw measurement string.
func Normalize(raw string, opts Options) string {
	return raw
}

// --- Measurement detection ---

var (
	tempRE    = regexp.MustCompile(`(\d+\.?\d*)\s*[℃度]`)
	dimRE     = regexp.MustCompile(`[t=]?\s*(\d+\.?\d*)\s*(mm|cm|m)\b`)
	densityRE = regexp.MustCompile(`(\d+\.?\d*)\s*%`)
	generalRE = regexp.MustCompile(`\d+\.?\d*\s*(kg|g|L|kN|MPa)`)
)

// ContainsMeasurement reports whether text contains a measurement value.
// Mirrors contains_measurement in measurements.rs.
func ContainsMeasurement(text string) bool {
	if text == "" {
		return false
	}
	return tempRE.MatchString(text) ||
		dimRE.MatchString(text) ||
		densityRE.MatchString(text) ||
		generalRE.MatchString(text)
}

// ExtractMeasurements extracts all measurement strings found in text.
// Mirrors extract_measurements in measurements.rs.
func ExtractMeasurements(text string) []string {
	var out []string
	for _, m := range tempRE.FindAllString(text, -1) {
		out = append(out, m)
	}
	for _, m := range dimRE.FindAllString(text, -1) {
		out = append(out, m)
	}
	for _, m := range densityRE.FindAllString(text, -1) {
		out = append(out, m)
	}
	return out
}

// ExtractTemperature extracts the first temperature value (℃/度) from text.
// Mirrors extract_temperature in measurements.rs.
func ExtractTemperature(text string) (float64, bool) {
	m := tempRE.FindStringSubmatch(text)
	if m == nil {
		return 0, false
	}
	v, err := strconv.ParseFloat(m[1], 64)
	if err != nil {
		return 0, false
	}
	return v, true
}

// ExtractDimensionMM extracts the first dimension value, converted to mm.
// Mirrors extract_dimension_mm in measurements.rs.
func ExtractDimensionMM(text string) (float64, bool) {
	m := dimRE.FindStringSubmatch(text)
	if m == nil {
		return 0, false
	}
	v, err := strconv.ParseFloat(m[1], 64)
	if err != nil {
		return 0, false
	}
	unit := m[2]
	switch unit {
	case "m":
		v *= 1000
	case "cm":
		v *= 10
	}
	return v, true
}

// --- Temperature photo classification ---

// temperatureKindFromText mirrors TemperatureKind::from_text in temperature.rs.
func temperatureKindFromText(text string) (string, bool) {
	if text == "" {
		return "", false
	}
	if strings.Contains(text, "到着温度") {
		return "到着温度測定", true
	}
	if strings.Contains(text, "敷均し温度") || strings.Contains(text, "敷きならし") {
		return "敷均し温度測定", true
	}
	if strings.Contains(text, "初期転圧前") || strings.Contains(text, "初期締固め前") {
		return "初期締固め前温度測定", true
	}
	if strings.Contains(text, "開放温度") || strings.Contains(text, "解放温度") {
		return "開放温度測定", true
	}
	if strings.Contains(text, "舗装日外気温") || strings.Contains(text, "外気温") {
		return "舗装日外気温測定", true
	}
	return "", false
}

// IsTemperaturePhoto reports whether the text is from a temperature measurement photo.
// Mirrors is_temperature_photo in measurements.rs.
func IsTemperaturePhoto(text string) bool {
	if _, ok := temperatureKindFromText(text); ok {
		return true
	}
	if strings.Contains(text, "温度測定") ||
		strings.Contains(text, "温度計") ||
		strings.Contains(text, "出荷時") ||
		strings.Contains(text, "舗設温度") {
		return true
	}
	_, ok := ExtractTemperature(text)
	return ok
}

// ClassifyTemperature returns the temperature category label for a temperature value.
// Uses the same valid ranges as TemperatureType in measurements.rs.
func ClassifyTemperature(temp float64) string {
	switch {
	case temp >= 140 && temp <= 185:
		return "到着温度測定"
	case temp >= 130 && temp <= 175:
		return "敷均し温度測定"
	case temp >= 120 && temp <= 165:
		return "初期締固め前温度測定"
	case temp >= 30 && temp <= 70:
		return "開放温度測定"
	default:
		return ""
	}
}

// ExtractTemperatureForRemarks extracts a temperature value from detected text
// that matches the given remarks label.
// Mirrors extract_temperature_for_remarks in measurements.rs.
func ExtractTemperatureForRemarks(detectedText string) string {
	if detectedText == "" {
		return ""
	}

	// Leading temperature value (thermometer close-up: "160.2℃, ...")
	leadingRE := regexp.MustCompile(`^\s*(\d+\.?\d*)\s*[℃度]`)
	if m := leadingRE.FindStringSubmatch(strings.TrimSpace(detectedText)); m != nil {
		return m[1] + "℃"
	}

	// Trailing temperature value ("..., 153.7℃")
	trailingRE := regexp.MustCompile(`(?:,\s*|^)(\d+\.?\d*)\s*[℃度]\s*$`)
	if m := trailingRE.FindStringSubmatch(strings.TrimSpace(detectedText)); m != nil {
		return m[1] + "℃"
	}

	// Number only
	numberOnlyRE := regexp.MustCompile(`^\s*(\d+\.?\d*)\s*[℃度]?\s*$`)
	if m := numberOnlyRE.FindStringSubmatch(strings.TrimSpace(detectedText)); m != nil {
		return m[1] + "℃"
	}

	return ""
}

// ExtractDumpNumber extracts a dump truck sequence number ("N台目") from text.
// Mirrors extract_dump_number in measurements.rs.
func ExtractDumpNumber(text string) (int, bool) {
	dumpRE := regexp.MustCompile(`(\d+)\s*台目`)
	m := dumpRE.FindStringSubmatch(text)
	if m == nil {
		return 0, false
	}
	n, err := strconv.Atoi(m[1])
	if err != nil {
		return 0, false
	}
	return n, true
}
