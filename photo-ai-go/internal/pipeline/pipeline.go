// Package pipeline orchestrates the full photo analysis flow:
// scan → EXIF extraction → AI grouping → master matching → normalization → output.
//
// This is a Go port of src/analysis.rs (scan_and_analyze, scan_images, etc.).
package pipeline

import (
	"context"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"

	"github.com/YuujiKamura/photo-ai-go/internal/ai"
	"github.com/YuujiKamura/photo-ai-go/internal/matching"
	"github.com/YuujiKamura/photo-ai-go/internal/ocr"
	"github.com/YuujiKamura/photo-ai-go/internal/rules"
	"github.com/YuujiKamura/photo-ai-go/pkg/engine"
)

// imageExtensions is the set of supported photo file extensions (lower-cased).
var imageExtensions = map[string]bool{
	".jpg":  true,
	".jpeg": true,
	".png":  true,
	".tiff": true,
	".tif":  true,
}

// excludeFolderPatterns mirrors EXCLUDE_PATTERNS in scanner/mod.rs.
var excludeFolderPatterns = []string{"非使用", "hisiyou", "不要", "excluded"}

// ImageInfo holds lightweight metadata about a discovered photo file.
// Mirrors scanner::ImageInfo in Rust.
type ImageInfo struct {
	Path     string
	FileName string
	Date     string // EXIF DateTimeOriginal; empty if unavailable
}

// Config holds all settings for a single scan-and-analyse run.
// Mirrors ScanAnalysisConfig in analysis.rs.
type Config struct {
	Folder      string
	BatchSize   int
	Verbose     bool
	WorkType    string // optional master filter
	PhotoType   string // optional photo-category filter
	UseCache    bool
	Variety     string
	Station     string
	Recursive   bool
	IncludeAll  bool
	PayPerUse   bool
	FolderRules []rules.FolderRule
	AIClient    *ai.Client  // may be nil; DLL path is used when set
	Matcher     *matching.Matcher
}

// Result is the per-photo output of the analysis pipeline.
// Mirrors analyzer::AnalysisResult / engine.AnalysisResult.
type Result struct {
	FileName      string
	FilePath      string
	Date          string
	WorkType      string
	Variety       string
	Subphase      string
	Station       string
	Remarks       string
	Description   string
	HasBoard      bool
	DetectedText  string
	Measurements  string
	PhotoCategory string
	FocusTarget   string
	Reasoning     string
	Group         uint32
}

// isExcludedFolder reports whether the path contains an excluded folder name.
func isExcludedFolder(path string) bool {
	lower := strings.ToLower(path)
	for _, pat := range excludeFolderPatterns {
		if strings.Contains(lower, strings.ToLower(pat)) {
			return true
		}
	}
	return false
}

// ScanImages walks the directory and returns discovered image files, sorted by
// file name. Mirrors scan_folder_full in scanner/mod.rs.
func ScanImages(ctx context.Context, folder string, recursive, includeAll bool) ([]ImageInfo, error) {
	if _, err := os.Stat(folder); os.IsNotExist(err) {
		return nil, fmt.Errorf("folder not found: %s", folder)
	}

	var images []ImageInfo

	walkFn := func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return nil // skip unreadable entries
		}
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}

		if d.IsDir() {
			// Skip sub-directories when not recursive (but keep root itself).
			if !recursive && path != folder {
				return fs.SkipDir
			}
			return nil
		}

		if isExcludedFolder(path) {
			return nil
		}

		ext := strings.ToLower(filepath.Ext(path))
		if !includeAll && !imageExtensions[ext] {
			return nil
		}
		if includeAll && ext == "" {
			return nil
		}

		images = append(images, ImageInfo{
			Path:     path,
			FileName: filepath.Base(path),
		})
		return nil
	}

	if err := filepath.WalkDir(folder, walkFn); err != nil {
		return nil, fmt.Errorf("walk %s: %w", folder, err)
	}

	sort.Slice(images, func(i, j int) bool {
		return images[i].FileName < images[j].FileName
	})
	return images, nil
}

// ExtractEXIFDates populates the Date field of each ImageInfo using the DLL.
// Photos that fail EXIF extraction keep Date == "".
// Runs with a worker pool capped at 8 goroutines in parallel.
func ExtractEXIFDates(ctx context.Context, images []ImageInfo) []ImageInfo {
	const workers = 8
	type job struct {
		idx  int
		path string
	}

	out := make([]ImageInfo, len(images))
	copy(out, images)

	jobs := make(chan job, len(images))
	for i, img := range images {
		jobs <- job{i, img.Path}
	}
	close(jobs)

	var wg sync.WaitGroup
	var mu sync.Mutex

	for w := 0; w < workers; w++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for j := range jobs {
				select {
				case <-ctx.Done():
					return
				default:
				}
				res, err := engine.ExtractEXIF(engine.EXIFConfig{FilePath: j.path})
				if err == nil && res.DateTime != "" {
					mu.Lock()
					out[j.idx].Date = res.DateTime
					mu.Unlock()
				}
			}
		}()
	}
	wg.Wait()
	return out
}

// Run executes the full pipeline: scan → EXIF → AI grouping → matching →
// normalization. Mirrors scan_and_analyze in analysis.rs.
func Run(ctx context.Context, cfg Config) ([]Result, error) {
	// 1. Scan images
	suffix := ""
	if cfg.Recursive {
		suffix = " (再帰)"
	}
	fmt.Printf("[1] 写真をスキャン中...%s\n", suffix)
	images, err := ScanImages(ctx, cfg.Folder, cfg.Recursive, cfg.IncludeAll)
	if err != nil {
		return nil, err
	}
	if len(images) == 0 {
		return nil, fmt.Errorf("no images found in %s", cfg.Folder)
	}
	fmt.Printf("✓ %d枚の写真を検出\n\n", len(images))

	// 2. EXIF extraction (parallel via DLL)
	images = ExtractEXIFDates(ctx, images)

	// 3. AI grouping via DLL ProcessImage
	fmt.Println("[2] photo-tagger実行中...")
	imgResult, err := engine.ProcessImage(engine.ImageConfig{
		Folder:     cfg.Folder,
		BatchSize:  cfg.BatchSize,
		WorkType:   cfg.WorkType,
		PhotoType:  cfg.PhotoType,
		Variety:    cfg.Variety,
		Station:    cfg.Station,
		UseCache:   cfg.UseCache,
		Recursive:  cfg.Recursive,
		IncludeAll: cfg.IncludeAll,
		PayPerUse:  cfg.PayPerUse,
	})
	if err != nil {
		return nil, fmt.Errorf("ProcessImage: %w", err)
	}
	fmt.Printf("✓ AI解析完了 (%d枚)\n\n", imgResult.PhotoCount)

	// 4. Build results from images + master matching
	folderName := filepath.Base(cfg.Folder)
	results := buildResults(images, cfg.Matcher, folderName)

	// 5. Station bulk-apply + normalization (mirrors normalize_results_with_station)
	if cfg.Station != "" {
		applyStation(results, cfg.Station)
	}

	// 6. Folder-specific corrections via rule set
	if len(cfg.FolderRules) > 0 {
		rs := rules.RuleSet{Rules: cfg.FolderRules}
		applyFolderRules(results, cfg.Folder, &rs)
	}

	fmt.Printf("✓ マスタ照合完了（%d枚）\n\n", len(results))
	return results, nil
}

// buildResults converts scanned images + AI match into a Result slice,
// sorted by date then filename. Mirrors convert_groups_to_results.
func buildResults(images []ImageInfo, m *matching.Matcher, folderName string) []Result {
	_ = folderName // used for domain corrections (future)
	results := make([]Result, 0, len(images))

	for _, img := range images {
		r := Result{
			FileName: img.FileName,
			FilePath: img.Path,
			Date:     img.Date,
		}

		// OCR station extraction from detected text
		kvs := ocr.ExtractKV(r.DetectedText)
		for _, kv := range kvs {
			if kv.Key == "場所" || kv.Key == "測点" {
				r.Station = ocr.NormalizeStation(kv.Value)
				break
			}
		}

		// Master matching
		if m != nil {
			mr, ok := m.Match(r.DetectedText, r.PhotoCategory)
			if ok {
				r.WorkType = mr.Entry.WorkType
				r.Variety = mr.Entry.Variety
				r.Subphase = mr.Entry.Subphase
				r.Remarks = mr.Entry.Remarks
			}
		}

		results = append(results, r)
	}

	// Sort by date then file name (mirrors Rust sort)
	sort.Slice(results, func(i, j int) bool {
		if results[i].Date != results[j].Date {
			return results[i].Date < results[j].Date
		}
		return results[i].FileName < results[j].FileName
	})
	return results
}

// applyStation bulk-applies a station value to results.
// Mirrors apply_station in analysis.rs:
//   - 安全管理/品質管理 → apply as date string
//   - 区画線工 → skip
//   - others → apply directly
func applyStation(results []Result, station string) {
	stationIsDate := strings.Contains(station, "月") && strings.Contains(station, "日")

	for i := range results {
		r := &results[i]
		switch r.PhotoCategory {
		case "安全管理", "品質管理":
			alreadyDate := strings.Contains(r.Station, "月") && strings.Contains(r.Station, "日")
			if !alreadyDate {
				if stationIsDate {
					r.Station = station
				} else {
					r.Station = dateToMonthDay(r.Date)
				}
			}
		default:
			if r.WorkType == "区画線工" {
				// Clear previously mis-set station
				if r.Station == station {
					r.Station = ""
				}
			} else {
				r.Station = station
			}
		}
	}
}

// dateToMonthDay converts an EXIF date string like "2026-02-09 23:34:47"
// to "2月9日". Mirrors date_to_month_day in master_matcher.rs.
func dateToMonthDay(date string) string {
	// Expect "YYYY-MM-DD ..." or "YYYY:MM:DD ..."
	date = strings.TrimSpace(date)
	if len(date) < 10 {
		return ""
	}
	// Normalise separator
	s := strings.ReplaceAll(date[:10], ":", "-")
	parts := strings.SplitN(s, "-", 3)
	if len(parts) != 3 {
		return ""
	}
	m := strings.TrimLeft(parts[1], "0")
	d := strings.TrimLeft(parts[2], "0")
	if m == "" || d == "" {
		return ""
	}
	return m + "月" + d + "日"
}

// applyFolderRules applies the first matching folder rule to every result.
func applyFolderRules(results []Result, folderPath string, rs *rules.RuleSet) {
	rule, ok := rs.Match(folderPath)
	if !ok {
		return
	}
	if rule.Apply == nil {
		return
	}
	for i := range results {
		r := &results[i]
		if rule.Apply.WorkType != nil {
			r.WorkType = *rule.Apply.WorkType
		}
		if rule.Apply.Variety != nil {
			r.Variety = *rule.Apply.Variety
		}
		if rule.Apply.Subphase != nil {
			r.Subphase = *rule.Apply.Subphase
		}
		if rule.Apply.Remarks != nil {
			r.Remarks = *rule.Apply.Remarks
		}
		if rule.Apply.PhotoCategory != nil {
			r.PhotoCategory = *rule.Apply.PhotoCategory
		}
		if rule.Apply.Station != nil {
			r.Station = *rule.Apply.Station
		}
	}
}
