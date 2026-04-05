// Package alias transforms analysis result fields using configurable alias maps.
// Ported from src/normalizer/alias.rs and common/src/alias.rs
package alias

import "strings"

// Config holds alias maps for each transformable field.
type Config struct {
	PhotoCategory map[string]string `json:"photo_category"`
	WorkType      map[string]string `json:"work_type"`
	Variety       map[string]string `json:"variety"`
	Subphase      map[string]string `json:"subphase"`
}

// NewConfig returns an empty Config with initialised maps.
func NewConfig() Config {
	return Config{
		PhotoCategory: make(map[string]string),
		WorkType:      make(map[string]string),
		Variety:       make(map[string]string),
		Subphase:      make(map[string]string),
	}
}

// Merge merges other into c; other entries take precedence on conflict.
func (c *Config) Merge(other Config) {
	for k, v := range other.PhotoCategory {
		c.PhotoCategory[k] = v
	}
	for k, v := range other.WorkType {
		c.WorkType[k] = v
	}
	for k, v := range other.Variety {
		c.Variety[k] = v
	}
	for k, v := range other.Subphase {
		c.Subphase[k] = v
	}
}

// LongestMatchTransform applies alias map to value using exact-match first,
// then longest partial-match. Returns the original value if no match.
func LongestMatchTransform(value string, m map[string]string) string {
	if value == "" {
		return value
	}
	if rep, ok := m[value]; ok {
		return rep
	}
	bestLen := 0
	bestRep := ""
	for pattern, rep := range m {
		if strings.Contains(value, pattern) && len(pattern) > bestLen {
			bestLen = len(pattern)
			bestRep = rep
		}
	}
	if bestLen > 0 {
		return bestRep
	}
	return value
}

// AnalysisResult is the subset of fields that alias transformation touches.
type AnalysisResult struct {
	PhotoCategory string
	WorkType      string
	Variety       string
	Subphase      string
}

// Apply returns a copy of r with alias transformations applied.
func (c *Config) Apply(r AnalysisResult) AnalysisResult {
	return AnalysisResult{
		PhotoCategory: LongestMatchTransform(r.PhotoCategory, c.PhotoCategory),
		WorkType:      LongestMatchTransform(r.WorkType, c.WorkType),
		Variety:       LongestMatchTransform(r.Variety, c.Variety),
		Subphase:      LongestMatchTransform(r.Subphase, c.Subphase),
	}
}

// FromPreset returns a built-in alias Config for the named preset.
// Returns (Config, false) for unknown preset names; the Config will be empty.
func FromPreset(name string) (Config, bool) {
	switch strings.ToLower(name) {
	case "pavement", "舗装":
		return pavementPreset(), true
	case "marking", "区画線":
		return markingPreset(), true
	case "general", "汎用":
		return generalPreset(), true
	default:
		return NewConfig(), false
	}
}

func pavementPreset() Config {
	c := NewConfig()
	c.PhotoCategory["品質"] = "品質管理写真"
	c.PhotoCategory["品質管理"] = "品質管理写真"
	c.PhotoCategory["出来形"] = "出来形管理写真"
	c.PhotoCategory["出来形管理"] = "出来形管理写真"
	c.PhotoCategory["施工状況"] = "施工状況写真"
	c.PhotoCategory["施工中"] = "施工状況写真"
	c.PhotoCategory["安全"] = "安全管理写真"
	c.PhotoCategory["安全管理"] = "安全管理写真"
	c.PhotoCategory["材料"] = "使用材料写真"
	c.PhotoCategory["使用材料"] = "使用材料写真"
	c.WorkType["舗装"] = "舗装工"
	c.WorkType["As"] = "舗装工"
	c.WorkType["アスファルト"] = "舗装工"
	c.Variety["打換え"] = "舗装打換え工"
	c.Variety["打換"] = "舗装打換え工"
	c.Subphase["表層"] = "表層工"
	c.Subphase["基層"] = "基層工"
	c.Subphase["上層路盤"] = "上層路盤工"
	c.Subphase["下層路盤"] = "下層路盤工"
	return c
}

func markingPreset() Config {
	c := NewConfig()
	c.PhotoCategory["品質"] = "品質管理写真"
	c.PhotoCategory["出来形"] = "出来形管理写真"
	c.PhotoCategory["施工状況"] = "施工状況写真"
	c.WorkType["区画線"] = "区画線工"
	c.WorkType["ライン"] = "区画線工"
	c.WorkType["白線"] = "区画線工"
	c.Variety["溶融式"] = "溶融式区画線"
	c.Variety["ペイント"] = "ペイント式区画線"
	return c
}

func generalPreset() Config {
	c := NewConfig()
	c.PhotoCategory["品質"] = "品質管理写真"
	c.PhotoCategory["出来形"] = "出来形管理写真"
	c.PhotoCategory["施工"] = "施工状況写真"
	c.PhotoCategory["安全"] = "安全管理写真"
	c.PhotoCategory["材料"] = "使用材料写真"
	c.PhotoCategory["着工"] = "着工前写真"
	c.PhotoCategory["完成"] = "完成写真"
	return c
}

// ApplyAliases applies preset (and optional extra config) to a slice of results.
// Returns (transformed, warnings). Warnings are non-fatal (e.g. unknown preset).
func ApplyAliases(results []AnalysisResult, preset string, extra *Config) ([]AnalysisResult, []string) {
	cfg := NewConfig()
	var warnings []string

	if preset != "" {
		presetCfg, ok := FromPreset(preset)
		if !ok {
			warnings = append(warnings, "不明なプリセット '"+preset+"' (pavement/marking/general)")
		} else {
			cfg.Merge(presetCfg)
		}
	}
	if extra != nil {
		cfg.Merge(*extra)
	}

	out := make([]AnalysisResult, len(results))
	for i, r := range results {
		out[i] = cfg.Apply(r)
	}
	return out, warnings
}
