package rules

import (
	"testing"
)

func strPtr(s string) *string { return &s }
func boolPtr(b bool) *bool   { return &b }

func TestExtractDekigataStation(t *testing.T) {
	tests := []struct {
		name string
		text string
		want string
	}{
		{"dekigata paper No.4", "出来形管理用紙 No.4 測定", "No.4"},
		{"dekigata tool No.12", "出来形管理用具 No.12", "No.12"},
		{"tsuketsudouro No.3", "取付道路 No.3 付近", "取付道路 No.3"},
		{"unrelated text", "関係ないテキスト", ""},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := ExtractDekigataStation(tt.text)
			if got != tt.want {
				t.Errorf("ExtractDekigataStation(%q) = %q, want %q", tt.text, got, tt.want)
			}
		})
	}
}

func TestApplyFolderRules_Match(t *testing.T) {
	rule := FolderRule{
		ID:       "test",
		MatchAll: []string{"舗装"},
		Apply:    &FieldOverrides{PhotoCategory: strPtr("施工状況写真")},
	}
	rules := []FolderRule{rule}

	t.Run("matching folder", func(t *testing.T) {
		results := []AnalysisResult{{FileName: "a.jpg"}}
		results = ApplyFolderRules(results, "舗装工事", rules)
		if results[0].PhotoCategory != "施工状況写真" {
			t.Errorf("PhotoCategory = %q, want %q", results[0].PhotoCategory, "施工状況写真")
		}
	})

	t.Run("non-matching folder", func(t *testing.T) {
		results := []AnalysisResult{{FileName: "a.jpg"}}
		results = ApplyFolderRules(results, "土工事", rules)
		if results[0].PhotoCategory != "" {
			t.Errorf("PhotoCategory = %q, want empty", results[0].PhotoCategory)
		}
	})
}

func TestPerFileRules(t *testing.T) {
	tests := []struct {
		name         string
		perFile      []PerFileRule
		result       AnalysisResult
		wantRemarks  string
	}{
		{
			name: "detected_text_contains match",
			perFile: []PerFileRule{
				{
					Condition: PerFileCondition{DetectedTextContains: strPtr("黒板")},
					Apply:     FieldOverrides{Remarks: strPtr("黒板あり")},
				},
			},
			result:      AnalysisResult{DetectedText: "黒板が見える"},
			wantRemarks: "黒板あり",
		},
		{
			name: "default=true fallback",
			perFile: []PerFileRule{
				{
					Condition: PerFileCondition{DetectedTextContains: strPtr("存在しない")},
					Apply:     FieldOverrides{Remarks: strPtr("first")},
				},
				{
					Condition: PerFileCondition{Default: boolPtr(true)},
					Apply:     FieldOverrides{Remarks: strPtr("default")},
				},
			},
			result:      AnalysisResult{DetectedText: "何か"},
			wantRemarks: "default",
		},
		{
			name: "station_empty=true matches empty station",
			perFile: []PerFileRule{
				{
					Condition: PerFileCondition{StationEmpty: boolPtr(true)},
					Apply:     FieldOverrides{Remarks: strPtr("no station")},
				},
			},
			result:      AnalysisResult{Station: ""},
			wantRemarks: "no station",
		},
		{
			name: "station_empty=true does not match filled station",
			perFile: []PerFileRule{
				{
					Condition: PerFileCondition{StationEmpty: boolPtr(true)},
					Apply:     FieldOverrides{Remarks: strPtr("no station")},
				},
			},
			result:      AnalysisResult{Station: "No.1"},
			wantRemarks: "",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			rule := FolderRule{
				ID:       "pf",
				MatchAll: []string{},
				PerFile:  tt.perFile,
			}
			results := []AnalysisResult{tt.result}
			results = ApplyFolderRules(results, "any", []FolderRule{rule})
			if results[0].Remarks != tt.wantRemarks {
				t.Errorf("Remarks = %q, want %q", results[0].Remarks, tt.wantRemarks)
			}
		})
	}
}

func TestStationReplacements(t *testing.T) {
	rule := FolderRule{
		ID:                  "sr",
		MatchAll:            []string{},
		StationReplacements: map[string]string{"旧名": "新名"},
	}
	results := []AnalysisResult{
		{Station: "旧名"},
		{Station: "別の測点"},
	}
	results = ApplyFolderRules(results, "folder", []FolderRule{rule})
	if results[0].Station != "新名" {
		t.Errorf("Station[0] = %q, want %q", results[0].Station, "新名")
	}
	if results[1].Station != "別の測点" {
		t.Errorf("Station[1] = %q, want %q", results[1].Station, "別の測点")
	}
}

func TestFocusTargetRenames(t *testing.T) {
	rule := FolderRule{
		ID:                 "ftr",
		MatchAll:           []string{},
		FocusTargetRenames: map[string]string{"old_target": "new_target"},
	}
	results := []AnalysisResult{
		{FocusTarget: "old_target"},
		{FocusTarget: "other"},
	}
	results = ApplyFolderRules(results, "folder", []FolderRule{rule})
	if results[0].FocusTarget != "new_target" {
		t.Errorf("FocusTarget[0] = %q, want %q", results[0].FocusTarget, "new_target")
	}
	if results[1].FocusTarget != "other" {
		t.Errorf("FocusTarget[1] = %q, want %q", results[1].FocusTarget, "other")
	}
}

func TestRuleSetMatch(t *testing.T) {
	rs := RuleSet{
		Rules: []FolderRule{
			{ID: "r1", MatchAll: []string{"舗装", "工事"}},
			{ID: "r2", MatchAll: []string{"土工"}},
		},
	}

	tests := []struct {
		name       string
		folderPath string
		wantID     string
		wantFound  bool
	}{
		{"all keywords present", "舗装工事フォルダ", "r1", true},
		{"partial keyword missing", "舗装フォルダ", "", false},
		{"second rule match", "土工フォルダ", "r2", true},
		{"no match", "その他", "", false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			rule, found := rs.Match(tt.folderPath)
			if found != tt.wantFound {
				t.Errorf("Match(%q) found = %v, want %v", tt.folderPath, found, tt.wantFound)
			}
			if found && rule.ID != tt.wantID {
				t.Errorf("Match(%q) ID = %q, want %q", tt.folderPath, rule.ID, tt.wantID)
			}
		})
	}
}
