// Package pairing implements before/after photo pairing logic.
// Ported from src/pair_completion.rs
package pairing

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
)

// Pair links a before photo to its corresponding after photo.
type Pair struct {
	BeforePath string
	AfterPath  string
	Confidence float64
}

// PairFolder represents a matched pair folder (P{NN}_* layout).
type PairFolder struct {
	Name       string
	StationName string
	BeforePath string
	AfterPath  string
}

// pairJSON is the on-disk format for a serialised pair list.
type pairJSON struct {
	BeforePage    uint32  `json:"before_page"`
	BeforeStation string  `json:"before_station"`
	AfterFile     string  `json:"after_file"`
	Confidence    float64 `json:"confidence"`
}

// LoadPairsFromJSON reads a pairing JSON file and returns a Pair slice.
// Mirrors load_pairs_from_json in pair_completion.rs.
func LoadPairsFromJSON(path string) ([]Pair, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("read pairs JSON %q: %w", path, err)
	}
	var entries []pairJSON
	if err := json.Unmarshal(data, &entries); err != nil {
		return nil, fmt.Errorf("parse pairs JSON %q: %w", path, err)
	}
	pairs := make([]Pair, 0, len(entries))
	for _, e := range entries {
		if e.Confidence < 0.67 || e.AfterFile == "" {
			continue
		}
		pairs = append(pairs, Pair{
			BeforePath: fmt.Sprintf("page_%d", e.BeforePage),
			AfterPath:  e.AfterFile,
			Confidence: e.Confidence,
		})
	}
	return pairs, nil
}

// ScanAfterFolder returns sorted image file paths from a directory,
// excluding sub-directories. Mirrors scan_after_folder in pair_completion.rs.
func ScanAfterFolder(dir string) ([]string, error) {
	info, err := os.Stat(dir)
	if err != nil || !info.IsDir() {
		return nil, fmt.Errorf("not a directory: %s", dir)
	}
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil, fmt.Errorf("read dir %s: %w", dir, err)
	}
	var files []string
	for _, e := range entries {
		if e.IsDir() {
			continue
		}
		ext := strings.ToLower(filepath.Ext(e.Name()))
		if ext == ".jpg" || ext == ".jpeg" || ext == ".png" {
			files = append(files, filepath.Join(dir, e.Name()))
		}
	}
	return files, nil
}

// ScanPairFolders scans a directory for P{NN}_* pair folders and returns
// PairFolder entries sorted by pair number.
// Mirrors scan_pair_folders in pair_completion.rs.
func ScanPairFolders(dir string) ([]PairFolder, error) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil, fmt.Errorf("read dir %s: %w", dir, err)
	}

	type numbered struct {
		num  int
		name string
		path string
	}
	var dirs []numbered
	for _, e := range entries {
		if !e.IsDir() {
			continue
		}
		name := e.Name()
		if !strings.HasPrefix(name, "P") {
			continue
		}
		underIdx := strings.Index(name, "_")
		if underIdx < 0 {
			continue
		}
		prefix := name[1:underIdx]
		if prefix == "" {
			continue
		}
		n, err := strconv.Atoi(prefix)
		if err != nil {
			continue
		}
		dirs = append(dirs, numbered{n, name, filepath.Join(dir, name)})
	}
	// Sort by pair number (simple insertion sort for small slices).
	for i := 1; i < len(dirs); i++ {
		for j := i; j > 0 && dirs[j].num < dirs[j-1].num; j-- {
			dirs[j], dirs[j-1] = dirs[j-1], dirs[j]
		}
	}

	var result []PairFolder
	for _, d := range dirs {
		underIdx := strings.Index(d.name, "_")
		stationName := d.name[underIdx+1:]
		before, after := findBeforeAfter(d.path)
		result = append(result, PairFolder{
			Name:        d.name,
			StationName: stationName,
			BeforePath:  before,
			AfterPath:   after,
		})
	}
	return result, nil
}

// findBeforeAfter detects before/after image files inside a pair folder.
// Priority: "before"/"after" in filename, then uppercase ext = before.
func findBeforeAfter(dir string) (before, after string) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return
	}
	imgExts := map[string]bool{".jpg": true, ".jpeg": true, ".png": true, ".bmp": true}
	var images []string
	for _, e := range entries {
		if e.IsDir() {
			continue
		}
		ext := strings.ToLower(filepath.Ext(e.Name()))
		if imgExts[ext] {
			images = append(images, filepath.Join(dir, e.Name()))
		}
	}
	// 1. Name-based detection.
	for _, p := range images {
		stem := strings.ToLower(strings.TrimSuffix(filepath.Base(p), filepath.Ext(p)))
		if strings.Contains(stem, "before") && before == "" {
			before = p
		} else if strings.Contains(stem, "after") && after == "" {
			after = p
		}
	}
	// 2. Extension case fallback.
	if before == "" || after == "" {
		for _, p := range images {
			rawExt := filepath.Ext(p)
			isUpper := rawExt != "" && rawExt == strings.ToUpper(rawExt)
			if isUpper && before == "" {
				before = p
			} else if !isUpper && after == "" {
				after = p
			}
		}
	}
	return
}

// EnsembleResult holds parsed output from an AI ensemble pairing response.
type EnsembleResult struct {
	AfterNumber int
	Confidence  float64
	Raw         string
}

// ParseEnsembleResponse parses a Final/Scan line format response from the AI.
// Mirrors ensemble_pair_query result parsing in pair_ensemble.rs.
//
// Expected format (one or more lines):
//
//	Final: A03
//	Scan1: A03
//	Scan2: A03
//	Scan3: A04
func ParseEnsembleResponse(response string) (EnsembleResult, error) {
	scanRE := regexp.MustCompile(`(?i)(?:Final|Scan\d+)\s*:\s*A(\d+)`)
	matches := scanRE.FindAllStringSubmatch(response, -1)
	if len(matches) == 0 {
		return EnsembleResult{Raw: response}, fmt.Errorf("no A-number found in response: %q", response)
	}

	// Count votes.
	votes := make(map[int]int)
	for _, m := range matches {
		n, err := strconv.Atoi(m[1])
		if err != nil {
			continue
		}
		votes[n]++
	}

	// Find winner.
	best, bestCount := 0, 0
	for n, c := range votes {
		if c > bestCount || (c == bestCount && n < best) {
			best, bestCount = n, c
		}
	}

	confidence := float64(bestCount) / float64(len(matches))
	return EnsembleResult{
		AfterNumber: best,
		Confidence:  confidence,
		Raw:         response,
	}, nil
}

// ParsePairNumbers parses a pair number spec into a slice of ints.
// Mirrors parse_pair_numbers in pair_completion.rs.
//
// Formats: "1-3" → [1,2,3], "1,3,5" → [1,3,5], "2" → [2]
func ParsePairNumbers(spec string) ([]int, error) {
	s := strings.TrimSpace(spec)
	if s == "" {
		return nil, fmt.Errorf("empty pair spec")
	}

	if strings.Contains(s, "-") && !strings.Contains(s, ",") {
		parts := strings.SplitN(s, "-", 2)
		start, err1 := strconv.Atoi(strings.TrimSpace(parts[0]))
		end, err2 := strconv.Atoi(strings.TrimSpace(parts[1]))
		if err1 != nil || err2 != nil || start <= 0 || end <= 0 || start > end {
			return nil, fmt.Errorf("invalid range spec: %q", s)
		}
		nums := make([]int, 0, end-start+1)
		for i := start; i <= end; i++ {
			nums = append(nums, i)
		}
		return nums, nil
	}

	if strings.Contains(s, ",") {
		var nums []int
		for _, part := range strings.Split(s, ",") {
			n, err := strconv.Atoi(strings.TrimSpace(part))
			if err != nil || n <= 0 {
				return nil, fmt.Errorf("invalid number in pair spec: %q", part)
			}
			nums = append(nums, n)
		}
		return nums, nil
	}

	n, err := strconv.Atoi(s)
	if err != nil || n <= 0 {
		return nil, fmt.Errorf("invalid pair number: %q", s)
	}
	return []int{n}, nil
}

// PairPhotos runs a simple before/after pairing algorithm over lists of photo paths.
// queryFn is called with each before photo path and returns an AI response string.
// Mirrors the pairing loop in handle_pair_completion in pair_completion.rs.
func PairPhotos(beforePhotos []string, queryFn func(string) (string, error)) ([]Pair, error) {
	var pairs []Pair
	for _, before := range beforePhotos {
		response, err := queryFn(before)
		if err != nil {
			return nil, fmt.Errorf("query for %s: %w", before, err)
		}
		result, err := ParseEnsembleResponse(response)
		if err != nil {
			// Non-fatal: append with zero confidence.
			pairs = append(pairs, Pair{BeforePath: before, Confidence: 0})
			continue
		}
		pairs = append(pairs, Pair{
			BeforePath: before,
			AfterPath:  fmt.Sprintf("A%02d", result.AfterNumber),
			Confidence: result.Confidence,
		})
	}
	return pairs, nil
}
