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
	"github.com/YuujiKamura/photo-ai-go/internal/analyzer"
	"github.com/YuujiKamura/photo-ai-go/internal/matching"
	"github.com/YuujiKamura/photo-ai-go/internal/normalizer"
	"github.com/YuujiKamura/photo-ai-go/internal/ocr"
	"github.com/YuujiKamura/photo-ai-go/internal/pairing"
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
	// Matcher is the legacy bigram-only matcher (used when HierarchyMaster is nil).
	Matcher *matching.Matcher
	// HierarchyMaster is the canonical full two-phase matcher ported from Rust.
	// When set, it is used in preference to Matcher.
	HierarchyMaster *matching.HierarchyMaster
	// PairAfterFolder, when non-empty, enables pair-completion scanning after
	// the main analysis pass. Must point to the directory containing A* image files.
	PairAfterFolder string
	// ProcessImageFn, when non-nil, is called instead of engine.ProcessImage.
	// Inject a stub here in tests to exercise Run() without a real AI backend.
	ProcessImageFn func(engine.ImageConfig) (engine.ImageResult, error)
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
	// WorkTypeCandidates holds work types detected by the analyzer stage.
	// Populated before master matching so matchers can optionally scope their search.
	WorkTypeCandidates []string
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

// Run executes the full pipeline:
//
//	scan → EXIF → AI grouping → master match → normalize → pair completion → output
//
// Mirrors scan_and_analyze in analysis.rs.
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

	// 3. AI grouping via DLL ProcessImage (or injected stub for tests).
	fmt.Println("[2] photo-tagger実行中...")
	processImage := engine.ProcessImage
	if cfg.ProcessImageFn != nil {
		processImage = cfg.ProcessImageFn
	}
	imgResult, err := processImage(engine.ImageConfig{
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

	// 4a. Decode per-photo AI data from photo-groups.json.
	// OutputJSON is a file path produced by tagger.RunGrouping.
	var photoData map[string]ai.RawImageData
	if imgResult.OutputJSON != "" {
		var decodeErr error
		photoData, decodeErr = ai.DecodeGroupsFile(imgResult.OutputJSON)
		if decodeErr != nil {
			// Non-fatal: log and continue with empty map (matching still works
			// with empty DetectedText, just with lower precision).
			fmt.Printf("[pipeline] warn: AI decode skipped: %v\n", decodeErr)
			photoData = make(map[string]ai.RawImageData)
		}
	} else {
		photoData = make(map[string]ai.RawImageData)
	}

	// 4b. Analyzer stage: derive per-batch work-type candidates from all decoded
	// RawImageData.  Runs BEFORE master matching so results can optionally use
	// the candidates to restrict search scope.
	allRaw := make([]ai.RawImageData, 0, len(photoData))
	for _, rd := range photoData {
		allRaw = append(allRaw, rd)
	}
	batchWorkTypes := analyzer.DetectWorkTypes(allRaw)
	if len(batchWorkTypes) > 0 {
		fmt.Printf("[analyzer] detected work types: %v\n", batchWorkTypes)
	}

	// 4c. Build results from images + master matching.
	// Use HierarchyMaster (full OCR-aware two-phase scorer) when available,
	// falling back to the legacy bigram Matcher.
	folderName := filepath.Base(cfg.Folder)
	results := buildResults(images, photoData, batchWorkTypes, cfg.Matcher, cfg.HierarchyMaster, folderName)

	// 5. Station bulk-apply (mirrors normalize_results_with_station in analysis.rs)
	if cfg.Station != "" {
		applyStation(results, cfg.Station)
	}

	// 6. Normalizer pass: use StationParse to canonicalise station strings.
	// Mirrors the normalize phase in analysis.rs.
	normalizeStations(results)

	// 7. Folder-specific corrections via the full rules engine.
	// Replaces the previous partial reimplementation: now delegates to
	// rules.ApplyFolderRules which handles all 15 rule steps.
	if len(cfg.FolderRules) > 0 {
		// Snapshot WorkTypeCandidates before round-trip through rules package
		// (rules.AnalysisResult does not carry WorkTypeCandidates).
		candidatesSnapshot := make(map[string][]string, len(results))
		for _, r := range results {
			candidatesSnapshot[r.FileName] = r.WorkTypeCandidates
		}
		rulesResults := resultsToRulesSlice(results)
		rulesResults = rules.ApplyFolderRules(rulesResults, folderName, cfg.FolderRules)
		results = rulesSliceToResults(rulesResults)
		// Restore WorkTypeCandidates lost during rules conversion.
		for i := range results {
			results[i].WorkTypeCandidates = candidatesSnapshot[results[i].FileName]
		}
	}

	fmt.Printf("✓ マスタ照合完了（%d枚）\n\n", len(results))

	// 8. Pair completion pass (optional).
	// When PairAfterFolder is provided, scan for P* pair folders and log summary.
	// This mirrors handle_pair_completion in pair_completion.rs.
	if cfg.PairAfterFolder != "" {
		pairFolders, err := pairing.ScanPairFolders(cfg.PairAfterFolder)
		if err == nil && len(pairFolders) > 0 {
			fmt.Printf("[3] ペア完成スキャン: %d 組のペアフォルダを検出\n\n", len(pairFolders))
		}
	}

	return results, nil
}

// buildResults converts scanned images + AI match into a Result slice,
// sorted by date then filename. Mirrors convert_groups_to_results.
//
// photoData maps filename → RawImageData decoded from photo-groups.json.
// batchWorkTypes holds work types detected by the analyzer for the whole batch.
//
// Master matching priority:
//  1. hm (HierarchyMaster) — full OCR-aware two-phase scorer from master_matcher.go
//  2. m  (Matcher)         — legacy bigram-only scorer from matching.go
func buildResults(
	images []ImageInfo,
	photoData map[string]ai.RawImageData,
	batchWorkTypes []string,
	m *matching.Matcher,
	hm *matching.HierarchyMaster,
	folderName string,
) []Result {
	results := make([]Result, 0, len(images))

	for _, img := range images {
		r := Result{
			FileName: img.FileName,
			FilePath: img.Path,
			Date:     img.Date,
			// WorkTypeCandidates is shared for the batch (all photos in the same
			// tagger run share the same folder-level candidate set).
			WorkTypeCandidates: batchWorkTypes,
		}

		// Populate per-photo AI fields from decoded photo-groups.json.
		if rd, ok := photoData[img.FileName]; ok {
			r.HasBoard = rd.HasBoard
			r.DetectedText = rd.DetectedText
			r.Description = rd.SceneDescription
			// Measurements and PhotoCategory are not populated by the tagger;
			// they come from a Step1/Step2 AI pass when available.
		}

		// OCR station extraction from detected text.
		kvs := ocr.ExtractKV(r.DetectedText)
		for _, kv := range kvs {
			if kv.Key == "場所" || kv.Key == "測点" {
				r.Station = ocr.NormalizeStation(kv.Value)
				break
			}
		}

		// Master matching: prefer HierarchyMaster (full OCR-aware port).
		if hm != nil {
			var texts []string
			if r.DetectedText != "" {
				texts = []string{r.DetectedText}
			}
			if row := matching.MatchMasterFromDetectedTexts(hm, texts, folderName, r.FocusTarget); row != nil {
				r.WorkType = row.WorkType
				r.Variety = row.Variety
				r.Subphase = row.Subphase
				r.Remarks = row.Remarks
				r.PhotoCategory = row.PhotoType
			}
		} else if m != nil {
			// Fallback to legacy bigram matcher.
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

	// Sort by date then file name (mirrors Rust sort).
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

// normalizeStations applies normalizer.StationParse to canonicalise each result's
// Station field. Mirrors the normalize phase in analysis.rs where station strings
// go through Station::parse → to_display round-trip to reach canonical form.
func normalizeStations(results []Result) {
	for i := range results {
		if results[i].Station != "" {
			results[i].Station = normalizer.StationParse(results[i].Station).ToDisplay()
		}
	}
}

// resultsToRulesSlice converts pipeline.Result slice to rules.AnalysisResult slice.
// The two types mirror each other; the conversion bridges the two packages.
func resultsToRulesSlice(src []Result) []rules.AnalysisResult {
	out := make([]rules.AnalysisResult, len(src))
	for i, r := range src {
		out[i] = rules.AnalysisResult{
			FileName:      r.FileName,
			FilePath:      r.FilePath,
			PhotoCategory: r.PhotoCategory,
			WorkType:      r.WorkType,
			Variety:       r.Variety,
			Subphase:      r.Subphase,
			Remarks:       r.Remarks,
			Station:       r.Station,
			Measurements:  r.Measurements,
			FocusTarget:   r.FocusTarget,
			DetectedText:  r.DetectedText,
			Group:         r.Group,
		}
	}
	return out
}

// rulesSliceToResults converts rules.AnalysisResult slice back to pipeline.Result slice.
func rulesSliceToResults(src []rules.AnalysisResult) []Result {
	out := make([]Result, len(src))
	for i, r := range src {
		out[i] = Result{
			FileName:      r.FileName,
			FilePath:      r.FilePath,
			PhotoCategory: r.PhotoCategory,
			WorkType:      r.WorkType,
			Variety:       r.Variety,
			Subphase:      r.Subphase,
			Remarks:       r.Remarks,
			Station:       r.Station,
			Measurements:  r.Measurements,
			FocusTarget:   r.FocusTarget,
			DetectedText:  r.DetectedText,
			Group:         r.Group,
		}
	}
	return out
}
