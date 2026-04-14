package tagger

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
)

const defaultModel = "gemini-3-pro-preview"

// ClassifyConfig holds configuration for the Gemini classify call.
// APIKey is unused (retained for API compat); classification now goes through
// cli-ai-analyzer.exe → Gemini CLI subscription (TimeBasedQuota).
type ClassifyConfig struct {
	APIKey          string // deprecated, ignored
	Model           string
	Vocabulary      []string
	CliAnalyzerPath string // absolute path to cli-ai-analyzer.exe (caller resolves)
}

func (c *ClassifyConfig) model() string {
	if c.Model != "" {
		return c.Model
	}
	return defaultModel
}

func (c *ClassifyConfig) cliPath() (string, error) {
	if c.CliAnalyzerPath != "" {
		return c.CliAnalyzerPath, nil
	}
	if v := os.Getenv("CLI_AI_ANALYZER_EXE"); v != "" {
		return v, nil
	}
	if p, err := exec.LookPath("cli-ai-analyzer"); err == nil {
		return p, nil
	}
	if p, err := exec.LookPath("cli-ai-analyzer.exe"); err == nil {
		return p, nil
	}
	return "", fmt.Errorf("cli-ai-analyzer.exe not found: set CliAnalyzerPath in ClassifyConfig, CLI_AI_ANALYZER_EXE env, or add to PATH")
}

// ClassifyGroupBatch sends images to Gemini CLI (via cli-ai-analyzer.exe) and
// returns classified items. Subscription-only path; no API key required.
func ClassifyGroupBatch(ctx context.Context, images []string, cfg *ClassifyConfig) ([]GroupItem, error) {
	cliPath, err := cfg.cliPath()
	if err != nil {
		return nil, err
	}

	filenames := make([]string, len(images))
	for i, p := range images {
		filenames[i] = filepath.Base(p)
	}
	prompt := GroupPrompt(filenames, cfg.Vocabulary)

	absImages := make([]string, len(images))
	for i, p := range images {
		abs, err := filepath.Abs(p)
		if err != nil {
			return nil, fmt.Errorf("abs path %s: %w", p, err)
		}
		absImages[i] = abs
	}

	args := []string{"analyze", "--prompt", prompt, "--json", "--model", cfg.model()}
	args = append(args, absImages...)

	cmd := exec.CommandContext(ctx, cliPath, args...)
	output, err := cmd.Output()
	if err != nil {
		stderr := ""
		if exitErr, ok := err.(*exec.ExitError); ok {
			stderr = string(exitErr.Stderr)
		}
		return nil, fmt.Errorf("cli-ai-analyzer failed: %w\nstderr: %s", err, stderr)
	}

	rawText := string(output)
	jsonStr, err := extractJSONArray(rawText)
	if err != nil {
		return nil, fmt.Errorf("no JSON array in response: %w\nraw: %s", err, rawText)
	}

	var items []GroupItem
	if err := json.Unmarshal([]byte(jsonStr), &items); err != nil {
		return nil, fmt.Errorf("failed to parse group JSON: %w\njson: %s", err, jsonStr)
	}

	return items, nil
}

// extractJSONArray finds the outermost JSON array in a string.
// Tries ```json block first, then raw [...].
func extractJSONArray(s string) (string, error) {
	if idx := strings.Index(s, "```json"); idx >= 0 {
		start := idx + len("```json")
		rest := s[start:]
		if end := strings.Index(rest, "```"); end >= 0 {
			return strings.TrimSpace(rest[:end]), nil
		}
	}
	lastBracket := strings.LastIndex(s, "]")
	if lastBracket < 0 {
		return "", fmt.Errorf("no JSON array found")
	}
	for i := lastBracket; i >= 0; i-- {
		if s[i] == '[' {
			candidate := s[i : lastBracket+1]
			var v json.RawMessage
			if json.Unmarshal([]byte(candidate), &v) == nil {
				return candidate, nil
			}
		}
	}
	return "", fmt.Errorf("no valid JSON array found")
}
