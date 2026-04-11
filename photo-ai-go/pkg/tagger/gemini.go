package tagger

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/google/generative-ai-go/genai"
	"google.golang.org/api/option"
)

const defaultModel = "gemini-2.0-flash"

// ClassifyConfig holds configuration for the Gemini classify call.
type ClassifyConfig struct {
	APIKey     string
	Model      string
	Vocabulary []string
}

func (c *ClassifyConfig) model() string {
	if c.Model != "" {
		return c.Model
	}
	return defaultModel
}

func (c *ClassifyConfig) apiKey() (string, error) {
	if c.APIKey != "" {
		return c.APIKey, nil
	}
	key := os.Getenv("GEMINI_API_KEY")
	if key == "" {
		return "", fmt.Errorf("GEMINI_API_KEY environment variable not set")
	}
	return key, nil
}

// ClassifyGroupBatch sends images to Gemini and returns classified items.
func ClassifyGroupBatch(ctx context.Context, images []string, cfg *ClassifyConfig) ([]GroupItem, error) {
	key, err := cfg.apiKey()
	if err != nil {
		return nil, err
	}

	client, err := genai.NewClient(ctx, option.WithAPIKey(key))
	if err != nil {
		return nil, fmt.Errorf("failed to create Gemini client: %w", err)
	}
	defer client.Close()

	model := client.GenerativeModel(cfg.model())
	model.SetTemperature(0.1)
	model.SetMaxOutputTokens(8192)

	// Build filenames for the prompt
	filenames := make([]string, len(images))
	for i, p := range images {
		filenames[i] = filepath.Base(p)
	}

	prompt := GroupPrompt(filenames, cfg.Vocabulary)

	// Build parts: text prompt + inline images
	parts := []genai.Part{genai.Text(prompt)}
	for _, imgPath := range images {
		data, err := os.ReadFile(imgPath)
		if err != nil {
			return nil, fmt.Errorf("reading image %s: %w", imgPath, err)
		}
		mime := detectMIMEType(imgPath, data)
		parts = append(parts, genai.Blob{MIMEType: mime, Data: data})
	}

	resp, err := model.GenerateContent(ctx, parts...)
	if err != nil {
		return nil, fmt.Errorf("Gemini API call failed: %w", err)
	}

	// Extract text from response
	rawText := extractResponseText(resp)
	if rawText == "" {
		return nil, fmt.Errorf("Gemini returned no text content")
	}

	// Extract JSON array from response
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

func extractResponseText(resp *genai.GenerateContentResponse) string {
	if resp == nil {
		return ""
	}
	for _, cand := range resp.Candidates {
		if cand.Content == nil {
			continue
		}
		for _, part := range cand.Content.Parts {
			if t, ok := part.(genai.Text); ok {
				return string(t)
			}
		}
	}
	return ""
}

// extractJSONArray finds the outermost JSON array in a string.
// Tries ```json block first, then raw [...].
func extractJSONArray(s string) (string, error) {
	// Try ```json ... ``` block
	if idx := strings.Index(s, "```json"); idx >= 0 {
		start := idx + len("```json")
		rest := s[start:]
		if end := strings.Index(rest, "```"); end >= 0 {
			return strings.TrimSpace(rest[:end]), nil
		}
	}
	// Fall back to raw [...] - search from end for robustness
	lastBracket := strings.LastIndex(s, "]")
	if lastBracket < 0 {
		return "", fmt.Errorf("no JSON array found")
	}
	for i := lastBracket; i >= 0; i-- {
		if s[i] == '[' {
			candidate := s[i : lastBracket+1]
			// Validate it's valid JSON
			var v json.RawMessage
			if json.Unmarshal([]byte(candidate), &v) == nil {
				return candidate, nil
			}
		}
	}
	return "", fmt.Errorf("no valid JSON array found")
}

// detectMIMEType returns the MIME type based on magic bytes or extension.
func detectMIMEType(path string, data []byte) string {
	if len(data) >= 3 {
		if data[0] == 0xFF && data[1] == 0xD8 {
			return "image/jpeg"
		}
		if data[0] == 0x89 && data[1] == 0x50 && data[2] == 0x4E {
			return "image/png"
		}
	}
	ext := strings.ToLower(filepath.Ext(path))
	switch ext {
	case ".png":
		return "image/png"
	case ".heic":
		return "image/heic"
	default:
		return "image/jpeg"
	}
}
