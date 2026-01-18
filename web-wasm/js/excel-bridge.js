// excel-bridge.js
// Excel生成 (ExcelJS使用)
//
// ExcelJSはグローバル変数として読み込まれている前提
// <script src="https://cdn.jsdelivr.net/npm/exceljs/dist/exceljs.min.js"></script>

/**
 * 写真データからExcelファイルを生成
 * @param {string} photosJson - JsPhotoEntry[] のJSON文字列
 * @param {string} optionsJson - { title: string, photosPerPage: number } のJSON文字列
 * @returns {Promise<Uint8Array>} Excelバイナリ
 */
export async function generateExcel(photosJson, optionsJson) {
  // 入力データのパース
  const photos = JSON.parse(photosJson);
  const options = JSON.parse(optionsJson);

  // ExcelJSの存在確認
  if (typeof ExcelJS === 'undefined') {
    throw new Error('ExcelJS is not loaded. Please include ExcelJS library.');
  }

  // ワークブック作成
  const workbook = new ExcelJS.Workbook();
  workbook.creator = 'Photo AI';
  workbook.created = new Date();

  // ワークシート作成
  const sheetTitle = options.title || '写真台帳';
  const sheet = workbook.addWorksheet(sheetTitle);

  // 列幅設定
  sheet.columns = [
    { key: 'photo', width: 25 },       // A: 写真
    { key: 'fileName', width: 20 },    // B: ファイル名
    { key: 'date', width: 12 },        // C: 日付
    { key: 'workType', width: 15 },    // D: 工種
    { key: 'variety', width: 15 },     // E: 種別
    { key: 'detail', width: 15 },      // F: 細別
    { key: 'station', width: 12 },     // G: 測点
    { key: 'photoCategory', width: 15 }, // H: 写真区分
    { key: 'measurements', width: 15 }, // I: 計測値
    { key: 'remarks', width: 30 },     // J: 備考
  ];

  // ヘッダー行の設定
  const headerRow = sheet.addRow([
    '写真',
    'ファイル名',
    '日付',
    '工種',
    '種別',
    '細別',
    '測点',
    '写真区分',
    '計測値',
    '備考',
  ]);

  // ヘッダー行のスタイル設定
  headerRow.height = 25;
  headerRow.eachCell((cell) => {
    cell.fill = {
      type: 'pattern',
      pattern: 'solid',
      fgColor: { argb: 'FF4472C4' }, // 青色背景
    };
    cell.font = {
      bold: true,
      color: { argb: 'FFFFFFFF' }, // 白色文字
      size: 11,
    };
    cell.alignment = {
      horizontal: 'center',
      vertical: 'middle',
    };
    cell.border = {
      top: { style: 'thin', color: { argb: 'FF000000' } },
      bottom: { style: 'thin', color: { argb: 'FF000000' } },
      left: { style: 'thin', color: { argb: 'FF000000' } },
      right: { style: 'thin', color: { argb: 'FF000000' } },
    };
  });

  // データ行の追加
  for (let i = 0; i < photos.length; i++) {
    const photo = photos[i];
    const rowNumber = i + 2; // ヘッダーが1行目なので2行目から

    // データ行を追加
    const dataRow = sheet.addRow([
      '', // 写真列は後で画像を埋め込む
      photo.fileName || '',
      photo.date || '',
      photo.workType || '',
      photo.variety || '',
      photo.detail || '',
      photo.station || '',
      photo.photoCategory || '',
      photo.measurements || '',
      photo.remarks || '',
    ]);

    // 行高さを画像用に調整（約100ピクセル）
    dataRow.height = 75;

    // データ行のスタイル設定
    dataRow.eachCell((cell, colNumber) => {
      cell.alignment = {
        horizontal: colNumber === 1 ? 'center' : 'left',
        vertical: 'middle',
        wrapText: true,
      };
      cell.border = {
        top: { style: 'thin', color: { argb: 'FF000000' } },
        bottom: { style: 'thin', color: { argb: 'FF000000' } },
        left: { style: 'thin', color: { argb: 'FF000000' } },
        right: { style: 'thin', color: { argb: 'FF000000' } },
      };
      cell.font = {
        size: 10,
      };
    });

    // 偶数行に薄い背景色
    if (i % 2 === 1) {
      dataRow.eachCell((cell) => {
        cell.fill = {
          type: 'pattern',
          pattern: 'solid',
          fgColor: { argb: 'FFF2F2F2' }, // 薄いグレー
        };
      });
    }

    // 画像の埋め込み
    if (photo.imageDataUrl) {
      try {
        const imageId = await addImageToWorkbook(workbook, photo.imageDataUrl);
        if (imageId !== null) {
          sheet.addImage(imageId, {
            tl: { col: 0, row: rowNumber - 1 }, // 左上位置
            ext: { width: 140, height: 90 },    // サイズ（ピクセル）
          });
        }
      } catch (err) {
        console.warn(`Failed to embed image for ${photo.fileName}:`, err.message);
        // 画像埋め込み失敗時はスキップ（エラーにしない）
      }
    }
  }

  // フィルター設定（ヘッダー行）
  sheet.autoFilter = {
    from: { row: 1, column: 1 },
    to: { row: 1, column: 10 },
  };

  // 印刷設定
  sheet.pageSetup = {
    orientation: 'landscape',
    fitToPage: true,
    fitToWidth: 1,
    fitToHeight: 0,
    paperSize: 9, // A4
    margins: {
      left: 0.5,
      right: 0.5,
      top: 0.75,
      bottom: 0.75,
      header: 0.3,
      footer: 0.3,
    },
  };

  // ヘッダー行を固定（スクロール時に常に表示）
  sheet.views = [
    {
      state: 'frozen',
      xSplit: 0,
      ySplit: 1,
    },
  ];

  // バッファに書き出し
  const buffer = await workbook.xlsx.writeBuffer();
  return new Uint8Array(buffer);
}

/**
 * Base64 Data URLから画像をワークブックに追加
 * @param {ExcelJS.Workbook} workbook
 * @param {string} dataUrl - base64 data URL (data:image/jpeg;base64,...)
 * @returns {Promise<number|null>} 画像ID または null
 */
async function addImageToWorkbook(workbook, dataUrl) {
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
 * 簡易的なExcel生成（画像なし、テスト用）
 * @param {Object[]} photos - 写真データ配列
 * @param {Object} options - オプション
 * @returns {Promise<Uint8Array>}
 */
export async function generateExcelSimple(photos, options) {
  if (typeof ExcelJS === 'undefined') {
    throw new Error('ExcelJS is not loaded.');
  }

  const workbook = new ExcelJS.Workbook();
  const sheet = workbook.addWorksheet(options.title || '写真台帳');

  // シンプルなヘッダー
  sheet.addRow(['No.', 'ファイル名', '日付', '工種', '備考']);

  // データ追加
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
