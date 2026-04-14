package tagger

import (
	"context"
	"fmt"
	"path/filepath"
	"time"
)

const defaultBatchSize = 10

// Config holds parameters for RunGrouping.
type Config struct {
	Folder          string
	BatchSize       int
	PayPerUse       bool
	APIKey          string
	Model           string
	Vocabulary      []string
	CliAnalyzerPath string // absolute path to cli-ai-analyzer.exe (resolved by caller)
}

// Result is returned by RunGrouping.
type Result struct {
	PhotoCount int
	OutputJSON string
}

// RunGrouping scans a folder, classifies pending images via Gemini,
// assigns groups, and saves photo-groups.json.
func RunGrouping(ctx context.Context, cfg Config) (Result, error) {
	if cfg.BatchSize <= 0 {
		cfg.BatchSize = defaultBatchSize
	}

	fmt.Printf("[photo-tagger] run_grouping:start folder=%s batch_size=%d\n", cfg.Folder, cfg.BatchSize)

	records := LoadGroupRecords(cfg.Folder)
	fmt.Printf("[photo-tagger] loaded_records count=%d\n", len(records))

	images := CollectImagesFlat(cfg.Folder)
	fmt.Printf("[photo-tagger] collected_images count=%d\n", len(images))

	captureTimes := collectCaptureTimes(images)

	if len(images) == 0 {
		fmt.Println("[photo-tagger] no images found")
		return Result{PhotoCount: 0, OutputJSON: filepath.Join(cfg.Folder, "photo-groups.json")}, nil
	}

	forceReclassify := forceReclassifyEnabled()

	// Determine pending images
	var pending []string
	if forceReclassify {
		pending = images
	} else {
		for _, img := range images {
			fname := filepath.Base(img)
			if _, exists := records[fname]; !exists {
				pending = append(pending, img)
			}
		}
	}

	skip := len(images) - len(pending)
	if skip > 0 {
		fmt.Printf("[photo-tagger] skipping %d already grouped\n", skip)
	}

	if len(pending) > 0 {
		classifyCfg := &ClassifyConfig{
			APIKey:          cfg.APIKey,
			Model:           cfg.Model,
			Vocabulary:      cfg.Vocabulary,
			CliAnalyzerPath: cfg.CliAnalyzerPath,
		}

		start := time.Now()
		for batchIdx := 0; batchIdx*cfg.BatchSize < len(pending); batchIdx++ {
			lo := batchIdx * cfg.BatchSize
			hi := lo + cfg.BatchSize
			if hi > len(pending) {
				hi = len(pending)
			}
			batch := pending[lo:hi]

			fmt.Printf("[photo-tagger] batch %d: %d images\n", batchIdx+1, len(batch))

			items, err := ClassifyGroupBatch(ctx, batch, classifyCfg)
			if err != nil {
				return Result{}, fmt.Errorf("classify batch %d: %w", batchIdx+1, err)
			}

			for _, item := range items {
				records[item.File] = &GroupRecord{
					GroupCore: item.GroupCore,
					Group:     0,
				}
			}
		}
		fmt.Printf("[photo-tagger] classify done in %s\n", fmtDuration(time.Since(start)))
	}

	applyCaptureTimes(records, captureTimes)
	assignGroups(records)

	if err := SaveGroupRecords(cfg.Folder, records); err != nil {
		return Result{}, fmt.Errorf("save group records: %w", err)
	}

	fmt.Printf("[photo-tagger] done count=%d\n", len(records))

	return Result{
		PhotoCount: len(records),
		OutputJSON: filepath.Join(cfg.Folder, "photo-groups.json"),
	}, nil
}
