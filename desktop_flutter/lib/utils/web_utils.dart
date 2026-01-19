// Web用ユーティリティ
// ignore: avoid_web_libraries_in_flutter
import 'dart:html' as html;
import 'dart:convert';

/// Web: ファイルをダウンロード
void downloadFile(String content, String fileName, {String mimeType = 'application/json'}) {
  final bytes = utf8.encode(content);
  final blob = html.Blob([bytes], mimeType);
  final url = html.Url.createObjectUrlFromBlob(blob);

  final anchor = html.AnchorElement(href: url)
    ..setAttribute('download', fileName)
    ..style.display = 'none';

  html.document.body!.children.add(anchor);
  anchor.click();

  // クリーンアップ
  html.document.body!.children.remove(anchor);
  html.Url.revokeObjectUrl(url);
}
