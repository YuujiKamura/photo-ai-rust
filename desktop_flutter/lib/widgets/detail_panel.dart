import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../models/result_item.dart';

class DetailPanel extends StatefulWidget {
  const DetailPanel({
    required this.item,
    this.selectedCount = 1,
    this.onUpdate,
    super.key,
  });

  final ResultItem? item;
  final int selectedCount;
  final void Function(String key, String value)? onUpdate;

  @override
  State<DetailPanel> createState() => _DetailPanelState();
}

class _DetailPanelState extends State<DetailPanel> {
  final Map<String, TextEditingController> _controllers = {};

  @override
  void initState() {
    super.initState();
    _syncControllers(widget.item);
  }

  @override
  void didUpdateWidget(covariant DetailPanel oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.item?.filePath != oldWidget.item?.filePath ||
        widget.item != oldWidget.item) {
      _syncControllers(widget.item);
    }
  }

  @override
  void dispose() {
    for (final controller in _controllers.values) {
      controller.dispose();
    }
    super.dispose();
  }

  void _syncControllers(ResultItem? item) {
    if (item == null) return;
    _setControllerText('date', item.date);
    _setControllerText('photoCategory', item.photoCategory);
    _setControllerText('workType', item.workType);
    _setControllerText('variety', item.variety);
    _setControllerText('subphase', item.subphase ?? '');
    _setControllerText('remarks', item.remarks);
    _setControllerText('station', item.station);
    _setControllerText('measurements', item.measurements);
    _setControllerText('description', item.description);
  }

  TextEditingController _controllerFor(String key) {
    return _controllers.putIfAbsent(key, () => TextEditingController());
  }

  void _setControllerText(String key, String value) {
    final controller = _controllerFor(key);
    if (controller.text != value) {
      controller.text = value;
    }
  }

  Widget _buildEditableRow({
    required String label,
    required String key,
    required String value,
    int maxLines = 1,
  }) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 6),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(label, style: const TextStyle(color: Colors.white70)),
          const SizedBox(height: 4),
          TextField(
            controller: _controllerFor(key),
            maxLines: maxLines,
            style: const TextStyle(color: Colors.white),
            decoration: const InputDecoration(
              isDense: true,
              filled: true,
            ),
            onChanged: (value) => widget.onUpdate?.call(key, value),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final item = widget.item;
    if (item == null) {
      return const Center(child: Text('Select a card'));
    }
    final canEdit = widget.onUpdate != null && widget.selectedCount == 1;
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: const BoxDecoration(
        color: Color(0xFF0F121A),
      ),
      child: SelectionArea(
        child: ListView(
          children: [
            if (widget.selectedCount > 1)
              Padding(
                padding: const EdgeInsets.only(bottom: 8),
                child: Text(
                  '${widget.selectedCount} 件選択中',
                  style: const TextStyle(
                    fontSize: 14,
                    fontWeight: FontWeight.bold,
                    color: Color(0xFFF6C445),
                  ),
                ),
              ),
            if (!canEdit)
              const Padding(
                padding: EdgeInsets.only(bottom: 8),
                child: Text(
                  '複数選択中は編集できません',
                  style: TextStyle(fontSize: 12, color: Colors.white70),
                ),
              ),
            _DetailRow(
              label: 'File Name',
              value: item.fileName,
              onCopyPath: item.filePath,
            ),
            const SizedBox(height: 8),
            if (canEdit)
              _buildEditableRow(label: 'Date', key: 'date', value: item.date),
            if (!canEdit) _DetailRow(label: 'Date', value: item.date),
            if (canEdit)
              _buildEditableRow(
                label: 'Photo Category',
                key: 'photoCategory',
                value: item.photoCategory,
              ),
            if (!canEdit)
              _DetailRow(label: 'Photo Category', value: item.photoCategory),
            if (canEdit)
              _buildEditableRow(
                label: 'Work Type',
                key: 'workType',
                value: item.workType,
              ),
            if (!canEdit) _DetailRow(label: 'Work Type', value: item.workType),
            if (canEdit)
              _buildEditableRow(label: 'Variety', key: 'variety', value: item.variety),
            if (!canEdit) _DetailRow(label: 'Variety', value: item.variety),
            if (canEdit)
              _buildEditableRow(
                label: '細別',
                key: 'subphase',
                value: item.subphase ?? '',
              ),
            if (!canEdit) _DetailRow(label: '細別', value: item.subphase ?? ''),
            if (canEdit)
              _buildEditableRow(
                label: 'Remarks',
                key: 'remarks',
                value: item.remarks,
                maxLines: 2,
              ),
            if (!canEdit) _DetailRow(label: 'Remarks', value: item.remarks),
            if (canEdit)
              _buildEditableRow(
                label: 'Station',
                key: 'station',
                value: item.station,
              ),
            if (!canEdit) _DetailRow(label: 'Station', value: item.station),
            if (canEdit)
              _buildEditableRow(
                label: 'Measurements',
                key: 'measurements',
                value: item.measurements,
                maxLines: 2,
              ),
            if (!canEdit) _DetailRow(label: 'Measurements', value: item.measurements),
            _DetailRow(label: 'Detected Text', value: item.detectedText),
            _DetailRow(label: 'Has Board', value: item.hasBoard ? 'true' : 'false'),
            if (canEdit)
              _buildEditableRow(
                label: 'Description',
                key: 'description',
                value: item.description,
                maxLines: 3,
              ),
            if (!canEdit) _DetailRow(label: 'Description', value: item.description),
            _DetailRow(label: 'Reasoning', value: item.reasoning),
          ],
        ),
      ),
    );
  }
}

class _DetailRow extends StatelessWidget {
  const _DetailRow({required this.label, required this.value, this.onCopyPath});

  final String label;
  final String value;
  final String? onCopyPath;

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onSecondaryTapDown: (details) async {
        if (onCopyPath == null || onCopyPath!.isEmpty) return;
        final overlay = Overlay.of(context).context.findRenderObject() as RenderBox;
        final position = RelativeRect.fromRect(
          Rect.fromPoints(
            details.globalPosition,
            details.globalPosition,
          ),
          Offset.zero & overlay.size,
        );
        final selected = await showMenu<String>(
          context: context,
          position: position,
          items: [
            const PopupMenuItem(value: 'copy_path', child: Text('Copy Path')),
          ],
        );
        if (selected == 'copy_path') {
          await Clipboard.setData(ClipboardData(text: onCopyPath!));
          ScaffoldMessenger.of(context).showSnackBar(
            const SnackBar(content: Text('Path copied')),
          );
        }
      },
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 6),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(label, style: const TextStyle(color: Colors.white70)),
            const SizedBox(height: 4),
            SelectableText(
              value.isEmpty ? '-' : value,
              style: const TextStyle(color: Colors.white),
            ),
          ],
        ),
      ),
    );
  }
}
