//! AI解析の統一エントリポイント
//!
//! resident-ai (ACP) を通じて Gemini CLI を呼び出す。
//! photo-engine 内の全解析はここを経由する。

use anyhow::Result;
use std::path::PathBuf;

use resident_ai::{analyze as cli_analyze, AnalyzeOptions, Backend};
use photo_ai_rust::grouping::{AiProvider, CarrierConfig};

/// プロンプト + ファイルを AI に送信し、応答テキストを返す。
pub fn analyze(prompt: &str, files: &[PathBuf], carrier: CarrierConfig) -> Result<String> {
    let options = carrier_to_options(carrier);
    let raw = cli_analyze(prompt, files, options).map_err(|e| anyhow::anyhow!("{}", e))?;
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
            // 区切り線（─ ━ ▀ ▄ の連続）
            if trimmed.chars().all(|c| matches!(c, '─' | '━' | '▀' | '▄' | '▝' | '▜')) {
                return false;
            }
            // Gemini CLI UI要素
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

fn carrier_to_options(carrier: CarrierConfig) -> AnalyzeOptions {
    let mut opts = AnalyzeOptions::default().json();

    // Provider → Backend
    match carrier.provider {
        AiProvider::Claude => { opts = opts.with_backend(Backend::Claude); }
        AiProvider::Codex => { opts = opts.with_backend(Backend::Codex); }
        _ => {} // Auto, Gemini → default (Gemini)
    }

    // UsageMode
    let usage_mode = carrier.effective_usage_mode();
    opts = opts.with_usage_mode(usage_mode_to_cli(usage_mode));

    opts
}

fn usage_mode_to_cli(mode: photo_ai_rust::grouping::UsageMode) -> resident_ai::UsageMode {
    match mode {
        photo_ai_rust::grouping::UsageMode::PayPerUse => resident_ai::UsageMode::PayPerUse,
        photo_ai_rust::grouping::UsageMode::Resident => resident_ai::UsageMode::Resident,
        photo_ai_rust::grouping::UsageMode::TimeBasedQuota => resident_ai::UsageMode::TimeBasedQuota,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === ユニット: strip_tui_artifacts ===

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

    // === ユニット: carrier_to_options ===

    #[test]
    fn carrier_default_is_gemini_json() {
        let carrier = CarrierConfig::default();
        let opts = carrier_to_options(carrier);
        assert_eq!(opts.backend, Backend::Gemini);
        assert_eq!(opts.output_format, resident_ai::OutputFormat::Json);
    }

    // === リレーション: photo-engine → resident-ai → gemini ACP → 応答 ===

    #[test]
    #[ignore] // requires gemini CLI authenticated
    fn relation_text_prompt_returns_answer() {
        let carrier = CarrierConfig::default();
        let result = analyze("2+2は？数字だけ答えろ", &[], carrier);
        assert!(result.is_ok(), "analyze failed: {:?}", result.err());
        let text = result.unwrap();
        assert!(text.contains('4'), "expected '4', got: {}", text);
    }

    #[test]
    #[ignore] // requires gemini CLI authenticated
    fn relation_json_prompt_returns_json() {
        let carrier = CarrierConfig::default();
        let result = analyze(
            "2+2の答えを{\"answer\": N}の形式で返せ",
            &[],
            carrier,
        );
        assert!(result.is_ok(), "analyze failed: {:?}", result.err());
        let text = result.unwrap();
        assert!(text.contains("answer"), "expected JSON with 'answer', got: {}", text);
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
        // 舗装関連の書類なので、それっぽい単語が含まれるはず
        let has_keyword = text.contains("舗装")
            || text.contains("アスファルト")
            || text.contains("混合物")
            || text.contains("品質")
            || text.contains("配合")
            || text.contains("書類")
            || text.contains("表");
        assert!(has_keyword, "expected construction-related content, got: {}", text);
    }
}
