// Package normalizer — dekigata.go
//
// Ports src/normalizer/dekigata.rs (Rust) to Go.
// Handles 出来形管理用紙 (construction quality record sheet) OCR parsing,
// lane determination, and measurements string formatting.
//
// Public API prefixed with Dekigata* to avoid symbol collisions.
package normalizer

import (
	"fmt"
	"regexp"
	"sort"
	"strings"
)

// -----------------------------------------------------------------------
// DekigataLane — mirrors Lane in dekigata.rs
// -----------------------------------------------------------------------

// DekigataLane identifies which road lane the measurement data belongs to.
type DekigataLane int

const (
	DekigataLaneLeft  DekigataLane = iota // 左車線: V1–V3, W1
	DekigataLaneRight                     // 右車線: V4–V5, W2
	DekigataLaneBoth                      // 両車線
)

func (l DekigataLane) String() string {
	switch l {
	case DekigataLaneLeft:
		return "左車線"
	case DekigataLaneRight:
		return "右車線"
	default:
		return "両車線"
	}
}

// -----------------------------------------------------------------------
// DekigataValues — mirrors DekigataValues in dekigata.rs
// -----------------------------------------------------------------------

// DekigataValues holds values parsed from a 出来形管理用紙 OCR text.
type DekigataValues struct {
	// Station name ("No.1" etc.)
	Station string
	// 切削高(設計) V1–V5; nil means absent
	VDesign [5]*float64
	// 切削高(実施) V1–V5; nil means absent
	VActual [5]*float64
	// Left width (W1)
	WidthLeftDesign  *float64
	WidthLeftActual  *float64
	// Right width (W2)
	WidthRightDesign *float64
	WidthRightActual *float64
}

func f64ptr(v float64) *float64 { return &v }

// -----------------------------------------------------------------------
// Regexps used by ParseDekigataOCR
// -----------------------------------------------------------------------

var (
	dekigataStationRE        = regexp.MustCompile(`出来形管理用紙\s*(No\.\d+)`)
	dekigataVRE              = regexp.MustCompile(`V(\d)=(\d+\.?\d*)`)
	dekigataWidthLeftRE      = regexp.MustCompile(`左幅員\s*設計\s*(\d+\.?\d*)\s*実測\s*(\d+\.?\d*)`)
	dekigataWidthRightRE     = regexp.MustCompile(`右幅員\s*設計\s*(\d+\.?\d*)\s*実測\s*(\d+\.?\d*)`)
	dekigataWidthLeftActRE   = regexp.MustCompile(`左幅員\s*実測\s*(\d+\.?\d*)`)
	dekigataWidthRightActRE  = regexp.MustCompile(`右幅員\s*実測\s*(\d+\.?\d*)`)
	dekigataGHFHRE           = regexp.MustCompile(`[GF]H=\d+\.?\d*`)
	dekigataNumRE            = regexp.MustCompile(`(\d+\.\d+)`)
)

// sectionKeywords mirrors the keyword list in split_by_keywords (dekigata.rs).
var sectionKeywords = []string{
	"切削高(設計)", "切削高（設計）",
	"切削高(実施)", "切削高（実施）",
	"計画高(設計)", "計画高（設計）",
	"計画高(実施)", "計画高（実施）",
	"左幅員", "右幅員",
}

// splitByKeywords mirrors split_by_keywords in dekigata.rs.
func splitByKeywords(text string) []string {
	// Collect all keyword start positions
	type pos struct{ start, end int }
	var positions []int
	seen := map[int]bool{}
	for _, kw := range sectionKeywords {
		idx := 0
		for {
			i := strings.Index(text[idx:], kw)
			if i < 0 {
				break
			}
			abs := idx + i
			if !seen[abs] {
				positions = append(positions, abs)
				seen[abs] = true
			}
			idx = abs + len(kw)
		}
	}
	if len(positions) == 0 {
		return []string{text}
	}
	sort.Ints(positions)

	var sections []string
	if positions[0] > 0 {
		sections = append(sections, strings.TrimSpace(text[:positions[0]]))
	}
	for i, start := range positions {
		end := len(text)
		if i+1 < len(positions) {
			end = positions[i+1]
		}
		sections = append(sections, strings.TrimSpace(text[start:end]))
	}
	return sections
}

// ParseDekigataOCR parses OCR text from a 出来形管理用紙 into DekigataValues.
// Returns nil when the text does not contain "出来形管理用紙" or no V values found.
//
// Mirrors parse_dekigata_ocr in dekigata.rs.
func ParseDekigataOCR(text string) *DekigataValues {
	if !strings.Contains(text, "出来形管理用紙") {
		return nil
	}

	v := &DekigataValues{}

	// Station
	if m := dekigataStationRE.FindStringSubmatch(text); m != nil {
		v.Station = m[1]
	}

	// Split into sections
	var sections []string
	if strings.Contains(text, ",") {
		for _, s := range strings.Split(text, ",") {
			sections = append(sections, strings.TrimSpace(s))
		}
	} else {
		sections = splitByKeywords(text)
	}

	parseFloat := func(s string) *float64 {
		var f float64
		_, err := fmt.Sscanf(s, "%f", &f)
		if err != nil {
			return nil
		}
		return f64ptr(f)
	}

	for _, sec := range sections {
		isDesign := strings.Contains(sec, "切削高(設計)") || strings.Contains(sec, "切削高（設計）") ||
			strings.Contains(sec, "計画高(設計)") || strings.Contains(sec, "計画高（設計）")
		isActual := strings.Contains(sec, "切削高(実施)") || strings.Contains(sec, "切削高（実施）") ||
			strings.Contains(sec, "計画高(実施)") || strings.Contains(sec, "計画高（実施）")

		if isDesign || isActual {
			target := &v.VDesign
			if isActual {
				target = &v.VActual
			}

			// V=labeled match
			vCount := 0
			for _, m := range dekigataVRE.FindAllStringSubmatch(sec, -1) {
				var idx int
				fmt.Sscanf(m[1], "%d", &idx)
				if idx >= 1 && idx <= 5 {
					(*target)[idx-1] = parseFloat(m[2])
					vCount++
				}
			}

			// Fallback: no V= labels — use positional decimal numbers, excluding GH/FH
			if vCount == 0 {
				clean := dekigataGHFHRE.ReplaceAllString(sec, "")
				for i, m := range dekigataNumRE.FindAllStringSubmatch(clean, -1) {
					if i >= 5 {
						break
					}
					(*target)[i] = parseFloat(m[1])
				}
			}
		}

		// Width: design+actual pattern takes priority
		if m := dekigataWidthLeftRE.FindStringSubmatch(sec); m != nil {
			v.WidthLeftDesign = parseFloat(m[1])
			v.WidthLeftActual = parseFloat(m[2])
		} else if m := dekigataWidthLeftActRE.FindStringSubmatch(sec); m != nil {
			v.WidthLeftActual = parseFloat(m[1])
		}
		if m := dekigataWidthRightRE.FindStringSubmatch(sec); m != nil {
			v.WidthRightDesign = parseFloat(m[1])
			v.WidthRightActual = parseFloat(m[2])
		} else if m := dekigataWidthRightActRE.FindStringSubmatch(sec); m != nil {
			v.WidthRightActual = parseFloat(m[1])
		}
	}

	// Require at least one V value
	hasDesign := anySet(v.VDesign[:])
	hasActual := anySet(v.VActual[:])
	if !hasDesign && !hasActual {
		return nil
	}
	return v
}

func anySet(vs []*float64) bool {
	for _, p := range vs {
		if p != nil {
			return true
		}
	}
	return false
}

// -----------------------------------------------------------------------
// DekigataLane determination
// -----------------------------------------------------------------------

// DekigataLaneDetermine infers the lane from width-actual availability,
// falling back to V-value diff comparison.
//
// Returns (lane, true) or (0, false) when undetermined.
//
// Mirrors determine_lane in dekigata.rs.
func DekigataLaneDetermine(v *DekigataValues) (DekigataLane, bool) {
	hasLeft := v.WidthLeftActual != nil
	hasRight := v.WidthRightActual != nil

	switch {
	case hasLeft && !hasRight:
		return DekigataLaneLeft, true
	case !hasLeft && hasRight:
		return DekigataLaneRight, true
	case !hasLeft && !hasRight:
		return 0, false
	}

	// Both present → V-diff comparison
	var leftDiff float64
	for i := 0; i < 3; i++ {
		if v.VDesign[i] != nil && v.VActual[i] != nil {
			d := *v.VDesign[i] - *v.VActual[i]
			if d < 0 {
				d = -d
			}
			leftDiff += d
		}
	}
	var rightDiff float64
	for i := 3; i < 5; i++ {
		if v.VDesign[i] != nil && v.VActual[i] != nil {
			d := *v.VDesign[i] - *v.VActual[i]
			if d < 0 {
				d = -d
			}
			rightDiff += d
		}
	}
	const eps = 0.001
	if leftDiff > rightDiff+eps {
		return DekigataLaneLeft, true
	}
	if rightDiff > leftDiff+eps {
		return DekigataLaneRight, true
	}
	return 0, false
}

// -----------------------------------------------------------------------
// Formatting
// -----------------------------------------------------------------------

// DekigataFormatMeasurements produces the 4-line measurements string for a lane.
// Mirrors format_measurements / format_left / format_right / format_both in dekigata.rs.
func DekigataFormatMeasurements(v *DekigataValues, lane DekigataLane) string {
	switch lane {
	case DekigataLaneLeft:
		return dekigataFormatLeft(v)
	case DekigataLaneRight:
		return dekigataFormatRight(v)
	default:
		return dekigataFormatBoth(v)
	}
}

func dekigataFormatVValues(vs []*float64, startNum int) string {
	var parts []string
	for i, p := range vs {
		if p != nil {
			parts = append(parts, fmt.Sprintf("V%d=%.3f", startNum+i, *p))
		}
	}
	return strings.Join(parts, " ")
}

func dekigataFormatWidth(label string, design, actual *float64) string {
	switch {
	case design != nil && actual != nil:
		return fmt.Sprintf("%s 設計: %.2f 実測: %.2f", label, *design, *actual)
	case design != nil:
		return fmt.Sprintf("%s 設計: %.2f", label, *design)
	default:
		return ""
	}
}

func dekigataFormatLeft(v *DekigataValues) string {
	vd := dekigataFormatVValues(v.VDesign[0:3], 1)
	va := dekigataFormatVValues(v.VActual[0:3], 1)
	w := dekigataFormatWidth("幅員W1", v.WidthLeftDesign, v.WidthLeftActual)
	return fmt.Sprintf("左車線\u3000切削基準高 幅員W1\n設計: %s\n実施: %s\n%s", vd, va, w)
}

func dekigataFormatRight(v *DekigataValues) string {
	// Mirror Rust logic: if V1,V2 actual are nil but V3 actual exists → right lane starts at V3
	actualV1V2Empty := v.VActual[0] == nil && v.VActual[1] == nil
	hasV3 := actualV1V2Empty && v.VActual[2] != nil
	start, startNum := 3, 4
	if hasV3 {
		start, startNum = 2, 3
	}
	vd := dekigataFormatVValues(v.VDesign[start:5], startNum)
	va := dekigataFormatVValues(v.VActual[start:5], startNum)
	w := dekigataFormatWidth("幅員W2", v.WidthRightDesign, v.WidthRightActual)
	return fmt.Sprintf("右車線\u3000切削基準高 幅員W2\n設計: %s\n実施: %s\n%s", vd, va, w)
}

func dekigataFormatBoth(v *DekigataValues) string {
	ld := dekigataFormatVValues(v.VDesign[0:3], 1)
	la := dekigataFormatVValues(v.VActual[0:3], 1)
	rd := dekigataFormatVValues(v.VDesign[3:5], 4)
	ra := dekigataFormatVValues(v.VActual[3:5], 4)
	w1 := dekigataFormatWidth("幅員W1", v.WidthLeftDesign, v.WidthLeftActual)
	w2 := dekigataFormatWidth("幅員W2", v.WidthRightDesign, v.WidthRightActual)
	return fmt.Sprintf("設計: %s\n実施: %s\n設計: %s\n実施: %s\n%s\n%s", ld, la, rd, ra, w1, w2)
}

// -----------------------------------------------------------------------
// DekigataIsDekigataPhoto — mirrors is_dekigata_photo in dekigata.rs
// (operates on raw fields instead of AnalysisResult which is a Rust type)
// -----------------------------------------------------------------------

// DekigataIsPhoto returns true when the photo is a 出来形管理写真.
// photoCategory, remarks, and detectedText correspond to the Rust AnalysisResult fields.
func DekigataIsPhoto(photoCategory, remarks, detectedText string) bool {
	return photoCategory == "出来形管理写真" ||
		strings.Contains(remarks, "出来形") ||
		strings.Contains(detectedText, "出来形管理用紙")
}

// DekigataFindBoardDetailIndex finds the index (within groupIndices) of the best
// 出来形管理用紙 detail photo in detectedTexts.
// Returns -1 when not found.
//
// Mirrors find_board_detail_photo in dekigata.rs.
func DekigataFindBoardDetailIndex(detectedTexts []string, groupIndices []int) int {
	keywords := []string{
		"切削高(設計)", "切削高（設計）",
		"切削高(実施)", "切削高（実施）",
		"計画高(設計)", "計画高（設計）",
		"計画高(実施)", "計画高（実施）",
	}
	// Iterate in reverse to prefer last (closest-up) photo
	for i := len(groupIndices) - 1; i >= 0; i-- {
		idx := groupIndices[i]
		if idx >= len(detectedTexts) {
			continue
		}
		dt := detectedTexts[idx]
		for _, kw := range keywords {
			if strings.Contains(dt, kw) {
				return i // index within groupIndices
			}
		}
	}
	return -1
}
