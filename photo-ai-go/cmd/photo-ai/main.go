// Command photo-ai is the Go CLI frontend for the photo-ai engine.
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/spf13/cobra"

	"github.com/YuujiKamura/photo-ai-go/internal/export"
	"github.com/YuujiKamura/photo-ai-go/pkg/engine"
)

// analyzeFlagSet groups the parsed flag values for the analyze command so they
// can be passed cleanly to runRustEngine / runGoEngine without re-reading cobra.
type analyzeFlagSet struct {
	batchSize   int
	workType    string
	photoType   string
	variety     string
	station     string
	useCache    bool
	recursive   bool
	includeAll  bool
	folderRules string
	payPerUse   bool
}

var gitCommit = "unknown"

func main() {
	if err := rootCmd.Execute(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

var rootCmd = &cobra.Command{
	Use:   "photo-ai",
	Short: "工事写真AI解析・写真台帳生成ツール (Go frontend)",
	Long: `photo-ai — Construction photo AI analysis and photo ledger generation tool.

The Go frontend is the primary entrypoint. It delegates tagging, analysis,
PDF rendering, and Excel rendering to Rust engine executables.`,
	PersistentPreRun: func(cmd *cobra.Command, args []string) {
		engine.GitCommit = gitCommit
	},
	RunE: func(cmd *cobra.Command, args []string) error {
		// No subcommand given (e.g. double-click) → launch Web UI
		return serveCmd.RunE(serveCmd, args)
	},
}

func init() {
	rootCmd.AddCommand(analyzeCmd)
	rootCmd.AddCommand(exportCmd)
	rootCmd.AddCommand(pairCmd)
	rootCmd.AddCommand(serveCmd)

	exportCmd.AddCommand(exportPDFCmd)
	exportCmd.AddCommand(exportExcelCmd)

	// analyze flags
	analyzeCmd.Flags().StringP("output", "o", "", "Output JSON file (default: <folder>/result.json)")
	analyzeCmd.Flags().IntP("batch-size", "b", 5, "Number of photos per AI batch")
	analyzeCmd.Flags().StringP("master", "m", "", "Work-type master CSV/JSON file")
	analyzeCmd.Flags().StringP("work-type", "w", "", "Work type override (1-step analysis mode)")
	analyzeCmd.Flags().StringP("photo-type", "t", "", "Photo type override (e.g. 使用機械)")
	analyzeCmd.Flags().String("variety", "", "Variety override")
	analyzeCmd.Flags().StringP("station", "s", "", "Station override (applied to all photos)")
	analyzeCmd.Flags().Bool("use-cache", false, "Skip re-analysis of already-cached photos")
	analyzeCmd.Flags().BoolP("recursive", "r", false, "Scan sub-folders recursively")
	analyzeCmd.Flags().Bool("include-all", false, "Include folders normally excluded (e.g. 非使用)")
	analyzeCmd.Flags().String("line-types", "", "Line-type list JSON file (区画線工)")
	analyzeCmd.Flags().String("folder-rules", "", "Folder-rule override JSON file")
	analyzeCmd.Flags().Bool("pay-per-use", false, "Use pay-per-use billing instead of subscription/resident mode")
	analyzeCmd.Flags().String("engine", "rust", "Analysis engine to use: rust (Rust DLL path) or go (Go-native pipeline)")

	// export pdf flags
	exportPDFCmd.Flags().StringP("output", "o", "", "Output PDF file or directory")
	exportPDFCmd.Flags().IntP("photos-per-page", "p", 3, "Photos per page (2 or 3)")
	exportPDFCmd.Flags().String("quality", "medium", "PDF image quality (high/medium/low)")
	exportPDFCmd.Flags().String("preset", "", "Alias preset (pavement/marking/general)")
	exportPDFCmd.Flags().String("alias", "", "Custom alias JSON file")

	// export excel flags
	exportExcelCmd.Flags().StringP("output", "o", "", "Output Excel file or directory")
	exportExcelCmd.Flags().String("preset", "", "Alias preset (pavement/marking/general)")
	exportExcelCmd.Flags().String("alias", "", "Custom alias JSON file")

	// pair flags
	pairCmd.Flags().StringP("input", "i", "", "Input analysis result JSON (required)")
	pairCmd.Flags().StringP("output", "o", "", "Output paired JSON file")
}

// analyzeCmd runs the AI analysis pipeline on a photo folder.
var analyzeCmd = &cobra.Command{
	Use:   "analyze <folder>",
	Short: "写真フォルダを解析してJSONを出力",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		folder := args[0]

		outputFlag, _ := cmd.Flags().GetString("output")
		master, _ := cmd.Flags().GetString("master")
		engineName, _ := cmd.Flags().GetString("engine")

		// Validate --engine value.
		if engineName != "rust" && engineName != "go" {
			return fmt.Errorf("--engine must be 'rust' or 'go', got %q", engineName)
		}

		// Collect remaining flags into a typed struct shared by both engine paths.
		batchSize, _ := cmd.Flags().GetInt("batch-size")
		workType, _ := cmd.Flags().GetString("work-type")
		photoType, _ := cmd.Flags().GetString("photo-type")
		variety, _ := cmd.Flags().GetString("variety")
		station, _ := cmd.Flags().GetString("station")
		useCache, _ := cmd.Flags().GetBool("use-cache")
		recursive, _ := cmd.Flags().GetBool("recursive")
		includeAll, _ := cmd.Flags().GetBool("include-all")
		folderRules, _ := cmd.Flags().GetString("folder-rules")
		payPerUse, _ := cmd.Flags().GetBool("pay-per-use")

		flags := analyzeFlagSet{
			batchSize:   batchSize,
			workType:    workType,
			photoType:   photoType,
			variety:     variety,
			station:     station,
			useCache:    useCache,
			recursive:   recursive,
			includeAll:  includeAll,
			folderRules: folderRules,
			payPerUse:   payPerUse,
		}

		destJSON := resolveOutputJSON(folder, outputFlag)

		fmt.Fprintln(os.Stderr, "写真解析を開始しています...")

		if engineName == "go" {
			return runGoEngine(folder, destJSON, master, flags)
		}
		// Default: Rust engine path (identical to pre-refactor behaviour).
		return runRustEngine(cmd, folder, destJSON, master, flags)
	},
}

// exportCmd is the parent for pdf/excel sub-commands.
var exportCmd = &cobra.Command{
	Use:   "export",
	Short: "解析結果からPDFまたはExcelを生成",
}

// exportPDFCmd generates a PDF photo book from a result JSON.
var exportPDFCmd = &cobra.Command{
	Use:   "pdf <result.json>",
	Short: "解析結果からPDF写真帳を生成",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		inputJSON := args[0]

		outputFlag, _ := cmd.Flags().GetString("output")
		photosPerPage, _ := cmd.Flags().GetInt("photos-per-page")
		quality, _ := cmd.Flags().GetString("quality")
		preset, _ := cmd.Flags().GetString("preset")
		aliasFile, _ := cmd.Flags().GetString("alias")

		fmt.Fprintln(os.Stderr, "PDF生成を開始しています...")

		result, err := export.GeneratePDF(inputJSON, outputFlag, photosPerPage, quality, preset, aliasFile)
		if err != nil {
			return fmt.Errorf("export pdf: %w", err)
		}

		fmt.Fprintf(os.Stderr, "PDF生成完了: %s (%dページ)\n", result.OutputPath, result.PageCount)
		return nil
	},
}

// exportExcelCmd generates an Excel workbook from a result JSON.
var exportExcelCmd = &cobra.Command{
	Use:   "excel <result.json>",
	Short: "解析結果からExcelワークブックを生成",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		inputJSON := args[0]

		outputFlag, _ := cmd.Flags().GetString("output")
		preset, _ := cmd.Flags().GetString("preset")
		aliasFile, _ := cmd.Flags().GetString("alias")

		fmt.Fprintln(os.Stderr, "Excel生成を開始しています...")

		result, err := export.GenerateExcel(inputJSON, outputFlag, preset, aliasFile)
		if err != nil {
			return fmt.Errorf("export excel: %w", err)
		}

		fmt.Fprintf(os.Stderr, "Excel生成完了: %s (%dシート)\n", result.OutputPath, result.SheetCount)
		return nil
	},
}

// pairCmd runs the before/after pairing algorithm.
var pairCmd = &cobra.Command{
	Use:   "pair",
	Short: "施工前後の写真ペアリングを実行",
	RunE: func(cmd *cobra.Command, args []string) error {
		inputFlag, _ := cmd.Flags().GetString("input")
		outputFlag, _ := cmd.Flags().GetString("output")

		if inputFlag == "" {
			return fmt.Errorf("pair: --input is required")
		}

		fmt.Fprintln(os.Stderr, "ペアリングを開始しています...")

		results, err := export.LoadResults(inputFlag)
		if err != nil {
			return fmt.Errorf("pair: load results: %w", err)
		}

		paired := runPairing(results)

		destPath := outputFlag
		if destPath == "" {
			dir := filepath.Dir(inputFlag)
			destPath = filepath.Join(dir, "paired.json")
		}

		data, err := json.MarshalIndent(paired, "", "  ")
		if err != nil {
			return fmt.Errorf("pair: marshal: %w", err)
		}
		if err := os.WriteFile(destPath, data, 0o644); err != nil {
			return fmt.Errorf("pair: write %s: %w", destPath, err)
		}

		fmt.Fprintf(os.Stderr, "ペアリング完了: %dペア → %s\n", len(paired), destPath)
		return nil
	},
}

// PairedResult holds a before/after photo pair with a confidence score.
type PairedResult struct {
	Before engine.AnalysisResult `json:"before"`
	After  engine.AnalysisResult `json:"after"`
	Score  float64               `json:"score"`
}

// runPairing performs a simple subphase-based pairing over the results.
// A proper ensemble algorithm (see src/pair_ensemble.rs) is not yet ported;
// this stub groups photos by subphase keyword.
func runPairing(results []engine.AnalysisResult) []PairedResult {
	var before, after []engine.AnalysisResult
	for _, r := range results {
		switch r.Subphase {
		case "施工前", "着手前":
			before = append(before, r)
		case "施工後", "竣工":
			after = append(after, r)
		}
	}

	var pairs []PairedResult
	limit := len(before)
	if len(after) < limit {
		limit = len(after)
	}
	for i := 0; i < limit; i++ {
		pairs = append(pairs, PairedResult{
			Before: before[i],
			After:  after[i],
			Score:  1.0,
		})
	}
	return pairs
}

// resolveOutputJSON determines where the analysis result JSON should be written.
func resolveOutputJSON(folder, outputFlag string) string {
	if outputFlag != "" {
		return outputFlag
	}
	return filepath.Join(folder, export.DefaultResultFileName)
}

type analyzeMetadata struct {
	MasterPath string
	WorkType   string
	PhotoType  string
	Variety    string
	PayPerUse  bool
}

func stampAnalysisMetadata(results []engine.AnalysisResult, meta analyzeMetadata) {
	timestamp := time.Now().Format("2006-01-02 15:04:05 -0700")
	billing := "subscription"
	if meta.PayPerUse {
		billing = "pay_per_use"
	}
	masterSelection := "all"
	if meta.MasterPath != "" {
		masterSelection = "single"
	}

	for i := range results {
		results[i].AnalysisTimestamp = timestamp
		results[i].AnalysisProvider = "auto"
		results[i].AnalysisBilling = billing
		results[i].AnalysisTransport = "binary_engine"
		results[i].AnalysisCommit = gitCommit
		results[i].AnalysisMasterSelection = masterSelection
		results[i].AnalysisMasterPath = meta.MasterPath
		results[i].AnalysisScopeWorkType = meta.WorkType
		results[i].AnalysisScopePhotoType = meta.PhotoType
		results[i].AnalysisScopeVariety = meta.Variety
	}
}
