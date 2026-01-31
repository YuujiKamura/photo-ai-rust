// ============================================
// AUTO-GENERATED FILE - DO NOT EDIT DIRECTLY
// Generated from: shared/layout-config/layout-config.json
// Version: 1.0.0
// Generated at: 2026-01-31T10:48:33.666Z
// ============================================

// ============================================
// 変換係数
// ============================================

export const MM_TO_PT = 2.8346456693;
export const PT_TO_PX = 1.3333333333;
export const PX_TO_PT = 0.75;
export const PT_PER_EXCEL_COL = 5.3;
export const PX_PER_EXCEL_COL = 7.1;
export const EXCEL_COL_OFFSET_PX = 5;

// ============================================
// ページ設定
// ============================================

export const A4_WIDTH_MM = 210;
export const A4_HEIGHT_MM = 297;
export const MARGIN_MM = 10;
export const PHOTO_GAP_MM = 10;

// ============================================
// 比率
// ============================================

export const IMAGE_RATIO = 0.65;
export const INFO_RATIO = 0.35;

// ============================================
// 導出値 (mm)
// ============================================

export const USABLE_WIDTH_MM = A4_WIDTH_MM - MARGIN_MM * 2;  // 190mm
export const PHOTO_WIDTH_MM = USABLE_WIDTH_MM * IMAGE_RATIO;  // 123.5mm
export const PHOTO_HEIGHT_MM = (A4_HEIGHT_MM - MARGIN_MM * 2 - PHOTO_GAP_MM * 2) / 3;  // 85.67mm

// ============================================
// 導出値 (pt)
// ============================================

export const PHOTO_HEIGHT_PT = PHOTO_HEIGHT_MM * MM_TO_PT;

// ============================================
// Excel設定
// ============================================

export const SCALE = 1.1;
export const PHOTO_ROWS = 10;
export const GAP_ROWS = 1;
export const ROWS_PER_BLOCK = PHOTO_ROWS + GAP_ROWS;  // 11
export const ROW_HEIGHT_PT = Math.floor((PHOTO_HEIGHT_PT / PHOTO_ROWS) * SCALE);  // 26pt

export const COL_A_WIDTH = 56.1;  // 写真列
export const COL_B_WIDTH = 11;  // ラベル列
export const COL_C_WIDTH = 28.6;  // 値列
export const FONT_NAME = 'MS Mincho';
export const FONT_SIZE = 10;

export const BORDER_THIN = {
  top: { style: 'thin', color: { argb: 'FFB5B5B5' } },
  left: { style: 'thin', color: { argb: 'FFB5B5B5' } },
  right: { style: 'thin', color: { argb: 'FFB5B5B5' } },
  bottom: { style: 'thin', color: { argb: 'FFB5B5B5' } }
};

// ============================================
// PDF設定
// ============================================

export const PDF_GAP_PT = 5;
export const PDF_BASE_FONT_SIZE = 12;
export const PDF_LINE_HEIGHT_MULTIPLIER = 1.4;
export const PDF_TEXT_PADDING = 5;
export const PDF_TITLE_FONT_SIZE = 14;
export const PDF_PAGE_NUM_FONT_SIZE = 9;

// ============================================
// フィールド定義
// ============================================

export const LAYOUT_FIELDS = [
  {
    "key": "date",
    "label": "日時",
    "rowSpan": 1
  },
  {
    "key": "photoCategory",
    "label": "区分",
    "rowSpan": 1
  },
  {
    "key": "workType",
    "label": "工種",
    "rowSpan": 1
  },
  {
    "key": "variety",
    "label": "種別",
    "rowSpan": 1
  },
  {
    "key": "subphase",
    "label": "細別",
    "rowSpan": 1
  },
  {
    "key": "station",
    "label": "測点",
    "rowSpan": 1
  },
  {
    "key": "remarks",
    "label": "備考",
    "rowSpan": 2
  },
  {
    "key": "measurements",
    "label": "測定値",
    "rowSpan": 3
  }
];

// 2枚/ページ用のフィールド（測点と備考のみ）
export const LAYOUT_FIELDS_2UP = LAYOUT_FIELDS.filter(
  f => f.key === 'station' || f.key === 'remarks'
);

// ============================================
// ヘルパー関数
// ============================================

export function mmToPt(mm) {
  return mm * MM_TO_PT;
}

export function ptToMm(pt) {
  return pt / MM_TO_PT;
}

export function pxToPt(px) {
  return px * PX_TO_PT;
}

export function ptToPx(pt) {
  return pt * PT_TO_PX;
}
