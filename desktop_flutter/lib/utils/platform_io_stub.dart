// Web用 dart:io スタブ (何もしない)
import 'package:flutter/widgets.dart';

bool get isDesktop => false;

class PlatformFile {
  final String path;
  PlatformFile(this.path);

  bool existsSync() => false;
  Future<String> readAsString() async => '';
  Future<List<int>> readAsBytes() async => [];
  Future<void> writeAsString(String content) async {}
  Future<void> writeAsBytes(List<int> bytes) async {}
}

class PlatformDirectory {
  PlatformDirectory(String path);

  static PlatformDirectory get systemTemp => PlatformDirectory('');
  static String get currentPath => '';

  bool existsSync() => false;
  String get path => '';
  PlatformDirectory createTempSync(String prefix) => PlatformDirectory('');
  List<dynamic> listSync() => [];
  Stream<dynamic> list() => const Stream.empty();
}

bool get isWindows => false;

Future<ProcessResult> runProcess(
  String executable,
  List<String> arguments, {
  String? workingDirectory,
}) async {
  return ProcessResult(1, '', 'Not supported on web');
}

class ProcessResult {
  final int exitCode;
  final String stdout;
  final String stderr;
  ProcessResult(this.exitCode, this.stdout, this.stderr);
}

class ProcessHandle {
  Stream<List<int>> get stdout => const Stream.empty();
  Stream<List<int>> get stderr => const Stream.empty();
  Future<int> get exitCode async => 1;
  dynamic get stdin => null;
  int get pid => 0;
  bool kill([dynamic signal]) => false;
}

Future<ProcessHandle> startProcess(
  String executable,
  List<String> arguments, {
  String? workingDirectory,
  bool runInShell = false,
}) async {
  throw UnsupportedError('Process not supported on web');
}

Future<ProcessResult> runTaskkill(int pid) async {
  return ProcessResult(1, '', 'Not supported on web');
}

/// Web用: 画像を表示 (プレースホルダー)
Widget buildImageFromPath(
  String filePath, {
  BoxFit fit = BoxFit.cover,
  Widget Function(BuildContext, Object, StackTrace?)? errorBuilder,
}) {
  return const Center(child: Text('Image preview not available on web'));
}
