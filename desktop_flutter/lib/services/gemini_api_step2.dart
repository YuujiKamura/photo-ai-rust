part of 'gemini_api.dart';

class GeminiStep2 {
  static Future<Map<String, dynamic>> analyze(
    GeminiApi api, {
    required Map<String, dynamic> rawData,
    required String masterCsv,
  }) async {
    final prompt = _buildStep2Prompt(rawData, masterCsv);

    final request = {
      'contents': [
        {
          'parts': [
            {'text': prompt}
          ]
        }
      ],
      'generationConfig': {
        'temperature': 0.1,
        'responseMimeType': 'application/json',
      }
    };

    final response = await api._callApi(request);
    final result = api._parseResponse(response);

    return {
      ...rawData,
      ...result,
    };
  }

  static String _buildStep2Prompt(Map<String, dynamic> rawData, String masterCsv) {
    return '''
以下の解析結果をマスタデータと照合し、正しい工種階層に修正してください。

## 解析結果
${jsonEncode(rawData)}

## マスタデータ (CSV)
$masterCsv

## 出力形式
{
  "workType": "マスタに合致する工種",
  "variety": "マスタに合致する種別",
  "detail": "マスタに合致する細別",
  "photoCategory": "写真区分"
}

マスタに完全一致しなくても、最も近い項目を選択してください。
''';
  }
}
