// Desktop用 dart:io ラッパー
import 'dart:io' as io;
import 'package:flutter/widgets.dart';

bool get isDesktop => true;

class PlatformFile {
  final io.File _file;
  PlatformFile(String path) : _file = io.File(path);

  String get path => _file.path;
  bool existsSync() => _file.existsSync();
  Future<String> readAsString() => _file.readAsString();
  Future<List<int>> readAsBytes() => _file.readAsBytes();
  Future<void> writeAsString(String content) => _file.writeAsString(content);
  Future<void> writeAsBytes(List<int> bytes) => _file.writeAsBytes(bytes);
}

class PlatformDirectory {
  final io.Directory _dir;
  PlatformDirectory(String path) : _dir = io.Directory(path);

  static PlatformDirectory get systemTemp => PlatformDirectory(io.Directory.systemTemp.path);
  static String get currentPath => io.Directory.current.path;

  bool existsSync() => _dir.existsSync();
  String get path => _dir.path;
  PlatformDirectory createTempSync(String prefix) {
    final temp = _dir.createTempSync(prefix);
    return PlatformDirectory(temp.path);
  }
  List<io.FileSystemEntity> listSync() => _dir.listSync();
  Stream<io.FileSystemEntity> list() => _dir.list();
}

bool get isWindows => io.Platform.isWindows;

Future<ProcessResult> runProcess(
  String executable,
  List<String> arguments, {
  String? workingDirectory,
}) async {
  final result = await io.Process.run(
    executable,
    arguments,
    workingDirectory: workingDirectory,
  );
  return ProcessResult(result.exitCode, result.stdout.toString(), result.stderr.toString());
}

class ProcessResult {
  final int exitCode;
  final String stdout;
  final String stderr;
  ProcessResult(this.exitCode, this.stdout, this.stderr);
}

class ProcessHandle {
  final io.Process _process;
  ProcessHandle(this._process);

  Stream<String> get stdout => _process.stdout
      .transform(const io.SystemEncoding().decoder)
      .transform(const io.LineSplitter());
  Stream<String> get stderr => _process.stderr
      .transform(const io.SystemEncoding().decoder)
      .transform(const io.LineSplitter());
  Future<int> get exitCode => _process.exitCode;
  io.IOSink get stdin => _process.stdin;
  int get pid => _process.pid;
  bool kill([io.ProcessSignal signal = io.ProcessSignal.sigterm]) => _process.kill(signal);
}

Future<ProcessHandle> startProcess(
  String executable,
  List<String> arguments, {
  String? workingDirectory,
  bool runInShell = false,
}) async {
  final process = await io.Process.start(
    executable,
    arguments,
    workingDirectory: workingDirectory,
    runInShell: runInShell,
  );
  return ProcessHandle(process);
}

Future<ProcessResult> runTaskkill(int pid) async {
  final result = await io.Process.run('taskkill', ['/PID', pid.toString(), '/T', '/F']);
  return ProcessResult(result.exitCode, result.stdout.toString(), result.stderr.toString());
}

/// Desktop用: ファイルパスから画像を表示
Widget buildImageFromPath(
  String filePath, {
  BoxFit fit = BoxFit.cover,
  Widget Function(BuildContext, Object, StackTrace?)? errorBuilder,
}) {
  return Image.file(
    io.File(filePath),
    fit: fit,
    errorBuilder: errorBuilder,
  );
}
