// Package matching implements master hierarchy matching for photo classification.
// Ports the bigram-based similarity scoring from the Rust matcher modules.
package matching

import (
	"math"
	"sort"
	"strings"
	"unicode"
)

// MasterEntry represents a single row in a work-type master CSV.
// Mirrors the Rust MasterEntry struct (工種/種別/細別 hierarchy).
type MasterEntry struct {
	WorkType string // 工種
	Variety  string // 種別
	Subphase string // 細別
	Remarks  string // 備考
}

// MatchResult holds the result of matching a photo against the master hierarchy.
type MatchResult struct {
	Entry      MasterEntry
	Confidence float64 // 0.0–1.0
}

// Matcher matches photos against a loaded master hierarchy using bigram similarity.
type Matcher struct {
	entries []MasterEntry
}

// NewMatcher creates a Matcher from a slice of master entries.
func NewMatcher(entries []MasterEntry) *Matcher {
	return &Matcher{entries: entries}
}

// Match returns the best-matching master entry for the given detected text and
// photo category. Uses bigram similarity scoring over the concatenated hierarchy
// fields (工種+種別+細別).
//
// Returns (result, true) if any entry scores above the minimum threshold,
// otherwise returns (zero, false).
func (m *Matcher) Match(detectedText, photoCategory string) (MatchResult, bool) {
	if len(m.entries) == 0 {
		return MatchResult{}, false
	}

	query := normalizeForMatch(detectedText + " " + photoCategory)
	queryBigrams := buildBigrams([]rune(query))

	const minConfidence = 0.05

	best := MatchResult{}
	bestScore := -1.0

	for _, e := range m.entries {
		candidate := normalizeForMatch(e.WorkType + e.Variety + e.Subphase + e.Remarks)
		candidateBigrams := buildBigrams([]rune(candidate))
		score := bigramSimilarity(queryBigrams, candidateBigrams)

		if score > bestScore {
			bestScore = score
			best = MatchResult{Entry: e, Confidence: score}
		}
	}

	if bestScore < minConfidence {
		return MatchResult{}, false
	}
	return best, true
}

// MatchAll returns all entries ranked by similarity score, highest first.
// Only entries scoring above the minimum threshold are included.
func (m *Matcher) MatchAll(detectedText, photoCategory string) []MatchResult {
	if len(m.entries) == 0 {
		return nil
	}

	query := normalizeForMatch(detectedText + " " + photoCategory)
	queryBigrams := buildBigrams([]rune(query))

	const minConfidence = 0.05

	results := make([]MatchResult, 0, len(m.entries))
	for _, e := range m.entries {
		candidate := normalizeForMatch(e.WorkType + e.Variety + e.Subphase + e.Remarks)
		candidateBigrams := buildBigrams([]rune(candidate))
		score := bigramSimilarity(queryBigrams, candidateBigrams)
		if score >= minConfidence {
			results = append(results, MatchResult{Entry: e, Confidence: score})
		}
	}

	sort.Slice(results, func(i, j int) bool {
		return results[i].Confidence > results[j].Confidence
	})
	return results
}

// MatchWorkType filters entries to only those with matching 工種, then scores
// within that subset. This mirrors the two-stage hierarchy matching in the
// Rust master_matcher: first narrow by WorkType, then score 種別/細別.
func (m *Matcher) MatchWorkType(workType, detectedText string) (MatchResult, bool) {
	normalized := normalizeForMatch(workType)
	var subset []MasterEntry
	for _, e := range m.entries {
		if normalizeForMatch(e.WorkType) == normalized {
			subset = append(subset, e)
		}
	}
	if len(subset) == 0 {
		return MatchResult{}, false
	}
	sub := NewMatcher(subset)
	return sub.Match(detectedText, "")
}

// bigramKey encodes a rune pair as a string key for use in a map.
// Using a 2-rune string directly avoids allocations for ASCII; for CJK runes
// this still works correctly since Go maps on string keys by value.
func bigramKey(a, b rune) string {
	return string([]rune{a, b})
}

// buildBigrams returns a set of character bigrams (pairs of consecutive runes)
// for the input rune slice. Mirrors Rust's HashSet<(char, char)> construction.
// The returned map values are counts, enabling Dice-style overlap counting.
func buildBigrams(runes []rune) map[string]int {
	if len(runes) < 2 {
		return map[string]int{}
	}
	bg := make(map[string]int, len(runes)-1)
	for i := 0; i < len(runes)-1; i++ {
		key := bigramKey(runes[i], runes[i+1])
		bg[key]++
	}
	return bg
}

// bigramSimilarity computes the Dice coefficient between two bigram multisets.
// This matches the Rust implementation which intersects two HashSets and divides
// by the geometric mean of their sizes.
//
//	similarity = 2 * |intersection| / (|A| + |B|)
//
// When both sets are empty the function returns 0.
func bigramSimilarity(a, b map[string]int) float64 {
	sizeA := bigramSetSize(a)
	sizeB := bigramSetSize(b)
	if sizeA == 0 && sizeB == 0 {
		return 0.0
	}
	if sizeA == 0 || sizeB == 0 {
		return 0.0
	}

	intersection := 0
	for key, countA := range a {
		if countB, ok := b[key]; ok {
			if countA < countB {
				intersection += countA
			} else {
				intersection += countB
			}
		}
	}

	return 2.0 * float64(intersection) / float64(sizeA+sizeB)
}

// bigramSetSize returns the total count of bigrams in the multiset.
func bigramSetSize(bg map[string]int) int {
	total := 0
	for _, v := range bg {
		total += v
	}
	return total
}

// normalizeForMatch lowercases and strips punctuation/whitespace from a string
// before bigram comparison. Japanese text is kept as-is (no lowercasing effect
// on CJK) but fullwidth ASCII is converted to halfwidth.
func normalizeForMatch(s string) string {
	s = strings.TrimSpace(s)
	var b strings.Builder
	b.Grow(len(s))
	for _, r := range s {
		r = normalizeRune(r)
		if unicode.IsSpace(r) || r == '　' {
			continue
		}
		b.WriteRune(r)
	}
	return b.String()
}

// normalizeRune converts fullwidth ASCII to halfwidth and lowercases ASCII letters.
func normalizeRune(r rune) rune {
	// Fullwidth ASCII: U+FF01–U+FF5E → U+0021–U+007E
	if r >= 0xFF01 && r <= 0xFF5E {
		r = r - 0xFF01 + 0x0021
	}
	// Ideographic space → skip (handled in caller)
	if r == '　' {
		return ' '
	}
	return unicode.ToLower(r)
}

// ScoreHierarchy computes an aggregate confidence score for a candidate entry
// against a query, weighting the 工種 match more heavily than 種別/細別.
// Weights: WorkType=0.5, Variety=0.3, Subphase=0.2.
func ScoreHierarchy(query string, entry MasterEntry) float64 {
	q := []rune(normalizeForMatch(query))
	qBG := buildBigrams(q)

	wtScore := bigramSimilarity(qBG, buildBigrams([]rune(normalizeForMatch(entry.WorkType))))
	vrScore := bigramSimilarity(qBG, buildBigrams([]rune(normalizeForMatch(entry.Variety))))
	spScore := bigramSimilarity(qBG, buildBigrams([]rune(normalizeForMatch(entry.Subphase))))

	weighted := 0.5*wtScore + 0.3*vrScore + 0.2*spScore

	// Boost when the query exactly contains the work type string.
	if entry.WorkType != "" && strings.Contains(normalizeForMatch(query), normalizeForMatch(entry.WorkType)) {
		weighted = math.Min(1.0, weighted+0.15)
	}

	return weighted
}
