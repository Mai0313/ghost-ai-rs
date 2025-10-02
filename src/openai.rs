use std::collections::VecDeque;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose, Engine};
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::multipart::{Form, Part};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::audio::RecordingResult;
use crate::config::{OpenAIConfig, TranscriptionLanguage};
use crate::session::{ConversationEntry, ConversationRole};

const CHAT_COMPLETIONS_PATH: &str = "chat/completions";
const AUDIO_TRANSCRIPTIONS_PATH: &str = "audio/transcriptions";

#[derive(Debug, Clone)]
pub struct AnalyzeRequest {
    pub request_id: Uuid,
    pub config: OpenAIConfig,
    pub text_prompt: String,
    pub custom_prompt: Option<String>,
    pub screenshot_png: Option<Vec<u8>>,
    pub history: VecDeque<ConversationEntry>,
}

#[derive(Debug, Clone)]
pub struct AnalyzeResponse {
    pub request_id: Uuid,
    pub answer: String,
    pub model: String,
}

#[derive(Debug, Clone)]
pub enum StreamEvent {
    Delta(String),
    Done(String),
    Error(String),
}

pub struct OpenAIClient {
    http: Client,
}

impl OpenAIClient {
    pub fn new() -> Result<Self> {
        let http = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(120))
            .build()?;
        Ok(Self { http })
    }

    pub async fn analyze_stream(
        &self,
        request: AnalyzeRequest,
        stream_tx: UnboundedSender<(Uuid, StreamEvent)>,
    ) -> Result<AnalyzeResponse> {
        let url = build_endpoint(&request.config.base_url, CHAT_COMPLETIONS_PATH)?;
        let mut headers = auth_headers(&request.config.api_key)?;
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let mut payload = build_chat_payload(&request)?;
        payload.stream = true;

        let request_id = request.request_id;
        let model = request.config.model.clone();

        let res = self
            .http
            .post(url)
            .headers(headers)
            .json(&payload)
            .send()
            .await
            .context("failed to send chat completion request")?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            let error = format!("OpenAI request failed with status {status}: {body}");
            let _ = stream_tx.send((request_id, StreamEvent::Error(error.clone())));
            return Err(anyhow!(error));
        }

        let mut stream = res.bytes_stream();
        let mut full_text = String::new();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("failed to read chunk from stream")?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim().to_string();
                buffer = buffer[line_end + 1..].to_string();

                if line.is_empty() || !line.starts_with("data: ") {
                    continue;
                }

                let data = &line[6..];
                if data == "[DONE]" {
                    break;
                }

                match serde_json::from_str::<StreamChunk>(data) {
                    Ok(chunk) => {
                        if let Some(choice) = chunk.choices.first() {
                            if let Some(content) = &choice.delta.content {
                                full_text.push_str(content);
                                let _ = stream_tx
                                    .send((request_id, StreamEvent::Delta(content.clone())));
                            }
                        }
                    }
                    Err(err) => {
                        log::warn!("failed to parse stream chunk: {err}");
                    }
                }
            }
        }

        let _ = stream_tx.send((request_id, StreamEvent::Done(full_text.clone())));

        Ok(AnalyzeResponse {
            request_id,
            answer: full_text,
            model,
        })
    }

    pub async fn analyze(&self, request: AnalyzeRequest) -> Result<AnalyzeResponse> {
        let url = build_endpoint(&request.config.base_url, CHAT_COMPLETIONS_PATH)?;
        let mut headers = auth_headers(&request.config.api_key)?;
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let payload = build_chat_payload(&request)?;

        let res = self
            .http
            .post(url)
            .headers(headers)
            .json(&payload)
            .send()
            .await
            .context("failed to send chat completion request")?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!(
                "OpenAI request failed with status {status}: {body}",
                status = status
            ));
        }

        let parsed: ChatCompletionResponse = res
            .json()
            .await
            .context("failed to decode chat completion response")?;

        let answer = parsed
            .choices
            .iter()
            .find_map(|choice| choice.message.text())
            .unwrap_or_else(|| "<empty response>".to_string());

        Ok(AnalyzeResponse {
            request_id: request.request_id,
            answer,
            model: parsed.model.unwrap_or(request.config.model),
        })
    }

    pub async fn validate(&self, config: &OpenAIConfig) -> Result<bool> {
        if config.api_key.trim().is_empty() {
            return Ok(false);
        }
        let url = build_endpoint(&config.base_url, "models")?;
        let headers = auth_headers(&config.api_key)?;
        let res = self
            .http
            .get(url)
            .headers(headers)
            .send()
            .await
            .context("failed to send model list request")?;
        Ok(res.status().is_success())
    }

    pub async fn transcribe(
        &self,
        config: &OpenAIConfig,
        recording: RecordingResult,
        language: TranscriptionLanguage,
        model: &str,
    ) -> Result<String> {
        let url = build_endpoint(&config.base_url, AUDIO_TRANSCRIPTIONS_PATH)?;
        let headers = auth_headers(&config.api_key)?;
        let file_part = Part::bytes(recording.wav_bytes)
            .file_name(format!("recording-{}.wav", recording.sample_rate))
            .mime_str("audio/wav")?;
        let language_code = match language {
            TranscriptionLanguage::En => "en",
            TranscriptionLanguage::Zh => "zh",
        };
        let form = Form::new()
            .part("file", file_part)
            .text("model", model.to_string())
            .text("language", language_code.to_string());

        let res = self
            .http
            .post(url)
            .headers(headers)
            .multipart(form)
            .send()
            .await
            .context("failed to send transcription request")?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(anyhow!(
                "transcription failed with status {status}: {body}",
                status = status
            ));
        }

        let parsed: TranscriptionResponse = res
            .json()
            .await
            .context("failed to parse transcription response")?;
        Ok(parsed.text.unwrap_or_default())
    }
}

fn build_endpoint(base: &str, path: &str) -> Result<String> {
    let trimmed = base.trim_end_matches('/');
    Ok(format!("{trimmed}/{path}"))
}

fn auth_headers(api_key: &str) -> Result<HeaderMap> {
    if api_key.trim().is_empty() {
        anyhow::bail!("missing OpenAI API key");
    }
    let mut headers = HeaderMap::new();
    let bearer = format!("Bearer {api_key}");
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&bearer).context("invalid API key for Authorization header")?,
    );
    Ok(headers)
}

fn build_chat_payload(request: &AnalyzeRequest) -> Result<ChatCompletionPayload> {
    let mut messages: Vec<ChatMessage> = Vec::new();

    if let Some(prompt) = request.custom_prompt.as_ref() {
        if !prompt.trim().is_empty() {
            messages.push(ChatMessage {
                role: "system".into(),
                content: vec![MessageContent::Text(TextContent {
                    text: prompt.trim().to_string(),
                })],
            });
        }
    }

    for entry in request.history.iter().rev().take(20).rev() {
        let role = match entry.role {
            ConversationRole::System => "system",
            ConversationRole::User => "user",
            ConversationRole::Assistant | ConversationRole::Reasoning => "assistant",
            ConversationRole::Error => "system",
        };
        messages.push(ChatMessage {
            role: role.to_string(),
            content: vec![MessageContent::Text(TextContent {
                text: entry.content.clone(),
            })],
        });
    }

    let mut user_content = vec![MessageContent::Text(TextContent {
        text: request.text_prompt.trim().to_string(),
    })];

    if let Some(png) = request.screenshot_png.as_ref() {
        let base64 = general_purpose::STANDARD.encode(png);
        let data_url = format!("data:image/png;base64,{base64}");
        user_content.push(MessageContent::Image(ImageContent {
            image_url: ImageUrl {
                url: data_url,
                detail: Some("auto".to_string()),
            },
        }));
    }

    messages.push(ChatMessage {
        role: "user".into(),
        content: user_content,
    });

    Ok(ChatCompletionPayload {
        model: request.config.model.clone(),
        messages,
        temperature: request.config.temperature,
        max_tokens: request.config.max_output_tokens,
        stream: false,
    })
}

#[derive(Debug, Serialize)]
struct ChatCompletionPayload {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    temperature: f32,
    #[serde(default)]
    stream: bool,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: Vec<MessageContent>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum MessageContent {
    #[serde(rename = "text")]
    Text(TextContent),
    #[serde(rename = "image_url")]
    Image(ImageContent),
}

#[derive(Debug, Serialize)]
struct TextContent {
    text: String,
}

#[derive(Debug, Serialize)]
struct ImageContent {
    image_url: ImageUrl,
}

#[derive(Debug, Serialize)]
struct ImageUrl {
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    pub id: Option<String>,
    pub model: Option<String>,
    pub choices: Vec<ChatCompletionChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChoice {
    pub message: ChatCompletionMessage,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionMessage {
    pub content: Vec<ChatCompletionContent>,
}

impl ChatCompletionMessage {
    fn text(&self) -> Option<String> {
        self.content.iter().find_map(|item| match item {
            ChatCompletionContent::Text { text } => Some(text.clone()),
            _ => None,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ChatCompletionContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct TranscriptionResponse {
    pub text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamChunk {
    pub choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    pub delta: StreamDelta,
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    pub content: Option<String>,
}
