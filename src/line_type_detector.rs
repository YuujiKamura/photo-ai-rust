//! 区画線工の線種判定
//!
//! 写真からGemini CLIを使って区画線の線種を判定する。

use photo_ai_common::LineTypeEntry;
use std::path::{Path, PathBuf};
use std::process::Stdio;

/// 区画線工の線種判定プロンプトを生成する
///
/// line_typesから選択肢リストを構築し、Gemini CLIに渡すプロンプトを返す
pub(crate) fn build_line_type_prompt(line_types: &[LineTypeEntry]) -> String {
    let choices: Vec<String> = line_types
        .iter()
        .enumerate()
        .map(|(i, lt)| {
            let label = (b'A' + i as u8) as char;
            format!("({}){}", label, lt.name)
        })
        .collect();
    let choices_str = choices.join(" ");

    format!(
        "夜間道路工事の写真。作業員が仮ラインテープを路面に貼っている。\
        以下の手順で答えろ。\
        Step1:作業員の手元に見えるテープの線は直線か曲線か角度がついているか？\
        Step2:テープ全体でどんな図形を作ろうとしているか？（直線/平行な帯/ひし形/矢印/文字）\
        Step3:以下から該当する線種を1つ選べ：{} \
        各Stepの回答を1行ずつ出力。判別不能なら「判別不能」。",
        choices_str
    )
}

/// AIレスポンスから線種名を抽出する
///
/// レスポンス例: "A 中央線", "(B) 停止線", "横断歩道線" など
pub(crate) fn extract_line_type_from_response(response: &str, line_types: &[LineTypeEntry]) -> Option<String> {
    let response = response.trim();
    if response.is_empty() || response.contains("判別不能") {
        return None;
    }

    // 最終行を取得し、CoTの "Step3:" プレフィックスを除去
    let raw_last = response
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or(response)
        .trim();
    let last_line = raw_last
        .strip_prefix("Step3:")
        .or_else(|| raw_last.strip_prefix("Step 3:"))
        .unwrap_or(raw_last)
        .trim();

    // 記号(A,B,C...)で判定（最終行から）
    for (i, lt) in line_types.iter().enumerate() {
        let label = (b'A' + i as u8) as char;
        if last_line.starts_with(label)
            || last_line.starts_with(&format!("({})", label))
        {
            return Some(lt.name.clone());
        }
    }

    // 線種名の直接マッチ（最終行から優先、次に全文）
    for lt in line_types {
        if last_line.contains(&lt.name) {
            return Some(lt.name.clone());
        }
    }
    for lt in line_types {
        if response.contains(&lt.name) {
            return Some(lt.name.clone());
        }
    }

    None
}

/// 区画線工の写真に対して、Gemini CLIで線種を判定する
///
/// 戻り値: Some("横断歩道線") など。判定失敗時はNone。
pub fn detect_line_type(
    photo_path: &str,
    line_types: &[LineTypeEntry],
) -> Option<String> {
    if line_types.is_empty() {
        return None;
    }

    let prompt = build_line_type_prompt(line_types);

    // Gemini CLIのワークスペース外（H:ドライブ等）の場合に備え、一時ファイルにコピー
    let photo = Path::new(photo_path);
    let temp_dir = std::env::temp_dir().join(format!("photo-ai-linetype-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).ok();

    let ext = photo.extension().and_then(|e| e.to_str()).unwrap_or("jpg");
    let temp_photo = temp_dir.join(format!("photo.{}", ext));
    if let Err(e) = std::fs::copy(photo, &temp_photo) {
        eprintln!("  Warning: 写真コピー失敗 {}: {}", photo_path, e);
        let _ = std::fs::remove_dir_all(&temp_dir);
        return None;
    }

    let temp_photo_unix = temp_photo.display().to_string().replace('\\', "/");
    let stdin_content = format!("@{} {}", temp_photo_unix, prompt);
    // Gemini CLI呼び出し: stdin経由、-pフラグ不使用
    let result = run_gemini_cli_for_line_type(&stdin_content);

    // 一時ファイル削除
    let _ = std::fs::remove_dir_all(&temp_dir);

    match result {
        Ok(response) => extract_line_type_from_response(&response, line_types),
        Err(e) => {
            eprintln!("  Warning: 線種判定失敗: {}", e);
            None
        }
    }
}

/// Git for Windows の bash.exe パスを探す
fn find_git_bash() -> Option<PathBuf> {
    // 既知のパスを順に確認
    let candidates = [
        r"C:\Program Files\Git\usr\bin\bash.exe",
        r"C:\Program Files (x86)\Git\usr\bin\bash.exe",
    ];
    for path in &candidates {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    // where git からGitルートを推定
    let output = std::process::Command::new("where.exe")
        .arg("git")
        .output()
        .ok()?;
    let git_path = String::from_utf8_lossy(&output.stdout);
    for line in git_path.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        // git.exe → 親の親 = Gitルート → usr/bin/bash.exe
        let mut p = PathBuf::from(line);
        p.pop(); // bin or cmd
        p.pop(); // Git root
        let bash = p.join("usr").join("bin").join("bash.exe");
        if bash.exists() {
            return Some(bash);
        }
    }
    None
}

/// Gemini CLIを呼び出す（線種判定用の軽量版）
///
/// Git bash経由で一時ファイル→echo | geminiのシェルパイプを使う。
/// Rust native stdinパイプだと@fileの画像参照が正しく処理されないため。
fn run_gemini_cli_for_line_type(stdin_content: &str) -> std::result::Result<String, String> {
    println!("  🔍 区画線種判定中...");

    // Git bashを使う（Windows System32のbash.exeはWSLなのでNG）
    let bash = if cfg!(windows) {
        find_git_bash().ok_or_else(|| "Git bash not found".to_string())?
    } else {
        PathBuf::from("bash")
    };

    // stdin内容を一時ファイルに書き出し、bash内でcatしてパイプ
    let temp_stdin = std::env::temp_dir().join(format!("gemini-stdin-{}.txt", std::process::id()));
    std::fs::write(&temp_stdin, format!("{}\n", stdin_content))
        .map_err(|e| format!("一時ファイル書き込みエラー: {}", e))?;
    let temp_unix = temp_stdin.display().to_string().replace('\\', "/");

    let shell_cmd = format!(
        r#"cat '{}' | gemini -m gemini-2.5-pro --yolo -o text"#,
        temp_unix
    );

    let output = std::process::Command::new(&bash)
        .args(["-c", &shell_cmd])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Gemini CLI実行エラー: {}", e))?;

    let _ = std::fs::remove_file(&temp_stdin);

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // Gemini CLIはexit 0でもエラーの場合がある → stdoutが空ならstderrを確認
    if stdout.trim().is_empty() {
        if !stderr.is_empty() {
            return Err(format!("Gemini CLI error: {}", stderr.trim()));
        }
        return Err("Gemini CLI returned empty response".to_string());
    }

    Ok(stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_line_types() -> Vec<LineTypeEntry> {
        vec![
            LineTypeEntry { name: "中央線".to_string(), length_m: 230.0 },
            LineTypeEntry { name: "停止線".to_string(), length_m: 43.0 },
            LineTypeEntry { name: "車線分離線".to_string(), length_m: 30.0 },
            LineTypeEntry { name: "横断歩道線".to_string(), length_m: 100.0 },
            LineTypeEntry { name: "停車禁止枠線".to_string(), length_m: 27.0 },
            LineTypeEntry { name: "停車禁止標示".to_string(), length_m: 29.0 },
            LineTypeEntry { name: "右左折禁止標示".to_string(), length_m: 38.0 },
        ]
    }

    #[test]
    fn test_build_line_type_prompt() {
        let lt = sample_line_types();
        let prompt = build_line_type_prompt(&lt);
        assert!(prompt.contains("(A)中央線"));
        assert!(prompt.contains("(B)停止線"));
        assert!(prompt.contains("(G)右左折禁止標示"));
        assert!(prompt.contains("該当する線種を1つ選べ"));
    }

    #[test]
    fn test_extract_line_type_from_response_letter() {
        let lt = sample_line_types();
        assert_eq!(
            extract_line_type_from_response("A 中央線", &lt),
            Some("中央線".to_string())
        );
        assert_eq!(
            extract_line_type_from_response("(B) 停止線", &lt),
            Some("停止線".to_string())
        );
        assert_eq!(
            extract_line_type_from_response("D 横断歩道線", &lt),
            Some("横断歩道線".to_string())
        );
        assert_eq!(
            extract_line_type_from_response("G", &lt),
            Some("右左折禁止標示".to_string())
        );
    }

    #[test]
    fn test_extract_line_type_from_response_name_match() {
        let lt = sample_line_types();
        assert_eq!(
            extract_line_type_from_response("この写真は横断歩道線です", &lt),
            Some("横断歩道線".to_string())
        );
        assert_eq!(
            extract_line_type_from_response("停車禁止枠線と判断します", &lt),
            Some("停車禁止枠線".to_string())
        );
    }

    #[test]
    fn test_extract_line_type_from_response_unknown() {
        let lt = sample_line_types();
        assert_eq!(extract_line_type_from_response("判別不能", &lt), None);
        assert_eq!(extract_line_type_from_response("", &lt), None);
        assert_eq!(extract_line_type_from_response("  ", &lt), None);
    }
}
