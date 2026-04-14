// golden_diff_test.go — Rust ↔ Go parity tests for issue #165 Stream E.
//
// Each Test*Golden function reads the shared fixture under
// tests/golden_diff/fixtures/<name>/{input,expected}.json
// (resolved relative to repo root via runtime.Caller), runs the Go
// implementation, marshals its output to the same schema, and asserts
// byte-for-byte JSON equality on each entry.
//
// Run with:
//   go test ./internal/normalizer/ -run Golden -v
package normalizer

import (
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

// repoRoot walks up from this file's location until it finds go.mod.
func repoRoot(t *testing.T) string {
	t.Helper()
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("runtime.Caller failed")
	}
	dir := filepath.Dir(file)
	for {
		if _, err := os.Stat(filepath.Join(dir, "go.mod")); err == nil {
			// go.mod is inside photo-ai-go — repo root is one level up
			return filepath.Dir(dir)
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			t.Fatal("could not locate go.mod walking up from " + file)
		}
		dir = parent
	}
}

func fixtureDir(t *testing.T, name string) string {
	t.Helper()
	return filepath.Join(repoRoot(t), "tests", "golden_diff", "fixtures", name)
}

func readJSON(t *testing.T, path string, v any) {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("readJSON %s: %v", path, err)
	}
	if err := json.Unmarshal(data, v); err != nil {
		t.Fatalf("unmarshal %s: %v", path, err)
	}
}

// ─── temperature_normalization ────────────────────────────────────────────────

type tempInput struct {
	ID   string `json:"id"`
	Text string `json:"text"`
}

type tempExpected struct {
	ID         string `json:"id"`
	Kind       string `json:"kind"`
	Label      string `json:"label"`
	ShortLabel string `json:"short_label"`
	Found      bool   `json:"found"`
}

type tempActual struct {
	ID         string `json:"id"`
	Kind       string `json:"kind"`
	Label      string `json:"label"`
	ShortLabel string `json:"short_label"`
	Found      bool   `json:"found"`
}

func kindName(k TemperatureKind) string {
	switch k {
	case TemperatureArrival:
		return "TemperatureArrival"
	case TemperatureSpreading:
		return "TemperatureSpreading"
	case TemperatureInitialCompaction:
		return "TemperatureInitialCompaction"
	case TemperatureOpening:
		return "TemperatureOpening"
	case TemperatureOutsideAir:
		return "TemperatureOutsideAir"
	default:
		return ""
	}
}

func TestTemperatureNormalizationGolden(t *testing.T) {
	dir := fixtureDir(t, "temperature_normalization")

	var inputs []tempInput
	readJSON(t, filepath.Join(dir, "input.json"), &inputs)

	var expected []tempExpected
	readJSON(t, filepath.Join(dir, "expected.json"), &expected)

	if len(inputs) != len(expected) {
		t.Fatalf("input/expected length mismatch: %d vs %d", len(inputs), len(expected))
	}

	for i, in := range inputs {
		exp := expected[i]
		if in.ID != exp.ID {
			t.Fatalf("row %d ID mismatch: input=%q expected=%q", i, in.ID, exp.ID)
		}

		var act tempActual
		act.ID = in.ID

		if k, ok := TemperatureKindFromText(in.Text); ok {
			act.Found = true
			act.Kind = kindName(k)
			act.Label = k.TemperatureLabel()
			act.ShortLabel = k.TemperatureShortLabel()
		}

		actJSON, _ := json.Marshal(act)
		expJSON, _ := json.Marshal(exp)
		if string(actJSON) != string(expJSON) {
			t.Errorf("[%s] mismatch:\n  got: %s\n want: %s", in.ID, actJSON, expJSON)
		}
	}
}

// ─── station_normalization ────────────────────────────────────────────────────

type stationInput struct {
	ID   string `json:"id"`
	Text string `json:"text"`
}

type stationExpected struct {
	ID            string `json:"id"`
	Kind          string `json:"kind"`
	Display       string `json:"display"`
	IsPost        bool   `json:"is_post"`
	IsDate        bool   `json:"is_date"`
	HasDumpNumber bool   `json:"has_dump_number"`
}

type stationActual struct {
	ID            string `json:"id"`
	Kind          string `json:"kind"`
	Display       string `json:"display"`
	IsPost        bool   `json:"is_post"`
	IsDate        bool   `json:"is_date"`
	HasDumpNumber bool   `json:"has_dump_number"`
}

func stationKindName(s Station) string {
	switch s.kind {
	case StationKindEmpty:
		return "Empty"
	case StationKindPost:
		return "Post"
	case StationKindDate:
		return "Date"
	case StationKindOther:
		return "Other"
	default:
		return ""
	}
}

func TestStationNormalizationGolden(t *testing.T) {
	dir := fixtureDir(t, "station_normalization")

	var inputs []stationInput
	readJSON(t, filepath.Join(dir, "input.json"), &inputs)

	var expected []stationExpected
	readJSON(t, filepath.Join(dir, "expected.json"), &expected)

	if len(inputs) != len(expected) {
		t.Fatalf("input/expected length mismatch: %d vs %d", len(inputs), len(expected))
	}

	for i, in := range inputs {
		exp := expected[i]
		if in.ID != exp.ID {
			t.Fatalf("row %d ID mismatch: input=%q expected=%q", i, in.ID, exp.ID)
		}

		s := StationParse(in.Text)
		act := stationActual{
			ID:            in.ID,
			Kind:          stationKindName(s),
			Display:       s.ToDisplay(),
			IsPost:        s.IsPost(),
			IsDate:        s.IsDate(),
			HasDumpNumber: s.HasDumpNumber(),
		}

		actJSON, _ := json.Marshal(act)
		expJSON, _ := json.Marshal(exp)
		if string(actJSON) != string(expJSON) {
			t.Errorf("[%s] mismatch:\n  got: %s\n want: %s", in.ID, actJSON, expJSON)
		}
	}
}
