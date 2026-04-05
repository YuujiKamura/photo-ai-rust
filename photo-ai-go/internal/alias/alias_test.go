package alias

import (
	"testing"
)

func TestLongestMatchTransform(t *testing.T) {
	tests := []struct {
		name  string
		value string
		m     map[string]string
		want  string
	}{
		{
			name:  "exact_match",
			value: "品質",
			m:     map[string]string{"品質": "品質管理写真"},
			want:  "品質管理写真",
		},
		{
			name:  "partial_match",
			value: "品質管理なんとか",
			m:     map[string]string{"品質管理": "品質管理写真"},
			want:  "品質管理写真",
		},
		{
			name:  "longest_wins",
			value: "品質管理",
			m:     map[string]string{"品質": "X", "品質管理": "Y"},
			want:  "Y",
		},
		{
			name:  "no_match",
			value: "unknown",
			m:     map[string]string{"品質": "X"},
			want:  "unknown",
		},
		{
			name:  "empty_value",
			value: "",
			m:     map[string]string{"品質": "X"},
			want:  "",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := LongestMatchTransform(tt.value, tt.m); got != tt.want {
				t.Errorf("LongestMatchTransform(%q, ...) = %q, want %q", tt.value, got, tt.want)
			}
		})
	}
}

func TestFromPreset(t *testing.T) {
	tests := []struct {
		name   string
		preset string
		wantOK bool
	}{
		{"pavement", "pavement", true},
		{"marking", "marking", true},
		{"unknown", "unknown", false},
		{"japanese_pavement", "舗装", true},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_, ok := FromPreset(tt.preset)
			if ok != tt.wantOK {
				t.Errorf("FromPreset(%q) ok = %v, want %v", tt.preset, ok, tt.wantOK)
			}
		})
	}
}

func TestApply_pavement(t *testing.T) {
	cfg, ok := FromPreset("pavement")
	if !ok {
		t.Fatal("FromPreset(pavement) returned false")
	}

	tests := []struct {
		name  string
		input AnalysisResult
		field string
		want  string
	}{
		{
			name:  "photo_category",
			input: AnalysisResult{PhotoCategory: "品質"},
			field: "PhotoCategory",
			want:  "品質管理写真",
		},
		{
			name:  "work_type",
			input: AnalysisResult{WorkType: "舗装"},
			field: "WorkType",
			want:  "舗装工",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := cfg.Apply(tt.input)
			var actual string
			switch tt.field {
			case "PhotoCategory":
				actual = got.PhotoCategory
			case "WorkType":
				actual = got.WorkType
			}
			if actual != tt.want {
				t.Errorf("Apply(...).%s = %q, want %q", tt.field, actual, tt.want)
			}
		})
	}
}

func TestApplyAliases(t *testing.T) {
	tests := []struct {
		name         string
		preset       string
		extra        *Config
		input        []AnalysisResult
		wantWarnings int
		checkFunc    func(t *testing.T, results []AnalysisResult)
	}{
		{
			name:         "pavement_preset",
			preset:       "pavement",
			extra:        nil,
			input:        []AnalysisResult{{PhotoCategory: "品質"}},
			wantWarnings: 0,
			checkFunc: func(t *testing.T, results []AnalysisResult) {
				if results[0].PhotoCategory != "品質管理写真" {
					t.Errorf("PhotoCategory = %q, want %q", results[0].PhotoCategory, "品質管理写真")
				}
			},
		},
		{
			name:   "custom_extra_only",
			preset: "",
			extra: &Config{
				PhotoCategory: map[string]string{"カスタム": "カスタム写真"},
				WorkType:      make(map[string]string),
				Variety:       make(map[string]string),
				Subphase:      make(map[string]string),
			},
			input:        []AnalysisResult{{PhotoCategory: "カスタム"}},
			wantWarnings: 0,
			checkFunc: func(t *testing.T, results []AnalysisResult) {
				if results[0].PhotoCategory != "カスタム写真" {
					t.Errorf("PhotoCategory = %q, want %q", results[0].PhotoCategory, "カスタム写真")
				}
			},
		},
		{
			name:         "unknown_preset_warning",
			preset:       "unknown",
			extra:        nil,
			input:        []AnalysisResult{{PhotoCategory: "品質"}},
			wantWarnings: 1,
			checkFunc: func(t *testing.T, results []AnalysisResult) {
				// unknown preset means no transformation
				if results[0].PhotoCategory != "品質" {
					t.Errorf("PhotoCategory = %q, want %q (untransformed)", results[0].PhotoCategory, "品質")
				}
			},
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			results, warnings := ApplyAliases(tt.input, tt.preset, tt.extra)
			if len(warnings) != tt.wantWarnings {
				t.Errorf("warnings count = %d, want %d; warnings: %v", len(warnings), tt.wantWarnings, warnings)
			}
			tt.checkFunc(t, results)
		})
	}
}
