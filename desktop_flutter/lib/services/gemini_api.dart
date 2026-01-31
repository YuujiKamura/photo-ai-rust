import 'dart:convert';
import 'package:http/http.dart' as http;

part 'gemini_api_step2.dart';

/// Gemini API サービス (Web版解析用)
class GeminiApi {
  final String apiKey;

  static const String apiUrl =
    'https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash-exp:generateContent';

  GeminiApi({required this.apiKey});

  /// 写真を解析 (2段階解析)
  Future<Map<String, dynamic>> analyzePhoto({
    required String base64Image,
    required String mimeType,
    required String fileName,
    String? masterCsv,
  }) async {
    // Step 1: 画像認識
    final step1Result = await _analyzeStep1(
      base64Image: base64Image,
      mimeType: mimeType,
      fileName: fileName,
    );

    // Step 2: マスタ照合 (マスタがあれば)
    if (masterCsv != null && masterCsv.isNotEmpty) {
      return await GeminiStep2.analyze(
        this,
        rawData: step1Result,
        masterCsv: masterCsv,
      );
    }

    return step1Result;
  }

  /// Step 1: 画像認識
  Future<Map<String, dynamic>> _analyzeStep1({
    required String base64Image,
    required String mimeType,
    required String fileName,
  }) async {
    final prompt = _buildStep1Prompt();

    final request = {
      'contents': [
        {
          'parts': [
            {'text': prompt},
            {
              'inline_data': {
                'mime_type': mimeType,
                'data': base64Image,
              }
            }
          ]
        }
      ],
      'generationConfig': {
        'temperature': 0.1,
        'responseMimeType': 'application/json',
      }
    };

    final response = await _callApi(request);
    final result = _parseResponse(response);

    // fileNameを追加
    result['fileName'] = fileName;

    return result;
  }


  /// API呼び出し
  Future<String> _callApi(Map<String, dynamic> request) async {
    final uri = Uri.parse('$apiUrl?key=$apiKey');

    final response = await http.post(
      uri,
      headers: {'Content-Type': 'application/json'},
      body: jsonEncode(request),
    );

    if (response.statusCode != 200) {
      throw Exception('Gemini API error: ${response.statusCode} - ${response.body}');
    }

    return response.body;
  }

  /// レスポンスパース
  Map<String, dynamic> _parseResponse(String responseBody) {
    final json = jsonDecode(responseBody);
    final candidates = json['candidates'] as List?;

    if (candidates == null || candidates.isEmpty) {
      throw Exception('No candidates in response');
    }

    final content = candidates[0]['content'];
    final parts = content['parts'] as List?;

    if (parts == null || parts.isEmpty) {
      throw Exception('No parts in response');
    }

    final text = parts[0]['text'] as String;

    // JSONを抽出 (```json ... ``` の場合も対応)
    final extracted = _extractJson(text);
    return jsonDecode(extracted);
  }

  /// JSONブロックを抽出
  String _extractJson(String text) {
    // ```json ... ``` パターン
    final jsonBlockRegex = RegExp(r'```json\s*([\s\S]*?)\s*```');
    final match = jsonBlockRegex.firstMatch(text);
    if (match != null) {
      return match.group(1)!;
    }

    // { ... } パターン
    final braceStart = text.indexOf('{');
    final braceEnd = text.lastIndexOf('}');
    if (braceStart != -1 && braceEnd > braceStart) {
      return text.substring(braceStart, braceEnd + 1);
    }

    return text;
  }

  /// Step 1 プロンプト
  String _buildStep1Prompt() {
    return '''
この工事写真を解析し、以下のJSON形式で情報を抽出してください。

{
  "photoCategory": "写真区分 (着手前及び完成写真/施工状況写真/安全管理写真/使用材料写真/品質管理写真/出来形管理写真)",
  "workType": "工種",
  "variety": "種別",
  "detail": "細別",
  "station": "測点 (例: No.10+5.0)",
  "measurements": "測定値・寸法 (例: 厚さ50mm)",
  "hasBoard": true/false,
  "detectedText": "黒板や看板から読み取れるテキスト",
  "description": "写真の説明"
}

注意:
- 黒板がある場合は黒板の内容を優先して読み取る
- 測点はNo.XX+XX.X形式で統一
- 読み取れない項目は空文字""にする
''';
  }

}

/// 画像をBase64に変換するユーティリティ
class ImageUtils {
  /// Uint8List → Base64
  static String bytesToBase64(List<int> bytes) {
    return base64Encode(bytes);
  }

  /// MIMEタイプを拡張子から推定
  static String getMimeType(String fileName) {
    final ext = fileName.toLowerCase().split('.').last;
    switch (ext) {
      case 'jpg':
      case 'jpeg':
        return 'image/jpeg';
      case 'png':
        return 'image/png';
      case 'gif':
        return 'image/gif';
      case 'webp':
        return 'image/webp';
      default:
        return 'image/jpeg';
    }
  }
}
