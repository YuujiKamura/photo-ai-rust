package ai

import (
	"encoding/json"
	"testing"
)

// fixtureGroupsJSON is a minimal photo-groups.json fixture.
// Filename keys use ダミー names (no proprietary names).
const fixtureGroupsJSON = `{
  "ダミー001.JPG": {
    "role": "main",
    "machine_type": "ローラー",
    "machine_id": "machine-1",
    "has_board": true,
    "detected_text": "舗装工\nNo.1+00",
    "description": "アスファルトフィニッシャーで舗設中の全景",
    "group": 1,
    "captured_at": 1700000000
  },
  "ダミー002.JPG": {
    "role": "board",
    "machine_type": "",
    "machine_id": "machine-1",
    "has_board": true,
    "detected_text": "到着温度 160℃",
    "description": "黒板アップ",
    "group": 1
  },
  "ダミー003.JPG": {
    "role": "sub",
    "machine_type": "",
    "machine_id": "machine-2",
    "has_board": false,
    "detected_text": "",
    "description": "区画線施工状況",
    "group": 2
  }
}`

func TestDecodeGroupsJSON_Basic(t *testing.T) {
	result, err := DecodeGroupsJSON([]byte(fixtureGroupsJSON))
	if err != nil {
		t.Fatalf("DecodeGroupsJSON error: %v", err)
	}

	if got, want := len(result), 3; got != want {
		t.Fatalf("len(result) = %d, want %d", got, want)
	}

	// Verify ダミー001.JPG
	r1, ok := result["ダミー001.JPG"]
	if !ok {
		t.Fatal("ダミー001.JPG not found in result")
	}
	if r1.FileName != "ダミー001.JPG" {
		t.Errorf("FileName = %q, want %q", r1.FileName, "ダミー001.JPG")
	}
	if !r1.HasBoard {
		t.Error("HasBoard should be true for ダミー001.JPG")
	}
	if r1.DetectedText != "舗装工\nNo.1+00" {
		t.Errorf("DetectedText = %q, want %q", r1.DetectedText, "舗装工\nNo.1+00")
	}
	if r1.SceneDescription != "アスファルトフィニッシャーで舗設中の全景" {
		t.Errorf("SceneDescription = %q, want %q", r1.SceneDescription, "アスファルトフィニッシャーで舗設中の全景")
	}
	if r1.Measurements != "" {
		t.Errorf("Measurements should be empty, got %q", r1.Measurements)
	}
	if r1.PhotoCategory != "" {
		t.Errorf("PhotoCategory should be empty, got %q", r1.PhotoCategory)
	}

	// Verify ダミー002.JPG (board photo)
	r2 := result["ダミー002.JPG"]
	if r2.DetectedText != "到着温度 160℃" {
		t.Errorf("ダミー002 DetectedText = %q, want %q", r2.DetectedText, "到着温度 160℃")
	}

	// Verify ダミー003.JPG (no board)
	r3 := result["ダミー003.JPG"]
	if r3.HasBoard {
		t.Error("HasBoard should be false for ダミー003.JPG")
	}
	if r3.SceneDescription != "区画線施工状況" {
		t.Errorf("ダミー003 SceneDescription = %q, want %q", r3.SceneDescription, "区画線施工状況")
	}
}

func TestDecodeGroupsJSON_Empty(t *testing.T) {
	result, err := DecodeGroupsJSON([]byte(`{}`))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(result) != 0 {
		t.Errorf("expected empty map, got %d entries", len(result))
	}
}

func TestDecodeGroupsJSON_InvalidJSON(t *testing.T) {
	_, err := DecodeGroupsJSON([]byte(`not-json`))
	if err == nil {
		t.Fatal("expected error for invalid JSON, got nil")
	}
}

func TestDecodeGroupsJSON_RoundTrip(t *testing.T) {
	// Build a GroupRecords equivalent and encode it, then decode.
	type groupRecord struct {
		Role         string `json:"role"`
		MachineType  string `json:"machine_type"`
		MachineID    string `json:"machine_id"`
		HasBoard     bool   `json:"has_board"`
		DetectedText string `json:"detected_text,omitempty"`
		Description  string `json:"description,omitempty"`
		Group        uint32 `json:"group"`
	}

	records := map[string]groupRecord{
		"ダミーA.jpg": {
			HasBoard:     true,
			DetectedText: "舗装工 ダミー測点",
			Description:  "ダミー施工現場",
			Group:        1,
		},
	}

	data, err := json.Marshal(records)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	result, err := DecodeGroupsJSON(data)
	if err != nil {
		t.Fatalf("DecodeGroupsJSON: %v", err)
	}

	r := result["ダミーA.jpg"]
	if r.FileName != "ダミーA.jpg" {
		t.Errorf("FileName = %q, want ダミーA.jpg", r.FileName)
	}
	if !r.HasBoard {
		t.Error("HasBoard should be true")
	}
	if r.DetectedText != "舗装工 ダミー測点" {
		t.Errorf("DetectedText = %q", r.DetectedText)
	}
}
