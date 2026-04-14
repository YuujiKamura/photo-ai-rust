// Package analyzer implements work-type automatic detection from Step1 image data.
//
// Ports common/src/analyzer.rs (detect_work_types, detect_work_types_with)
// and common/src/work_type_defs.rs (DEFAULT_WORK_TYPE_DEFINITIONS) from Rust.
//
// Issue #165 – Stream G.
package analyzer

import (
	"errors"
	"fmt"
	"sort"
	"strings"

	"github.com/YuujiKamura/photo-ai-go/internal/ai"
)

// AnalyzerError variants mirror the Rust AnalyzerError enum.
var (
	// ErrTaggerEmpty is returned when the photo-tagger produced no results.
	// Mirrors: AnalyzerError::TaggerEmpty
	ErrTaggerEmpty = errors.New("Photo tagger returned no results")
)

// ResponseParseError mirrors AnalyzerError::ResponseParse(String).
// Use fmt.Errorf("AI response parse error: %s%w", detail, ResponseParseError)
// to create wrapped instances, or use NewResponseParseError.
type ResponseParseError struct {
	Detail string
}

func (e *ResponseParseError) Error() string {
	return fmt.Sprintf("AI response parse error: %s", e.Detail)
}

// NewResponseParseError creates a ResponseParseError, mirroring
// AnalyzerError::ResponseParse(detail).
func NewResponseParseError(detail string) error {
	return &ResponseParseError{Detail: detail}
}

// WorkTypeDefinition holds keyword sets for a single work type.
// Mirrors common/src/types.rs WorkTypeDefinition (with owned strings for Go).
//
// NOTE: Rust uses &'static str slices; Go uses []string. Semantics identical.
type WorkTypeDefinition struct {
	// Name is the work-type label (e.g. "舗装工").
	Name string
	// CategoryKeywords match against RawImageData.PhotoCategory (case-insensitive).
	CategoryKeywords []string
	// TextKeywords match against RawImageData.DetectedText (case-insensitive).
	TextKeywords []string
	// SceneKeywords match against RawImageData.SceneDescription (case-insensitive).
	SceneKeywords []string
}

// DefaultWorkTypeDefinitions is the Go equivalent of DEFAULT_WORK_TYPE_DEFINITIONS
// from common/src/work_type_defs.rs. Values are reproduced verbatim.
var DefaultWorkTypeDefinitions = []WorkTypeDefinition{
	{
		Name:             "舗装工",
		CategoryKeywords: []string{"温度", "転圧", "舗設", "敷均し", "乳剤", "路盤"},
		TextKeywords:     []string{"アスファルト"},
		SceneKeywords:    []string{"アスファルト", "フィニッシャー", "ローラー"},
	},
	{
		Name:             "区画線工",
		CategoryKeywords: []string{"区画線"},
		TextKeywords:     []string{"区画線", "ライン"},
		SceneKeywords:    []string{"白線", "区画線"},
	},
	{
		Name:             "構造物撤去工",
		CategoryKeywords: []string{"取壊し"},
		TextKeywords:     []string{"撤去", "取壊"},
		SceneKeywords:    []string{"解体", "撤去"},
	},
	{
		Name:             "道路土工",
		CategoryKeywords: []string{"掘削", "路床"},
		TextKeywords:     []string{"掘削"},
		SceneKeywords:    []string{"掘削", "バックホウ"},
	},
	{
		Name:             "排水構造物工",
		CategoryKeywords: []string{},
		TextKeywords:     []string{"側溝", "集水", "人孔"},
		SceneKeywords:    []string{"側溝", "マンホール"},
	},
	{
		Name:             "人孔改良工",
		CategoryKeywords: []string{},
		TextKeywords:     []string{"人孔改良", "マンホール蓋"},
		SceneKeywords:    []string{},
	},
}

// DetectWorkTypes detects work types from Step1 image data using the default
// keyword definitions.
//
// Ports: pub fn detect_work_types(raw_data: &[RawImageData]) -> Vec<String>
//
// Input uses ai.RawImageData (internal/ai package) which mirrors the Rust
// RawImageData struct field-for-field.
//
// The returned slice is sorted for deterministic output (Rust returns
// HashSet::into_iter which is non-deterministic; we sort to compensate).
func DetectWorkTypes(rawData []ai.RawImageData) []string {
	return DetectWorkTypesWith(rawData, DefaultWorkTypeDefinitions)
}

// DetectWorkTypesWith detects work types using a caller-supplied definition list.
//
// Ports: pub fn detect_work_types_with(raw_data: &[RawImageData], definitions: &[WorkTypeDefinition]) -> Vec<String>
//
// Behavior is identical to the Rust version:
//   - Each RawImageData field is lowercased before matching.
//   - A definition matches if ANY keyword in ANY of its three keyword slices
//     appears as a substring of the corresponding (lowercased) field.
//   - Matched work-type names are deduplicated via a set (map[string]struct{}).
//
// The returned slice is sorted lexicographically for deterministic ordering.
// (Rust uses HashSet whose iteration order is undefined; sorting makes tests stable.)
func DetectWorkTypesWith(rawData []ai.RawImageData, definitions []WorkTypeDefinition) []string {
	seen := make(map[string]struct{})

	for _, r := range rawData {
		cat := strings.ToLower(r.PhotoCategory)
		text := strings.ToLower(r.DetectedText)
		scene := strings.ToLower(r.SceneDescription)

		for _, def := range definitions {
			matched := containsAny(cat, def.CategoryKeywords) ||
				containsAny(text, def.TextKeywords) ||
				containsAny(scene, def.SceneKeywords)
			if matched {
				seen[def.Name] = struct{}{}
			}
		}
	}

	result := make([]string, 0, len(seen))
	for name := range seen {
		result = append(result, name)
	}
	sort.Strings(result)
	return result
}

// containsAny reports whether s contains any of the given keywords as a
// substring (case-sensitive; callers must pre-lowercase both sides).
// Mirrors: def.category_keywords.iter().any(|kw| cat.contains(&kw.to_lowercase()))
func containsAny(s string, keywords []string) bool {
	for _, kw := range keywords {
		if strings.Contains(s, strings.ToLower(kw)) {
			return true
		}
	}
	return false
}
