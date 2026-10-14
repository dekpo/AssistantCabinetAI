//! P-4: an answer written from supplied material is asked at a low temperature, and an open conversation
//! is asked with none, so the gateway keeps its own default. A tiny stand-in for the gateway records the
//! body of each chat request.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use assistant_cabinet_ai_lib::gateway::{ChatTurn, GatewayClient, GROUNDED_TEMPERATURE};
use serde_json::Value;

fn start_recording_gateway() -> (String, Arc<Mutex<Vec<Value>>>) {
    let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").expect("binds a free port"));
    let url = format!("http://{}", server.server_addr());
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&bodies);
    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let mut body = String::new();
            let _ = request.as_reader().read_to_string(&mut body);
            if let Ok(value) = serde_json::from_str::<Value>(&body) {
                recorded.lock().expect("lock").push(value);
            }
            let answer =
                "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n";
            let response = tiny_http::Response::from_string(answer).with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/event-stream"[..])
                    .expect("a valid header"),
            );
            let _ = request.respond(response);
        }
    });
    (url, bodies)
}

fn turns() -> Vec<ChatTurn> {
    vec![ChatTurn {
        role: "user".to_string(),
        content: "Question".to_string(),
    }]
}

#[tokio::test]
async fn a_grounded_answer_carries_a_low_temperature_and_an_open_one_carries_none() {
    let (url, bodies) = start_recording_gateway();
    let gateway = GatewayClient::new().expect("builds a client");
    let wait = Duration::from_secs(10);

    gateway
        .chat_at(
            &url,
            "cabinet-chat",
            None,
            &turns(),
            wait,
            Some(GROUNDED_TEMPERATURE),
            |_| {},
        )
        .await
        .expect("a grounded answer");
    gateway
        .chat(&url, "cabinet-chat", None, &turns(), wait, |_| {})
        .await
        .expect("an open answer");

    let bodies = bodies.lock().expect("lock");
    assert_eq!(bodies.len(), 2);
    assert_eq!(
        bodies[0]["temperature"]
            .as_f64()
            .map(|t| (t * 10.0).round() / 10.0),
        Some(0.1),
        "{:?}",
        bodies[0]
    );
    assert!(
        bodies[1].get("temperature").is_none(),
        "an open conversation leaves the default: {:?}",
        bodies[1]
    );
    assert!((0.0..=0.2).contains(&GROUNDED_TEMPERATURE));
}
