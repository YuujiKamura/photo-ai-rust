// Package normalizer — measurements.go
//
// Ports src/normalizer/measurements.rs (Rust) to Go.
// Covers typed measurement extraction, TemperatureType classification,
// temperature validation, and remarks-based temperature extraction.
//
// NOTE: ContainsMeasurement / ExtractTemperature / ExtractDimensionMM /
// IsTemperaturePhoto / ExtractDumpNumber are already in normalizer.go
// (ported by Stream B1). This file adds the remaining symbols prefixed
// Measurement* / MeasurementTemperature* so that there are no collisions.
package normalizer

import (
	"fmt"
	"regexp"
	"strconv"
	"strings"
)

// -----------------------------------------------------------------------
// MeasurementKind — typed enum mirroring Rust MeasurementType
// -----------------------------------------------------------------------

// MeasurementKind identifies the category of a measurement value.
type MeasurementKind int

const (
	MeasurementKindTemperature MeasurementKind = iota // ℃ / 度
	MeasurementKindDimension                          // mm / cm / m (stored in mm)
	MeasurementKindDensity                            // %
)

// MeasurementValue holds one extracted measurement.
type MeasurementValue struct {
	Kind     MeasurementKind
	Value    float64
	Unit     string // original unit string for dimension ("mm","cm","m")
	RawUnit  string // for Temperature: "℃", for Density: "%"
}

var (
	measurementTempRE    = regexp.MustCompile(`(\d+\.?\d*)\s*[℃度]`)
	measurementDimRE     = regexp.MustCompile(`[t=]?\s*(\d+\.?\d*)\s*(mm|cm|m)\b`)
	measurementDensityRE = regexp.MustCompile(`(\d+\.?\d*)\s*%`)
)

// MeasurementExtractAll extracts all typed measurements from text.
// Mirrors extract_measurements in measurements.rs.
func MeasurementExtractAll(text string) []MeasurementValue {
	var out []MeasurementValue

	// Temperature
	for _, m := range measurementTempRE.FindAllStringSubmatch(text, -1) {
		if v, err := strconv.ParseFloat(m[1], 64); err == nil {
			out = append(out, MeasurementValue{
				Kind:    MeasurementKindTemperature,
				Value:   v,
				RawUnit: "℃",
			})
		}
	}

	// Dimension — convert to mm, keep original unit
	for _, m := range measurementDimRE.FindAllStringSubmatch(text, -1) {
		if v, err := strconv.ParseFloat(m[1], 64); err == nil {
			unit := m[2]
			mm := v
			switch unit {
			case "m":
				mm = v * 1000
			case "cm":
				mm = v * 10
			}
			out = append(out, MeasurementValue{
				Kind:  MeasurementKindDimension,
				Value: mm,
				Unit:  unit,
			})
		}
	}

	// Density
	for _, m := range measurementDensityRE.FindAllStringSubmatch(text, -1) {
		if v, err := strconv.ParseFloat(m[1], 64); err == nil {
			out = append(out, MeasurementValue{
				Kind:    MeasurementKindDensity,
				Value:   v,
				RawUnit: "%",
			})
		}
	}

	return out
}

// -----------------------------------------------------------------------
// MeasurementTemperatureType — mirrors TemperatureType in measurements.rs
// -----------------------------------------------------------------------

// MeasurementTemperatureType classifies a temperature photo by work phase.
type MeasurementTemperatureType int

const (
	MeasurementTemperatureArrival           MeasurementTemperatureType = iota // 到着温度
	MeasurementTemperatureSpreading                                            // 敷均し温度
	MeasurementTemperatureInitialCompaction                                    // 初期締固め前温度
	MeasurementTemperatureOpening                                              // 開放温度
	MeasurementTemperatureUnknown                                              // 不明
)

// validRanges mirrors TemperatureType::valid_range, delegating to the same
// canonical ranges used in normalizer.go's ClassifyTemperature.
// Rust source → photo_ai_common::domain::TemperatureKind::valid_range():
//
//	Arrival           140–185 ℃
//	Spreading         130–175 ℃
//	InitialCompaction 120–165 ℃
//	Opening            30– 60 ℃
//	Unknown            30–185 ℃  (fallback)
var measurementTempRanges = map[MeasurementTemperatureType][2]float64{
	MeasurementTemperatureArrival:           {140, 185},
	MeasurementTemperatureSpreading:         {130, 175},
	MeasurementTemperatureInitialCompaction: {120, 165},
	MeasurementTemperatureOpening:           {30, 60},
	MeasurementTemperatureUnknown:           {30, 185},
}

// MeasurementTemperatureTypeFromText classifies text into a temperature phase.
// Mirrors TemperatureType::from_text in measurements.rs.
func MeasurementTemperatureTypeFromText(text string) MeasurementTemperatureType {
	// Exact keyword matching (same order as Rust)
	switch {
	case strings.Contains(text, "到着温度"):
		return MeasurementTemperatureArrival
	case strings.Contains(text, "出荷時"):
		// Rust: broad match for 出荷 → Arrival
		return MeasurementTemperatureArrival
	case strings.Contains(text, "出荷"):
		return MeasurementTemperatureArrival
	case strings.Contains(text, "敷均し") || strings.Contains(text, "敷きならし"):
		return MeasurementTemperatureSpreading
	case strings.Contains(text, "舗設温度") || strings.Contains(text, "舗設"):
		return MeasurementTemperatureSpreading
	case strings.Contains(text, "初期転圧前") || strings.Contains(text, "初期締固め前") || strings.Contains(text, "初期"):
		return MeasurementTemperatureInitialCompaction
	case strings.Contains(text, "開放温度") || strings.Contains(text, "解放温度") || strings.Contains(text, "開放"):
		return MeasurementTemperatureOpening
	default:
		return MeasurementTemperatureUnknown
	}
}

// MeasurementValidRange returns the (min, max) temperature range in ℃.
func MeasurementValidRange(tt MeasurementTemperatureType) (min, max float64) {
	r := measurementTempRanges[tt]
	return r[0], r[1]
}

// MeasurementIsValidTemperature reports whether temp falls within the valid range.
func MeasurementIsValidTemperature(tt MeasurementTemperatureType, temp float64) bool {
	min, max := MeasurementValidRange(tt)
	return temp >= min && temp <= max
}

// -----------------------------------------------------------------------
// MeasurementValidateTemperature — mirrors validate_temperature
// -----------------------------------------------------------------------

// MeasurementValidateTemperature checks whether the temperature extracted from
// tempText is within the valid range for tt. If out-of-range, it attempts a
// decimal-point-correction heuristic (e.g. "126℃" → "12.6℃" for Opening).
// Returns a corrected string on success, empty string when no correction found.
//
// Mirrors validate_temperature in measurements.rs.
func MeasurementValidateTemperature(tempText string, tt MeasurementTemperatureType) string {
	temp, ok := ExtractTemperature(tempText)
	if !ok {
		return ""
	}
	if MeasurementIsValidTemperature(tt, temp) {
		return "" // already valid, no correction needed
	}

	if tt == MeasurementTemperatureOpening {
		if temp >= 100 && temp < 1000 {
			digits := fmt.Sprintf("%d", int(temp))
			if len(digits) == 3 {
				// Try first digit + "." + remaining two (e.g. "126" → "1.26")
				c1 := digits[0:1] + "." + digits[1:]
				if v, err := strconv.ParseFloat(c1, 64); err == nil && MeasurementIsValidTemperature(tt, v) {
					return fmt.Sprintf("%s℃", c1)
				}
				// Try first two digits + "." + last digit (e.g. "126" → "12.6")
				c2 := digits[0:2] + "." + digits[2:]
				if v, err := strconv.ParseFloat(c2, 64); err == nil && MeasurementIsValidTemperature(tt, v) {
					return fmt.Sprintf("%s℃", c2)
				}
			}
		}
	}

	return ""
}

// -----------------------------------------------------------------------
// MeasurementExtractTemperatureForRemarks
// Full port of extract_temperature_for_remarks from measurements.rs
// including remarks-name matching and alternative-name resolution.
// -----------------------------------------------------------------------

// measurementAlternativeNames mirrors alternative_names in measurements.rs.
// Strips trailing "測定" from remarks and returns canonical aliases.
func measurementAlternativeNames(remarks string) []string {
	base := strings.TrimSuffix(remarks, "測定")
	switch base {
	case "初期締固め前温度":
		return []string{"初期転圧前温度", "初期締固め前温度"}
	case "開放温度":
		return []string{"解放温度", "開放温度"}
	case "舗装日外気温":
		return []string{"外気温", "舗装日外気温"}
	case "到着温度":
		return []string{"到着温度"}
	case "敷均し温度":
		return []string{"敷均し温度"}
	default:
		return nil
	}
}

var (
	measurementNumberOnlyRE = regexp.MustCompile(`^\s*(\d+\.?\d*)\s*[℃度]?\s*$`)
	measurementLeadingRE    = regexp.MustCompile(`^\s*(\d+\.?\d*)\s*[℃度]`)
	measurementTrailingRE   = regexp.MustCompile(`(?:,\s*|^)(\d+\.?\d*)\s*[℃度]\s*$`)
)

// MeasurementExtractTemperatureForRemarks extracts the temperature for the given
// remarks label from detectedText (OCR, comma-separated).
//
// Mirrors extract_temperature_for_remarks in measurements.rs.
//
// Return format: "NNN.N℃" or "" when no match found.
//
// Lookup order (same as Rust):
//  1. remarks name (+ aliases) followed by a temperature value before comma/EOL
//  2. Leading temperature value (thermometer close-up)
//  3. Trailing temperature value
//  4. Number-only text (thermometer close-up, no unit)
func MeasurementExtractTemperatureForRemarks(detectedText, remarks string) string {
	if detectedText == "" || remarks == "" {
		return ""
	}

	// 1. Named match: "備考名[^,]*?(\d+\.?\d*)[℃度](?:,|$)"
	names := append([]string{remarks}, measurementAlternativeNames(remarks)...)
	for _, name := range names {
		// RE2 has no lookahead; use simple non-greedy match up to comma/EOL.
		// regexp.QuoteMeta is the Go equivalent of regex::escape.
		pat := regexp.QuoteMeta(name) + `[^,]*?(\d+\.?\d*)\s*[℃度]\s*(?:,|$)`
		re, err := regexp.Compile(pat)
		if err != nil {
			continue
		}
		if m := re.FindStringSubmatch(detectedText); m != nil {
			return m[1] + "℃"
		}
	}

	// 2 + 3. Leading / trailing temperature
	trimmed := strings.TrimSpace(detectedText)
	if m := measurementLeadingRE.FindStringSubmatch(trimmed); m != nil {
		return m[1] + "℃"
	}
	if m := measurementTrailingRE.FindStringSubmatch(trimmed); m != nil {
		return m[1] + "℃"
	}

	// 4. Number only
	if m := measurementNumberOnlyRE.FindStringSubmatch(trimmed); m != nil {
		return m[1] + "℃"
	}

	return ""
}
