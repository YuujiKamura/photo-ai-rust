package export

import (
	"math"
	"testing"
)

func approx(a, b, tolerance float64) bool {
	return math.Abs(a-b) < tolerance
}

func TestLayoutConstants(t *testing.T) {
	tests := []struct {
		name string
		got  float64
		want float64
		tol  float64
	}{
		{"A4WidthPt", A4WidthPt, 595.35, 0.01},
		{"A4HeightPt", A4HeightPt, 842.0, 0.01},
		{"MarginPt", MarginPt, 28.35, 0.01},
		{"GapPt", GapPt, 5.0, 0.01},
		{"ImageRatio", ImageRatio, 0.65, 0.01},
		{"InfoRatio", InfoRatio, 0.35, 0.01},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if !approx(tt.got, tt.want, tt.tol) {
				t.Errorf("%s = %f, want ≈ %f", tt.name, tt.got, tt.want)
			}
		})
	}
}

func TestThreeUpLayout(t *testing.T) {
	l := ThreeUp()
	if l.PhotosPerPage != 3 {
		t.Errorf("PhotosPerPage = %d, want 3", l.PhotosPerPage)
	}
	if !approx(l.UsableWidthMM(), 190.0, 0.1) {
		t.Errorf("UsableWidthMM = %f, want ≈ 190.0", l.UsableWidthMM())
	}
	if !approx(l.UsableHeightMM(), 277.0, 0.1) {
		t.Errorf("UsableHeightMM = %f, want ≈ 277.0", l.UsableHeightMM())
	}
}

func TestTwoUpLayout(t *testing.T) {
	l := TwoUp()
	if l.PhotosPerPage != 2 {
		t.Errorf("PhotosPerPage = %d, want 2", l.PhotosPerPage)
	}
}

func TestFitImageCentered(t *testing.T) {
	tests := []struct {
		name                         string
		imgW, imgH, tgtW, tgtH      float64
		wantW, wantH, wantX, wantY  float64
	}{
		{
			name: "square into wide target",
			imgW: 100, imgH: 100, tgtW: 200, tgtH: 100,
			wantW: 100, wantH: 100, wantX: 50, wantY: 0,
		},
		{
			name: "landscape into tall target",
			imgW: 200, imgH: 100, tgtW: 200, tgtH: 200,
			wantW: 200, wantH: 100, wantX: 0, wantY: 50,
		},
		{
			name: "portrait into tall target",
			imgW: 100, imgH: 200, tgtW: 200, tgtH: 200,
			wantW: 100, wantH: 200, wantX: 50, wantY: 0,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			sw, sh, ox, oy := FitImageCentered(tt.imgW, tt.imgH, tt.tgtW, tt.tgtH)
			if !approx(sw, tt.wantW, 0.01) {
				t.Errorf("scaledW = %f, want %f", sw, tt.wantW)
			}
			if !approx(sh, tt.wantH, 0.01) {
				t.Errorf("scaledH = %f, want %f", sh, tt.wantH)
			}
			if !approx(ox, tt.wantX, 0.01) {
				t.Errorf("offsetX = %f, want %f", ox, tt.wantX)
			}
			if !approx(oy, tt.wantY, 0.01) {
				t.Errorf("offsetY = %f, want %f", oy, tt.wantY)
			}
		})
	}
}

func TestExcelThreeUpLayout(t *testing.T) {
	l := ExcelThreeUp()
	if l.RowsPerBlock != 11 {
		t.Errorf("RowsPerBlock = %d, want 11", l.RowsPerBlock)
	}
	if l.PhotoRows != 10 {
		t.Errorf("PhotoRows = %d, want 10", l.PhotoRows)
	}
	w, h := l.PhotoAreaPx()
	if w == 0 || h == 0 {
		t.Errorf("PhotoAreaPx returned zero: w=%d, h=%d", w, h)
	}
}

func TestExcelTwoUpLayout(t *testing.T) {
	l := ExcelTwoUp()
	if l.RowsPerBlock != 17 {
		t.Errorf("RowsPerBlock = %d, want 17", l.RowsPerBlock)
	}
	if l.PhotoRows != 16 {
		t.Errorf("PhotoRows = %d, want 16", l.PhotoRows)
	}
	w, h := l.PhotoAreaPx()
	if w == 0 || h == 0 {
		t.Errorf("PhotoAreaPx returned zero: w=%d, h=%d", w, h)
	}
}
