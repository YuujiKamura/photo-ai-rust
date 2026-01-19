import 'dart:convert';
import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:path/path.dart' as p;
import 'package:csv/csv.dart';

void main() {
  runApp(const PhotoAiApp());
}

class PhotoAiApp extends StatelessWidget {
  const PhotoAiApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Photo AI Desktop Viewer',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        useMaterial3: true,
        brightness: Brightness.dark,
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xFF2F3545),
          brightness: Brightness.dark,
        ),
        scaffoldBackgroundColor: const Color(0xFF0F121A),
      ),
      home: const ViewerScreen(),
    );
  }
}

enum AnalyzeProvider { claude, codex }

enum ExportFormat { pdf, excel, both }

class ViewerScreen extends StatefulWidget {
  const ViewerScreen({super.key});

  @override
  State<ViewerScreen> createState() => _ViewerScreenState();
}

class _ViewerScreenState extends State<ViewerScreen>
    with SingleTickerProviderStateMixin {
  List<ResultItem> items = [];
  List<ResultItem> originalItems = [];
  int? selectedIndex;
  String? sourcePath;
  String status = '';
  String analyzeStatus = '';
  String exportStatus = '';
  String cliPath = '';
  final List<String> logs = [];
  AnalyzeProvider analyzeProvider = AnalyzeProvider.claude;
  ExportFormat exportFormat = ExportFormat.pdf;
  int batchSize = 5;
  bool analyzing = false;
  bool exporting = false;
  bool verboseAnalyze = true;
  String workTypeInput = '';
  String varietyInput = '';
  String stationInput = '';
  String stationOverride = '';
  List<String> workTypeOptions = [];
  AnimationController? _pulseController;
  Animation<Color?>? _pulseColor;

  @override
  void initState() {
    super.initState();
    _ensurePulse();
  }

  void _ensurePulse() {
    if (_pulseController != null) return;
    _pulseController = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 900),
    )..repeat(reverse: true);
    _pulseColor = TweenSequence<Color?>([
      TweenSequenceItem(
        tween: ColorTween(begin: const Color(0xFFE7C86C), end: const Color(0xFF9ED9FF)),
        weight: 1,
      ),
      TweenSequenceItem(
        tween: ColorTween(begin: const Color(0xFF9ED9FF), end: const Color(0xFFE7C86C)),
        weight: 1,
      ),
    ]).animate(_pulseController!);
  }

  @override
  void dispose() {
    _pulseController?.dispose();
    super.dispose();
  }

  Future<void> openJson() async {
    final result = await FilePicker.platform.pickFiles(
      type: FileType.custom,
      allowedExtensions: ['json'],
    );
    if (result == null || result.files.isEmpty) return;
    final path = result.files.single.path;
    if (path == null) return;
    await loadFromPath(path);
  }

  Future<void> reloadJson() async {
    if (sourcePath == null) {
      setStatus('No source file loaded');
      return;
    }
    await loadFromPath(sourcePath!);
  }

  Future<void> loadFromPath(String path) async {
    try {
      final file = File(path);
      final content = await file.readAsString();
      final data = jsonDecode(content);
      if (data is! List) {
        setStatus('result.json should be an array');
        return;
      }
      items = data
          .map((e) => ResultItem.fromJson(e as Map<String, dynamic>))
          .toList();
      if (stationOverride.isNotEmpty) {
        items = items.map((item) => item.copyWith(station: stationOverride)).toList();
      }
      originalItems = List<ResultItem>.from(items);
      selectedIndex = items.isEmpty ? null : 0;
      sourcePath = path;
      setStatus('Loaded ${p.basename(path)}');
      appendLog('Loaded ${p.basename(path)}');
      setState(() {});
    } catch (err) {
      setStatus('Load failed: $err');
      appendLog('Load failed: $err');
    }
  }

  Future<void> saveSorted() async {
    if (items.isEmpty) return;
    final defaultName = sourcePath != null
        ? p.basename(sourcePath!).replaceAll('.json', '.sorted.json')
        : 'result.sorted.json';
    final output = await FilePicker.platform.saveFile(
      dialogTitle: 'Save sorted JSON',
      fileName: defaultName,
    );
    if (output == null) return;
    final file = File(output);
    final jsonText = const JsonEncoder.withIndent('  ')
        .convert(items.map((e) => e.toJson()).toList());
    await file.writeAsString(jsonText);
    setStatus('Saved ${p.basename(output)}');
    appendLog('Saved ${p.basename(output)}');
  }

  Future<void> resetOrder() async {
    items = List<ResultItem>.from(originalItems);
    setState(() {});
  }

  Future<void> runAnalyze() async {
    final folder = await FilePicker.platform.getDirectoryPath();
    if (folder == null) return;
    await runAnalyzeForFolder(folder);
  }

  Future<void> reanalyzeCurrent() async {
    if (sourcePath == null) {
      setAnalyzeStatus('No source file loaded');
      return;
    }
    final folder = _inferFolderFromItems() ?? p.dirname(sourcePath!);
    await runAnalyzeForFolder(folder, outputPath: sourcePath);
  }

  String? _inferFolderFromItems() {
    for (final item in items) {
      if (item.filePath.isNotEmpty) {
        return p.dirname(item.filePath);
      }
    }
    return null;
  }

  Future<void> runAnalyzeForFolder(String folder, {String? outputPath}) async {
    if (analyzing) return;
    String? output = outputPath;
    if (output == null) {
      output = await FilePicker.platform.saveFile(
        dialogTitle: 'Save result.json',
        fileName: 'result.json',
      );
      if (output == null) return;
    }

    try {
      analyzing = true;
      setState(() {});
      final resolvedCli = await resolveCliPath();
      final process = await Process.start(
        resolvedCli,
        buildAnalyzeArgs(folder, output),
        workingDirectory: resolveRepoRoot(),
        runInShell: true,
      );
      process.stdout
          .transform(utf8.decoder)
          .transform(const LineSplitter())
          .listen((line) {
        appendLog(line);
      });
      process.stderr
          .transform(utf8.decoder)
          .transform(const LineSplitter())
          .listen((line) {
        appendLog(line);
      });

      final exitCode = await process.exitCode;
      if (exitCode != 0) {
        setAnalyzeStatus('Analyze failed (code $exitCode)');
        appendLog('Analyze failed (code $exitCode)');
      } else {
        setAnalyzeStatus('Analyze complete');
        appendLog('Analyze complete');
        await loadFromPath(output);
      }
    } catch (err) {
      setAnalyzeStatus('Analyze failed: $err');
      appendLog('Analyze failed: $err');
    } finally {
      analyzing = false;
      setState(() {});
    }
  }

  Future<void> reanalyzeEntry(ResultItem item) async {
    if (analyzing) return;
    if (item.filePath.isEmpty) return;
    final srcFile = File(item.filePath);
    if (!srcFile.existsSync()) {
      setAnalyzeStatus('File not found: ${item.fileName}');
      appendLog('File not found: ${item.filePath}');
      return;
    }
    final tempDir = Directory.systemTemp.createTempSync('photo-ai-single');
    final tempFile = File(p.join(tempDir.path, p.basename(item.filePath)));
    await tempFile.writeAsBytes(await srcFile.readAsBytes());
    final output = p.join(tempDir.path, 'result.json');

    try {
      analyzing = true;
      setState(() {});
      final resolvedCli = await resolveCliPath();
      final process = await Process.start(
        resolvedCli,
        buildAnalyzeArgs(tempDir.path, output),
        workingDirectory: resolveRepoRoot(),
        runInShell: true,
      );
      process.stdout
          .transform(utf8.decoder)
          .transform(const LineSplitter())
          .listen(appendLog);
      process.stderr
          .transform(utf8.decoder)
          .transform(const LineSplitter())
          .listen(appendLog);

      final exitCode = await process.exitCode;
      if (exitCode != 0) {
        setAnalyzeStatus('Analyze failed (code $exitCode)');
        appendLog('Analyze failed (code $exitCode)');
        return;
      }

      final data = jsonDecode(await File(output).readAsString());
      if (data is! List || data.isEmpty) {
        setAnalyzeStatus('Analyze produced no results');
        appendLog('Analyze produced no results');
        return;
      }
      final updated = ResultItem.fromJson(data.first as Map<String, dynamic>);
      final normalized = stationOverride.isNotEmpty
          ? updated.copyWith(station: stationOverride)
          : updated;
      setState(() {
        final index = items.indexWhere((e) => e.filePath == item.filePath);
        if (index != -1) {
          items[index] = normalized;
        }
        final originalIndex = originalItems.indexWhere((e) => e.filePath == item.filePath);
        if (originalIndex != -1) {
          originalItems[originalIndex] = normalized;
        }
      });
      setAnalyzeStatus('Re-analyze complete');
      appendLog('Re-analyze complete: ${item.fileName}');
    } catch (err) {
      setAnalyzeStatus('Analyze failed: $err');
      appendLog('Analyze failed: $err');
    } finally {
      analyzing = false;
      setState(() {});
    }
  }

  Future<void> runExport(ExportFormat format) async {
    if (items.isEmpty || exporting) return;
    if (sourcePath == null) {
      setExportStatus('No source file loaded');
      return;
    }
    final defaultStem = sourcePath == null
        ? 'export'
        : p.basenameWithoutExtension(sourcePath!);
    final suggestedName = switch (format) {
      ExportFormat.pdf => '$defaultStem.pdf',
      ExportFormat.excel => '$defaultStem.xlsx',
      ExportFormat.both => '$defaultStem.pdf',
    };
    final outputPath = await FilePicker.platform.saveFile(
      dialogTitle: 'Save export file',
      fileName: suggestedName,
    );
    if (outputPath == null) return;
    final tempDir = Directory.systemTemp.createTempSync('photo-ai-export');
    final tempJson = File(p.join(tempDir.path, 'result.sorted.json'));
    final jsonText = const JsonEncoder.withIndent('  ')
        .convert(items.map((e) => e.toJson()).toList());
    await tempJson.writeAsString(jsonText);

    final formatArg = switch (format) {
      ExportFormat.pdf => 'pdf',
      ExportFormat.excel => 'excel',
      ExportFormat.both => 'both',
    };

    try {
      exporting = true;
      setState(() {});
      final resolvedCli = await resolveCliPath();
      appendLog('Export start: $resolvedCli export ${tempJson.path} --format $formatArg --output $outputPath');
      final process = await Process.start(
        resolvedCli,
        ['export', tempJson.path, '--format', formatArg, '--output', outputPath],
        workingDirectory: resolveRepoRoot(),
        runInShell: true,
      );
      process.stdout
          .transform(utf8.decoder)
          .transform(const LineSplitter())
          .listen(appendLog);
      process.stderr
          .transform(utf8.decoder)
          .transform(const LineSplitter())
          .listen(appendLog);

      final exitCode = await process.exitCode;
      if (exitCode != 0) {
        setExportStatus('Export failed (code $exitCode)');
        appendLog('Export failed (code $exitCode)');
      } else {
        setExportStatus('Export complete');
        appendLog('Export output: $outputPath');
        appendLog('Export complete');
      }
    } catch (err) {
      setExportStatus('Export failed: $err');
      appendLog('Export failed: $err');
    } finally {
      exporting = false;
      setState(() {});
    }
  }

  Future<String> resolveCliPath() async {
    if (cliPath.isNotEmpty) return cliPath;
    final current = Directory.current.path;
    final candidate = Platform.isWindows ? 'photo-ai-rust.exe' : 'photo-ai-rust';
    final debugPath = p.join(current, '..', 'target', 'debug', candidate);
    final releasePath = p.join(current, '..', 'target', 'release', candidate);
    if (File(debugPath).existsSync()) return debugPath;
    if (File(releasePath).existsSync()) return releasePath;
    return candidate;
  }

  Future<void> pickCliPath() async {
    final result = await FilePicker.platform.pickFiles(
      type: FileType.custom,
      allowedExtensions: Platform.isWindows ? ['exe'] : null,
    );
    if (result == null || result.files.isEmpty) return;
    cliPath = result.files.single.path ?? '';
    if (cliPath.isNotEmpty) {
      setStatus('CLI: ${p.basename(cliPath)}');
      appendLog('CLI path set: ${p.basename(cliPath)}');
      setState(() {});
    }
  }

  List<String> buildAnalyzeArgs(String folder, String output) {
    final args = <String>[
      'analyze',
      folder,
      '--output',
      output,
      '--batch-size',
      batchSize.toString(),
      '--ai-provider',
      analyzeProvider == AnalyzeProvider.claude ? 'claude' : 'codex',
    ];
    if (verboseAnalyze) {
      args.add('--verbose');
    }
    if (workTypeInput.isNotEmpty) {
      args.addAll(['--work-type', workTypeInput]);
      // 工種に対応するマスタCSVを自動解決
      final workTypeMaster = getMasterPathForWorkType(workTypeInput);
      if (workTypeMaster != null) {
        args.addAll(['--master', workTypeMaster]);
        appendLog('Master: $workTypeMaster');
      }
    }
    if (varietyInput.isNotEmpty) {
      args.addAll(['--variety', varietyInput]);
    }
    if (stationInput.isNotEmpty) {
      args.addAll(['--station', stationInput]);
    }
    return args;
  }

  Future<void> openAnalyzeOptions() async {
    final workController = TextEditingController(text: workTypeInput);
    final varietyController = TextEditingController(text: varietyInput);
    final stationController = TextEditingController(text: stationInput);
    await showDialog<void>(
      context: context,
      builder: (context) {
        String dropdownValue = workTypeInput;
        void setDropdown(String? value, StateSetter setStateDialog) {
          dropdownValue = value ?? '';
          workController.text = dropdownValue;
          setStateDialog(() {});
        }

        return AlertDialog(
          title: const Text('Analyze Options'),
          content: StatefulBuilder(
            builder: (context, setStateDialog) {
              return Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Align(
                    alignment: Alignment.centerLeft,
                    child: Wrap(
                      spacing: 8,
                      runSpacing: 4,
                      children: [
                        TextButton.icon(
                          onPressed: () async {
                            await loadDefaultMasterCsv();
                            setStateDialog(() {});
                          },
                          icon: const Icon(Icons.auto_fix_high),
                          label: const Text('Load Work Types'),
                        ),
                      ],
                    ),
                  ),
                  if (workTypeOptions.isNotEmpty)
                    DropdownButtonFormField<String>(
                      value: dropdownValue.isEmpty ? null : dropdownValue,
                      items: workTypeOptions
                          .map((e) => DropdownMenuItem(value: e, child: Text(e)))
                          .toList(),
                      onChanged: (value) => setDropdown(value, setStateDialog),
                      decoration: const InputDecoration(
                        labelText: 'Work Type (from master)',
                      ),
                    ),
                  TextField(
                    controller: workController,
                    decoration: const InputDecoration(labelText: 'Work Type (manual)'),
                  ),
                  TextField(
                    controller: varietyController,
                    decoration: const InputDecoration(labelText: 'Variety (optional)'),
                  ),
                  TextField(
                    controller: stationController,
                    decoration: const InputDecoration(labelText: 'Station (optional)'),
                  ),
                ],
              );
            },
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(),
              child: const Text('Cancel'),
            ),
            FilledButton(
              onPressed: () {
                setState(() {
                  workTypeInput = workController.text.trim();
                  varietyInput = varietyController.text.trim();
                  stationInput = stationController.text.trim();
                });
                Navigator.of(context).pop();
              },
              child: const Text('Save'),
            ),
          ],
        );
      },
    );
  }

  Future<void> applyStationToAll() async {
    if (items.isEmpty) return;
    final controller = TextEditingController();
    final value = await showDialog<String>(
      context: context,
      builder: (context) {
        return AlertDialog(
          title: const Text('Apply Station to All'),
          content: TextField(
            controller: controller,
            decoration: const InputDecoration(labelText: 'Station'),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(),
              child: const Text('Cancel'),
            ),
            FilledButton(
              onPressed: () => Navigator.of(context).pop(controller.text.trim()),
              child: const Text('Apply'),
            ),
          ],
        );
      },
    );
    if (value == null) return;
    setState(() {
      stationOverride = value;
      items = items.map((item) => item.copyWith(station: value)).toList();
      originalItems = originalItems
          .map((item) => item.copyWith(station: value))
          .toList();
    });
    appendLog('Applied station to all: $value');
  }

  Future<void> loadDefaultMasterCsv() async {
    final dirPath = resolveByWorkTypeDir();
    if (dirPath == null) {
      setStatus('by_work_type directory not found');
      appendLog('by_work_type directory not found');
      return;
    }
    final types = await loadWorkTypesFromDirectory();
    if (types.isEmpty) {
      setStatus('No work type masters found');
      appendLog('No work type masters found in: $dirPath');
      return;
    }
    setState(() {
      workTypeOptions = types;
    });
    appendLog('Loaded ${types.length} work types from by_work_type/');
  }

  String resolveRepoRoot() {
    final current = Directory.current.path;
    return p.normalize(p.join(current, '..'));
  }

  String? resolveByWorkTypeDir() {
    final root = resolveRepoRoot();
    final dirPath = p.normalize(p.join(root, 'master', 'by_work_type'));
    if (Directory(dirPath).existsSync()) {
      return dirPath;
    }
    return null;
  }

  String? getMasterPathForWorkType(String workType) {
    final dir = resolveByWorkTypeDir();
    if (dir == null) return null;
    final csvPath = p.join(dir, '$workType.csv');
    if (File(csvPath).existsSync()) {
      return csvPath;
    }
    return null;
  }

  Future<List<String>> loadWorkTypesFromDirectory() async {
    try {
      final dirPath = resolveByWorkTypeDir();
      if (dirPath == null) return [];
      final dir = Directory(dirPath);
      final files = await dir.list().toList();
      final types = <String>[];
      for (final file in files) {
        if (file is File && file.path.endsWith('.csv')) {
          final name = p.basenameWithoutExtension(file.path);
          types.add(name);
        }
      }
      types.sort();
      return types;
    } catch (_) {
      return [];
    }
  }

  void setStatus(String value) => setState(() => status = value);

  void setAnalyzeStatus(String value) => setState(() => analyzeStatus = value);

  void setExportStatus(String value) => setState(() => exportStatus = value);

  void appendLog(String message) {
    logs.add('${DateTime.now().toIso8601String()}  $message');
    if (logs.length > 200) {
      logs.removeAt(0);
    }
    setState(() {});
  }

  Widget _buildAnalyzeStatus() {
    if (analyzing) {
      _ensurePulse();
      final pulse = _pulseColor;
      if (pulse == null) {
        return const Text('Analyzing...', style: TextStyle(fontSize: 12));
      }
      return _buildWaveText('Analyzing...', pulse);
    }
    return Text(
      analyzeStatus.isEmpty ? status : analyzeStatus,
      style: const TextStyle(fontSize: 12),
    );
  }

  Widget _buildExportStatus() {
    if (exporting) {
      _ensurePulse();
      final pulse = _pulseColor;
      if (pulse == null) {
        return const Text('Exporting...', style: TextStyle(fontSize: 12));
      }
      return _buildWaveText('Exporting...', pulse);
    }
    return Text(
      exportStatus,
      style: const TextStyle(fontSize: 12),
    );
  }

  Widget _buildWaveText(String text, Animation<Color?> pulse) {
    return AnimatedBuilder(
      animation: pulse,
      builder: (context, _) {
        final controller = _pulseController;
        final t = controller?.value ?? 0.0;
        const baseA = Color(0xFFFFF2B2);
        const baseB = Color(0xFF2C6BFF);
        final spans = <InlineSpan>[];
        for (var i = 0; i < text.length; i++) {
          final phase = (t + i * 0.08) % 1.0;
          final color = Color.lerp(baseA, baseB, phase) ?? baseA;
          spans.add(TextSpan(text: text[i], style: TextStyle(color: color)));
        }
        return RichText(
          text: TextSpan(
            style: const TextStyle(fontSize: 12),
            children: spans,
          ),
        );
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Column(
        children: [
          MenuBar(
            children: [
              SubmenuButton(
                menuChildren: [
                  MenuItemButton(
                    onPressed: openJson,
                    child: const Text('Open JSON'),
                  ),
                  MenuItemButton(
                    onPressed: reloadJson,
                    child: const Text('Reload JSON'),
                  ),
                  MenuItemButton(
                    onPressed: pickCliPath,
                    child: const Text('Set CLI Path'),
                  ),
                  MenuItemButton(
                    onPressed: items.isEmpty ? null : saveSorted,
                    child: const Text('Save Sorted'),
                  ),
                  MenuItemButton(
                    onPressed: items.isEmpty ? null : resetOrder,
                    child: const Text('Reset Order'),
                  ),
                ],
                child: const Text('File'),
              ),
              SubmenuButton(
                menuChildren: [
                  MenuItemButton(
                    onPressed: analyzing ? null : runAnalyze,
                    child: const Text('Run Analyze'),
                  ),
                  MenuItemButton(
                    onPressed: openAnalyzeOptions,
                    child: const Text('Analyze Options'),
                  ),
                  MenuItemButton(
                    onPressed: analyzing ? null : reanalyzeCurrent,
                    child: const Text('Re-analyze Current JSON'),
                  ),
                  const Divider(),
                  MenuItemButton(
                    onPressed: () => setState(() => verboseAnalyze = !verboseAnalyze),
                    child: Text(verboseAnalyze ? 'Verbose: ON' : 'Verbose: OFF'),
                  ),
                  const Divider(),
                  MenuItemButton(
                    onPressed: () => setState(() => analyzeProvider = AnalyzeProvider.claude),
                    child: const Text('Claude'),
                  ),
                  MenuItemButton(
                    onPressed: () => setState(() => analyzeProvider = AnalyzeProvider.codex),
                    child: const Text('Codex'),
                  ),
                  const Divider(),
                  MenuItemButton(
                    onPressed: () => setState(() => batchSize = 1),
                    child: const Text('Batch 1'),
                  ),
                  MenuItemButton(
                    onPressed: () => setState(() => batchSize = 5),
                    child: const Text('Batch 5'),
                  ),
                  MenuItemButton(
                    onPressed: () => setState(() => batchSize = 10),
                    child: const Text('Batch 10'),
                  ),
                ],
                child: const Text('Analyze'),
              ),
              SubmenuButton(
                menuChildren: [
                  MenuItemButton(
                    onPressed: items.isEmpty || exporting
                        ? null
                        : () => runExport(ExportFormat.pdf),
                    child: const Text('Export PDF'),
                  ),
                  MenuItemButton(
                    onPressed: items.isEmpty || exporting
                        ? null
                        : () => runExport(ExportFormat.excel),
                    child: const Text('Export Excel'),
                  ),
                  MenuItemButton(
                    onPressed: items.isEmpty || exporting
                        ? null
                        : () => runExport(ExportFormat.both),
                    child: const Text('Export Both'),
                  ),
                ],
                child: const Text('Export'),
              ),
              SubmenuButton(
                menuChildren: [
                  MenuItemButton(
                    onPressed: items.isEmpty ? null : applyStationToAll,
                    child: const Text('Apply Station to All'),
                  ),
                ],
                child: const Text('Edit'),
              ),
              const SizedBox(width: 16),
              _buildAnalyzeStatus(),
              const SizedBox(width: 12),
              _buildExportStatus(),
            ],
          ),
          Expanded(
            child: Row(
              children: [
                Expanded(
                  flex: 2,
                  child: ReorderableListView.builder(
                    itemCount: items.length,
                    onReorder: (oldIndex, newIndex) {
                      setState(() {
                        if (newIndex > oldIndex) newIndex -= 1;
                        final item = items.removeAt(oldIndex);
                        items.insert(newIndex, item);
                      });
                    },
                    itemBuilder: (context, index) {
                      final item = items[index];
                      return GestureDetector(
                        key: ValueKey(item.fileName + index.toString()),
                        onTap: () {
                          setState(() {
                            selectedIndex = index;
                          });
                        },
                        onSecondaryTapDown: (details) async {
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
                              const PopupMenuItem(
                                value: 'analyze',
                                child: Text('Analyze Folder'),
                              ),
                              const PopupMenuItem(
                                value: 'reanalyze_entry',
                                child: Text('Re-analyze Entry'),
                              ),
                              const PopupMenuItem(
                                value: 'copy_path',
                                child: Text('Copy File Path'),
                              ),
                            ],
                          );

                          if (selected == 'analyze') {
                            if (item.filePath.isEmpty) return;
                            final folder = p.dirname(item.filePath);
                            await runAnalyzeForFolder(folder);
                          } else if (selected == 'reanalyze_entry') {
                            await reanalyzeEntry(item);
                          } else if (selected == 'copy_path') {
                            if (item.filePath.isEmpty) return;
                            await Clipboard.setData(ClipboardData(text: item.filePath));
                            if (context.mounted) {
                              ScaffoldMessenger.of(context).showSnackBar(
                                const SnackBar(content: Text('Path copied')),
                              );
                            }
                          }
                        },
                        child: Container(
                          margin: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                          padding: const EdgeInsets.all(12),
                          decoration: BoxDecoration(
                            color: selectedIndex == index
                                ? const Color(0xFF1F232E)
                                : const Color(0xFF141821),
                            borderRadius: BorderRadius.circular(12),
                            border: Border.all(
                              color: selectedIndex == index
                                  ? const Color(0xFFF6C445)
                                  : const Color(0xFF2A2F3A),
                            ),
                          ),
                          child: Row(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              SizedBox(
                                width: 180,
                                height: 130,
                                child: item.filePath.isEmpty
                                    ? const Center(child: Text('No image'))
                                    : Image.file(
                                        File(item.filePath),
                                        fit: BoxFit.cover,
                                        errorBuilder: (_, __, ___) =>
                                            const Center(child: Text('No preview')),
                                      ),
                              ),
                              const SizedBox(width: 12),
                              Expanded(
                                child: Column(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    _FieldGrid(item: item),
                                    const SizedBox(height: 8),
                                    Text(
                                      'キャプション',
                                      style: const TextStyle(
                                        fontSize: 12,
                                        color: Colors.white70,
                                      ),
                                    ),
                                    const SizedBox(height: 4),
                                    Text(
                                      item.description.isEmpty ? '-' : item.description,
                                      style: const TextStyle(fontSize: 12, color: Colors.white),
                                    ),
                                  ],
                                ),
                              )
                            ],
                          ),
                        ),
                      );
                    },
                  ),
                ),
                Expanded(
                  flex: 1,
                  child: _DetailPanel(
                    item: selectedIndex == null ? null : items[selectedIndex!],
                  ),
                ),
              ],
            ),
          ),
          Container(
            height: 160,
            decoration: const BoxDecoration(
              color: Color(0xFF0B0E14),
              border: Border(top: BorderSide(color: Color(0xFF2A2F3A))),
            ),
            child: Column(
              children: [
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
                  child: Row(
                    children: [
                      const Text('Terminal Log', style: TextStyle(color: Colors.white70)),
                      const Spacer(),
                      TextButton(
                        onPressed: logs.isEmpty
                            ? null
                            : () {
                                Clipboard.setData(ClipboardData(text: logs.join('\n')));
                                ScaffoldMessenger.of(context).showSnackBar(
                                  const SnackBar(content: Text('Log copied')),
                                );
                              },
                        child: const Text('Copy'),
                      ),
                      TextButton(
                        onPressed: logs.isEmpty
                            ? null
                            : () {
                                setState(() => logs.clear());
                              },
                        child: const Text('Clear'),
                      ),
                    ],
                  ),
                ),
                const Divider(height: 1),
                Expanded(
                  child: ListView.builder(
                    itemCount: logs.length,
                    itemBuilder: (context, index) {
                      return Padding(
                        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 2),
                        child: SelectableText(
                          logs[index],
                          style: const TextStyle(fontSize: 11, color: Colors.white70),
                        ),
                      );
                    },
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _FieldGrid extends StatelessWidget {
  const _FieldGrid({required this.item});

  final ResultItem item;

  static const fields = [
    FieldDef(key: 'date', label: '日時'),
    FieldDef(key: 'photoCategory', label: '区分'),
    FieldDef(key: 'workType', label: '工種'),
    FieldDef(key: 'variety', label: '種別'),
    FieldDef(key: 'subphase', label: '作業段階'),
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

class _DetailPanel extends StatelessWidget {
  const _DetailPanel({required this.item});

  final ResultItem? item;

  @override
  Widget build(BuildContext context) {
    if (item == null) {
      return const Center(child: Text('Select a card'));
    }
    final it = item!;
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: const BoxDecoration(
        color: Color(0xFF0F121A),
      ),
      child: ListView(
        children: [
          _DetailRow(
            label: 'File Name',
            value: it.fileName,
            onCopyPath: it.filePath,
          ),
          const SizedBox(height: 8),
          _DetailRow(label: 'Date', value: it.date),
          _DetailRow(label: 'Photo Category', value: it.photoCategory),
          _DetailRow(label: 'Work Type', value: it.workType),
          _DetailRow(label: 'Variety', value: it.variety),
          _DetailRow(label: 'Subphase', value: it.subphase),
          _DetailRow(label: 'Remarks', value: it.remarks),
          _DetailRow(label: 'Station', value: it.station),
          _DetailRow(label: 'Measurements', value: it.measurements),
          _DetailRow(label: 'Detected Text', value: it.detectedText),
          _DetailRow(label: 'Has Board', value: it.hasBoard ? 'true' : 'false'),
          _DetailRow(label: 'Description', value: it.description),
          _DetailRow(label: 'Reasoning', value: it.reasoning),
        ],
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
            Text(
              value.isEmpty ? '-' : value,
              style: const TextStyle(color: Colors.white),
            ),
          ],
        ),
      ),
    );
  }
}

class FieldDef {
  const FieldDef({required this.key, required this.label});

  final String key;
  final String label;
}

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
  final String subphase;
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
        return subphase;
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
    String? station,
  }) {
    return ResultItem(
      fileName: fileName,
      filePath: filePath,
      date: date,
      photoCategory: photoCategory,
      workType: workType,
      variety: variety,
      subphase: subphase,
      remarks: remarks,
      station: station ?? this.station,
      description: description,
      measurements: measurements,
      detectedText: detectedText,
      hasBoard: hasBoard,
      reasoning: reasoning,
    );
  }
}
