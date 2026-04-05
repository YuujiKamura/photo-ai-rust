package ocr

import (
	"testing"
)

func TestExtractKV(t *testing.T) {
	tests := []struct {
		name      string
		input     string
		wantPairs []KV
	}{
		{
			name:  "simple_kv",
			input: "工種：舗装工",
			wantPairs: []KV{
				{Key: "工種", Value: "舗装工"},
			},
		},
		{
			name:  "station_with_extra_keywords",
			input: "場所：No. 4 L 左車線",
			// "No. 4 L" is non-wide, "左車線" is wide => split off as keyword
			wantPairs: []KV{
				{Key: "", Value: "左車線"},
				{Key: "場所", Value: "No. 4 L"},
			},
		},
		{
			name:  "multi_line_kv",
			input: "工事名：テスト工事\n工種：土工",
			wantPairs: []KV{
				{Key: "工事名", Value: "テスト工事"},
				{Key: "工種", Value: "土工"},
			},
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := ExtractKV(tt.input)
			if len(got) != len(tt.wantPairs) {
				t.Fatalf("ExtractKV(%q) returned %d pairs, want %d\ngot: %+v", tt.input, len(got), len(tt.wantPairs), got)
			}
			for i, w := range tt.wantPairs {
				if got[i].Key != w.Key || got[i].Value != w.Value {
					t.Errorf("ExtractKV(%q)[%d] = {%q, %q}, want {%q, %q}", tt.input, i, got[i].Key, got[i].Value, w.Key, w.Value)
				}
			}
		})
	}
}

func TestNormalizeStation(t *testing.T) {
	tests := []struct {
		name  string
		input string
		want  string
	}{
		{"L_suffix", "No. 4 L", "No.4 左車線"},
		{"R_suffix", "No. 4 R", "No.4 右車線"},
		{"date_format", "2/11", "2月11日"},
		{"empty", "", ""},
		{"no_suffix", "No. 1", "No.1"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := NormalizeStation(tt.input); got != tt.want {
				t.Errorf("NormalizeStation(%q) = %q, want %q", tt.input, got, tt.want)
			}
		})
	}
}

func TestParse(t *testing.T) {
	tests := []struct {
		name         string
		input        string
		wantWorkType string
		wantStation  string
	}{
		{
			name:         "basic_board",
			input:        "工種：舗装工\n場所：No.1",
			wantWorkType: "舗装工",
			wantStation:  "No.1",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := Parse(tt.input)
			if got.WorkType != tt.wantWorkType {
				t.Errorf("Parse(%q).WorkType = %q, want %q", tt.input, got.WorkType, tt.wantWorkType)
			}
			if got.Station != tt.wantStation {
				t.Errorf("Parse(%q).Station = %q, want %q", tt.input, got.Station, tt.wantStation)
			}
		})
	}
}
