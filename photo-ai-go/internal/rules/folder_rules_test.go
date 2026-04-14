package rules

import (
	"testing"
)

// ---------------------------------------------------------------------------
// TestFolderInferCategory — golden tests with realistic Japanese folder names
// ---------------------------------------------------------------------------

func TestFolderInferCategory(t *testing.T) {
	tests := []struct {
		name       string
		folder     string
		wantCat    string
	}{
		// ── Golden tests (realistic construction-site folder names) ──────────
		{
			name:    "舗装工 folder → 施工状況写真",
			folder:  "01_舗装工",
			wantCat: "施工状況写真",
		},
		{
			name:    "区画線工 folder → 施工状況写真",
			folder:  "02_区画線工",
			wantCat: "施工状況写真",
		},
		{
			name:    "撤去前後 folder → 着手前及び完成写真",
			folder:  "撤去前後",
			wantCat: "着手前及び完成写真",
		},
		{
			name:    "品質管理 → 品質管理写真",
			folder:  "品質管理写真_温度管理",
			wantCat: "品質管理写真",
		},
		{
			name:    "出来形 → 出来形管理写真",
			folder:  "出来形管理_No5",
			wantCat: "出来形管理写真",
		},
		// ── Rust test-derived cases ──────────────────────────────────────────
		{
			name:    "着手前 → 着手前及び完成写真",
			folder:  "着手前写真フォルダ",
			wantCat: "着手前及び完成写真",
		},
		{
			name:    "完成 → 着手前及び完成写真",
			folder:  "完成写真",
			wantCat: "着手前及び完成写真",
		},
		{
			name:    "施工状況 → 施工状況写真",
			folder:  "表層工_施工状況",
			wantCat: "施工状況写真",
		},
		{
			name:    "朝礼 → 安全管理写真",
			folder:  "朝礼・KY活動",
			wantCat: "安全管理写真",
		},
		{
			name:    "出荷指示確認 → 品質管理写真",
			folder:  "出荷指示確認",
			wantCat: "品質管理写真",
		},
		// ── No-match case ────────────────────────────────────────────────────
		{
			name:    "unrelated folder → empty",
			folder:  "雑多なフォルダ",
			wantCat: "",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := FolderInferCategory(tt.folder)
			if got != tt.wantCat {
				t.Errorf("FolderInferCategory(%q) = %q, want %q", tt.folder, got, tt.wantCat)
			}
		})
	}
}

// ---------------------------------------------------------------------------
// TestFolderInferCategoryWith — custom table injection
// ---------------------------------------------------------------------------

func TestFolderInferCategoryWith(t *testing.T) {
	table := []FolderCategoryEntry{
		{Keywords: []string{"テスト工事"}, PhotoCategory: "施工状況写真"},
		{Keywords: []string{"サンプル", "出来形"}, PhotoCategory: "出来形管理写真"},
	}

	t.Run("single keyword match", func(t *testing.T) {
		got := FolderInferCategoryWith("テスト工事_着手前", table)
		if got != "施工状況写真" {
			t.Errorf("got %q, want %q", got, "施工状況写真")
		}
	})

	t.Run("multi keyword match (AND)", func(t *testing.T) {
		got := FolderInferCategoryWith("サンプル_出来形_No1", table)
		if got != "出来形管理写真" {
			t.Errorf("got %q, want %q", got, "出来形管理写真")
		}
	})

	t.Run("partial keyword → no match", func(t *testing.T) {
		got := FolderInferCategoryWith("サンプルのみ", table)
		if got != "" {
			t.Errorf("got %q, want empty", got)
		}
	})
}

// ---------------------------------------------------------------------------
// TestFolderApplyInferredCategory — integration with AnalysisResult slice
// ---------------------------------------------------------------------------

func TestFolderApplyInferredCategory(t *testing.T) {
	t.Run("sets category on results without one", func(t *testing.T) {
		results := []AnalysisResult{
			{FileName: "img1.jpg"},
			{FileName: "img2.jpg"},
		}
		results = FolderApplyInferredCategory(results, "品質管理写真_温度管理")
		for i, r := range results {
			if r.PhotoCategory != "品質管理写真" {
				t.Errorf("results[%d].PhotoCategory = %q, want %q", i, r.PhotoCategory, "品質管理写真")
			}
		}
	})

	t.Run("does not overwrite existing category", func(t *testing.T) {
		results := []AnalysisResult{
			{FileName: "img1.jpg", PhotoCategory: "安全管理写真"},
		}
		results = FolderApplyInferredCategory(results, "品質管理写真_温度管理")
		if results[0].PhotoCategory != "安全管理写真" {
			t.Errorf("PhotoCategory = %q, want %q", results[0].PhotoCategory, "安全管理写真")
		}
	})

	t.Run("no-match folder leaves results unchanged", func(t *testing.T) {
		results := []AnalysisResult{
			{FileName: "img1.jpg"},
		}
		results = FolderApplyInferredCategory(results, "不明なフォルダ名")
		if results[0].PhotoCategory != "" {
			t.Errorf("PhotoCategory = %q, want empty", results[0].PhotoCategory)
		}
	})
}

// ---------------------------------------------------------------------------
// TestFolderMatchAllSemantics — ensures multi-keyword AND logic matches rules.go
// ---------------------------------------------------------------------------

func TestFolderMatchAllSemantics(t *testing.T) {
	// Uses the same folderKeywordsMatch semantics as rules.go matchesRule.
	tests := []struct {
		name     string
		folder   string
		keywords []string
		want     bool
	}{
		{"all present", "舗装工_施工状況", []string{"舗装工", "施工状況"}, true},
		{"one missing", "舗装工", []string{"舗装工", "施工状況"}, false},
		{"empty keywords always match", "anything", []string{}, true},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := folderKeywordsMatch(tt.folder, tt.keywords)
			if got != tt.want {
				t.Errorf("folderKeywordsMatch(%q, %v) = %v, want %v", tt.folder, tt.keywords, got, tt.want)
			}
		})
	}
}
