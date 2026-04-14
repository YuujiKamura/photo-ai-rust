//! AI解析の統一エントリポイント
//!
//! Gemini CLI (time-based quota / サブスク) を `cli-ai-analyzer` 経由で呼び出す。
//! ACP/resident-ai/pay-per-use は全廃。photo-engine 内の全解析はここを経由する。

use anyhow::Result;
use std::path::PathBuf;

use cli_ai_analyzer::{analyze as cli_analyze, AnalyzeOptions, Backend, UsageMode};
use photo_ai_rust::grouping::CarrierConfig;

/// プロンプト + ファイルを Gemini CLI に送信し、応答テキストを返す。
///
/// `CarrierConfig` は後方互換のために受け取るが無視する。
/// 経路は常に Gemini + TimeBasedQuota (サブスク CLI) 固定。
pub fn analyze(prompt: &str, files: &[PathBuf], _carrier: CarrierConfig) -> Result<String> {
    let opts = AnalyzeOptions::default()
        .with_backend(Backend::Gemini)
        .with_usage_mode(UsageMode::TimeBasedQuota)
        .json();
    let raw = cli_analyze(prompt, files, opts).map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(strip_tui_artifacts(&raw))
}

/// Gemini CLI の TUI 装飾を除去する。
/// ステータスバー、区切り線、ショートカットヒント等を取り除く。
fn strip_tui_artifacts(raw: &str) -> String {
    raw.lines()
        .filter(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return true;
            }
            if trimmed.chars().all(|c| matches!(c, '─' | '━' | '▀' | '▄' | '▝' | '▜')) {
                return false;
            }
            if trimmed.contains("? for shortcuts")
                || trimmed.contains("Shift+Tab to accept")
                || trimmed.contains("GEMINI.md file")
                || (trimmed.contains("skills") && trimmed.contains("·"))
            {
                return false;
            }
            true
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_removes_separator_lines() {
        assert_eq!(strip_tui_artifacts("hello\n───────\nworld"), "hello\nworld");
    }

    #[test]
    fn strip_removes_shortcut_hints() {
        assert_eq!(strip_tui_artifacts("data\n? for shortcuts"), "data");
    }

    #[test]
    fn strip_preserves_normal_text() {
        assert_eq!(strip_tui_artifacts("line1\nline2"), "line1\nline2");
    }

    #[test]
    #[ignore] // requires gemini CLI authenticated (subscription)
    fn relation_text_prompt_returns_answer() {
        let carrier = CarrierConfig::default();
        let result = analyze("2+2は？数字だけ答えろ", &[], carrier);
        assert!(result.is_ok(), "analyze failed: {:?}", result.err());
        let text = result.unwrap();
        assert!(text.contains('4'), "expected '4', got: {}", text);
    }

    #[test]
    #[ignore] // requires gemini CLI + test image
    fn relation_image_analyze_returns_content() {
        let image = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("test_fixtures")
            .join("sample.png");
        assert!(image.exists(), "test image missing: {:?}", image);

        let carrier = CarrierConfig::default();
        let result = analyze(
            "この画像に何が写っているか、日本語で1行で説明しろ",
            &[image],
            carrier,
        );
        assert!(result.is_ok(), "image analyze failed: {:?}", result.err());
        let text = result.unwrap();
        assert!(!text.is_empty(), "response should not be empty");
    }
}
