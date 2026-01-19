// excel-bridge.js
// Excel生成 (ExcelJS使用)
// GASPhotoAIManager/shared/generators/excelCore.ts をベースに移植
//
// ExcelJSはグローバル変数として読み込まれている前提
// <script src="https://unpkg.com/exceljs@4.4.0/dist/exceljs.min.js"></script>

// ============================================
// レイアウト定数 (layoutConfig.ts から移植)
// ============================================

const MM_TO_PT = 2.835;

// A4サイズ
const A4_WIDTH_MM = 210;
const A4_HEIGHT_MM = 297;
const MARGIN_MM = 10;
const PHOTO_GAP_MM = 10;

// 比率
const IMAGE_RATIO = 0.65;
const INFO_RATIO = 0.35;

// 計算値
const USABLE_WIDTH_MM = A4_WIDTH_MM - MARGIN_MM * 2;  // 190mm
const PHOTO_WIDTH_MM = USABLE_WIDTH_MM * IMAGE_RATIO;  // 123.5mm
const PHOTO_HEIGHT_MM = (A4_HEIGHT_MM - MARGIN_MM * 2 - PHOTO_GAP_MM * 2) / 3;  // 85.67mm

// Excel用設定
const SCALE = 1.1;
const PHOTO_ROWS = 10;  // 写真部分の行数
const GAP_ROWS = 1;     // 余白行数
const ROWS_PER_BLOCK = PHOTO_ROWS + GAP_ROWS;  // 11行/ブロック

const PHOTO_HEIGHT_PT = PHOTO_HEIGHT_MM * MM_TO_PT;
const ROW_HEIGHT_PT = Math.floor((PHOTO_HEIGHT_PT / PHOTO_ROWS) * SCALE);  // 26pt

// 列幅
const COL_A_WIDTH = 56.1;  // 写真列
const COL_B_WIDTH = 11;    // ラベル列
const COL_C_WIDTH = 28.6;  // 値列

// ============================================
// フィールド定義 (LAYOUT_FIELDS)
// ============================================

const LAYOUT_FIELDS = [
  { key: 'date', label: '日時', rowSpan: 1 },
  { key: 'photoCategory', label: '区分', rowSpan: 1 },
  { key: 'workType', label: '工種', rowSpan: 1 },
  { key: 'variety', label: '種別', rowSpan: 1 },
  { key: 'subphase', label: '作業段階', rowSpan: 1 },
  { key: 'station', label: '測点', rowSpan: 1 },
  { key: 'remarks', label: '備考', rowSpan: 2 },
  { key: 'measurements', label: '測定値', rowSpan: 2 },
];

// 2枚/ページ用のフィールド（測点と備考のみ）
const LAYOUT_FIELDS_2UP = LAYOUT_FIELDS.filter(
  f => f.key === 'station' || f.key === 'remarks'
);

// ============================================
// メイン関数
// ============================================

/**
 * 写真データからExcelファイルを生成（写真台帳形式）
 * @param {string} photosJson - JsPhotoEntry[] のJSON文字列
 * @param {string} optionsJson - { title: string, photosPerPage: number } のJSON文字列
 * @returns {Promise<Uint8Array>} Excelバイナリ
 */
export async function generateExcel(photosJson, optionsJson) {
  const photos = JSON.parse(photosJson);
  const options = JSON.parse(optionsJson);

  if (typeof ExcelJS === 'undefined') {
    throw new Error('ExcelJS is not loaded. Please include ExcelJS library.');
  }

  const photosPerPage = options.photosPerPage || 3;
  const title = options.title || '写真台帳';

  const workbook = new ExcelJS.Workbook();
  workbook.creator = 'Photo AI';
  workbook.created = new Date();

  const totalPages = Math.ceil(photos.length / photosPerPage);

  // ページごとにシートを作成
  for (let pageNum = 0; pageNum < totalPages; pageNum++) {
    const pagePhotos = photos.slice(
      pageNum * photosPerPage,
      (pageNum + 1) * photosPerPage
    );
    const sheetName = `${pageNum + 1}`;

    const sheet = workbook.addWorksheet(sheetName, {
      pageSetup: {
        paperSize: 9, // A4
        orientation: 'portrait',
        fitToPage: true,
        fitToWidth: 1,
        fitToHeight: 1,
        horizontalCentered: true,
        verticalCentered: true,
        margins: {
          left: MARGIN_MM / 25.4,
          right: MARGIN_MM / 25.4,
          top: MARGIN_MM / 25.4,
          bottom: MARGIN_MM / 25.4,
          header: 0.2,
          footer: 0.2
        }
      },
      views: [{ showGridLines: false }]
    });

    // 列幅設定
    sheet.columns = [
      { width: COL_A_WIDTH },
      { width: COL_B_WIDTH },
      { width: COL_C_WIDTH }
    ];

    let currentRow = 1;

    // 写真を配置
    for (let i = 0; i < pagePhotos.length; i++) {
      const photo = pagePhotos[i];
      const startRow = currentRow;
      const endRow = startRow + ROWS_PER_BLOCK - 1;

      // 行高さ設定
      for (let r = startRow; r <= endRow; r++) {
        sheet.getRow(r).height = ROW_HEIGHT_PT;
      }

      // 画像セル（列A）- マージ
      const photoEndRow = endRow - GAP_ROWS;
      sheet.mergeCells(startRow, 1, photoEndRow, 1);
      const imgCell = sheet.getCell(startRow, 1);
      imgCell.border = {
        top: { style: 'thin', color: { argb: 'FFCCCCCC' } },
        left: { style: 'thin', color: { argb: 'FFCCCCCC' } },
        right: { style: 'thin', color: { argb: 'FFCCCCCC' } },
        bottom: { style: 'thin', color: { argb: 'FFCCCCCC' } }
      };

      const imageDataUrl = photo.imageDataUrl || photo.filePath;
      // 画像を追加
      if (imageDataUrl) {
        try {
          const imageId = addImageToWorkbook(workbook, imageDataUrl);
          if (imageId !== null) {
            sheet.addImage(imageId, {
              tl: { col: 0, row: startRow - 1 },
              br: { col: 1, row: startRow - 1 + PHOTO_ROWS },
              editAs: 'absolute'
            });
          }
        } catch (err) {
          console.warn(`Failed to embed image for ${photo.fileName}:`, err.message);
        }
      }

      // 情報フィールド（列B & C）
      const fields = photosPerPage === 2 ? LAYOUT_FIELDS_2UP : LAYOUT_FIELDS;
      let fieldRow = startRow;

      for (const field of fields) {
        let value = '';
        if (field.key === 'date') {
          value = photo.date || '';
        } else if (field.key === 'subphase') {
          value = photo.subphase || photo.detail || '';
        } else {
          value = photo[field.key] || '';
        }

        createFieldCell(sheet, fieldRow, field.label, value, field.rowSpan);
        fieldRow += field.rowSpan;
      }

      currentRow = endRow + 1;
    }
  }

  // Bufferとして返す
  const buffer = await workbook.xlsx.writeBuffer();
  return new Uint8Array(buffer);
}

/**
 * フィールドセルを作成
 */
function createFieldCell(sheet, row, label, value, rowSpan) {
  // ラベルセル（列B）
  const labelCell = sheet.getCell(row, 2);
  labelCell.value = label;
  labelCell.font = { bold: true, size: 9, color: { argb: 'FF555555' } };
  labelCell.alignment = { vertical: 'middle', horizontal: 'center' };
  labelCell.fill = {
    type: 'pattern',
    pattern: 'solid',
    fgColor: { argb: 'FFF5F5F5' }
  };
  labelCell.border = {
    top: { style: 'hair', color: { argb: 'FFAAAAAA' } },
    left: { style: 'hair', color: { argb: 'FFAAAAAA' } },
    right: { style: 'hair', color: { argb: 'FFAAAAAA' } },
    bottom: { style: 'hair', color: { argb: 'FFAAAAAA' } }
  };

  // 値セル（列C）
  const valueCell = sheet.getCell(row, 3);
  valueCell.value = value;
  valueCell.alignment = { vertical: 'middle', horizontal: 'left', wrapText: true };
  valueCell.font = { size: 11 };
  valueCell.border = {
    top: { style: 'hair', color: { argb: 'FFCCCCCC' } },
    right: { style: 'hair', color: { argb: 'FFCCCCCC' } },
    bottom: { style: 'hair', color: { argb: 'FFCCCCCC' } }
  };

  // 複数行の場合はマージ
  if (rowSpan > 1) {
    sheet.mergeCells(row, 2, row + rowSpan - 1, 2);
    sheet.mergeCells(row, 3, row + rowSpan - 1, 3);
  }
}

/**
 * Base64 Data URLから画像をワークブックに追加
 */
function addImageToWorkbook(workbook, dataUrl) {
  if (!dataUrl || !dataUrl.startsWith('data:image/')) {
    return null;
  }

  // Data URLからBase64部分を抽出
  const matches = dataUrl.match(/^data:image\/(png|jpeg|jpg|gif);base64,(.+)$/i);
  if (!matches) {
    return null;
  }

  const extension = matches[1].toLowerCase() === 'jpg' ? 'jpeg' : matches[1].toLowerCase();
  const base64Data = matches[2];

  // ExcelJSは 'jpeg', 'png', 'gif' をサポート
  const supportedExtensions = ['jpeg', 'png', 'gif'];
  if (!supportedExtensions.includes(extension)) {
    console.warn(`Unsupported image format: ${extension}`);
    return null;
  }

  // 画像をワークブックに追加
  const imageId = workbook.addImage({
    base64: base64Data,
    extension: extension,
  });

  return imageId;
}

/**
 * 簡易的なExcel生成（テスト用・後方互換）
 */
export async function generateExcelSimple(photos, options) {
  if (typeof ExcelJS === 'undefined') {
    throw new Error('ExcelJS is not loaded.');
  }

  const workbook = new ExcelJS.Workbook();
  const sheet = workbook.addWorksheet(options.title || '写真台帳');

  sheet.addRow(['No.', 'ファイル名', '日付', '工種', '備考']);

  photos.forEach((photo, index) => {
    sheet.addRow([
      index + 1,
      photo.fileName || '',
      photo.date || '',
      photo.workType || '',
      photo.remarks || '',
    ]);
  });

  const buffer = await workbook.xlsx.writeBuffer();
  return new Uint8Array(buffer);
}

// ============================================
// CLI エントリポイント (Node.js用)
// ============================================

// Node.js環境かどうかを判定
const isNode = typeof process !== 'undefined' && process.versions && process.versions.node;

if (isNode) {
  // Node.jsで実行された場合
  const args = process.argv.slice(2);

  if (args.length >= 2) {
    const inputJson = args[0];
    const outputPath = args[1];

    import('fs').then(async (fs) => {
      const path = await import('path');
      const ExcelJSModule = await import('exceljs');
      globalThis.ExcelJS = ExcelJSModule.default || ExcelJSModule;

      try {
        const inputData = JSON.parse(fs.readFileSync(inputJson, 'utf-8'));
        const photos = inputData.photos || inputData;
        const options = inputData.options || { title: '写真台帳', photosPerPage: 3 };

        // filePathがある場合はbase64に変換して埋め込み用にする
        for (const photo of photos) {
          if (photo.imageDataUrl || !photo.filePath) continue;
          if (!fs.existsSync(photo.filePath)) continue;
          const ext = path.extname(photo.filePath).replace('.', '').toLowerCase();
          const normalized = ext === 'jpg' ? 'jpeg' : ext;
          if (!['jpeg', 'png', 'gif'].includes(normalized)) continue;
          const buffer = fs.readFileSync(photo.filePath);
          photo.imageDataUrl = `data:image/${normalized};base64,${buffer.toString('base64')}`;
        }

        const buffer = await generateExcel(
          JSON.stringify(photos),
          JSON.stringify(options)
        );

        fs.writeFileSync(outputPath, Buffer.from(buffer));
        console.log(`✔ Excel generated: ${outputPath}`);
      } catch (err) {
        console.error(`❌ Excel generation failed: ${err.message}`);
        process.exit(1);
      }
    });
  }
}
