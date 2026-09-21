//! The only place that speaks to the gateway.
//!
//! The webview never reaches the network itself: it calls a command, and this module applies the
//! address, the alias, the output locale and the context cap before anything leaves the machine.

use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::AppError;

/// Ceiling on what one request may carry. The gateway caps again; this one keeps a runaway
/// conversation from being sent at all.
pub const MAX_CONTEXT_CHARS: usize = 24_000;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const HEALTH_TIMEOUT: Duration = Duration::from_secs(4);
const CHAT_TIMEOUT: Duration = Duration::from_secs(300);
const EMBEDDING_TIMEOUT: Duration = Duration::from_secs(60);

/// Caps mirrored from the gateway (`docs/RETRIEVAL.md`): the client caps too, because indexing
/// sends batches rather than one conversation and a runaway folder should never leave the
/// machine as a single request.
pub const MAX_EMBEDDING_INPUTS: usize = 256;
pub const MAX_EMBEDDING_CHARS: usize = 200_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTurn {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthSnapshot {
    pub status: String,
    pub provider_reachable: bool,
    pub aliases: Vec<String>,
    /// The chat alias to fall back to when the client's own choice is no longer valid, e.g.
    /// after a rename in `MODEL_ALIASES` (`docs/TROUBLESHOOTING.md`).
    pub default_model_alias: String,
    /// The embedding alias indexing must use right now. A client never chooses this - it only
    /// needs to notice when it changed, so a rename self-heals instead of failing indexing.
    pub embedding_alias: String,
    pub issues: Vec<String>,
    pub default_output_locale: String,
    pub output_locales: Vec<String>,
}

/// The gateway body, in its own snake_case vocabulary.
#[derive(Debug, Deserialize)]
struct HealthBody {
    status: String,
    provider: HealthProviderBody,
    aliases: Vec<String>,
    default_model_alias: String,
    embedding_alias: String,
    issues: Vec<String>,
    default_output_locale: String,
    output_locales: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct HealthProviderBody {
    reachable: bool,
}

pub struct GatewayClient {
    http: reqwest::Client,
}

impl GatewayClient {
    pub fn new() -> Result<Self, AppError> {
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(CHAT_TIMEOUT)
            .build()
            .map_err(|_| AppError::Internal)?;
        Ok(Self { http })
    }

    pub async fn health(&self, server_url: &str) -> Result<HealthSnapshot, AppError> {
        let url = endpoint(server_url, "/health");
        let response = self
            .http
            .get(&url)
            .timeout(HEALTH_TIMEOUT)
            .send()
            .await
            .map_err(|error| transport_error(error, server_url))?;
        if !response.status().is_success() {
            return Err(AppError::ServerError {
                status: response.status().as_u16(),
            });
        }
        let body: HealthBody = response
            .json()
            .await
            .map_err(|_| AppError::ServerResponseInvalid)?;
        Ok(HealthSnapshot {
            status: body.status,
            provider_reachable: body.provider.reachable,
            aliases: body.aliases,
            default_model_alias: body.default_model_alias,
            embedding_alias: body.embedding_alias,
            issues: body.issues,
            default_output_locale: body.default_output_locale,
            output_locales: body.output_locales,
        })
    }

    /// Stream one answer, handing each piece of text to `on_delta`. Returns the whole answer.
    pub async fn chat(
        &self,
        server_url: &str,
        model_alias: &str,
        output_locale: Option<&str>,
        turns: &[ChatTurn],
        mut on_delta: impl FnMut(&str),
    ) -> Result<String, AppError> {
        let size: usize = turns.iter().map(|turn| turn.content.chars().count()).sum();
        if size > MAX_CONTEXT_CHARS {
            return Err(AppError::ContextTooLarge {
                chars: size,
                limit: MAX_CONTEXT_CHARS,
            });
        }

        let mut payload = json!({
            "model": model_alias,
            "messages": turns,
            "stream": true,
        });
        // Absent rather than guessed: the gateway then applies its own default instead of
        // answering in whatever language the model prefers.
        if let Some(locale) = output_locale {
            payload["output_locale"] = json!(locale);
        }

        let url = endpoint(server_url, "/v1/chat/completions");
        let response = self
            .http
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|error| transport_error(error, server_url))?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            let parsed: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            return Err(gateway_error(&parsed).unwrap_or(AppError::ServerError { status }));
        }

        let mut answer = String::new();
        let mut buffer = String::new();
        let mut stream = response.bytes_stream();
        while let Some(piece) = stream.next().await {
            let bytes = piece.map_err(|error| transport_error(error, server_url))?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));
            while let Some(line_end) = buffer.find('\n') {
                let line: String = buffer.drain(..=line_end).collect();
                match read_event(line.trim())? {
                    Event::Delta(text) => {
                        on_delta(&text);
                        answer.push_str(&text);
                    }
                    Event::Done => return Ok(answer),
                    Event::Ignored => {}
                }
            }
        }
        Ok(answer)
    }

    /// One vector per input, in the order they were sent (OpenAI-compatible). The caller must
    /// already respect the batch caps; this only enforces them defensively.
    pub async fn embed(
        &self,
        server_url: &str,
        model_alias: &str,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, AppError> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        if inputs.len() > MAX_EMBEDDING_INPUTS {
            return Err(AppError::ContextTooLarge {
                chars: inputs.len(),
                limit: MAX_EMBEDDING_INPUTS,
            });
        }
        let total_chars: usize = inputs.iter().map(|text| text.chars().count()).sum();
        if total_chars > MAX_EMBEDDING_CHARS {
            return Err(AppError::ContextTooLarge {
                chars: total_chars,
                limit: MAX_EMBEDDING_CHARS,
            });
        }

        let url = endpoint(server_url, "/v1/embeddings");
        let payload = json!({ "model": model_alias, "input": inputs });
        let response = self
            .http
            .post(&url)
            .timeout(EMBEDDING_TIMEOUT)
            .json(&payload)
            .send()
            .await
            .map_err(|error| transport_error(error, server_url))?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            let parsed: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            return Err(gateway_error(&parsed).unwrap_or(AppError::ServerError { status }));
        }

        let body: EmbeddingsBody = response
            .json()
            .await
            .map_err(|_| AppError::ServerResponseInvalid)?;
        let mut ordered: Vec<(usize, Vec<f32>)> = body
            .data
            .into_iter()
            .map(|entry| (entry.index, entry.embedding))
            .collect();
        ordered.sort_by_key(|(index, _)| *index);
        let vectors: Vec<Vec<f32>> = ordered.into_iter().map(|(_, vector)| vector).collect();
        if vectors.len() != inputs.len() {
            return Err(AppError::ServerResponseInvalid);
        }
        Ok(vectors)
    }
}

#[derive(Debug, Deserialize)]
struct EmbeddingsBody {
    data: Vec<EmbeddingEntryBody>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingEntryBody {
    index: usize,
    embedding: Vec<f32>,
}

#[derive(Debug)]
enum Event {
    Delta(String),
    Done,
    Ignored,
}

fn read_event(line: &str) -> Result<Event, AppError> {
    let Some(payload) = line.strip_prefix("data: ") else {
        return Ok(Event::Ignored);
    };
    if payload == "[DONE]" {
        return Ok(Event::Done);
    }
    let event: Value =
        serde_json::from_str(payload).map_err(|_| AppError::ServerResponseInvalid)?;
    if let Some(error) = gateway_error(&event) {
        return Err(error);
    }
    let delta = event["choices"][0]["delta"]["content"]
        .as_str()
        .unwrap_or("");
    if delta.is_empty() {
        Ok(Event::Ignored)
    } else {
        Ok(Event::Delta(delta.to_string()))
    }
}

/// Recognise `{"error": {"code": ..., "data": ...}}` and keep the gateway's own code.
fn gateway_error(parsed: &Value) -> Option<AppError> {
    let code = parsed["error"]["code"].as_str()?;
    Some(AppError::Gateway {
        code: code.to_string(),
        data: parsed["error"]["data"].clone(),
    })
}

fn transport_error(error: reqwest::Error, server_url: &str) -> AppError {
    if error.is_timeout() {
        AppError::ServerTimeout {
            url: server_url.to_string(),
        }
    } else if error.is_connect() || error.is_request() {
        AppError::ServerUnreachable {
            url: server_url.to_string(),
        }
    } else {
        AppError::ServerResponseInvalid
    }
}

fn endpoint(server_url: &str, path: &str) -> String {
    format!("{}{}", server_url.trim_end_matches('/'), path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_with_a_trailing_slash_still_builds_one_url() {
        assert_eq!(
            endpoint("http://mac-mini.local:8080/", "/health"),
            "http://mac-mini.local:8080/health"
        );
    }

    #[test]
    fn a_delta_is_read_from_a_stream_line() {
        let line = r#"data: {"choices":[{"delta":{"content":"Bonjour"}}]}"#;

        match read_event(line).expect("a readable event") {
            Event::Delta(text) => assert_eq!(text, "Bonjour"),
            _ => panic!("expected a delta"),
        }
    }

    #[test]
    fn the_terminator_ends_the_stream() {
        assert!(matches!(
            read_event("data: [DONE]").expect("a readable event"),
            Event::Done
        ));
    }

    #[test]
    fn a_gateway_code_in_the_stream_becomes_an_error() {
        let line =
            r#"data: {"error":{"code":"provider_unreachable","data":{"provider":"ollama"}}}"#;

        let error = read_event(line).expect_err("expected a refusal");

        assert_eq!(error.code(), "provider_unreachable");
        assert_eq!(error.data()["provider"], "ollama");
    }

    #[test]
    fn an_empty_delta_carries_nothing() {
        let line = r#"data: {"choices":[{"delta":{"role":"assistant","content":""}}]}"#;

        assert!(matches!(
            read_event(line).expect("a readable event"),
            Event::Ignored
        ));
    }
}
