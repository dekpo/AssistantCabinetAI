//! A stand-in for the gateway's `/v1/chat/completions` that counts every request and records the
//! last turn of each one. A test that needs "the model was never dialled" reads `calls`; a test
//! that needs "what the model was sent" reads `requests`.
//!
//! One thread serves the requests in order, so a test that dials it once sees one entry.

#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub struct CountingGateway {
    pub url: String,
    pub calls: Arc<AtomicUsize>,
    requests: Arc<Mutex<Vec<String>>>,
}

impl CountingGateway {
    /// How many requests reached the gateway so far.
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    /// The content of the last turn of every request, in arrival order.
    pub fn requests(&self) -> Vec<String> {
        self.requests.lock().expect("lock").clone()
    }
}

/// Answers every request with the body `{"unsupported": true}`, which is not a stream: any call
/// reaching it is already the failure the test exists to catch.
pub fn start_counting_gateway() -> CountingGateway {
    start(|| r#"{"unsupported": true}"#.to_string())
}

/// Answers every request with `content`, as the one chunk of a streamed reply.
pub fn start_gateway_replying(content: &str) -> CountingGateway {
    let escaped = serde_json::to_string(content).expect("a string always serialises");
    let stream = format!(
        "data: {{\"choices\":[{{\"delta\":{{\"content\":{escaped}}}}}]}}\n\ndata: [DONE]\n\n"
    );
    start(move || stream.clone())
}

/// Answers the requests with `replies` in order, as streamed chunks; a request beyond the last one gets
/// the last reply again. A reply that is `None` is the body of an unsupported gateway (an error to
/// the caller).
pub fn start_gateway_replying_in_turn(replies: &[Option<&str>]) -> CountingGateway {
    let streams: Vec<String> = replies
        .iter()
        .map(|reply| match reply {
            Some(content) => {
                let escaped = serde_json::to_string(content).expect("a string always serialises");
                format!(
                    "data: {{\"choices\":[{{\"delta\":{{\"content\":{escaped}}}}}]}}

data: [DONE]

"
                )
            }
            None => r#"{"unsupported": true}"#.to_string(),
        })
        .collect();
    let served = AtomicUsize::new(0);
    start(move || {
        let turn = served.fetch_add(1, Ordering::SeqCst).min(streams.len() - 1);
        streams[turn].clone()
    })
}

fn start(reply: impl Fn() -> String + Send + 'static) -> CountingGateway {
    let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").expect("binds a free port"));
    let url = format!("http://{}", server.server_addr());
    let calls = Arc::new(AtomicUsize::new(0));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let counted = Arc::clone(&calls);
    let recorded = Arc::clone(&requests);
    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let mut body = String::new();
            let _ = request.as_reader().read_to_string(&mut body);
            let payload: serde_json::Value =
                serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
            let content = payload["messages"]
                .as_array()
                .and_then(|turns| turns.last())
                .and_then(|turn| turn["content"].as_str())
                .unwrap_or(&body)
                .to_string();
            recorded.lock().expect("lock").push(content);
            counted.fetch_add(1, Ordering::SeqCst);
            let _ = request.respond(tiny_http::Response::from_string(reply()));
        }
    });
    CountingGateway {
        url,
        calls,
        requests,
    }
}
