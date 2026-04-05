// Package ocr parses OCR text extracted from construction sign boards (黒板).
// Ported from src/ocr_parser.rs
package ocr

import "strings"

// KV is a key-value pair extracted from OCR text.
type KV struct {
	Key   string
	Value string
}

// ParsedBoard holds structured fields extracted from raw OCR text.
type ParsedBoard struct {
	WorkType     string
	Variety      string
	Station      string
	Measurements string
	RawText      string
	KVs          []KV
}

// knownKeys mirrors KNOWN_KEYS in ocr_parser.rs.
var knownKeys = []string{"工事名", "場所", "工種", "測点", "車番", "車両番号"}

// normalizeDetectedText expands literal "\n" and inserts newlines before known keys.
// Mirrors normalize_detected_text in ocr_parser.rs.
func normalizeDetectedText(text string) string {
	// Expand literal "\n" (two chars: backslash + n) to real newline.
	s := strings.ReplaceAll(text, `\n`, "\n")

	// Insert newlines before known key + fullwidth/halfwidth colon patterns.
	for _, key := range knownKeys {
		for _, colon := range []string{"：", ":"} {
			pattern := key + colon
			idx := 0
			for {
				rel := strings.Index(s[idx:], pattern)
				if rel < 0 {
					break
				}
				absPos := idx + rel
				if absPos > 0 && s[absPos-1] != '\n' {
					s = s[:absPos] + "\n" + s[absPos:]
					idx = absPos + 1 + len(pattern)
				} else {
					idx = absPos + len(pattern)
				}
			}
		}
	}
	return s
}

// ExtractKV extracts key-value pairs from OCR detected text.
// Mirrors extract_kv_from_text in ocr_parser.rs.
func ExtractKV(text string) []KV {
	var result []KV

	normalized := normalizeDetectedText(text)

	// Split on newlines, then expand comma-separated segments.
	var lines []string
	for _, segment := range strings.Split(normalized, "\n") {
		segment = strings.TrimSpace(segment)
		if segment == "" {
			continue
		}
		if strings.Contains(segment, ", ") {
			for _, part := range strings.Split(segment, ", ") {
				p := strings.TrimSpace(part)
				if p != "" {
					lines = append(lines, p)
				}
			}
		} else {
			lines = append(lines, segment)
		}
	}

	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}

		// Try fullwidth colon then halfwidth colon split.
		k, v, found := splitOnColon(line)
		if found && k != "" {
			v = strings.TrimSpace(strings.Trim(v, ","))
			// For 場所/測点: split off trailing Japanese keywords.
			if k == "場所" || k == "測点" {
				tokens := strings.Fields(v)
				var stationParts []string
				foundJP := false
				for _, t := range tokens {
					if !foundJP && !containsWideChar(t) {
						stationParts = append(stationParts, t)
					} else {
						foundJP = true
						result = append(result, KV{Key: "", Value: t})
					}
				}
				result = append(result, KV{Key: k, Value: strings.Join(stationParts, " ")})
			} else {
				result = append(result, KV{Key: k, Value: v})
			}
			continue
		}

		// Try known-key space split ("場所 No. 4 L" etc.).
		matched := false
		for _, key := range knownKeys {
			if strings.HasPrefix(line, key) && len(line) > len(key) {
				after := []rune(line[len(key):])
				if len(after) > 0 && (after[0] == ' ' || after[0] == '\u3000') {
					rest := strings.TrimLeft(line[len(key):], " \u3000")
					result = append(result, KV{Key: key, Value: rest})
					matched = true
					break
				}
			}
		}
		if !matched {
			// No key matched — treat as keyword value.
			result = append(result, KV{Key: "", Value: line})
		}
	}
	return result
}

// splitOnColon splits on fullwidth "：" first, then halfwidth ":".
func splitOnColon(s string) (k, v string, found bool) {
	if i := strings.Index(s, "："); i >= 0 {
		return s[:i], s[i+len("："):], true
	}
	if i := strings.Index(s, ":"); i >= 0 {
		return s[:i], s[i+1:], true
	}
	return "", "", false
}

// containsWideChar reports whether s contains any character above U+3000 (CJK range).
func containsWideChar(s string) bool {
	for _, r := range s {
		if r > '\u3000' {
			return true
		}
	}
	return false
}

// NormalizeStation normalises a station string.
// Mirrors normalize_station in ocr_parser.rs.
func NormalizeStation(s string) string {
	if s == "" {
		return ""
	}
	// "2/11" → "2月11日" date format normalisation.
	if i := strings.Index(s, "/"); i >= 0 {
		m := strings.TrimSpace(s[:i])
		d := strings.TrimSpace(s[i+1:])
		if isAllDigits(m) && isAllDigits(d) {
			mm := atoi(m)
			dd := atoi(d)
			if mm >= 1 && mm <= 12 && dd >= 1 && dd <= 31 {
				return itoa(mm) + "月" + itoa(dd) + "日"
			}
		}
	}
	// Collapse ". " → "." (e.g. "No. 4" → "No.4").
	for strings.Contains(s, ". ") {
		s = strings.ReplaceAll(s, ". ", ".")
	}
	// Trailing " L" → " 左車線", " R" → " 右車線".
	if strings.HasSuffix(s, " L") {
		s = s[:len(s)-2] + " 左車線"
	} else if strings.HasSuffix(s, " R") {
		s = s[:len(s)-2] + " 右車線"
	}
	return s
}

func isAllDigits(s string) bool {
	if s == "" {
		return false
	}
	for _, r := range s {
		if r < '0' || r > '9' {
			return false
		}
	}
	return true
}

func atoi(s string) int {
	n := 0
	for _, r := range s {
		n = n*10 + int(r-'0')
	}
	return n
}

func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	digits := []byte{}
	for n > 0 {
		digits = append([]byte{byte('0' + n%10)}, digits...)
		n /= 10
	}
	return string(digits)
}

// Parse extracts structured data from the raw OCR text of a sign board.
func Parse(rawText string) ParsedBoard {
	kvs := ExtractKV(rawText)
	board := ParsedBoard{
		RawText: rawText,
		KVs:     kvs,
	}
	for _, kv := range kvs {
		switch kv.Key {
		case "工種":
			board.WorkType = kv.Value
		case "場所", "測点":
			if board.Station == "" {
				board.Station = NormalizeStation(kv.Value)
			}
		}
	}
	return board
}
