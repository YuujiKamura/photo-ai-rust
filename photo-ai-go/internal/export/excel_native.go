// Package export – excel_native.go
//
// Pure-Go Excel backend using github.com/xuri/excelize/v2.
//
// Feature flag: set env var PHOTO_EXCEL_BACKEND=go to activate.
//
// Parity status (2026-04-14):
//   - Sheet layout: page-number sheet names ("1", "2", …), 3-up default
//   - Column widths: A (photo col), B (label col), C (value col) — derived
//     from layout.go constants to match the DLL output
//   - Field order: 8 fields in GAS-compliant order (日時→区分→工種→種別→細別→測点→備考→測定値)
//   - Merged cells: photo area merged over photo rows; Remarks and
//     Measurements (row_span=2) merged in B/C columns
//   - Row heights: uniform RowHeightPt per row
//   - Styles: label cells (bold, grey bg, 9pt, hair border 0xAAAAAA);
//     value cells (11pt, left, wrap, hair border 0xCCCCCC);
//     photo cells (thin border 0xCCCCCC, no fill)
//   - Images: NOT embedded (no file access in the native path; images require
//     the DLL path for now — see GenerateExcelNative signature)
//   - Known cosmetic divergences tolerated by golden test (see excel_native_test.go):
//     ZIP timestamps, XML attribute order, xmlns redeclarations,
//     creator/lastModifiedBy metadata

package export

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/xuri/excelize/v2"

	"github.com/YuujiKamura/photo-ai-go/pkg/engine"
)

// excelNativeBuildWorkbook builds an excelize.File from the given analysis results.
// photosPerPage selects the Excel layout (2 or 3 per page).
func excelNativeBuildWorkbook(results []engine.AnalysisResult, photosPerPage int) (*excelize.File, error) {
	layout := ExcelForPhotosPerPage(photosPerPage)

	f := excelize.NewFile()

	// Remove default "Sheet1" — we will rename first page to "1".
	defaultSheet := f.GetSheetName(0)

	totalPages := (len(results) + photosPerPage - 1) / photosPerPage
	if totalPages == 0 {
		totalPages = 1
	}

	// Pre-create named styles.
	labelStyleID, err := buildLabelStyle(f)
	if err != nil {
		return nil, fmt.Errorf("excel_native: create label style: %w", err)
	}
	valueStyleID, err := buildValueStyle(f)
	if err != nil {
		return nil, fmt.Errorf("excel_native: create value style: %w", err)
	}
	photoStyleID, err := buildPhotoStyle(f)
	if err != nil {
		return nil, fmt.Errorf("excel_native: create photo style: %w", err)
	}

	for pageIdx := 0; pageIdx < totalPages; pageIdx++ {
		sheetName := fmt.Sprintf("%d", pageIdx+1)

		if pageIdx == 0 {
			// Rename the default sheet.
			if err := f.SetSheetName(defaultSheet, sheetName); err != nil {
				return nil, fmt.Errorf("excel_native: rename sheet: %w", err)
			}
		} else {
			_, err = f.NewSheet(sheetName)
			if err != nil {
				return nil, fmt.Errorf("excel_native: new sheet %s: %w", sheetName, err)
			}
		}

		// Column widths (Excel character units, same conversion as layout.go).
		// PhotoColWidth / LabelColWidth / ValueColWidth are Excel units.
		if err := f.SetColWidth(sheetName, "A", "A", float64(layout.ColAWidth)); err != nil {
			return nil, fmt.Errorf("excel_native: col A width: %w", err)
		}
		if err := f.SetColWidth(sheetName, "B", "B", float64(layout.ColBWidth)); err != nil {
			return nil, fmt.Errorf("excel_native: col B width: %w", err)
		}
		if err := f.SetColWidth(sheetName, "C", "C", float64(layout.ColCWidth)); err != nil {
			return nil, fmt.Errorf("excel_native: col C width: %w", err)
		}

		startIdx := pageIdx * photosPerPage
		endIdx := startIdx + photosPerPage
		if endIdx > len(results) {
			endIdx = len(results)
		}
		pagePhotos := results[startIdx:endIdx]

		rowsPerBlock := int(layout.RowsPerBlock)
		photoRows := int(layout.PhotoRows)
		rowHeightPt := layout.RowHeightPt

		currentRow := 1 // 1-based for excelize

		for _, photo := range pagePhotos {
			startRow := currentRow
			endPhotoRow := startRow + photoRows - 1

			// Row heights.
			for r := startRow; r < startRow+rowsPerBlock; r++ {
				if err := f.SetRowHeight(sheetName, r, rowHeightPt); err != nil {
					return nil, fmt.Errorf("excel_native: row height row %d: %w", r, err)
				}
			}

			// Photo area: merge A[startRow]:A[endPhotoRow], set style.
			photoCell := fmt.Sprintf("A%d", startRow)
			photoEnd := fmt.Sprintf("A%d", endPhotoRow)
			if err := f.MergeCell(sheetName, photoCell, photoEnd); err != nil {
				return nil, fmt.Errorf("excel_native: merge photo cell: %w", err)
			}
			if err := f.SetCellStyle(sheetName, photoCell, photoEnd, photoStyleID); err != nil {
				return nil, fmt.Errorf("excel_native: photo cell style: %w", err)
			}

			// Information fields (B=label, C=value).
			fieldRow := startRow
			for _, field := range LayoutFields {
				rowSpan := int(field.RowSpan)
				label := fieldLabel(field)
				value := photoFieldValue(photo, field.Key)

				bCell := fmt.Sprintf("B%d", fieldRow)
				cCell := fmt.Sprintf("C%d", fieldRow)

				if rowSpan > 1 {
					bEnd := fmt.Sprintf("B%d", fieldRow+rowSpan-1)
					cEnd := fmt.Sprintf("C%d", fieldRow+rowSpan-1)
					if err := f.MergeCell(sheetName, bCell, bEnd); err != nil {
						return nil, fmt.Errorf("excel_native: merge label: %w", err)
					}
					if err := f.MergeCell(sheetName, cCell, cEnd); err != nil {
						return nil, fmt.Errorf("excel_native: merge value: %w", err)
					}
				}
				if err := f.SetCellValue(sheetName, bCell, label); err != nil {
					return nil, fmt.Errorf("excel_native: set label: %w", err)
				}
				if err := f.SetCellValue(sheetName, cCell, value); err != nil {
					return nil, fmt.Errorf("excel_native: set value: %w", err)
				}
				if rowSpan > 1 {
					bEnd := fmt.Sprintf("B%d", fieldRow+rowSpan-1)
					cEnd := fmt.Sprintf("C%d", fieldRow+rowSpan-1)
					if err := f.SetCellStyle(sheetName, bCell, bEnd, labelStyleID); err != nil {
						return nil, fmt.Errorf("excel_native: label style: %w", err)
					}
					if err := f.SetCellStyle(sheetName, cCell, cEnd, valueStyleID); err != nil {
						return nil, fmt.Errorf("excel_native: value style: %w", err)
					}
				} else {
					if err := f.SetCellStyle(sheetName, bCell, bCell, labelStyleID); err != nil {
						return nil, fmt.Errorf("excel_native: label style: %w", err)
					}
					if err := f.SetCellStyle(sheetName, cCell, cCell, valueStyleID); err != nil {
						return nil, fmt.Errorf("excel_native: value style: %w", err)
					}
				}

				fieldRow += rowSpan
			}

			currentRow = startRow + rowsPerBlock
		}
	}

	return f, nil
}

// GenerateExcelNative generates an xlsx file using the pure-Go excelize backend.
// inputJSON is read, results are laid out per the GAS-compliant layout,
// and the file is written to outputPath.
func GenerateExcelNative(inputJSON, outputPath string, photosPerPage int) (engine.ExcelResult, error) {
	var result engine.ExcelResult

	data, err := os.ReadFile(inputJSON)
	if err != nil {
		return result, fmt.Errorf("excel_native: read %s: %w", inputJSON, err)
	}

	var results []engine.AnalysisResult
	if err := json.Unmarshal(data, &results); err != nil {
		return result, fmt.Errorf("excel_native: parse JSON: %w", err)
	}

	if photosPerPage <= 0 {
		photosPerPage = 3
	}

	f, err := excelNativeBuildWorkbook(results, photosPerPage)
	if err != nil {
		return result, err
	}

	if err := f.SaveAs(outputPath); err != nil {
		return result, fmt.Errorf("excel_native: save %s: %w", outputPath, err)
	}

	totalPages := (len(results) + photosPerPage - 1) / photosPerPage
	if totalPages == 0 {
		totalPages = 1
	}
	result.OutputPath = outputPath
	result.SheetCount = totalPages
	return result, nil
}

// ---- style helpers ----

func rgbHex(rgb uint32) string {
	return fmt.Sprintf("FF%06X", rgb)
}

func buildLabelStyle(f *excelize.File) (int, error) {
	style := &excelize.Style{
		Font: &excelize.Font{
			Bold:  true,
			Size:  LabelFontSize,
			Color: rgbHex(LabelFontColor),
		},
		Fill: excelize.Fill{
			Type:    "pattern",
			Pattern: 1,
			Color:   []string{rgbHex(LabelBgColor)},
		},
		Alignment: &excelize.Alignment{
			Horizontal: "center",
			Vertical:   "center",
			WrapText:   false,
		},
		Border: hairBorders(LabelBorderColor),
	}
	return f.NewStyle(style)
}

func buildValueStyle(f *excelize.File) (int, error) {
	style := &excelize.Style{
		Font: &excelize.Font{
			Size: ValueFontSize,
		},
		Alignment: &excelize.Alignment{
			Horizontal: "left",
			Vertical:   "center",
			WrapText:   true,
		},
		Border: hairBorders(CellBorderColor),
	}
	return f.NewStyle(style)
}

func buildPhotoStyle(f *excelize.File) (int, error) {
	style := &excelize.Style{
		Border: thinBorders(CellBorderColor),
	}
	return f.NewStyle(style)
}

func hairBorders(rgb uint32) []excelize.Border {
	c := rgbHex(rgb)
	sides := []string{"left", "right", "top", "bottom"}
	borders := make([]excelize.Border, 0, len(sides))
	for _, s := range sides {
		borders = append(borders, excelize.Border{Type: s, Color: c, Style: 1}) // 1 = hair
	}
	return borders
}

func thinBorders(rgb uint32) []excelize.Border {
	c := rgbHex(rgb)
	sides := []string{"left", "right", "top", "bottom"}
	borders := make([]excelize.Border, 0, len(sides))
	for _, s := range sides {
		borders = append(borders, excelize.Border{Type: s, Color: c, Style: 2}) // 2 = thin
	}
	return borders
}

// ---- field helpers ----

func fieldLabel(fd FieldDefinition) string {
	return fd.Label
}

// photoFieldValue maps a FieldKey to the corresponding value from AnalysisResult.
func photoFieldValue(r engine.AnalysisResult, key FieldKey) string {
	switch key {
	case FieldDate:
		return r.Date
	case FieldPhotoCategory:
		return r.PhotoCategory
	case FieldWorkType:
		return r.WorkType
	case FieldVariety:
		return r.Variety
	case FieldSubphase:
		return r.Subphase
	case FieldStation:
		return r.Station
	case FieldRemarks:
		return r.Remarks
	case FieldMeasurements:
		return r.Measurements
	default:
		return ""
	}
}

// defaultExcelName derives a default output filename from the input JSON path.
func defaultExcelName(inputJSON string) string {
	base := filepath.Base(inputJSON)
	stem := strings.TrimSuffix(base, filepath.Ext(base))
	return stem + ".xlsx"
}
