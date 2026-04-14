// Package main — engine_go.go
// runGoEngine contains the Go-native pipeline path for the analyze command.
// It calls internal/pipeline.Run and serialises []pipeline.Result to the same
// result.json schema that the Rust path produces.
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"

	"github.com/YuujiKamura/photo-ai-go/internal/pipeline"
	"github.com/YuujiKamura/photo-ai-go/pkg/engine"
)

// runGoEngine executes the Go-native pipeline and writes result.json.
// The output schema mirrors engine.AnalysisResult so downstream consumers
// (export pdf/excel, pair) can work with either engine's output unchanged.
func runGoEngine(folder, destJSON string, flags analyzeFlagSet) error {
	cfg := pipeline.Config{
		Folder:     folder,
		BatchSize:  flags.batchSize,
		WorkType:   flags.workType,
		PhotoType:  flags.photoType,
		Variety:    flags.variety,
		Station:    flags.station,
		UseCache:   flags.useCache,
		Recursive:  flags.recursive,
		IncludeAll: flags.includeAll,
		PayPerUse:  flags.payPerUse,
		// FolderRules: populated from file below when the flag is set.
		// Matcher / HierarchyMaster: nil — caller may set via environment or future flag.
	}

	results, err := pipeline.Run(context.Background(), cfg)
	if err != nil {
		return fmt.Errorf("go engine: %w", err)
	}

	// Convert pipeline.Result → engine.AnalysisResult so the JSON output matches
	// the established result.json schema.
	engineResults := pipelineResultsToEngineResults(results)

	data, err := json.MarshalIndent(engineResults, "", "  ")
	if err != nil {
		return fmt.Errorf("go engine: marshal results: %w", err)
	}
	if err := os.WriteFile(destJSON, data, 0o644); err != nil {
		return fmt.Errorf("go engine: write %s: %w", destJSON, err)
	}
	fmt.Fprintf(os.Stderr, "[go engine] 完了: %d枚 → %s\n", len(engineResults), destJSON)
	return nil
}

// pipelineResultsToEngineResults converts the Go-native pipeline output to the
// canonical engine.AnalysisResult schema used by result.json.
// Fields absent from pipeline.Result are left at their zero values; they will
// be populated once Stream H completes the pipeline port (see repo issue #165).
func pipelineResultsToEngineResults(src []pipeline.Result) []engine.AnalysisResult {
	out := make([]engine.AnalysisResult, len(src))
	for i, r := range src {
		out[i] = engine.AnalysisResult{
			FileName:      r.FileName,
			FilePath:      r.FilePath,
			Date:          r.Date,
			WorkType:      r.WorkType,
			Variety:       r.Variety,
			Subphase:      r.Subphase,
			Station:       r.Station,
			Remarks:       r.Remarks,
			Description:   r.Description,
			HasBoard:      r.HasBoard,
			DetectedText:  r.DetectedText,
			Measurements:  r.Measurements,
			PhotoCategory: r.PhotoCategory,
			Reasoning:     r.Reasoning,
			FocusTarget:   r.FocusTarget,
			Group:         r.Group,
			// AnalysisTransport marks origin so consumers can distinguish engine outputs.
			AnalysisTransport: "go_pipeline",
		}
	}
	return out
}
