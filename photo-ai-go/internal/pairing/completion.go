// Package pairing — completion.go
//
// Supplementary helpers derived from src/pair_completion.rs that are not yet
// covered by pairing.go.  All new exported symbols are prefixed with
// "PairCompletion" to avoid collisions with the existing API.
//
// The existing pairing.go already contains: Pair, PairFolder, PairPhotos,
// ScanPairFolders, ScanAfterFolder, ParseEnsembleResponse, ParsePairNumbers.
// This file adds: PairCompletionResult, FindFileInPFolders, FilterPairs,
// and StationNameFromFolderName.
package pairing

import (
	"os"
	"path/filepath"
	"strings"
)

// ConfidenceMajority is the minimum confidence required to include a pair.
// Mirrors CONFIDENCE_MAJORITY in pair_completion.rs.
const ConfidenceMajority = 0.67

// ConfidenceUnanimous represents a 100% consensus across all ensemble scans.
// Mirrors CONFIDENCE_UNANIMOUS in pair_completion.rs.
const ConfidenceUnanimous = 1.0

// PairCompletionResult is the serialisable result record written to pairing.json.
// Mirrors PairResult in pair_completion.rs.
type PairCompletionResult struct {
	BeforePage    uint32  `json:"before_page"`
	BeforeStation string  `json:"before_station"`
	AfterFile     string  `json:"after_file"`
	Confidence    float64 `json:"confidence"`
}

// FilterPairs removes entries whose confidence is below ConfidenceMajority
// or whose AfterFile is empty, mirroring the filter applied in
// build_pair_folders and load_pairs_from_json in pair_completion.rs.
func FilterPairs(results []PairCompletionResult) []PairCompletionResult {
	out := results[:0:0] // reuse backing array's type but start fresh
	for _, r := range results {
		if r.Confidence >= ConfidenceMajority && r.AfterFile != "" {
			out = append(out, r)
		}
	}
	return out
}

// FindFileInPFolders searches for filename inside P* sub-directories of root.
// Returns the full path when found, or "" when not found.
// Mirrors find_file_in_pfolders in pair_completion.rs.
func FindFileInPFolders(root, filename string) string {
	if filename == "" {
		return ""
	}
	entries, err := os.ReadDir(root)
	if err != nil {
		return ""
	}
	for _, e := range entries {
		if !e.IsDir() {
			continue
		}
		if !strings.HasPrefix(e.Name(), "P") {
			continue
		}
		candidate := filepath.Join(root, e.Name(), filename)
		if _, err := os.Stat(candidate); err == nil {
			return candidate
		}
	}
	return ""
}

// StationNameFromFolderName derives a human-readable station name from a
// P{NN}_<station> folder name, mirroring the station extraction used in
// scan_pair_folders and build_pair_folders in pair_completion.rs.
//
// "P01_No.5 右車線" → "No.5 右車線"
// "P12_サンプル測点" → "サンプル測点"
// "unrelated"         → "unrelated"
func StationNameFromFolderName(folderName string) string {
	idx := strings.Index(folderName, "_")
	if idx < 0 {
		return folderName
	}
	return folderName[idx+1:]
}

// PairCompletionSummary holds the confidence breakdown of a pairing run.
// Mirrors the summary printed in handle_pair_completion in pair_completion.rs.
type PairCompletionSummary struct {
	Unanimous int // confidence == 1.0
	Majority  int // 0.67 <= confidence < 1.0
	Split     int // confidence < 0.67
}

// SummarisePairs computes a PairCompletionSummary over a slice of results.
func SummarisePairs(results []PairCompletionResult) PairCompletionSummary {
	var s PairCompletionSummary
	for _, r := range results {
		switch {
		case r.Confidence >= ConfidenceUnanimous:
			s.Unanimous++
		case r.Confidence >= ConfidenceMajority:
			s.Majority++
		default:
			s.Split++
		}
	}
	return s
}
