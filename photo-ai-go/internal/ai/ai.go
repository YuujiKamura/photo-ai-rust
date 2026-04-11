// Package ai provides the Gemini API client for photo analysis.
// Photo tagging is now handled by pkg/tagger (pure Go).
// TODO: Port remaining analysis logic from src/analyzer/*.rs
package ai


// Config holds Gemini API configuration.
type Config struct {
	// APIKey is the Gemini API key. If empty, the GEMINI_API_KEY environment
	// variable is used.
	APIKey string
	// Model is the Gemini model identifier, e.g. "gemini-1.5-pro".
	Model string
	// PayPerUse enables per-request billing mode (vs. free-tier usage).
	PayPerUse bool
}

// ImageAnalysisRequest is sent to Gemini for a batch of images.
type ImageAnalysisRequest struct {
	ImagePaths []string
	WorkType   string
	PhotoType  string
	Prompt     string
}

// ImageAnalysisResponse contains raw AI output for one image.
type ImageAnalysisResponse struct {
	FileName string
	RawJSON  string
	Error    string
}

// Client is the Gemini API client.
// TODO: implement using the Gemini REST or gRPC API.
type Client struct {
	cfg Config
}

// NewClient creates a new Gemini API client.
func NewClient(cfg Config) *Client {
	return &Client{cfg: cfg}
}

