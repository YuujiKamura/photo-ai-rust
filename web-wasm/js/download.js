// download.js
// ブラウザダウンロード処理

/**
 * ファイル名から不正な文字を除去する
 * @param {string} filename - サニタイズするファイル名
 * @returns {string} - サニタイズされたファイル名
 */
function sanitizeFilename(filename) {
  if (!filename || typeof filename !== 'string') {
    return 'download';
  }

  // Windows/Unix で禁止されている文字を除去
  // < > : " / \ | ? * および制御文字 (0x00-0x1F)
  let sanitized = filename.replace(/[<>:"/\\|?*\x00-\x1F]/g, '');

  // 先頭・末尾の空白とドットを除去
  sanitized = sanitized.replace(/^[\s.]+|[\s.]+$/g, '');

  // 連続するスペースを1つに
  sanitized = sanitized.replace(/\s+/g, ' ');

  // 空になった場合はデフォルト名を返す
  if (!sanitized) {
    return 'download';
  }

  // ファイル名の長さ制限（拡張子含めて255文字以内）
  if (sanitized.length > 200) {
    // 拡張子を保持しつつ切り詰め
    const lastDot = sanitized.lastIndexOf('.');
    if (lastDot > 0 && lastDot > sanitized.length - 10) {
      const ext = sanitized.slice(lastDot);
      const name = sanitized.slice(0, lastDot);
      sanitized = name.slice(0, 200 - ext.length) + ext;
    } else {
      sanitized = sanitized.slice(0, 200);
    }
  }

  return sanitized;
}

/**
 * Uint8Array データをファイルとしてダウンロードする
 * @param {Uint8Array} data - ダウンロードするバイナリデータ
 * @param {string} filename - ダウンロードファイル名
 * @param {string} mimeType - MIMEタイプ
 */
export function downloadBlob(data, filename, mimeType) {
  // 入力検証
  if (!(data instanceof Uint8Array)) {
    throw new Error('data must be a Uint8Array');
  }
  if (!filename || typeof filename !== 'string') {
    throw new Error('filename must be a non-empty string');
  }
  if (!mimeType || typeof mimeType !== 'string') {
    throw new Error('mimeType must be a non-empty string');
  }

  // ファイル名をサニタイズ
  const safeFilename = sanitizeFilename(filename);

  // Uint8Array から Blob を作成
  const blob = new Blob([data], { type: mimeType });

  // ダウンロード用のオブジェクト URL を作成
  const url = URL.createObjectURL(blob);

  // ダウンロードリンクを作成
  const link = document.createElement('a');
  link.href = url;
  link.download = safeFilename;

  // リンクをDOMに追加（一部ブラウザで必要）
  link.style.display = 'none';
  document.body.appendChild(link);

  // 自動クリックでダウンロードを開始
  link.click();

  // クリーンアップ: リンクをDOMから削除し、オブジェクトURLを解放
  // setTimeout を使用して、ダウンロードが開始されてからリソースを解放
  setTimeout(() => {
    document.body.removeChild(link);
    URL.revokeObjectURL(url);
  }, 100);
}

/**
 * PDFファイルをダウンロードする
 * @param {Uint8Array} data - PDFバイナリデータ
 * @param {string} filename - ダウンロードファイル名
 */
export function downloadPdf(data, filename) {
  downloadBlob(data, filename, 'application/pdf');
}

/**
 * Excelファイル (.xlsx) をダウンロードする
 * @param {Uint8Array} data - Excelバイナリデータ
 * @param {string} filename - ダウンロードファイル名
 */
export function downloadExcel(data, filename) {
  downloadBlob(data, filename, 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet');
}
