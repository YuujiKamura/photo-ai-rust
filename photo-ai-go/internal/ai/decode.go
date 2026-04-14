package ai

import (
	"encoding/json"
	"fmt"
	"os"
)

// photoGroupsRecord is the per-file record stored in photo-groups.json.
// It mirrors pkg/tagger.GroupRecord (GroupCore embedded) without importing
// the tagger package (to avoid a dependency cycle).
type photoGroupsRecord struct {
	Role         string `json:"role"`
	MachineType  string `json:"machine_type"`
	MachineID    string `json:"machine_id"`
	HasBoard     bool   `json:"has_board"`
	DetectedText string `json:"detected_text,omitempty"`
	Description  string `json:"description,omitempty"`
	Group        uint32 `json:"group"`
	CapturedAt   *int64 `json:"captured_at,omitempty"`
}

// DecodeGroupsFile reads the photo-groups.json file at jsonPath and returns a
// map from filename to RawImageData.
//
// photo-groups.json schema (written by pkg/tagger.SaveGroupRecords):
//
//	{
//	  "IMG_0001.JPG": {
//	    "role": "...",
//	    "machine_type": "...",
//	    "machine_id": "...",
//	    "has_board": true,
//	    "detected_text": "黒板テキスト",
//	    "description": "scene description",
//	    "group": 1,
//	    "captured_at": 1700000000
//	  },
//	  ...
//	}
//
// The returned RawImageData is populated from the tagger fields:
//   - FileName        ← map key (filename)
//   - HasBoard        ← has_board
//   - DetectedText    ← detected_text
//   - SceneDescription← description
//   - Measurements, PhotoCategory are left empty (tagger does not produce these)
func DecodeGroupsFile(jsonPath string) (map[string]RawImageData, error) {
	data, err := os.ReadFile(jsonPath)
	if err != nil {
		return nil, fmt.Errorf("DecodeGroupsFile: read %q: %w", jsonPath, err)
	}
	return DecodeGroupsJSON(data)
}

// DecodeGroupsJSON parses the raw bytes of a photo-groups.json and returns a
// map from filename to RawImageData.  Separated from DecodeGroupsFile so that
// tests can pass a synthetic fixture without touching the filesystem.
func DecodeGroupsJSON(data []byte) (map[string]RawImageData, error) {
	var raw map[string]photoGroupsRecord
	if err := json.Unmarshal(data, &raw); err != nil {
		return nil, fmt.Errorf("DecodeGroupsJSON: %w", err)
	}

	out := make(map[string]RawImageData, len(raw))
	for filename, rec := range raw {
		out[filename] = RawImageData{
			FileName:         filename,
			HasBoard:         rec.HasBoard,
			DetectedText:     rec.DetectedText,
			SceneDescription: rec.Description,
		}
	}
	return out, nil
}
