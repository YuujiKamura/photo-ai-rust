package tagger

import (
	"testing"
)

func TestExtractJSONArray(t *testing.T) {
	tests := []struct {
		name    string
		input   string
		wantErr bool
	}{
		{
			name:  "raw array",
			input: `some text [{"file":"a.jpg","role":"test"}] trailing`,
		},
		{
			name:  "code block",
			input: "```json\n[{\"file\":\"a.jpg\"}]\n```",
		},
		{
			name:    "no array",
			input:   "no json here",
			wantErr: true,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := extractJSONArray(tt.input)
			if tt.wantErr {
				if err == nil {
					t.Fatal("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if result[0] != '[' {
				t.Fatalf("expected JSON array, got: %s", result)
			}
		})
	}
}

func TestExtractNo(t *testing.T) {
	tests := []struct {
		input string
		want  string
	}{
		{"No.1 測点", "No.1"},
		{"取付道路 No.12", "No.12"},
		{"NO.3 abc", "No.3"},
		{"nothing here", ""},
	}
	for _, tt := range tests {
		got := extractNo(tt.input)
		if got != tt.want {
			t.Errorf("extractNo(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

func TestAssignGroups(t *testing.T) {
	ts1 := int64(1000)
	ts2 := int64(1100)
	ts3 := int64(2000) // >300s gap from ts1

	records := GroupRecords{
		"a.jpg": &GroupRecord{
			GroupCore:  GroupCore{MachineID: "roller-1"},
			CapturedAt: &ts1,
		},
		"b.jpg": &GroupRecord{
			GroupCore:  GroupCore{MachineID: "roller-1"},
			CapturedAt: &ts2,
		},
		"c.jpg": &GroupRecord{
			GroupCore:  GroupCore{MachineID: "roller-1"},
			CapturedAt: &ts3,
		},
	}

	assignGroups(records)

	// a and b should be in the same group (100s gap < 300s)
	if records["a.jpg"].Group != records["b.jpg"].Group {
		t.Errorf("a.jpg and b.jpg should be same group, got %d and %d",
			records["a.jpg"].Group, records["b.jpg"].Group)
	}
	// c should be in a different group (900s gap > 300s)
	if records["a.jpg"].Group == records["c.jpg"].Group {
		t.Errorf("a.jpg and c.jpg should be different groups, both got %d",
			records["a.jpg"].Group)
	}
}

func TestGroupPrompt(t *testing.T) {
	prompt := GroupPrompt([]string{"a.jpg", "b.jpg"}, nil)
	if len(prompt) == 0 {
		t.Fatal("prompt should not be empty")
	}
	// Should contain filenames
	if !contains(prompt, "a.jpg") || !contains(prompt, "b.jpg") {
		t.Error("prompt should contain filenames")
	}
}

func TestGroupPromptWithVocabulary(t *testing.T) {
	prompt := GroupPrompt([]string{"a.jpg"}, []string{"タイヤローラー", "マカダムローラー"})
	if !contains(prompt, "タイヤローラー") {
		t.Error("prompt should contain vocabulary")
	}
}

func TestIsImage(t *testing.T) {
	tests := []struct {
		path string
		want bool
	}{
		{"photo.jpg", true},
		{"photo.JPEG", true},
		{"photo.png", true},
		{"photo.heic", true},
		{"doc.pdf", false},
		{"data.json", false},
	}
	for _, tt := range tests {
		if got := isImage(tt.path); got != tt.want {
			t.Errorf("isImage(%q) = %v, want %v", tt.path, got, tt.want)
		}
	}
}

func contains(s, sub string) bool {
	return len(s) >= len(sub) && searchString(s, sub)
}

func searchString(s, sub string) bool {
	for i := 0; i <= len(s)-len(sub); i++ {
		if s[i:i+len(sub)] == sub {
			return true
		}
	}
	return false
}
