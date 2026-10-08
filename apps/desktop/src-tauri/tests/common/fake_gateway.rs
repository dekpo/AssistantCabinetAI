//! A stand-in for the gateway's `/v1/embeddings`, scripted per request.
//!
//! Every request is served on its own thread, so a deliberately slow answer never queues the
//! retry behind it - exactly like the real gateway, which serves requests concurrently.

#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

/// What the client sent in one request, with the order the server saw it in.
#[derive(Debug, Clone)]
pub struct Seen {
    pub number: usize,
    pub inputs: Vec<String>,
}

impl Seen {
    pub fn chars(&self) -> usize {
        self.inputs.iter().map(|text| text.chars().count()).sum()
    }
}

pub enum Reply {
    /// One vector per input, after an optional pause.
    Vectors(Duration),
    /// An HTTP status with a JSON body, like the gateway's own errors.
    Status(u16, Value),
}

impl Reply {
    pub fn vectors() -> Self {
        Reply::Vectors(Duration::ZERO)
    }

    pub fn slow(pause: Duration) -> Self {
        Reply::Vectors(pause)
    }

    /// The body the gateway writes for a refusal: `{"error": {"code": ..., "data": {...}}}`.
    pub fn refusal(status: u16, code: &str) -> Self {
        Reply::Status(status, json!({ "error": { "code": code, "data": {} } }))
    }
}

pub struct FakeGateway {
    pub url: String,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl FakeGateway {
    /// `script` decides each reply from the request it is answering (numbered from 1).
    pub fn start(script: impl Fn(&Seen) -> Reply + Send + Sync + 'static) -> Self {
        let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").expect("binds a free port"));
        let url = format!("http://{}", server.server_addr());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let counter = Arc::new(AtomicUsize::new(0));
        let script = Arc::new(script);

        let accepting = Arc::clone(&seen);
        std::thread::spawn(move || {
            for mut request in server.incoming_requests() {
                let mut body = String::new();
                let _ = request.as_reader().read_to_string(&mut body);
                let payload: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                let inputs: Vec<String> = match &payload["input"] {
                    Value::Array(items) => items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_string))
                        .collect(),
                    Value::String(text) => vec![text.clone()],
                    _ => Vec::new(),
                };
                let entry = Seen {
                    number: counter.fetch_add(1, Ordering::SeqCst) + 1,
                    inputs,
                };
                accepting.lock().expect("lock").push(entry.clone());
                let script = Arc::clone(&script);
                std::thread::spawn(move || {
                    let response = match script(&entry) {
                        Reply::Vectors(pause) => {
                            std::thread::sleep(pause);
                            // Answered out of order on purpose: a client that trusts the order
                            // of `data` instead of its `index` would shuffle the vectors.
                            let mut data: Vec<Value> = entry
                                .inputs
                                .iter()
                                .enumerate()
                                .map(|(index, text)| {
                                    json!({ "index": index, "embedding": vector_for(text) })
                                })
                                .collect();
                            data.reverse();
                            tiny_http::Response::from_string(json!({ "data": data }).to_string())
                                .with_status_code(200)
                        }
                        Reply::Status(status, body) => {
                            tiny_http::Response::from_string(body.to_string())
                                .with_status_code(status)
                        }
                    };
                    let _ = request.respond(response);
                });
            }
        });

        Self { url, seen }
    }

    pub fn requests(&self) -> Vec<Seen> {
        self.seen.lock().expect("lock").clone()
    }
}

/// A vector that says which text it was made from: the number in a `para-0042` marker when the
/// text starts with one, so a test can tell the order vectors came back in.
pub fn vector_for(text: &str) -> Vec<f32> {
    let marker = text
        .strip_prefix("para-")
        .and_then(|rest| rest.get(..4))
        .and_then(|digits| digits.parse::<f32>().ok())
        .unwrap_or(-1.0);
    vec![marker, text.len() as f32]
}

/// A reply server address nobody listens on: the connection is refused at once.
pub fn closed_address() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("binds a free port");
    let address = listener.local_addr().expect("local address");
    drop(listener);
    format!("http://{address}")
}
