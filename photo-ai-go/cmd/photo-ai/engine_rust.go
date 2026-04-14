// Package main — engine_rust.go
// runRustEngine contains the Rust-engine-backed analyze path, extracted verbatim
// from the original analyzeCmd.RunE handler so the routing in main.go stays clean.
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"github.com/YuujiKamura/photo-ai-go/pkg/engine"
	"github.com/spf13/cobra"
)

// runRustEngine is the Rust-engine-backed analyze path.
// Behaviour is identical to the original analyzeCmd handler.
func runRustEngine(cmd *cobra.Command, folder, destJSON, master string, flags analyzeFlagSet) error {
	cfg := engine.ImageConfig{
		Folder:      folder,
		BatchSize:   flags.batchSize,
		WorkType:    flags.workType,
		PhotoType:   flags.photoType,
		Variety:     flags.variety,
		Station:     flags.station,
		UseCache:    flags.useCache,
		Recursive:   flags.recursive,
		IncludeAll:  flags.includeAll,
		FolderRules: flags.folderRules,
		PayPerUse:   flags.payPerUse,
		OutputJSON:  destJSON,
	}

	result, err := engine.ProcessImage(cfg)
	if err != nil {
		return fmt.Errorf("analyze (tagging): %w", err)
	}
	fmt.Fprintf(os.Stderr, "タグ付け完了: %d枚\n", result.PhotoCount)

	if master == "" {
		fmt.Fprintf(os.Stderr, "マスタ指定がないため、タグ付け結果のみを保存します: %s\n", result.OutputJSON)
		return nil
	}

	// Stage 2: Master Matching
	fmt.Fprintln(os.Stderr, "マスタ照合を開始しています...")
	lineTypes, _ := cmd.Flags().GetString("line-types")
	folderRulesFlag, _ := cmd.Flags().GetString("folder-rules")

	matchedResults, err := engine.MatchMaster(result.OutputJSON, master, folder, lineTypes, folderRulesFlag)
	if err != nil {
		return fmt.Errorf("analyze (matching): %w", err)
	}

	matchedJSON := filepath.Join(folder, "matched-results.json")
	data, err := json.MarshalIndent(matchedResults, "", "  ")
	if err != nil {
		return fmt.Errorf("failed to serialize matched results: %w", err)
	}
	if err := os.WriteFile(matchedJSON, data, 0o644); err != nil {
		return fmt.Errorf("failed to save matched results: %w", err)
	}

	// Stage 3: Normalization
	fmt.Fprintln(os.Stderr, "正規化を開始しています...")
	finalResults, err := engine.Normalize(matchedJSON, folder, flags.station, folderRulesFlag)
	if err != nil {
		return fmt.Errorf("analyze (normalization): %w", err)
	}
	stampAnalysisMetadata(finalResults, analyzeMetadata{
		MasterPath: master,
		WorkType:   flags.workType,
		PhotoType:  flags.photoType,
		Variety:    flags.variety,
		PayPerUse:  flags.payPerUse,
	})

	finalData, err := json.MarshalIndent(finalResults, "", "  ")
	if err != nil {
		return fmt.Errorf("failed to serialize final result: %w", err)
	}
	if err := os.WriteFile(destJSON, finalData, 0o644); err != nil {
		return fmt.Errorf("failed to save final result: %w", err)
	}
	fmt.Fprintf(os.Stderr, "最終結果を保存: %s\n", destJSON)
	return nil
}

