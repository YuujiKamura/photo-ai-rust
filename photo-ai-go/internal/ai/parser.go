package ai

import (
	"encoding/json"
	"fmt"
	"strings"
)

// RawImageData holds Step1 image recognition output.
// Mirrors prompts.rs RawImageData (camelCase JSON keys).
type RawImageData struct {
	FileName        string `json:"fileName"`
	HasBoard        bool   `json:"hasBoard"`
	DetectedText    string `json:"detectedText"`
	Measurements    string `json:"measurements"`
	SceneDescription string `json:"sceneDescription"`
	PhotoCategory   string `json:"photoCategory"`
}

// AnalysisResult holds the final per-image analysis result.
// Mirrors types.rs AnalysisResult (camelCase JSON keys).
type AnalysisResult struct {
	FileName          string            `json:"fileName"`
	FilePath          string            `json:"filePath,omitempty"`
	Date              string            `json:"date,omitempty"`
	WorkType          string            `json:"workType,omitempty"`
	Variety           string            `json:"variety,omitempty"`
	Subphase          string            `json:"subphase,omitempty"`
	Station           string            `json:"station,omitempty"`
	Remarks           string            `json:"remarks,omitempty"`
	RemarksCandidates []string          `json:"remarksCandidates,omitempty"`
	Description       string            `json:"description,omitempty"`
	HasBoard          bool              `json:"hasBoard,omitempty"`
	DetectedText      string            `json:"detectedText,omitempty"`
	Measurements      string            `json:"measurements,omitempty"`
	PhotoCategory     string            `json:"photoCategory,omitempty"`
	Reasoning         string            `json:"reasoning,omitempty"`
	FocusTarget       string            `json:"focusTarget,omitempty"`
	Skip              bool              `json:"skip,omitempty"`
	Group             uint32            `json:"group,omitempty"`
	LabelOverrides    map[string]string `json:"labelOverrides,omitempty"`
}

// ExtractJSON extracts the JSON array from an AI response string.
// Extraction priority:
//  1. ```json ... ``` code block
//  2. Raw [...] array
//  3. Error
//
// Ported from parser.rs: extract_json.
func ExtractJSON(response string) (string, error) {
	// Try ```json ... ``` block first.
	if idx := strings.Index(response, "```json"); idx >= 0 {
		start := idx + len("```json")
		rest := response[start:]
		if end := strings.Index(rest, "```"); end >= 0 {
			return strings.TrimSpace(rest[:end]), nil
		}
	}

	// Fall back to raw [...] array.
	start := strings.Index(response, "[")
	end := strings.LastIndex(response, "]")
	if start >= 0 && end >= start {
		return response[start : end+1], nil
	}

	return "", fmt.Errorf("JSONが見つかりません")
}

// ParseStep1Response parses a Gemini response into a slice of RawImageData.
// Ported from parser.rs: parse_step1_response.
func ParseStep1Response(response string) ([]RawImageData, error) {
	jsonStr, err := ExtractJSON(response)
	if err != nil {
		return nil, fmt.Errorf("Step1 JSONパースエラー: %w", err)
	}
	var results []RawImageData
	if err := json.Unmarshal([]byte(strings.TrimSpace(jsonStr)), &results); err != nil {
		return nil, fmt.Errorf("Step1 JSONパースエラー: %w", err)
	}
	return results, nil
}

// ParseSingleStepResponse parses a Gemini response into a slice of AnalysisResult.
// Ported from parser.rs: parse_single_step_response.
func ParseSingleStepResponse(response string) ([]AnalysisResult, error) {
	jsonStr, err := ExtractJSON(response)
	if err != nil {
		return nil, fmt.Errorf("1ステップ解析 JSONパースエラー: %w", err)
	}
	var results []AnalysisResult
	if err := json.Unmarshal([]byte(strings.TrimSpace(jsonStr)), &results); err != nil {
		return nil, fmt.Errorf("1ステップ解析 JSONパースエラー: %w", err)
	}
	return results, nil
}
