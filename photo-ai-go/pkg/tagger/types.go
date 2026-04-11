// Package tagger classifies and groups construction site photos using Gemini AI.
// Ported from deps/photo-tagger (Rust).
package tagger

// GroupCore holds the AI-classified fields for a single photo.
type GroupCore struct {
	Role         string `json:"role"`
	MachineType  string `json:"machine_type"`
	MachineID    string `json:"machine_id"`
	HasBoard     bool   `json:"has_board"`
	DetectedText string `json:"detected_text,omitempty"`
	Description  string `json:"description,omitempty"`
}

// GroupItem is the intermediate parse target from Gemini JSON output.
type GroupItem struct {
	File string `json:"file"`
	GroupCore
}

// GroupRecord is a classified photo with group assignment and capture time.
type GroupRecord struct {
	GroupCore
	Group      uint32 `json:"group"`
	CapturedAt *int64 `json:"captured_at,omitempty"`
}

// GroupRecords maps filename -> GroupRecord.
type GroupRecords map[string]*GroupRecord
