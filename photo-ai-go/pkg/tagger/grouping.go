package tagger

import (
	"fmt"
	"math"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
	"unicode"
)

const groupGapSecs int64 = 5 * 60

// collectCaptureTimes returns filename -> unix timestamp from file modification time.
func collectCaptureTimes(images []string) map[string]int64 {
	out := make(map[string]int64, len(images))
	for _, p := range images {
		fname := filepath.Base(p)
		if fname == "" {
			continue
		}
		info, err := os.Stat(p)
		if err != nil {
			continue
		}
		out[fname] = info.ModTime().Unix()
	}
	return out
}

// applyCaptureTimes fills in capture times and normalizes machine IDs.
func applyCaptureTimes(records GroupRecords, captureTimes map[string]int64) {
	for fname, rec := range records {
		normalizeMachineID(rec)
		if rec.CapturedAt == nil {
			if ts, ok := captureTimes[fname]; ok {
				rec.CapturedAt = &ts
			}
		}
	}
	propagateAttachmentByTime(records)
}

// assignGroups groups records by machine_id and time proximity.
func assignGroups(records GroupRecords) {
	// Group filenames by machine_id
	byID := make(map[string][]string)
	for fname, rec := range records {
		byID[rec.MachineID] = append(byID[rec.MachineID], fname)
	}

	type segHead struct {
		ts        int64
		machineID string
		tmpGroup  uint32
	}
	var segmentHeads []segHead
	fnameTmpGroup := make(map[string]uint32)
	var nextTmpGroup uint32 = 1

	for machineID, files := range byID {
		sort.Slice(files, func(i, j int) bool {
			tsI := capturedAtOrMax(records[files[i]])
			tsJ := capturedAtOrMax(records[files[j]])
			if tsI != tsJ {
				return tsI < tsJ
			}
			return files[i] < files[j]
		})
		if len(files) == 0 {
			continue
		}

		currentGroup := nextTmpGroup
		nextTmpGroup++
		firstTS := capturedAtOrMax(records[files[0]])
		segmentHeads = append(segmentHeads, segHead{firstTS, machineID, currentGroup})
		fnameTmpGroup[files[0]] = currentGroup

		for k := 1; k < len(files); k++ {
			prev := records[files[k-1]]
			curr := records[files[k]]
			prevTS := capturedAtOrMax(prev)
			currTS := capturedAtOrMax(curr)
			var gap int64
			if prevTS == math.MaxInt64 || currTS == math.MaxInt64 {
				gap = 0
			} else {
				gap = currTS - prevTS
				if gap < 0 {
					gap = -gap
				}
			}
			prevAttach := hasAttachmentHint(prev)
			currAttach := hasAttachmentHint(curr)

			if gap > groupGapSecs || prevAttach != currAttach {
				currentGroup = nextTmpGroup
				nextTmpGroup++
				segmentHeads = append(segmentHeads, segHead{currTS, machineID, currentGroup})
			}
			fnameTmpGroup[files[k]] = currentGroup
		}
	}

	// Sort segment heads and build compact mapping
	sort.Slice(segmentHeads, func(i, j int) bool {
		if segmentHeads[i].ts != segmentHeads[j].ts {
			return segmentHeads[i].ts < segmentHeads[j].ts
		}
		if segmentHeads[i].machineID != segmentHeads[j].machineID {
			return segmentHeads[i].machineID < segmentHeads[j].machineID
		}
		return segmentHeads[i].tmpGroup < segmentHeads[j].tmpGroup
	})

	compactMap := make(map[uint32]uint32)
	for idx, sh := range segmentHeads {
		compactMap[sh.tmpGroup] = uint32(idx + 1)
	}

	for fname, rec := range records {
		if tmp, ok := fnameTmpGroup[fname]; ok {
			if compact, ok2 := compactMap[tmp]; ok2 {
				rec.Group = compact
			} else {
				rec.Group = tmp
			}
		} else {
			rec.Group = 0
		}
	}
}

func capturedAtOrMax(rec *GroupRecord) int64 {
	if rec.CapturedAt != nil {
		return *rec.CapturedAt
	}
	return math.MaxInt64
}

func hasAttachmentHint(rec *GroupRecord) bool {
	return strings.Contains(rec.MachineID, "取付") ||
		strings.Contains(rec.DetectedText, "取付")
}

// extractNo extracts "No.XXX" from text.
func extractNo(text string) string {
	for _, marker := range []string{"No.", "No ", "NO.", "NO "} {
		idx := strings.Index(text, marker)
		if idx < 0 {
			continue
		}
		rest := text[idx+len(marker):]
		// skip non-digits, then collect digits
		digits := ""
		started := false
		for _, c := range rest {
			if !started {
				if unicode.IsDigit(c) {
					started = true
					digits += string(c)
				}
			} else {
				if unicode.IsDigit(c) {
					digits += string(c)
				} else {
					break
				}
			}
		}
		if digits != "" {
			return "No." + digits
		}
	}
	return ""
}

func normalizeMachineID(rec *GroupRecord) {
	merged := rec.DetectedText + " " + rec.Description
	if strings.Contains(merged, "取付") {
		no := extractNo(merged)
		if no == "" {
			no = extractNo(rec.MachineID)
		}
		if no != "" {
			rec.MachineID = fmt.Sprintf("取付道路 %s", no)
		}
	}
}

func propagateAttachmentByTime(records GroupRecords) {
	byNo := make(map[string][]string)
	for fname, rec := range records {
		no := extractNo(rec.MachineID)
		if no == "" {
			no = extractNo(rec.DetectedText)
		}
		if no == "" {
			no = extractNo(rec.Description)
		}
		if no != "" {
			byNo[no] = append(byNo[no], fname)
		}
	}

	for no, files := range byNo {
		sort.Slice(files, func(i, j int) bool {
			tsI := capturedAtOrMax(records[files[i]])
			tsJ := capturedAtOrMax(records[files[j]])
			if tsI != tsJ {
				return tsI < tsJ
			}
			return files[i] < files[j]
		})
		if len(files) == 0 {
			continue
		}

		chunk := []string{files[0]}
		for k := 1; k < len(files); k++ {
			prev := records[files[k-1]]
			curr := records[files[k]]
			prevTS := capturedAtOrMax(prev)
			currTS := capturedAtOrMax(curr)
			var gap int64
			if prevTS == math.MaxInt64 || currTS == math.MaxInt64 {
				gap = 0
			} else {
				gap = currTS - prevTS
				if gap < 0 {
					gap = -gap
				}
			}
			if gap > groupGapSecs {
				applyAttachToChunk(records, chunk, no)
				chunk = nil
			}
			chunk = append(chunk, files[k])
		}
		if len(chunk) > 0 {
			applyAttachToChunk(records, chunk, no)
		}
	}
}

func applyAttachToChunk(records GroupRecords, chunk []string, no string) {
	hasAttach := false
	for _, fname := range chunk {
		if rec, ok := records[fname]; ok && hasAttachmentHint(rec) {
			hasAttach = true
			break
		}
	}
	if !hasAttach {
		return
	}
	for _, fname := range chunk {
		if rec, ok := records[fname]; ok {
			rec.MachineID = fmt.Sprintf("取付道路 %s", no)
		}
	}
}

// forceReclassifyEnabled checks the PHOTO_TAGGER_FORCE_RECLASSIFY env var.
func forceReclassifyEnabled() bool {
	v := strings.TrimSpace(strings.ToLower(os.Getenv("PHOTO_TAGGER_FORCE_RECLASSIFY")))
	switch v {
	case "1", "true", "yes", "on":
		return true
	}
	return false
}

// fmtDuration formats a duration for display.
func fmtDuration(d time.Duration) string {
	ms := d.Milliseconds()
	if ms < 1000 {
		return fmt.Sprintf("%dms", ms)
	}
	return fmt.Sprintf("%.1fs", d.Seconds())
}
