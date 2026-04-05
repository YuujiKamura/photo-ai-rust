package pairing

import (
	"fmt"
	"math"
	"testing"
)

func TestParseEnsembleResponse(t *testing.T) {
	tests := []struct {
		name           string
		response       string
		wantAfterNum   int
		wantConfidence float64
		wantErr        bool
	}{
		{
			name:           "majority vote A03",
			response:       "Final: A03\nScan1: A03\nScan2: A04",
			wantAfterNum:   3,
			wantConfidence: 2.0 / 3.0,
		},
		{
			name:           "unanimous A01",
			response:       "Final: A01\nScan1: A01\nScan2: A01\nScan3: A01",
			wantAfterNum:   1,
			wantConfidence: 1.0,
		},
		{
			name:    "no match",
			response: "no match",
			wantErr: true,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := ParseEnsembleResponse(tt.response)
			if tt.wantErr {
				if err == nil {
					t.Fatalf("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if result.AfterNumber != tt.wantAfterNum {
				t.Errorf("AfterNumber = %d, want %d", result.AfterNumber, tt.wantAfterNum)
			}
			if math.Abs(result.Confidence-tt.wantConfidence) > 0.01 {
				t.Errorf("Confidence = %f, want %f", result.Confidence, tt.wantConfidence)
			}
		})
	}
}

func TestParsePairNumbers(t *testing.T) {
	tests := []struct {
		name    string
		spec    string
		want    []int
		wantErr bool
	}{
		{"range", "1-3", []int{1, 2, 3}, false},
		{"comma separated", "1,3,5", []int{1, 3, 5}, false},
		{"single", "2", []int{2}, false},
		{"empty", "", nil, true},
		{"zero", "0", nil, true},
		{"reverse range", "3-1", nil, true},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := ParsePairNumbers(tt.spec)
			if tt.wantErr {
				if err == nil {
					t.Fatalf("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if len(got) != len(tt.want) {
				t.Fatalf("len = %d, want %d", len(got), len(tt.want))
			}
			for i := range got {
				if got[i] != tt.want[i] {
					t.Errorf("got[%d] = %d, want %d", i, got[i], tt.want[i])
				}
			}
		})
	}
}

func TestPairPhotos(t *testing.T) {
	queryFn := func(before string) (string, error) {
		return "Final: A01\nScan1: A01", nil
	}
	pairs, err := PairPhotos([]string{"before1.jpg", "before2.jpg"}, queryFn)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(pairs) != 2 {
		t.Fatalf("len(pairs) = %d, want 2", len(pairs))
	}
	for i, p := range pairs {
		if p.AfterPath != "A01" {
			t.Errorf("pairs[%d].AfterPath = %q, want %q", i, p.AfterPath, "A01")
		}
		if p.Confidence != 1.0 {
			t.Errorf("pairs[%d].Confidence = %f, want 1.0", i, p.Confidence)
		}
	}
}

func TestPairPhotos_ParseError(t *testing.T) {
	queryFn := func(before string) (string, error) {
		return "invalid", nil
	}
	pairs, err := PairPhotos([]string{"before1.jpg"}, queryFn)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(pairs) != 1 {
		t.Fatalf("len(pairs) = %d, want 1", len(pairs))
	}
	if pairs[0].Confidence != 0 {
		t.Errorf("Confidence = %f, want 0", pairs[0].Confidence)
	}
}

func TestPairPhotos_QueryError(t *testing.T) {
	queryFn := func(before string) (string, error) {
		return "", fmt.Errorf("network error")
	}
	_, err := PairPhotos([]string{"before1.jpg"}, queryFn)
	if err == nil {
		t.Fatal("expected error, got nil")
	}
}
