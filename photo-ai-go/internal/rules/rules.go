// Package rules parses folder-rule JSON files that describe per-folder
// classification overrides. Ported from src/folder_rules.rs
package rules

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"runtime"
	"strings"
)

// AnalysisResult mirrors the fields used by the rule engine.
type AnalysisResult struct {
	FileName      string
	FilePath      string
	PhotoCategory string
	WorkType      string
	Variety       string
	Subphase      string
	Remarks       string
	Station       string
	Measurements  string
	FocusTarget   string
	DetectedText  string
	Group         uint32
}

// FieldOverrides holds optional override values for each field.
type FieldOverrides struct {
	PhotoCategory *string `json:"photo_category,omitempty"`
	WorkType      *string `json:"work_type,omitempty"`
	Variety       *string `json:"variety,omitempty"`
	Subphase      *string `json:"subphase,omitempty"`
	Remarks       *string `json:"remarks,omitempty"`
	Station       *string `json:"station,omitempty"`
	Measurements  *string `json:"measurements,omitempty"`
	FocusTarget   *string `json:"focus_target,omitempty"`
}

// PerFileCondition describes matching criteria for a per-file rule.
type PerFileCondition struct {
	DetectedTextContains *string `json:"detected_text_contains,omitempty"`
	DetectedTextEmpty    *bool   `json:"detected_text_empty,omitempty"`
	StationEmpty         *bool   `json:"station_empty,omitempty"`
	RemarksEq            *string `json:"remarks_eq,omitempty"`
	Default              *bool   `json:"default,omitempty"`
}

// PerFileRule applies overrides when a condition matches.
type PerFileRule struct {
	Condition   PerFileCondition `json:"condition"`
	Apply       FieldOverrides   `json:"apply,omitempty"`
	ClearFields []string         `json:"clear_fields,omitempty"`
}

// FocusTargetMerge merges multiple focus_target values into one.
type FocusTargetMerge struct {
	From []string `json:"from"`
	To   string   `json:"to"`
}

// RemarksVarietyOverride overrides work_type and variety for specific remarks.
type RemarksVarietyOverride struct {
	RemarksList []string `json:"remarks_list"`
	WorkType    string   `json:"work_type"`
	Variety     string   `json:"variety"`
}

// TackcoatCounter assigns focus_target values sequentially for tackcoat photos.
type TackcoatCounter struct {
	Remarks          string `json:"remarks"`
	FirstFocusTarget string `json:"first_focus_target"`
	RestFocusTarget  string `json:"rest_focus_target"`
}

// FolderRule is a single rule loaded from folder_rules.json.
type FolderRule struct {
	ID                     string                       `json:"id"`
	MatchAll               []string                     `json:"match_all"`
	Apply                  *FieldOverrides              `json:"apply,omitempty"`
	PerFile                []PerFileRule                `json:"per_file,omitempty"`
	StationReplacements    map[string]string            `json:"station_replacements,omitempty"`
	FocusTargetRenames     map[string]string            `json:"focus_target_renames,omitempty"`
	FocusTargetMerge       *FocusTargetMerge            `json:"focus_target_merge,omitempty"`
	RemarksVarietyOverride *RemarksVarietyOverride      `json:"remarks_variety_override,omitempty"`
	TackcoatCounter        *TackcoatCounter             `json:"tackcoat_counter,omitempty"`
	IndexFocusTarget       map[string]string            `json:"index_focus_target,omitempty"`
	DekigataStationExtract bool                         `json:"dekigata_station_extraction,omitempty"`
	StationPropagation     bool                         `json:"station_propagation,omitempty"`
	ClearSubphaseFor       []string                     `json:"clear_subphase_for,omitempty"`
	DefaultSubphase        *string                      `json:"default_subphase,omitempty"`
	MeasurementMaps        map[string]map[string]string `json:"measurement_maps,omitempty"`
	StationRenames         map[string]map[string]string `json:"station_renames,omitempty"`
	FileOverrides          map[string]FieldOverrides    `json:"file_overrides,omitempty"`
	ClearFields            []string                     `json:"clear_fields,omitempty"`
}

// LoadFolderRules reads and parses a folder_rules.json file.
func LoadFolderRules(path string) ([]FolderRule, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("read folder rules %q: %w", path, err)
	}
	var rules []FolderRule
	if err := json.Unmarshal(data, &rules); err != nil {
		return nil, fmt.Errorf("parse folder rules %q: %w", path, err)
	}
	return rules, nil
}

// ResolveRulesPath finds folder_rules.json relative to the executable or source.
func ResolveRulesPath(cliPath string) string {
	if cliPath != "" {
		if _, err := os.Stat(cliPath); err == nil {
			return cliPath
		}
	}
	if exe, err := os.Executable(); err == nil {
		candidate := filepath.Join(filepath.Dir(exe), "config", "folder_rules.json")
		if _, err := os.Stat(candidate); err == nil {
			return candidate
		}
	}
	_, thisFile, _, ok := runtime.Caller(0)
	if ok {
		candidate := filepath.Join(filepath.Dir(thisFile), "..", "..", "config", "folder_rules.json")
		if _, err := os.Stat(candidate); err == nil {
			return candidate
		}
	}
	return ""
}

func matchesRule(folderName string, rule *FolderRule) bool {
	for _, s := range rule.MatchAll {
		if !strings.Contains(folderName, s) {
			return false
		}
	}
	return true
}

func applyOverrides(r *AnalysisResult, ov *FieldOverrides) {
	if ov == nil {
		return
	}
	if ov.PhotoCategory != nil {
		r.PhotoCategory = *ov.PhotoCategory
	}
	if ov.WorkType != nil {
		r.WorkType = *ov.WorkType
	}
	if ov.Variety != nil {
		r.Variety = *ov.Variety
	}
	if ov.Subphase != nil {
		r.Subphase = *ov.Subphase
	}
	if ov.Remarks != nil {
		r.Remarks = *ov.Remarks
	}
	if ov.Station != nil {
		r.Station = *ov.Station
	}
	if ov.Measurements != nil {
		r.Measurements = *ov.Measurements
	}
	if ov.FocusTarget != nil {
		r.FocusTarget = *ov.FocusTarget
	}
}

func clearField(r *AnalysisResult, field string) {
	switch field {
	case "photo_category":
		r.PhotoCategory = ""
	case "work_type":
		r.WorkType = ""
	case "variety":
		r.Variety = ""
	case "subphase":
		r.Subphase = ""
	case "remarks":
		r.Remarks = ""
	case "station":
		r.Station = ""
	case "measurements":
		r.Measurements = ""
	case "focus_target":
		r.FocusTarget = ""
	}
}

func matchesCondition(r *AnalysisResult, cond *PerFileCondition) bool {
	if cond.Default != nil && *cond.Default {
		return true
	}
	hasAny := false
	if cond.DetectedTextContains != nil {
		hasAny = true
		if !strings.Contains(r.DetectedText, *cond.DetectedTextContains) {
			return false
		}
	}
	if cond.DetectedTextEmpty != nil && *cond.DetectedTextEmpty {
		hasAny = true
		if strings.TrimSpace(r.DetectedText) != "" {
			return false
		}
	}
	if cond.StationEmpty != nil && *cond.StationEmpty {
		hasAny = true
		if r.Station != "" {
			return false
		}
	}
	if cond.RemarksEq != nil {
		hasAny = true
		if r.Remarks != *cond.RemarksEq {
			return false
		}
	}
	return hasAny
}

var dekigataMarkers = []string{"出来形管理用紙", "出来形管理用具"}
var noRE = regexp.MustCompile(`No\.(\d+)`)

// ExtractDekigataStation extracts a station label from OCR text.
func ExtractDekigataStation(text string) string {
	for _, marker := range dekigataMarkers {
		idx := strings.Index(text, marker)
		if idx < 0 {
			continue
		}
		after := strings.TrimLeft(text[idx+len(marker):], " \u3000,")
		after = strings.TrimSpace(after)
		for _, prefix := range []string{"No.", "NO.", "no."} {
			if strings.HasPrefix(after, prefix) {
				rest := after[3:]
				i := 0
				for i < len(rest) && rest[i] >= '0' && rest[i] <= '9' {
					i++
				}
				if i > 0 {
					return after[:3+i]
				}
			}
		}
	}
	if m := noRE.FindStringSubmatchIndex(text); m != nil {
		numStart := m[2]
		numEnd := m[3]
		if strings.Contains(text[:m[0]], "取付道路") {
			return "取付道路 No." + text[numStart:numEnd]
		}
		return "No." + text[numStart:numEnd]
	}
	return ""
}

func propagateStations(results []AnalysisResult) {
	groups := make(map[uint32][]int)
	var ungrouped []int
	for i, r := range results {
		if r.Group > 0 {
			groups[r.Group] = append(groups[r.Group], i)
		} else {
			ungrouped = append(ungrouped, i)
		}
	}
	for _, indices := range groups {
		var st string
		for _, idx := range indices {
			if results[idx].Station != "" {
				st = results[idx].Station
				break
			}
		}
		if st != "" {
			for _, idx := range indices {
				if results[idx].Station == "" {
					results[idx].Station = st
				}
			}
		}
	}
	for wi := 1; wi < len(ungrouped); wi++ {
		i := ungrouped[wi]
		prev := ungrouped[wi-1]
		if results[i].Station == "" && results[prev].Station != "" {
			results[i].Station = results[prev].Station
		}
	}
	for wi := len(ungrouped) - 2; wi >= 0; wi-- {
		i := ungrouped[wi]
		next := ungrouped[wi+1]
		if results[i].Station == "" && results[next].Station != "" {
			results[i].Station = results[next].Station
		}
	}
}

func applySingleRule(results []AnalysisResult, folderName string, rule *FolderRule) []AnalysisResult {
	// 1. Global overrides
	if rule.Apply != nil {
		for i := range results {
			applyOverrides(&results[i], rule.Apply)
		}
	}
	// 2. Clear fields
	for _, field := range rule.ClearFields {
		for i := range results {
			clearField(&results[i], field)
		}
	}
	// 3. Station replacements
	for old, newVal := range rule.StationReplacements {
		for i := range results {
			if strings.Contains(results[i].Station, old) {
				results[i].Station = strings.ReplaceAll(results[i].Station, old, newVal)
			}
		}
	}
	// 4. Focus target renames
	for i := range results {
		if newName, ok := rule.FocusTargetRenames[results[i].FocusTarget]; ok {
			results[i].FocusTarget = newName
		}
	}
	// 5. Focus target merge
	if rule.FocusTargetMerge != nil {
		fromSet := make(map[string]bool, len(rule.FocusTargetMerge.From))
		for _, f := range rule.FocusTargetMerge.From {
			fromSet[f] = true
		}
		for i := range results {
			if fromSet[results[i].FocusTarget] {
				results[i].FocusTarget = rule.FocusTargetMerge.To
			}
		}
	}
	// 6. Per-file rules (first match wins)
	for i := range results {
		for _, pf := range rule.PerFile {
			if matchesCondition(&results[i], &pf.Condition) {
				applyOverrides(&results[i], &pf.Apply)
				for _, field := range pf.ClearFields {
					clearField(&results[i], field)
				}
				break
			}
		}
	}
	// 7. Remarks variety override
	if rule.RemarksVarietyOverride != nil {
		rvo := rule.RemarksVarietyOverride
		remarksSet := make(map[string]bool, len(rvo.RemarksList))
		for _, r := range rvo.RemarksList {
			remarksSet[r] = true
		}
		for i := range results {
			if remarksSet[results[i].Remarks] {
				results[i].WorkType = rvo.WorkType
				results[i].Variety = rvo.Variety
			}
		}
	}
	// 8. Tackcoat counter
	if rule.TackcoatCounter != nil {
		tc := rule.TackcoatCounter
		seen := 0
		for i := range results {
			if results[i].Remarks == tc.Remarks {
				seen++
				if seen == 1 {
					results[i].FocusTarget = tc.FirstFocusTarget
				} else {
					results[i].FocusTarget = tc.RestFocusTarget
				}
			}
		}
	}
	// 9. Index-based focus_target
	if rule.IndexFocusTarget != nil {
		defaultFT := rule.IndexFocusTarget["default"]
		for i := range results {
			key := fmt.Sprintf("%d", i)
			if ft, ok := rule.IndexFocusTarget[key]; ok {
				results[i].FocusTarget = ft
			} else if defaultFT != "" {
				results[i].FocusTarget = defaultFT
			}
		}
	}
	// 10. Dekigata station extraction
	if rule.DekigataStationExtract {
		for i := range results {
			if results[i].Station == "" {
				results[i].Station = ExtractDekigataStation(results[i].DetectedText)
			}
		}
	}
	// 11. Clear subphase / default subphase
	if len(rule.ClearSubphaseFor) > 0 || rule.DefaultSubphase != nil {
		shouldClear := false
		for _, pat := range rule.ClearSubphaseFor {
			if strings.Contains(folderName, pat) {
				shouldClear = true
				break
			}
		}
		for i := range results {
			if shouldClear {
				results[i].Subphase = ""
			} else if rule.DefaultSubphase != nil {
				results[i].Subphase = *rule.DefaultSubphase
			}
		}
	}
	// 12. Station propagation
	if rule.StationPropagation {
		propagateStations(results)
	}
	// 13. Station renames
	for pattern, renameMap := range rule.StationRenames {
		if strings.Contains(folderName, pattern) {
			for i := range results {
				if newSt, ok := renameMap[results[i].Station]; ok {
					results[i].Station = newSt
				}
			}
		}
	}
	// 14. Measurement maps
	for pattern, mMap := range rule.MeasurementMaps {
		if strings.Contains(folderName, pattern) {
			for i := range results {
				if m, ok := mMap[results[i].Station]; ok {
					results[i].Measurements = m
				}
			}
		}
	}
	// 15. File overrides
	for fileName, ov := range rule.FileOverrides {
		for i := range results {
			if results[i].FileName == fileName {
				ovCopy := ov
				applyOverrides(&results[i], &ovCopy)
			}
		}
	}
	return results
}

// ApplyFolderRules applies all matching rules to results for the given folder.
func ApplyFolderRules(results []AnalysisResult, folderName string, rules []FolderRule) []AnalysisResult {
	for i := range rules {
		if matchesRule(folderName, &rules[i]) {
			results = applySingleRule(results, folderName, &rules[i])
		}
	}
	return results
}

// RuleSet is kept for backward compatibility with the placeholder API.
type RuleSet struct {
	Rules []FolderRule
}

// LoadRuleSet reads and parses a folder_rules.json file into a RuleSet.
func LoadRuleSet(path string) (RuleSet, error) {
	rules, err := LoadFolderRules(path)
	if err != nil {
		return RuleSet{}, err
	}
	return RuleSet{Rules: rules}, nil
}

// Match returns the first rule whose MatchAll strings all appear in folderPath.
func (rs *RuleSet) Match(folderPath string) (FolderRule, bool) {
	for _, r := range rs.Rules {
		if matchesRule(folderPath, &r) {
			return r, true
		}
	}
	return FolderRule{}, false
}
