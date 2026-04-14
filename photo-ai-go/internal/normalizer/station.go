// Package normalizer — station (測点) domain types.
// Ported from common/src/domain/station.rs (issue #165 Stream B1).
//
// Station represents the four semantic cases a station string can take in
// construction-photo metadata.  All public API names use the prefix
// "Station*" to avoid collision with other streams in this package.
package normalizer

import (
	"fmt"
	"strconv"
	"strings"
)

// StationKind identifies which variant a parsed Station holds.
type StationKind int

const (
	StationKindEmpty StationKind = iota
	StationKindPost
	StationKindDate
	StationKindOther
)

// Lane represents the road carriageway direction.
// Mirrors the Lane enum in common/src/domain/station.rs.
type Lane int

const (
	LaneNone  Lane = iota // no lane specified
	LaneLeft              // L / 左
	LaneRight             // R / 右
)

// LaneFromStr parses a lane designator.
// Mirrors Lane::from_str.
func LaneFromStr(s string) (Lane, bool) {
	switch s {
	case "L", "l", "左":
		return LaneLeft, true
	case "R", "r", "右":
		return LaneRight, true
	}
	return LaneNone, false
}

// LaneString returns the canonical single-character representation.
func (l Lane) LaneString() string {
	switch l {
	case LaneLeft:
		return "L"
	case LaneRight:
		return "R"
	default:
		return ""
	}
}

// PostStation holds a road-post station ("No.N [L|R]").
// Mirrors PostStation in common/src/domain/station.rs.
type PostStation struct {
	Label      string // e.g. "No.6"
	Lane       Lane
	AccessRoad bool // true for 取付道路
}

// DateStation holds a date-based station ("X月Y日 [N台目]").
// Mirrors DateStation in common/src/domain/station.rs.
type DateStation struct {
	Month      uint8
	Day        uint8
	DumpNumber uint8 // 0 means not set
}

// Station is the result of parsing a station string.
// Mirrors the Station enum in common/src/domain/station.rs.
type Station struct {
	kind StationKind
	post *PostStation
	date *DateStation
	text string // StationKindOther
}

// Kind returns the discriminant.
func (s Station) Kind() StationKind { return s.kind }

// IsEmpty mirrors Station::is_empty.
func (s Station) IsEmpty() bool { return s.kind == StationKindEmpty }

// IsDate mirrors Station::is_date.
func (s Station) IsDate() bool { return s.kind == StationKindDate }

// IsPost mirrors Station::is_post.
func (s Station) IsPost() bool { return s.kind == StationKindPost }

// HasDumpNumber mirrors Station::has_dump_number.
func (s Station) HasDumpNumber() bool {
	return s.kind == StationKindDate && s.date != nil && s.date.DumpNumber != 0
}

// DumpNumber returns the dump number, or 0 if absent.
// Mirrors Station::dump_number.
func (s Station) DumpNumber() uint8 {
	if s.kind == StationKindDate && s.date != nil {
		return s.date.DumpNumber
	}
	return 0
}

// Post returns the PostStation, or nil.
func (s Station) Post() *PostStation { return s.post }

// Date returns the DateStation, or nil.
func (s Station) Date() *DateStation { return s.date }

// ToDisplay converts the parsed station back to the canonical display string.
// Mirrors Station::to_display in common/src/domain/station.rs.
func (s Station) ToDisplay() string {
	switch s.kind {
	case StationKindEmpty:
		return ""
	case StationKindPost:
		p := s.post
		var b strings.Builder
		if p.AccessRoad {
			b.WriteString("取付道路 ")
		}
		b.WriteString(p.Label)
		if p.Lane != LaneNone {
			b.WriteByte(' ')
			b.WriteString(p.Lane.LaneString())
		}
		return b.String()
	case StationKindDate:
		d := s.date
		base := fmt.Sprintf("%d月%d日", d.Month, d.Day)
		if d.DumpNumber != 0 {
			return fmt.Sprintf("%s %d台目", base, d.DumpNumber)
		}
		return base
	case StationKindOther:
		return s.text
	}
	return ""
}

// WithDumpNumber returns a copy of s with the dump number set to n.
// Only affects Date stations; other variants are returned unchanged.
// Mirrors Station::with_dump_number in common/src/domain/station.rs.
func (s Station) WithDumpNumber(n uint8) Station {
	if s.kind != StationKindDate || s.date == nil {
		return s
	}
	d := *s.date
	d.DumpNumber = n
	return Station{kind: StationKindDate, date: &d}
}

// StationParse parses a raw station string into a Station.
// Mirrors Station::parse in common/src/domain/station.rs.
//
// Parsing order (identical to Rust):
//  1. Empty / dash → StationKindEmpty
//  2. 取付道路 prefix → StationKindPost (access_road=true)
//  3. Date (X月Y日 [N台目]) → StationKindDate
//  4. No.N [lane] → StationKindPost
//  5. Anything else → StationKindOther
func StationParse(s string) Station {
	trimmed := strings.TrimSpace(s)
	if trimmed == "" || trimmed == "-" {
		return Station{kind: StationKindEmpty}
	}

	// 取付道路
	if rest, ok := strings.CutPrefix(trimmed, "取付道路"); ok {
		rest = strings.TrimLeft(rest, " \t　")
		if label, ok := extractPostLabel(rest); ok {
			return Station{
				kind: StationKindPost,
				post: &PostStation{Label: label, AccessRoad: true},
			}
		}
	}

	// Date (X月Y日 [N台目])
	if ds, ok := parseDateStation(trimmed); ok {
		return Station{kind: StationKindDate, date: ds}
	}

	// No.N [lane]
	if label, lane, ok := parsePost(trimmed); ok {
		return Station{
			kind: StationKindPost,
			post: &PostStation{Label: label, Lane: lane},
		}
	}

	return Station{kind: StationKindOther, text: trimmed}
}

// extractPostLabel accepts a string that should start with "No.N" (case-insensitive)
// and returns the normalised label "No.<digits>".
// Mirrors extract_post_label in common/src/domain/station.rs.
func extractPostLabel(s string) (string, bool) {
	// Skip leading non-alpha (e.g. spaces)
	s = strings.TrimLeft(s, " \t　")
	if len(s) < 3 {
		return "", false
	}
	prefix := s[:3]
	if !strings.EqualFold(prefix, "No.") {
		return "", false
	}
	tail := s[3:]
	digits := leadingDigits(tail)
	if digits == "" {
		return "", false
	}
	return "No." + digits, true
}

// parsePost mirrors parse_post in common/src/domain/station.rs.
// Returns (label, lane, ok).
func parsePost(s string) (string, Lane, bool) {
	s = strings.TrimSpace(s)
	if len(s) < 3 {
		return "", LaneNone, false
	}
	prefix := s[:3]
	if !strings.EqualFold(prefix, "No.") {
		return "", LaneNone, false
	}
	rest := s[3:]
	digits := leadingDigits(rest)
	if digits == "" {
		return "", LaneNone, false
	}
	afterDigits := strings.TrimSpace(rest[len(digits):])

	// Optional lane + optional "車線" suffix
	lanePart, trailer, hasCut := strings.Cut(afterDigits, "車線")
	if hasCut {
		if strings.TrimSpace(trailer) != "" {
			// unexpected trailing content
			return "", LaneNone, false
		}
		afterDigits = lanePart
	}
	afterDigits = strings.TrimSpace(afterDigits)

	var lane Lane
	if afterDigits == "" {
		lane = LaneNone
	} else {
		var ok bool
		lane, ok = LaneFromStr(afterDigits)
		if !ok {
			return "", LaneNone, false
		}
	}

	return "No." + digits, lane, true
}

// parseDateStation mirrors parse_date_station in common/src/domain/station.rs.
func parseDateStation(s string) (*DateStation, bool) {
	monthStr, rest, ok := strings.Cut(s, "月")
	if !ok {
		return nil, false
	}
	month64, err := strconv.ParseUint(strings.TrimSpace(monthStr), 10, 8)
	if err != nil || month64 < 1 || month64 > 12 {
		return nil, false
	}

	dayStr, tail, ok := strings.Cut(rest, "日")
	if !ok {
		return nil, false
	}
	day64, err := strconv.ParseUint(strings.TrimSpace(dayStr), 10, 8)
	if err != nil || day64 < 1 || day64 > 31 {
		return nil, false
	}

	tail = strings.TrimSpace(tail)
	var dumpNumber uint8
	if tail != "" {
		numStr, suffix, hasCut := strings.Cut(tail, "台目")
		if !hasCut || strings.TrimSpace(suffix) != "" {
			return nil, false
		}
		n, err := strconv.ParseUint(strings.TrimSpace(numStr), 10, 8)
		if err != nil || n == 0 {
			return nil, false
		}
		dumpNumber = uint8(n)
	}

	return &DateStation{
		Month:      uint8(month64),
		Day:        uint8(day64),
		DumpNumber: dumpNumber,
	}, true
}

// leadingDigits returns the leading ASCII digit run of s.
func leadingDigits(s string) string {
	i := 0
	for i < len(s) && s[i] >= '0' && s[i] <= '9' {
		i++
	}
	return s[:i]
}
