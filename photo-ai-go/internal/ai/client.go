// Package ai provides the Gemini API client for photo analysis.
package ai

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"math"
	"net/http"
	"os"
	"time"
)

const (
	geminiAPIBase      = "https://generativelanguage.googleapis.com/v1beta/models"
	defaultModel       = "gemini-2.0-flash"
	maxRetries         = 3
	initialBackoff     = 1 * time.Second
)

// geminiRequest is the JSON body sent to the Gemini generateContent endpoint.
type geminiRequest struct {
	Contents         []geminiContent        `json:"contents"`
	GenerationConfig *geminiGenerationConfig `json:"generationConfig,omitempty"`
}

type geminiContent struct {
	Parts []geminiPart `json:"parts"`
}

type geminiPart struct {
	Text       string            `json:"text,omitempty"`
	InlineData *geminiInlineData `json:"inlineData,omitempty"`
}

type geminiInlineData struct {
	MimeType string `json:"mimeType"`
	Data     string `json:"data"` // base64-encoded
}

type geminiGenerationConfig struct {
	Temperature     float64 `json:"temperature,omitempty"`
	MaxOutputTokens int     `json:"maxOutputTokens,omitempty"`
}

// geminiResponse is the JSON response from the Gemini generateContent endpoint.
type geminiResponse struct {
	Candidates []struct {
		Content struct {
			Parts []struct {
				Text string `json:"text"`
			} `json:"parts"`
		} `json:"content"`
		FinishReason string `json:"finishReason"`
	} `json:"candidates"`
	Error *struct {
		Code    int    `json:"code"`
		Message string `json:"message"`
		Status  string `json:"status"`
	} `json:"error"`
}

// apiKey returns the Gemini API key, preferring cfg.APIKey over env var.
func (c *Client) apiKey() (string, error) {
	if c.cfg.APIKey != "" {
		return c.cfg.APIKey, nil
	}
	key := os.Getenv("GEMINI_API_KEY")
	if key == "" {
		return "", fmt.Errorf("GEMINI_API_KEY environment variable not set")
	}
	return key, nil
}

// model returns the configured model, falling back to defaultModel.
func (c *Client) model() string {
	if c.cfg.Model != "" {
		return c.cfg.Model
	}
	return defaultModel
}

// AnalyzeBatch sends a batch of images to Gemini and returns the raw text response.
// imagePaths must be readable local file paths.
func (c *Client) AnalyzeBatch(ctx context.Context, req ImageAnalysisRequest) ([]ImageAnalysisResponse, error) {
	key, err := c.apiKey()
	if err != nil {
		return nil, err
	}

	// Build the request parts: text prompt first, then image data.
	parts := []geminiPart{
		{Text: req.Prompt},
	}

	for _, path := range req.ImagePaths {
		data, mimeType, readErr := readImageFile(path)
		if readErr != nil {
			return nil, fmt.Errorf("reading image %s: %w", path, readErr)
		}
		parts = append(parts, geminiPart{
			InlineData: &geminiInlineData{
				MimeType: mimeType,
				Data:     base64.StdEncoding.EncodeToString(data),
			},
		})
	}

	body := geminiRequest{
		Contents: []geminiContent{
			{Parts: parts},
		},
		GenerationConfig: &geminiGenerationConfig{
			Temperature:     0.1,
			MaxOutputTokens: 8192,
		},
	}

	rawText, err := c.postWithRetry(ctx, key, body)
	if err != nil {
		return nil, err
	}

	// Return as a single aggregated response; parsing is done by parser.go.
	return []ImageAnalysisResponse{
		{RawJSON: rawText},
	}, nil
}

// postWithRetry posts to the Gemini API with exponential backoff on rate-limit errors.
func (c *Client) postWithRetry(ctx context.Context, key string, body geminiRequest) (string, error) {
	url := fmt.Sprintf("%s/%s:generateContent?key=%s", geminiAPIBase, c.model(), key)

	bodyBytes, err := json.Marshal(body)
	if err != nil {
		return "", fmt.Errorf("marshaling request: %w", err)
	}

	var lastErr error
	for attempt := 0; attempt < maxRetries; attempt++ {
		if attempt > 0 {
			backoff := time.Duration(math.Pow(2, float64(attempt-1))) * initialBackoff
			select {
			case <-ctx.Done():
				return "", ctx.Err()
			case <-time.After(backoff):
			}
		}

		text, retry, err := c.doPost(ctx, url, bodyBytes)
		if err == nil {
			return text, nil
		}
		lastErr = err
		if !retry {
			break
		}
	}
	return "", lastErr
}

// doPost performs a single HTTP POST to the Gemini API.
// Returns (text, shouldRetry, error).
func (c *Client) doPost(ctx context.Context, url string, bodyBytes []byte) (string, bool, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(bodyBytes))
	if err != nil {
		return "", false, fmt.Errorf("creating request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return "", true, fmt.Errorf("HTTP request failed: %w", err)
	}
	defer resp.Body.Close()

	respBytes, err := io.ReadAll(resp.Body)
	if err != nil {
		return "", true, fmt.Errorf("reading response body: %w", err)
	}

	// Rate limiting: retry on 429.
	if resp.StatusCode == http.StatusTooManyRequests {
		return "", true, fmt.Errorf("rate limited (429)")
	}
	if resp.StatusCode >= 500 {
		return "", true, fmt.Errorf("server error %d: %s", resp.StatusCode, string(respBytes))
	}
	if resp.StatusCode != http.StatusOK {
		return "", false, fmt.Errorf("unexpected status %d: %s", resp.StatusCode, string(respBytes))
	}

	var gemResp geminiResponse
	if err := json.Unmarshal(respBytes, &gemResp); err != nil {
		return "", false, fmt.Errorf("parsing Gemini response: %w", err)
	}

	if gemResp.Error != nil {
		retry := gemResp.Error.Code == 429 || gemResp.Error.Code >= 500
		return "", retry, fmt.Errorf("Gemini API error %d %s: %s",
			gemResp.Error.Code, gemResp.Error.Status, gemResp.Error.Message)
	}

	if len(gemResp.Candidates) == 0 || len(gemResp.Candidates[0].Content.Parts) == 0 {
		return "", false, fmt.Errorf("Gemini returned no candidates")
	}

	return gemResp.Candidates[0].Content.Parts[0].Text, false, nil
}

// readImageFile reads an image file and returns its bytes and MIME type.
func readImageFile(path string) ([]byte, string, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, "", err
	}
	mimeType := detectMIMEType(path, data)
	return data, mimeType, nil
}

// detectMIMEType returns the MIME type based on file extension and magic bytes.
func detectMIMEType(path string, data []byte) string {
	// Check magic bytes first.
	if len(data) >= 3 {
		if data[0] == 0xFF && data[1] == 0xD8 {
			return "image/jpeg"
		}
		if data[0] == 0x89 && data[1] == 0x50 && data[2] == 0x4E {
			return "image/png"
		}
		if len(data) >= 4 && data[0] == 0x47 && data[1] == 0x49 && data[2] == 0x46 {
			return "image/gif"
		}
		if len(data) >= 4 && data[0] == 0x52 && data[1] == 0x49 && data[2] == 0x46 && data[3] == 0x46 {
			return "image/webp"
		}
	}
	// Fall back to extension.
	for i := len(path) - 1; i >= 0; i-- {
		if path[i] == '.' {
			ext := path[i+1:]
			switch ext {
			case "jpg", "jpeg", "JPG", "JPEG":
				return "image/jpeg"
			case "png", "PNG":
				return "image/png"
			case "gif", "GIF":
				return "image/gif"
			case "webp", "WEBP":
				return "image/webp"
			}
			break
		}
	}
	return "image/jpeg"
}
