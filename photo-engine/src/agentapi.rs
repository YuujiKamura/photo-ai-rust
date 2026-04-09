use anyhow::{anyhow, Context, Result};
use reqwest::blocking::{multipart, Client};
use serde::Deserialize;
use std::fs::File;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use photo_ai_rust::grouping::{AiProvider, BillingMode, CarrierConfig, TransportMode};

const DEFAULT_AGENTAPI_URL: &str = "http://127.0.0.1:3284";
const DEFAULT_TIMEOUT_SECS: u64 = 300;

#[derive(Debug, Deserialize)]
struct UploadResponse {
    #[serde(rename = "filePath")]
    file_path: String,
    ok: bool,
}

#[derive(Debug, Deserialize)]
struct Message {
    id: i64,
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct MessagesResponse {
    messages: Vec<Message>,
}

#[derive(Debug, Deserialize)]
struct StatusResponse {
    status: String,
}

pub fn analyze(prompt: &str, files: &[PathBuf], carrier: CarrierConfig) -> Result<String> {
    let base_url = std::env::var("PHOTO_AI_AGENTAPI_URL")
        .unwrap_or_else(|_| DEFAULT_AGENTAPI_URL.to_string())
        .trim_end_matches('/')
        .to_string();
    let timeout_secs = std::env::var("PHOTO_AI_AGENTAPI_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(DEFAULT_TIMEOUT_SECS);

    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .context("AgentAPI client 初期化失敗")?;

    let uploaded_files = upload_files(&client, &base_url, files)?;
    let mut last_err = None;

    for provider in provider_attempt_order(carrier) {
        let attempt_carrier = CarrierConfig { provider, ..carrier };
        match analyze_once(
            &client,
            &base_url,
            timeout_secs,
            prompt,
            &uploaded_files,
            attempt_carrier,
        ) {
            Ok(content) => return Ok(content),
            Err(err) => last_err = Some((provider, err)),
        }
    }

    let (provider, err) = last_err.expect("provider_attempt_order must not be empty");
    Err(anyhow!(
        "AgentAPI 呼び出し失敗 (last provider={}): {}",
        provider_name(provider),
        err
    ))
}

fn analyze_once(
    client: &Client,
    base_url: &str,
    timeout_secs: u64,
    prompt: &str,
    uploaded_files: &[String],
    carrier: CarrierConfig,
) -> Result<String> {
    let before = fetch_messages(client, base_url)?;
    let last_seen_id = before.messages.last().map(|m| m.id).unwrap_or(0);
    let message = build_message(prompt, uploaded_files, carrier);

    send_message(client, base_url, &message, carrier)?;
    wait_until_stable(client, base_url, timeout_secs)?;

    let after = fetch_messages(client, base_url)?;
    extract_latest_agent_message(after.messages, last_seen_id)
}

fn upload_files(client: &Client, base_url: &str, files: &[PathBuf]) -> Result<Vec<String>> {
    let mut uploaded = Vec::with_capacity(files.len());

    for file_path in files {
        let file = File::open(file_path)
            .with_context(|| format!("ファイルを開けません: {}", file_path.display()))?;
        let file_name = file_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("upload.bin")
            .to_string();

        let part = multipart::Part::reader(file).file_name(file_name);
        let form = multipart::Form::new().part("file", part);
        let response = client
            .post(format!("{base_url}/upload"))
            .multipart(form)
            .send()
            .context("AgentAPI upload 失敗")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(anyhow!("AgentAPI upload エラー: {status} {body}"));
        }

        let uploaded_file: UploadResponse = response
            .json()
            .context("AgentAPI upload 応答JSON不正")?;

        if !uploaded_file.ok {
            return Err(anyhow!("AgentAPI upload が失敗を返しました: {}", file_path.display()));
        }

        uploaded.push(uploaded_file.file_path);
    }

    Ok(uploaded)
}

fn build_message(prompt: &str, uploaded_files: &[String], carrier: CarrierConfig) -> String {
    let mut message = String::from(
        "You are the analysis backend for photo-ai-rust.\n\
Read every uploaded file directly from disk.\n\
Return only the final answer requested by the task prompt.\n\
Do not add preamble, explanation, markdown fences, or commentary unless the task prompt asks for them.\n",
    );

    if !uploaded_files.is_empty() {
        message.push_str("\nUploaded files:\n");
        for path in uploaded_files {
            message.push_str("- ");
            message.push_str(path);
            message.push('\n');
        }
    }

    message.push_str("\nRouting hints:\n");
    message.push_str("- provider: ");
    message.push_str(provider_name(carrier.provider));
    message.push('\n');
    message.push_str("- billing: ");
    message.push_str(billing_name(carrier.billing));
    message.push('\n');
    message.push_str("- transport: ");
    message.push_str(transport_name(carrier.transport));
    message.push('\n');

    message.push_str("\nTask prompt:\n");
    message.push_str(prompt);
    message
}

fn send_message(client: &Client, base_url: &str, content: &str, carrier: CarrierConfig) -> Result<()> {
    let response = client
        .post(format!("{base_url}/message"))
        .header("X-PhotoAI-Provider", provider_name(carrier.provider))
        .header("X-PhotoAI-Billing", billing_name(carrier.billing))
        .header("X-PhotoAI-Transport", transport_name(carrier.transport))
        .json(&serde_json::json!({
            "content": content,
            "type": "user"
        }))
        .send()
        .context("AgentAPI message 送信失敗")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(anyhow!("AgentAPI message エラー: {status} {body}"));
    }

    Ok(())
}

fn wait_until_stable(client: &Client, base_url: &str, timeout_secs: u64) -> Result<()> {
    let started = Instant::now();
    let minimum_wait = Duration::from_millis(500);

    loop {
        if started.elapsed() > Duration::from_secs(timeout_secs) {
            return Err(anyhow!(
                "AgentAPI タイムアウト: {}秒待っても stable になりませんでした",
                timeout_secs
            ));
        }

        let response = client
            .get(format!("{base_url}/status"))
            .send()
            .context("AgentAPI status 取得失敗")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(anyhow!("AgentAPI status エラー: {status} {body}"));
        }

        let status: StatusResponse = response.json().context("AgentAPI status JSON不正")?;
        if status.status == "stable" && started.elapsed() >= minimum_wait {
            return Ok(());
        }

        thread::sleep(Duration::from_secs(1));
    }
}

fn fetch_messages(client: &Client, base_url: &str) -> Result<MessagesResponse> {
    let response = client
        .get(format!("{base_url}/messages"))
        .send()
        .context("AgentAPI messages 取得失敗")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(anyhow!("AgentAPI messages エラー: {status} {body}"));
    }

    response.json().context("AgentAPI messages JSON不正")
}

fn extract_latest_agent_message(messages: Vec<Message>, last_seen_id: i64) -> Result<String> {
    messages
        .into_iter()
        .rev()
        .find(|message| message.role == "agent" && message.id > last_seen_id)
        .map(|message| message.content.trim().to_string())
        .filter(|content| !content.is_empty())
        .ok_or_else(|| anyhow!("AgentAPI 応答から agent message を取得できませんでした"))
}

fn provider_name(provider: AiProvider) -> &'static str {
    match provider {
        AiProvider::Auto => "auto",
        AiProvider::Gemini => "gemini",
        AiProvider::Claude => "claude",
        AiProvider::Codex => "codex",
    }
}

fn billing_name(billing: BillingMode) -> &'static str {
    match billing {
        BillingMode::Auto => "auto",
        BillingMode::Subscription => "subscription",
        BillingMode::PayPerUse => "pay_per_use",
    }
}

fn transport_name(transport: TransportMode) -> &'static str {
    match transport {
        TransportMode::Auto => "auto",
        TransportMode::AgentApi => "agent_api",
        TransportMode::ResidentAgent => "resident_agent",
        TransportMode::DirectCli => "direct_cli",
    }
}

fn provider_attempt_order(carrier: CarrierConfig) -> Vec<AiProvider> {
    if carrier.billing == BillingMode::PayPerUse {
        return vec![carrier.provider];
    }

    match carrier.provider {
        AiProvider::Auto => vec![AiProvider::Auto],
        AiProvider::Gemini => vec![AiProvider::Gemini, AiProvider::Claude, AiProvider::Codex],
        AiProvider::Claude => vec![AiProvider::Claude, AiProvider::Codex, AiProvider::Gemini],
        AiProvider::Codex => vec![AiProvider::Codex, AiProvider::Claude, AiProvider::Gemini],
    }
}
