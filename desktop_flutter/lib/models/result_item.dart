import 'dart:convert';

class ResultItem {
  ResultItem({
    required this.fileName,
    required this.filePath,
    required this.date,
    required this.photoCategory,
    required this.workType,
    required this.variety,
    required this.subphase,
    required this.remarks,
    required this.station,
    required this.description,
    required this.measurements,
    required this.detectedText,
    required this.hasBoard,
    required this.reasoning,
  });

  final String fileName;
  final String filePath;
  final String date;
  final String photoCategory;
  final String workType;
  final String variety;
  final String? subphase;
  final String remarks;
  final String station;
  final String description;
  final String measurements;
  final String detectedText;
  final bool hasBoard;
  final String reasoning;

  String valueByKey(String key) {
    switch (key) {
      case 'date':
        return date;
      case 'photoCategory':
        return photoCategory;
      case 'workType':
        return workType;
      case 'variety':
        return variety;
      case 'subphase':
        return subphase ?? '';
      case 'station':
        return station;
      case 'remarks':
        return remarks;
      case 'measurements':
        return measurements;
    }
    return '';
  }

  Map<String, dynamic> toJson() {
    return {
      'fileName': fileName,
      'filePath': filePath,
      'date': date,
      'photoCategory': photoCategory,
      'workType': workType,
      'variety': variety,
      'subphase': subphase,
      'remarks': remarks,
      'station': station,
      'description': description,
      'measurements': measurements,
      'detectedText': detectedText,
      'hasBoard': hasBoard,
      'reasoning': reasoning,
    };
  }

  factory ResultItem.fromJson(Map<String, dynamic> json) {
    return ResultItem(
      fileName: json['fileName']?.toString() ?? '',
      filePath: json['filePath']?.toString() ?? '',
      date: json['date']?.toString() ?? '',
      photoCategory: json['photoCategory']?.toString() ?? '',
      workType: json['workType']?.toString() ?? '',
      variety: json['variety']?.toString() ?? '',
      subphase: (json['subphase'] ?? json['detail'])?.toString() ?? '',
      remarks: json['remarks']?.toString() ?? '',
      station: json['station']?.toString() ?? '',
      description: json['description']?.toString() ?? '',
      measurements: json['measurements']?.toString() ?? '',
      detectedText: json['detectedText']?.toString() ?? '',
      hasBoard: json['hasBoard'] == true,
      reasoning: json['reasoning']?.toString() ?? '',
    );
  }

  ResultItem copyWith({
    String? date,
    String? photoCategory,
    String? workType,
    String? variety,
    String? subphase,
    String? remarks,
    String? station,
    String? description,
    String? measurements,
    String? detectedText,
    bool? hasBoard,
    String? reasoning,
  }) {
    return ResultItem(
      fileName: fileName,
      filePath: filePath,
      date: date ?? this.date,
      photoCategory: photoCategory ?? this.photoCategory,
      workType: workType ?? this.workType,
      variety: variety ?? this.variety,
      subphase: subphase ?? this.subphase,
      remarks: remarks ?? this.remarks,
      station: station ?? this.station,
      description: description ?? this.description,
      measurements: measurements ?? this.measurements,
      detectedText: detectedText ?? this.detectedText,
      hasBoard: hasBoard ?? this.hasBoard,
      reasoning: reasoning ?? this.reasoning,
    );
  }
}

class ClipboardPayload {
  const ClipboardPayload({required this.type, required this.data});

  final String type;
  final Map<String, String> data;

  String toJsonString() {
    return jsonEncode({'type': type, 'data': data});
  }
}
