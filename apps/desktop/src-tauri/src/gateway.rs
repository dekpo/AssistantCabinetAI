//! The only place that speaks to the gateway.
//!
//! The webview never reaches the network itself: it calls a command, and this module applies the
//! address, the alias, the output locale and the context cap before anything leaves the machine.

use std::collections::HashMap;
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

/// How long one embeddings request may take, and how often it is tried.
///
/// Measured on the CPU-only reference stack (`docs/TROUBLESHOOTING.md`, embedding timeout):
/// about 0.7 s per 1 000-character chunk, so a 16-chunk batch takes about 11 s and a cold model
/// load adds a few seconds. 120 s is ten times the batch; 240 s for the first request of a pass
/// absorbs a model that has to be loaded into memory first (`OLLAMA_MAX_LOADED_MODELS=1` evicts
/// it). These are not user settings: nothing she could change would be the right answer, and a
/// timeout she cannot act on does not belong on the settings screen. Injectable so a test can use
/// milliseconds.
#[derive(Debug, Clone, Copy)]
pub struct EmbeddingDeadlines {
    pub first_batch: Duration,
    pub batch: Duration,
    /// Pause before the single retry, so a server that just restarted has a moment to be ready.
    pub retry_pause: Duration,
}

impl Default for EmbeddingDeadlines {
    fn default() -> Self {
        Self {
            first_batch: Duration::from_secs(240),
            batch: Duration::from_secs(120),
            retry_pause: Duration::from_secs(2),
        }
    }
}

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
    /// Each chat alias's context window, in tokens, as the server device set it. Rust keeps the
    /// latest copy to decide how much of the conversation each question can carry
    /// (`docs/SELECTION-AND-MEMORY.md`). Empty from a gateway that predates it.
    pub context_windows: HashMap<String, usize>,
    /// What the gateway reserves for one answer (`MAX_OUTPUT_TOKENS`).
    pub max_output_tokens: Option<usize>,
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
    /// Absent from an older gateway, which then gets the defaults rather than an error.
    #[serde(default)]
    context_windows: HashMap<String, usize>,
    #[serde(default)]
    max_output_tokens: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct HealthProviderBody {
    reachable: bool,
}

pub struct GatewayClient {
    http: reqwest::Client,
    embedding_deadlines: EmbeddingDeadlines,
}

impl GatewayClient {
    pub fn new() -> Result<Self, AppError> {
        // No client-wide request timeout: that would be a deadline on the whole answer, which is
        // precisely what must not bound a streamed one. `health` and `embed` set their own, and
        // `chat` bounds silence instead.
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .map_err(|_| AppError::Internal)?;
        Ok(Self {
            http,
            embedding_deadlines: EmbeddingDeadlines::default(),
        })
    }

    pub fn with_embedding_deadlines(mut self, deadlines: EmbeddingDeadlines) -> Self {
        self.embedding_deadlines = deadlines;
        self
    }

    pub fn embedding_deadlines(&self) -> EmbeddingDeadlines {
        self.embedding_deadlines
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
            context_windows: body.context_windows,
            max_output_tokens: body.max_output_tokens,
        })
    }

    /// Stream one answer, handing each piece of text to `on_delta`. Returns the whole answer.
    ///
    /// `idle_timeout` bounds **silence**, not duration. An answer that keeps arriving is never
    /// cut off, however long the whole thing takes; one where nothing arrives for that long is
    /// abandoned. A total deadline would be the wrong tool here: a large model on a slow
    /// workstation legitimately spends minutes reading a long prompt before the first word, and
    /// killing a healthy answer at an arbitrary clock time throws away work the practitioner was
    /// reading (`docs/HARDWARE.md`).
    pub async fn chat(
        &self,
        server_url: &str,
        model_alias: &str,
        output_locale: Option<&str>,
        turns: &[ChatTurn],
        idle_timeout: Duration,
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
        // The wait for the response head is silence too: the gateway answers once the model has
        // been reached, and on a busy machine that queueing is exactly the case this bounds.
        let response =
            tokio::time::timeout(idle_timeout, self.http.post(&url).json(&payload).send())
                .await
                .map_err(|_| AppError::ServerTimeout {
                    url: server_url.to_string(),
                })?
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
        // The clock restarts on every piece that arrives, which is what makes this an inactivity
        // bound rather than a deadline.
        while let Some(piece) = tokio::time::timeout(idle_timeout, stream.next())
            .await
            .map_err(|_| AppError::ServerTimeout {
                url: server_url.to_string(),
            })?
        {
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

    /// One vector per input, in the order they were sent (OpenAI-compatible), with the ordinary
    /// per-batch deadline. The caller must already respect the batch caps; this only enforces
    /// them defensively.
    pub async fn embed(
        &self,
        server_url: &str,
        model_alias: &str,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, AppError> {
        self.embed_batch(
            server_url,
            model_alias,
            inputs,
            self.embedding_deadlines.batch,
        )
        .await
    }

    /// `embed` with the deadline chosen by the caller: indexing gives the first request of a pass
    /// the longer one, because the model may still have to be loaded.
    ///
    /// One retry, and only for what a second try can fix: a deadline that passed, or the gateway
    /// answering 502, 503 or 504. A refusal of the request itself (4xx) is never repeated, and an
    /// unreachable server is not either - it said so at once. A deadline that passes twice is
    /// `embedding_timeout`, never `server_timeout`: that code belongs to a chat that went silent,
    /// and its sentence points at a setting that has no effect here.
    pub async fn embed_batch(
        &self,
        server_url: &str,
        model_alias: &str,
        inputs: &[String],
        deadline: Duration,
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

        let first = self
            .embed_once(server_url, model_alias, inputs, deadline)
            .await;
        let Err(failure) = first else {
            return first.map_err(|failure| failure.error);
        };
        if !failure.retryable {
            return Err(failure.error);
        }
        tokio::time::sleep(self.embedding_deadlines.retry_pause).await;
        self.embed_once(server_url, model_alias, inputs, deadline)
            .await
            .map_err(|failure| failure.error)
    }

    async fn embed_once(
        &self,
        server_url: &str,
        model_alias: &str,
        inputs: &[String],
        deadline: Duration,
    ) -> Result<Vec<Vec<f32>>, Attempt> {
        let url = endpoint(server_url, "/v1/embeddings");
        let payload = json!({ "model": model_alias, "input": inputs });
        let response = self
            .http
            .post(&url)
            .timeout(deadline)
            .json(&payload)
            .send()
            .await
            .map_err(|error| embedding_transport_error(error, server_url))?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            let parsed: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            return Err(Attempt {
                error: gateway_error(&parsed).unwrap_or(AppError::ServerError { status }),
                retryable: RETRYABLE_STATUSES.contains(&status),
            });
        }

        let body: EmbeddingsBody = response.json().await.map_err(|error| {
            // A body cut short by the deadline is a timeout like any other; one that arrived
            // whole but unreadable is not worth asking for again.
            if error.is_timeout() {
                embedding_transport_error(error, server_url)
            } else {
                Attempt::final_error(AppError::ServerResponseInvalid)
            }
        })?;
        let mut ordered: Vec<(usize, Vec<f32>)> = body
            .data
            .into_iter()
            .map(|entry| (entry.index, entry.embedding))
            .collect();
        ordered.sort_by_key(|(index, _)| *index);
        let vectors: Vec<Vec<f32>> = ordered.into_iter().map(|(_, vector)| vector).collect();
        if vectors.len() != inputs.len() {
            return Err(Attempt::final_error(AppError::ServerResponseInvalid));
        }
        Ok(vectors)
    }
}

/// Statuses a second attempt can fix: the gateway or the model behind it was busy or restarting.
/// Anything else the gateway refuses with (4xx above all) would be refused the same way again.
const RETRYABLE_STATUSES: [u16; 3] = [502, 503, 504];

/// One failed embeddings attempt, and whether asking again could help.
struct Attempt {
    error: AppError,
    retryable: bool,
}

impl Attempt {
    fn final_error(error: AppError) -> Self {
        Self {
            error,
            retryable: false,
        }
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

/// The embeddings call's own reading of a transport failure: a deadline that passed is
/// `embedding_timeout` and worth one more try; a server that cannot be reached is not.
///
/// A failure to connect comes first, even when it is also a timeout: a machine that is off or on
/// another network never answers the connection, which is "unreachable", not "too slow". Telling
/// her to retry in that case sends her looking at the wrong thing.
fn embedding_transport_error(error: reqwest::Error, server_url: &str) -> Attempt {
    if error.is_connect() {
        Attempt::final_error(AppError::ServerUnreachable {
            url: server_url.to_string(),
        })
    } else if error.is_timeout() {
        Attempt {
            error: AppError::EmbeddingTimeout,
            retryable: true,
        }
    } else {
        Attempt::final_error(transport_error(error, server_url))
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
    fn an_empty_answer_code_in_the_stream_becomes_an_error_with_no_text() {
        // The gateway ends a generation that produced no visible text with this code; the client
        // keeps it as it is and the interface localises it.
        let line = r#"data: {"error":{"code":"empty_answer","message":"empty_answer","data":{}}}"#;

        let error = read_event(line).expect_err("expected a refusal");

        assert_eq!(error.code(), "empty_answer");
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
