// Package export provides layout constants and export orchestration for PDF/Excel generation.
//
// Constants are ported from common/src/layout.rs (GAS-compliant layout).
package export

// A4 page dimensions in points (GAS-compliant).
const (
	A4WidthPt  float64 = 595.35
	A4HeightPt float64 = 842.0

	A4WidthMM  float64 = 210.0
	A4HeightMM float64 = 297.0
)

// A4 landscape dimensions in points.
const (
	A4LandscapeWidthPt  float64 = 842.0
	A4LandscapeHeightPt float64 = 595.35
)

// Layout spacing in points (GAS-compliant).
const (
	MarginPt float64 = 28.35 // 10mm
	GapPt    float64 = 5.0
)

// Photo/info area ratios (GAS-compliant).
const (
	ImageRatio float64 = 0.65
	InfoRatio  float64 = 0.35
)

// Header height in points (GAS-compliant: no header).
const HeaderHeightPt float64 = 0.0

// Pair PDF margins/dimensions in points.
const (
	PairMarginYPt      float64 = 28.35
	PairMarginXPt      float64 = 19.2
	PairCaptionAreaPt  float64 = 43.2
	PairRuleWidthPt    float64 = 264.0
)

// mm-based constants (Excel use).
const (
	MarginMM   float64 = 10.0
	PhotoGapMM float64 = 10.0

	UsableWidthMM float64 = A4WidthMM - MarginMM*2   // 190mm
	PhotoWidthMM  float64 = UsableWidthMM * ImageRatio // 123.5mm
	InfoWidthMM   float64 = UsableWidthMM * InfoRatio  // 66.5mm

	PhotoAspectRatio float64 = 4.0 / 3.0
	PhotoHeightMM    float64 = PhotoWidthMM / PhotoAspectRatio // 92.625mm

	// Legacy page-division heights (kept for compatibility).
	PhotoHeightMM3Up float64 = (A4HeightMM - MarginMM*2 - PhotoGapMM*2) / 3 // 85.67mm
	PhotoHeightMM2Up float64 = (A4HeightMM - MarginMM*2 - PhotoGapMM) / 2   // 128.5mm
)

// Unit conversion factors.
const (
	MMToPt float64 = 72.0 / 25.4 // ≈ 2.835
	PtToPx float64 = 96.0 / 72.0
	PxToPt float64 = 72.0 / 96.0

	PtPerExcelCol     float64 = 5.3
	PxPerExcelCol     float64 = 7.1
	ExcelColOffsetPx  float64 = 5.0
)

// Derived pt values.
const (
	PageWidthPt  float64 = A4WidthPt
	PageHeightPt float64 = A4HeightPt

	PhotoWidthPt    float64 = PhotoWidthMM * MMToPt
	PhotoHeightPt3Up float64 = PhotoHeightMM3Up * MMToPt
	PhotoHeightPt2Up float64 = PhotoHeightMM2Up * MMToPt

	InfoWidthPt float64 = InfoWidthMM * MMToPt

	BlockHeight3UpPt float64 = PhotoHeightPt3Up + GapPt*MMToPt
)

// Excel style constants.
const (
	LabelFontSize    float64 = 9.0
	LabelFontColor   uint32  = 0x555555
	LabelBgColor     uint32  = 0xF5F5F5
	LabelBorderColor uint32  = 0xAAAAAA
	ValueFontSize    float64 = 11.0
	CellBorderColor  uint32  = 0xCCCCCC
)

// Excel layout constants.
const (
	ExcelScale float64 = 1.1

	PhotoRows3Up      uint8   = 10
	PhotoRows2Up      uint8   = 16
	GapRows           uint8   = 1
	RowsPerBlock3Up   uint8   = PhotoRows3Up + GapRows  // 11
	RowsPerBlock2Up   uint8   = PhotoRows2Up + GapRows  // 17

	RowHeightPt    float64 = 26.0
	PhotoColWidth  float64 = 56.1
	LabelColWidth  float64 = 11.0
	ValueColWidth  float64 = 28.6
	InfoColWidth   float64 = 39.6
)

// FieldKey identifies an information panel field.
type FieldKey int

const (
	FieldDate FieldKey = iota
	FieldPhotoCategory
	FieldWorkType
	FieldVariety
	FieldSubphase
	FieldStation
	FieldRemarks
	FieldMeasurements
)

// FieldDefinition describes one field in the information panel.
type FieldDefinition struct {
	Key     FieldKey
	Label   string
	RowSpan uint8
}

// LayoutFields lists the 8 information panel fields in GAS-compliant order.
var LayoutFields = []FieldDefinition{
	{FieldDate, "日時", 1},
	{FieldPhotoCategory, "区分", 1},
	{FieldWorkType, "工種", 1},
	{FieldVariety, "種別", 1},
	{FieldSubphase, "細別", 1},
	{FieldStation, "測点", 1},
	{FieldRemarks, "備考", 2},
	{FieldMeasurements, "測定値", 2},
}

// PdfLayout holds PDF page layout configuration.
type PdfLayout struct {
	PageWidthMM  float64
	PageHeightMM float64
	MarginMM     float64
	GapMM        float64
	PhotoWidthMM float64
	PhotoHeightMM float64
	InfoWidthMM  float64
	PhotosPerPage int
}

// ThreeUp returns a 3-photos/page PDF layout.
func ThreeUp() PdfLayout {
	return PdfLayout{
		PageWidthMM:   A4WidthMM,
		PageHeightMM:  A4HeightMM,
		MarginMM:      MarginMM,
		GapMM:         PhotoGapMM,
		PhotoWidthMM:  PhotoWidthMM,
		PhotoHeightMM: PhotoHeightMM3Up,
		InfoWidthMM:   InfoWidthMM,
		PhotosPerPage: 3,
	}
}

// TwoUp returns a 2-photos/page PDF layout.
func TwoUp() PdfLayout {
	return PdfLayout{
		PageWidthMM:   A4WidthMM,
		PageHeightMM:  A4HeightMM,
		MarginMM:      MarginMM,
		GapMM:         PhotoGapMM,
		PhotoWidthMM:  PhotoWidthMM,
		PhotoHeightMM: PhotoHeightMM2Up,
		InfoWidthMM:   InfoWidthMM,
		PhotosPerPage: 2,
	}
}

// ForPhotosPerPage returns the layout for n photos per page.
func ForPhotosPerPage(n int) PdfLayout {
	if n == 2 {
		return TwoUp()
	}
	return ThreeUp()
}

// BlockHeightMM returns the height of one photo block (photo + gap) in mm.
func (l PdfLayout) BlockHeightMM() float64 {
	return l.PhotoHeightMM + l.GapMM
}

// UsableWidthMM returns the printable width in mm.
func (l PdfLayout) UsableWidthMM() float64 {
	return l.PageWidthMM - l.MarginMM*2
}

// UsableHeightMM returns the printable height in mm.
func (l PdfLayout) UsableHeightMM() float64 {
	return l.PageHeightMM - l.MarginMM*2
}

// ExcelLayout holds Excel workbook layout configuration.
type ExcelLayout struct {
	RowsPerBlock  uint8
	PhotoRows     uint8
	RowHeightPt   float64
	ColAWidth     float64
	ColBWidth     float64
	ColCWidth     float64
	PhotoWidthPt  float64
	PhotoHeightPt float64
	InfoWidthPt   float64
}

// ExcelThreeUp returns a 3-photos/page Excel layout.
func ExcelThreeUp() ExcelLayout {
	return ExcelLayout{
		RowsPerBlock:  RowsPerBlock3Up,
		PhotoRows:     PhotoRows3Up,
		RowHeightPt:   RowHeightPt,
		ColAWidth:     PhotoColWidth,
		ColBWidth:     LabelColWidth,
		ColCWidth:     ValueColWidth,
		PhotoWidthPt:  PhotoWidthPt,
		PhotoHeightPt: PhotoHeightPt3Up,
		InfoWidthPt:   InfoWidthPt,
	}
}

// ExcelTwoUp returns a 2-photos/page Excel layout.
func ExcelTwoUp() ExcelLayout {
	return ExcelLayout{
		RowsPerBlock:  RowsPerBlock2Up,
		PhotoRows:     PhotoRows2Up,
		RowHeightPt:   RowHeightPt,
		ColAWidth:     PhotoColWidth,
		ColBWidth:     LabelColWidth,
		ColCWidth:     ValueColWidth,
		PhotoWidthPt:  PhotoWidthPt,
		PhotoHeightPt: PhotoHeightPt2Up,
		InfoWidthPt:   InfoWidthPt,
	}
}

// ExcelForPhotosPerPage returns the Excel layout for n photos per page.
func ExcelForPhotosPerPage(n int) ExcelLayout {
	if n == 2 {
		return ExcelTwoUp()
	}
	return ExcelThreeUp()
}

// PhotoAreaPx returns the photo area dimensions in pixels.
func (l ExcelLayout) PhotoAreaPx() (width, height uint32) {
	rowHeightPx := uint32(l.RowHeightPt * PtToPx)
	w := uint32(l.ColAWidth*PxPerExcelCol + ExcelColOffsetPx)
	h := rowHeightPx * uint32(l.PhotoRows)
	return w, h
}

// MmToPt converts millimetres to points.
func MmToPt(mm float64) float64 { return mm * MMToPt }

// PtToMm converts points to millimetres.
func PtToMm(pt float64) float64 { return pt / MMToPt }

// FitImageCentered computes scaled dimensions and offsets to fit an image
// centred inside a target rectangle while preserving aspect ratio.
// Returns (scaledW, scaledH, offsetX, offsetY).
func FitImageCentered(imageW, imageH, targetW, targetH float64) (float64, float64, float64, float64) {
	scale := min64(targetW/imageW, targetH/imageH)
	scaledW := imageW * scale
	scaledH := imageH * scale
	offsetX := max64((targetW-scaledW)/2, 0)
	offsetY := max64((targetH-scaledH)/2, 0)
	return scaledW, scaledH, offsetX, offsetY
}

func min64(a, b float64) float64 {
	if a < b {
		return a
	}
	return b
}

func max64(a, b float64) float64 {
	if a > b {
		return a
	}
	return b
}
