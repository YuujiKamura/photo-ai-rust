// Package engine provides Go types matching the photo-engine DLL JSON contract.
// These structs mirror the Rust types defined in common/src/types.rs.
package engine

// EngineResponse is the unified JSON envelope returned by all DLL functions.
// It mirrors the Rust EngineResponse<T> in photo-engine/src/types.rs.
//
//	{ "ok": true,  "data": <T> }
//	{ "ok": false, "error": "<message>" }
type EngineResponse struct {
	OK    bool    `json:"ok"`
	Data  any     `json:"data,omitempty"`
	Error *string `json:"error,omitempty"`
}

// PDFConfig holds parameters for PDF generation.
type PDFConfig struct {
	InputJSON     string `json:"inputJson"`
	OutputPath    string `json:"outputPath"`
	PhotosPerPage int    `json:"photosPerPage"`
	Quality       string `json:"quality"` // "high" | "medium" | "low"
	Preset        string `json:"preset,omitempty"`
	AliasFile     string `json:"aliasFile,omitempty"`
}

// PDFResult is the response from GeneratePDF.
type PDFResult struct {
	OutputPath string `json:"outputPath"`
	PageCount  int    `json:"pageCount"`
	Error      string `json:"error,omitempty"`
}

// ExcelConfig holds parameters for Excel workbook generation.
type ExcelConfig struct {
	InputJSON  string `json:"inputJson"`
	OutputPath string `json:"outputPath"`
	Preset     string `json:"preset,omitempty"`
	AliasFile  string `json:"aliasFile,omitempty"`
}

// ExcelResult is the response from GenerateExcel.
type ExcelResult struct {
	OutputPath string `json:"outputPath"`
	SheetCount int    `json:"sheetCount"`
	Error      string `json:"error,omitempty"`
}

// ImageConfig holds parameters for image processing / AI analysis.
type ImageConfig struct {
	Folder      string `json:"folder"`
	OutputJSON  string `json:"outputJson,omitempty"`
	BatchSize   int    `json:"batchSize"`
	WorkType    string `json:"workType,omitempty"`
	PhotoType   string `json:"photoType,omitempty"`
	Variety     string `json:"variety,omitempty"`
	Station     string `json:"station,omitempty"`
	UseCache    bool   `json:"useCache"`
	Recursive   bool   `json:"recursive"`
	IncludeAll  bool   `json:"includeAll"`
	FolderRules string `json:"folderRules,omitempty"`
	PayPerUse   bool   `json:"payPerUse"`
	Resident    bool   `json:"resident"`
}

// ImageResult is the response from ProcessImage.
type ImageResult struct {
	OutputJSON string `json:"outputJson"`
	PhotoCount int    `json:"photoCount"`
	Error      string `json:"error,omitempty"`
}

// EXIFConfig holds parameters for EXIF extraction.
type EXIFConfig struct {
	FilePath string `json:"filePath"`
}

// EXIFResult is the response from ExtractEXIF.
type EXIFResult struct {
	DateTime string `json:"dateTime"`
	Width    int    `json:"width"`
	Height   int    `json:"height"`
	Error    string `json:"error,omitempty"`
}

// AnalysisResult mirrors common/src/types.rs AnalysisResult.
// Used for in-process manipulation of analysis JSON.
type AnalysisResult struct {
	FileName       string            `json:"fileName"`
	FilePath       string            `json:"filePath,omitempty"`
	Date           string            `json:"date,omitempty"`
	WorkType       string            `json:"workType,omitempty"`
	Variety        string            `json:"variety,omitempty"`
	Subphase       string            `json:"subphase,omitempty"`
	Station        string            `json:"station,omitempty"`
	Remarks        string            `json:"remarks,omitempty"`
	Description    string            `json:"description,omitempty"`
	HasBoard       bool              `json:"hasBoard,omitempty"`
	DetectedText   string            `json:"detectedText,omitempty"`
	Measurements   string            `json:"measurements,omitempty"`
	PhotoCategory  string            `json:"photoCategory,omitempty"`
	Reasoning      string            `json:"reasoning,omitempty"`
	FocusTarget    string            `json:"focusTarget,omitempty"`
	Skip           bool              `json:"skip,omitempty"`
	Group          uint32            `json:"group,omitempty"`
	LabelOverrides map[string]string `json:"labelOverrides,omitempty"`
}

// RawImageData mirrors common/src/types.rs RawImageData.
type RawImageData struct {
	FileName         string `json:"fileName"`
	HasBoard         bool   `json:"hasBoard"`
	DetectedText     string `json:"detectedText"`
	Measurements     string `json:"measurements"`
	SceneDescription string `json:"sceneDescription"`
	PhotoCategory    string `json:"photoCategory"`
}
