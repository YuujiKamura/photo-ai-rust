// pdf-bridge.js
// PDF生成 (pdf-lib使用)

// ============================================
// レイアウト定数 (layout-constants.generated.js から読み込み)
// ============================================

import {
  MM_TO_PT,
  PDF_GAP_PT,
  PDF_BASE_FONT_SIZE,
  PDF_LINE_HEIGHT_MULTIPLIER,
  PDF_TEXT_PADDING,
  PDF_TITLE_FONT_SIZE,
  PDF_PAGE_NUM_FONT_SIZE
} from './layout-constants.generated.js';

let cachedFont = null;

/**
 * フォントを事前読み込みしてキャッシュ
 * @param {string} fontUrl - フォントファイルのURL
 * @returns {Promise<void>}
 */
export async function loadFont(fontUrl) {
  try {
    const response = await fetch(fontUrl);
    if (!response.ok) {
      throw new Error(`Failed to fetch font: ${response.status} ${response.statusText}`);
    }
    const fontBytes = await response.arrayBuffer();
    cachedFont = fontBytes;
    console.log('Font loaded and cached successfully');
  } catch (error) {
    console.error('Font loading error:', error);
    throw new Error(`フォント読み込みエラー: ${error.message}`);
  }
}

/**
 * PDFを生成
 * @param {string} photosJson - JsPhotoEntry[] のJSON文字列
 * @param {string} layoutJson - JsLayoutConfig のJSON文字列
 * @param {string} optionsJson - { title: string } のJSON文字列
 * @returns {Promise<Uint8Array>} PDFバイナリ
 */
export async function generatePdf(photosJson, layoutJson, optionsJson) {
  // グローバル変数のチェック
  if (typeof PDFLib === 'undefined') {
    throw new Error('PDFLib が読み込まれていません。pdf-lib をインクルードしてください。');
  }
  if (typeof fontkit === 'undefined') {
    throw new Error('fontkit が読み込まれていません。fontkit をインクルードしてください。');
  }

  const photos = JSON.parse(photosJson);
  const layout = JSON.parse(layoutJson);
  const options = JSON.parse(optionsJson);

  // レイアウト値をptに変換
  const pageWidth = layout.pageWidthMm * MM_TO_PT;
  const pageHeight = layout.pageHeightMm * MM_TO_PT;
  const margin = layout.marginMm * MM_TO_PT;
  const photoWidth = layout.photoWidthMm * MM_TO_PT;
  const photoHeight = layout.photoHeightMm * MM_TO_PT;
  const infoWidth = layout.infoWidthMm * MM_TO_PT;
  const photosPerPage = layout.photosPerPage || 2;

  // PDF作成
  const pdfDoc = await PDFLib.PDFDocument.create();
  pdfDoc.registerFontkit(fontkit);

  // フォント埋め込み
  let font;
  if (cachedFont) {
    try {
      font = await pdfDoc.embedFont(cachedFont);
    } catch (error) {
      console.warn('Cached font embedding failed, using standard font:', error);
      font = await pdfDoc.embedFont(PDFLib.StandardFonts.Helvetica);
    }
  } else {
    console.warn('No cached font available, using standard font');
    font = await pdfDoc.embedFont(PDFLib.StandardFonts.Helvetica);
  }

  // エントリ間の垂直スペースを計算
  const availableHeight = pageHeight - (2 * margin);
  const entryHeight = availableHeight / photosPerPage;
  const textPadding = PDF_TEXT_PADDING;
  const fontSize = PDF_BASE_FONT_SIZE;
  const lineHeight = fontSize * PDF_LINE_HEIGHT_MULTIPLIER;

  // 写真をページごとにグループ化して処理
  for (let pageIndex = 0; pageIndex < Math.ceil(photos.length / photosPerPage); pageIndex++) {
    const page = pdfDoc.addPage([pageWidth, pageHeight]);
    const startIndex = pageIndex * photosPerPage;
    const endIndex = Math.min(startIndex + photosPerPage, photos.length);

    // タイトルを描画（最初のページのみ）
    if (pageIndex === 0 && options.title) {
      const titleFontSize = PDF_TITLE_FONT_SIZE;
      const titleWidth = font.widthOfTextAtSize(options.title, titleFontSize);
      page.drawText(options.title, {
        x: (pageWidth - titleWidth) / 2,
        y: pageHeight - margin,
        size: titleFontSize,
        font: font,
        color: PDFLib.rgb(0, 0, 0),
      });
    }

    // 各エントリを描画
    for (let i = startIndex; i < endIndex; i++) {
      const photo = photos[i];
      const entryIndex = i - startIndex;
      const yOffset = pageHeight - margin - (entryIndex + 1) * entryHeight + entryHeight - photoHeight - 20;

      const imageDataUrl = photo.imageDataUrl || photo.filePath;
      // 写真を埋め込み
      if (imageDataUrl) {
        try {
          const image = await embedImage(pdfDoc, imageDataUrl);
          if (image) {
            // アスペクト比を維持してサイズを調整
            const imgDims = image.scale(1);
            const scale = Math.min(
              photoWidth / imgDims.width,
              photoHeight / imgDims.height
            );
            const scaledWidth = imgDims.width * scale;
            const scaledHeight = imgDims.height * scale;

            // 中央配置
            const imgX = margin + (photoWidth - scaledWidth) / 2;
            const imgY = yOffset + (photoHeight - scaledHeight) / 2;

            page.drawImage(image, {
              x: imgX,
              y: imgY,
              width: scaledWidth,
              height: scaledHeight,
            });
          }
        } catch (error) {
          console.warn(`Image embedding failed for ${photo.fileName}:`, error);
        }
      }

      // 写真枠を描画
      page.drawRectangle({
        x: margin,
        y: yOffset,
        width: photoWidth,
        height: photoHeight,
        borderColor: PDFLib.rgb(0.7, 0.7, 0.7),
        borderWidth: 0.5,
      });

      // 情報テキストを描画
      const textX = margin + photoWidth + textPadding;
      let textY = yOffset + photoHeight - lineHeight;

      const infoLines = [
        { label: 'ファイル名', value: photo.fileName || '' },
        { label: '撮影日', value: photo.date || '' },
        { label: '工種', value: photo.workType || '' },
        { label: '種別', value: photo.variety || '' },
        { label: '作業段階', value: photo.subphase || photo.detail || '' },
        { label: '測点', value: photo.station || '' },
        { label: '写真区分', value: photo.photoCategory || '' },
        { label: '計測値', value: photo.measurements || '' },
        { label: '備考', value: photo.remarks || '' },
      ];

      for (const info of infoLines) {
        if (textY < yOffset + lineHeight) break; // ページ外には描画しない

        const text = `${info.label}: ${info.value}`;
        const truncatedText = truncateText(text, infoWidth - textPadding * 2, font, fontSize);

        page.drawText(truncatedText, {
          x: textX,
          y: textY,
          size: fontSize,
          font: font,
          color: PDFLib.rgb(0, 0, 0),
        });
        textY -= lineHeight;
      }

      // 情報エリアの枠を描画
      page.drawRectangle({
        x: margin + photoWidth,
        y: yOffset,
        width: infoWidth,
        height: photoHeight,
        borderColor: PDFLib.rgb(0.7, 0.7, 0.7),
        borderWidth: 0.5,
      });
    }

    // ページ番号を描画
    const pageNumText = `${pageIndex + 1} / ${Math.ceil(photos.length / photosPerPage)}`;
    const pageNumWidth = font.widthOfTextAtSize(pageNumText, PDF_PAGE_NUM_FONT_SIZE);
    page.drawText(pageNumText, {
      x: (pageWidth - pageNumWidth) / 2,
      y: margin / 2,
      size: PDF_PAGE_NUM_FONT_SIZE,
      font: font,
      color: PDFLib.rgb(0.5, 0.5, 0.5),
    });
  }

  // PDFをバイナリとして出力
  const pdfBytes = await pdfDoc.save();
  return new Uint8Array(pdfBytes);
}

/**
 * 画像をPDFに埋め込む
 * @param {PDFDocument} pdfDoc
 * @param {string} dataUrl - base64 data URL
 * @returns {Promise<PDFImage|null>}
 */
async function embedImage(pdfDoc, dataUrl) {
  if (!dataUrl || !dataUrl.startsWith('data:')) {
    return null;
  }

  try {
    // data URLからMIMEタイプとbase64データを抽出
    const matches = dataUrl.match(/^data:([^;]+);base64,(.+)$/);
    if (!matches) {
      console.warn('Invalid data URL format');
      return null;
    }

    const mimeType = matches[1];
    const base64Data = matches[2];
    const imageBytes = Uint8Array.from(atob(base64Data), c => c.charCodeAt(0));

    if (mimeType === 'image/jpeg' || mimeType === 'image/jpg') {
      return await pdfDoc.embedJpg(imageBytes);
    } else if (mimeType === 'image/png') {
      return await pdfDoc.embedPng(imageBytes);
    } else {
      console.warn(`Unsupported image type: ${mimeType}`);
      return null;
    }
  } catch (error) {
    console.error('Image embedding error:', error);
    return null;
  }
}

/**
 * テキストを指定幅に収まるよう切り詰め
 * @param {string} text
 * @param {number} maxWidth
 * @param {PDFFont} font
 * @param {number} fontSize
 * @returns {string}
 */
function truncateText(text, maxWidth, font, fontSize) {
  if (!text) return '';

  try {
    let width = font.widthOfTextAtSize(text, fontSize);
    if (width <= maxWidth) {
      return text;
    }

    // 省略記号付きで切り詰め
    let truncated = text;
    while (truncated.length > 0 && width > maxWidth) {
      truncated = truncated.slice(0, -1);
      width = font.widthOfTextAtSize(truncated + '...', fontSize);
    }
    return truncated + '...';
  } catch {
    // フォントが日本語に対応していない場合は元のテキストを返す
    return text.length > 20 ? text.slice(0, 20) + '...' : text;
  }
}
