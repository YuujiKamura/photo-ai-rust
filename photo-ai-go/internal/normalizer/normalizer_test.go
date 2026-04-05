package normalizer

import (
	"math"
	"testing"
)

func TestContainsMeasurement(t *testing.T) {
	tests := []struct {
		name  string
		input string
		want  bool
	}{
		{"temperature", "160.2℃", true},
		{"dimension", "t=50mm", true},
		{"density", "95.5%", true},
		{"general_kg", "10kg", true},
		{"no_measurement", "hello", false},
		{"empty", "", false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := ContainsMeasurement(tt.input); got != tt.want {
				t.Errorf("ContainsMeasurement(%q) = %v, want %v", tt.input, got, tt.want)
			}
		})
	}
}

func TestExtractTemperature(t *testing.T) {
	tests := []struct {
		name    string
		input   string
		wantVal float64
		wantOK  bool
	}{
		{"with_temp", "到着温度 160.2℃", 160.2, true},
		{"no_temp", "no temp", 0, false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			val, ok := ExtractTemperature(tt.input)
			if ok != tt.wantOK {
				t.Errorf("ExtractTemperature(%q) ok = %v, want %v", tt.input, ok, tt.wantOK)
			}
			if math.Abs(val-tt.wantVal) > 0.01 {
				t.Errorf("ExtractTemperature(%q) val = %v, want %v", tt.input, val, tt.wantVal)
			}
		})
	}
}

func TestExtractDimensionMM(t *testing.T) {
	tests := []struct {
		name    string
		input   string
		wantVal float64
		wantOK  bool
	}{
		{"mm", "t=50mm", 50, true},
		{"m_to_mm", "1.5m", 1500, true},
		{"cm_to_mm", "3cm", 30, true},
		{"no_dim", "no dim", 0, false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			val, ok := ExtractDimensionMM(tt.input)
			if ok != tt.wantOK {
				t.Errorf("ExtractDimensionMM(%q) ok = %v, want %v", tt.input, ok, tt.wantOK)
			}
			if math.Abs(val-tt.wantVal) > 0.01 {
				t.Errorf("ExtractDimensionMM(%q) val = %v, want %v", tt.input, val, tt.wantVal)
			}
		})
	}
}

func TestClassifyTemperature(t *testing.T) {
	tests := []struct {
		name  string
		input float64
		want  string
	}{
		{"arrival", 150.0, "到着温度測定"},
		{"spreading", 135.0, "敷均し温度測定"},
		{"initial_compaction", 125.0, "初期締固め前温度測定"},
		{"opening", 50.0, "開放温度測定"},
		{"out_of_range", 999.0, ""},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := ClassifyTemperature(tt.input); got != tt.want {
				t.Errorf("ClassifyTemperature(%v) = %q, want %q", tt.input, got, tt.want)
			}
		})
	}
}

func TestExtractDumpNumber(t *testing.T) {
	tests := []struct {
		name    string
		input   string
		wantNum int
		wantOK  bool
	}{
		{"found", "3台目", 3, true},
		{"not_found", "no dump", 0, false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			num, ok := ExtractDumpNumber(tt.input)
			if ok != tt.wantOK || num != tt.wantNum {
				t.Errorf("ExtractDumpNumber(%q) = (%d, %v), want (%d, %v)", tt.input, num, ok, tt.wantNum, tt.wantOK)
			}
		})
	}
}

func TestIsTemperaturePhoto(t *testing.T) {
	tests := []struct {
		name  string
		input string
		want  bool
	}{
		{"temp_keyword", "到着温度 計測中", true},
		{"not_temp", "施工状況", false},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := IsTemperaturePhoto(tt.input); got != tt.want {
				t.Errorf("IsTemperaturePhoto(%q) = %v, want %v", tt.input, got, tt.want)
			}
		})
	}
}

func TestExtractTemperatureForRemarks(t *testing.T) {
	tests := []struct {
		name  string
		input string
		want  string
	}{
		{"leading", "160.2℃, 到着", "160.2℃"},
		{"trailing", ", 153.7℃", "153.7℃"},
		{"no_temp", "no temp", ""},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := ExtractTemperatureForRemarks(tt.input); got != tt.want {
				t.Errorf("ExtractTemperatureForRemarks(%q) = %q, want %q", tt.input, got, tt.want)
			}
		})
	}
}
