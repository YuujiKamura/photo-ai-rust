import 'package:flutter/material.dart';

import '../models/result_item.dart';

class FieldDef {
  const FieldDef({required this.key, required this.label});

  final String key;
  final String label;
}

class FieldGrid extends StatelessWidget {
  const FieldGrid({required this.item, super.key});

  final ResultItem item;

  static const fields = [
    FieldDef(key: 'date', label: '日時'),
    FieldDef(key: 'photoCategory', label: '区分'),
    FieldDef(key: 'workType', label: '工種'),
    FieldDef(key: 'variety', label: '種別'),
    FieldDef(key: 'subphase', label: '細別'),
    FieldDef(key: 'station', label: '測点'),
    FieldDef(key: 'remarks', label: '備考'),
    FieldDef(key: 'measurements', label: '測定値'),
  ];

  @override
  Widget build(BuildContext context) {
    return Column(
      children: fields
          .map(
            (field) => Row(
              children: [
                SizedBox(
                  width: 60,
                  child: Text(
                    field.label,
                    style: const TextStyle(fontSize: 12, color: Colors.white70),
                  ),
                ),
                Expanded(
                  child: Text(
                    item.valueByKey(field.key).isEmpty
                        ? '-'
                        : item.valueByKey(field.key),
                    style: const TextStyle(fontSize: 12, color: Colors.white),
                  ),
                ),
              ],
            ),
          )
          .toList(),
    );
  }
}
