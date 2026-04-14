// Package rules — folder_rules.go
//
// Supplementary helpers derived from src/folder_rules.rs that are not yet
// covered by rules.go.  All new exported symbols are prefixed with "Folder"
// to avoid collisions with the existing API.
//
// The existing rules.go already contains: FolderRule, LoadFolderRules,
// ApplyFolderRules, ExtractDekigataStation, RuleSet, etc.
// This file adds: folder-name → photo-category inference helpers.
package rules

import "strings"

// FolderCategoryEntry maps a set of folder-name keywords to a photo category.
type FolderCategoryEntry struct {
	// Keywords holds the substrings that must ALL appear in the folder name
	// for this entry to match (same AND logic as FolderRule.MatchAll).
	Keywords []string
	// PhotoCategory is the inferred photo_category value.
	PhotoCategory string
}

// defaultFolderCategories is the built-in keyword → category table derived
// from common pavement-work folder naming conventions observed in the Rust
// rule engine tests and production config.
var defaultFolderCategories = []FolderCategoryEntry{
	// 着手前・完成
	{Keywords: []string{"着手前"}, PhotoCategory: "着手前及び完成写真"},
	{Keywords: []string{"完成"}, PhotoCategory: "着手前及び完成写真"},
	{Keywords: []string{"撤去前後"}, PhotoCategory: "着手前及び完成写真"},
	// 施工状況
	{Keywords: []string{"施工状況"}, PhotoCategory: "施工状況写真"},
	{Keywords: []string{"舗設状況"}, PhotoCategory: "施工状況写真"},
	{Keywords: []string{"転圧状況"}, PhotoCategory: "施工状況写真"},
	{Keywords: []string{"朝礼"}, PhotoCategory: "安全管理写真"},
	// 品質管理
	{Keywords: []string{"品質管理"}, PhotoCategory: "品質管理写真"},
	{Keywords: []string{"温度管理"}, PhotoCategory: "品質管理写真"},
	{Keywords: []string{"出荷指示確認"}, PhotoCategory: "品質管理写真"},
	// 出来形管理
	{Keywords: []string{"出来形"}, PhotoCategory: "出来形管理写真"},
	{Keywords: []string{"切削出来形"}, PhotoCategory: "出来形管理写真"},
	// 安全管理
	{Keywords: []string{"安全管理"}, PhotoCategory: "安全管理写真"},
	{Keywords: []string{"KY", "活動"}, PhotoCategory: "安全管理写真"},
	// 使用材料
	{Keywords: []string{"使用材料"}, PhotoCategory: "使用材料写真"},
	{Keywords: []string{"材料検収"}, PhotoCategory: "使用材料写真"},
	// 区画線工
	{Keywords: []string{"区画線工"}, PhotoCategory: "施工状況写真"},
	// 舗装工
	{Keywords: []string{"舗装工"}, PhotoCategory: "施工状況写真"},
}

// FolderInferCategory returns the photo_category inferred from the folder name
// using the built-in keyword table.  Returns an empty string when no entry matches.
func FolderInferCategory(folderName string) string {
	for _, entry := range defaultFolderCategories {
		if folderKeywordsMatch(folderName, entry.Keywords) {
			return entry.PhotoCategory
		}
	}
	return ""
}

// FolderInferCategoryWith is like FolderInferCategory but uses a caller-supplied
// table, enabling unit tests to inject custom entries without touching global state.
func FolderInferCategoryWith(folderName string, table []FolderCategoryEntry) string {
	for _, entry := range table {
		if folderKeywordsMatch(folderName, entry.Keywords) {
			return entry.PhotoCategory
		}
	}
	return ""
}

// folderKeywordsMatch returns true when all keywords are found in folderName
// (case-sensitive, same semantics as FolderRule.MatchAll / matchesRule).
func folderKeywordsMatch(folderName string, keywords []string) bool {
	for _, kw := range keywords {
		if !strings.Contains(folderName, kw) {
			return false
		}
	}
	return true
}

// FolderApplyInferredCategory sets PhotoCategory on each result that has an
// empty PhotoCategory, using FolderInferCategory to derive the value from
// folderName.  Results with a pre-existing PhotoCategory are left untouched.
func FolderApplyInferredCategory(results []AnalysisResult, folderName string) []AnalysisResult {
	cat := FolderInferCategory(folderName)
	if cat == "" {
		return results
	}
	for i := range results {
		if results[i].PhotoCategory == "" {
			results[i].PhotoCategory = cat
		}
	}
	return results
}
