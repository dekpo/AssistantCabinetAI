//! Stopping a question really stops it, rather than hiding it.
//!
//! A tiny local stand-in for the gateway streams an endless answer, slowly. The point of the test
//! is the second assertion: after the stop, no further piece of text arrives. A version that only
//! flipped a flag in the interface would keep consuming this stream - and, in the practice, keep
//! the machine generating at full load for another minute.

use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use assistant_cabinet_ai_lib::cancellation::{until_stopped, Cancellation};
use assistant_cabinet_ai_lib::gateway::{ChatTurn, GatewayClient};

const LINE: &[u8] = b"data: {\"choices\":[{\"delta\":{\"content\":\"mot \"}}]}\n\n";
const CHUNK_INTERVAL: Duration = Duration::from_millis(20);

/// An SSE body that never reaches `[DONE]`, for as long as the client keeps reading. Sent with no
/// content length, so tiny_http streams it chunked.
///
/// Every read fills the buffer it was given rather than handing back one line: tiny_http copies
/// through an 8 KiB buffer, so a reader that returns 45 bytes at a time is not slow, it is silent
/// until it has been read two hundred times.
#[derive(Default)]
struct EndlessAnswer {
    /// How far into the repeating line the last read stopped, so a buffer boundary never cuts a
    /// line in a place the client cannot read back.
    offset: usize,
}

impl Read for EndlessAnswer {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        std::thread::sleep(CHUNK_INTERVAL);
        let mut written = 0;
        while written < buffer.len() {
            let remaining = &LINE[self.offset..];
            let length = remaining.len().min(buffer.len() - written);
            buffer[written..written + length].copy_from_slice(&remaining[..length]);
            written += length;
            self.offset = (self.offset + length) % LINE.len();
        }
        Ok(written)
    }
}

fn start_endless_gateway() -> String {
    let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").expect("binds a free port"));
    let url = format!("http://{}", server.server_addr());

    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let mut body = String::new();
            let _ = request.as_reader().read_to_string(&mut body);
            let response = tiny_http::Response::new(
                tiny_http::StatusCode(200),
                vec![tiny_http::Header::from_bytes(
                    &b"Content-Type"[..],
                    &b"text/event-stream"[..],
                )
                .expect("a valid header")],
                EndlessAnswer::default(),
                None,
                None,
            );
            let _ = request.respond(response);
        }
    });

    url
}

#[tokio::test]
async fn a_stop_ends_the_stream_and_no_further_text_arrives() {
    let server_url = start_endless_gateway();
    let gateway = GatewayClient::new().expect("builds a client");
    let cancellation = Arc::new(Cancellation::default());
    let mut stopped = cancellation.begin();
    let deltas = Arc::new(AtomicUsize::new(0));

    let asked_to_stop = Arc::clone(&cancellation);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        asked_to_stop.cancel();
    });

    let counted = Arc::clone(&deltas);
    let turns = [ChatTurn {
        role: "user".to_string(),
        content: "Resume ce courrier.".to_string(),
    }];
    let outcome = until_stopped(
        &mut stopped,
        gateway.chat(
            &server_url,
            "cabinet-chat",
            None,
            &turns,
            Duration::from_secs(30),
            |_| {
                counted.fetch_add(1, Ordering::SeqCst);
            },
        ),
    )
    .await;

    assert_eq!(
        outcome.expect_err("the run was stopped").code(),
        "chat_cancelled"
    );
    assert!(
        deltas.load(Ordering::SeqCst) > 0,
        "the stand-in gateway must have been streaming before the stop"
    );

    // The connection is closed, so the pieces stop arriving. Generously more than the interval,
    // because a loaded build machine is slow and this must not be a flaky test.
    let at_the_stop = deltas.load(Ordering::SeqCst);
    tokio::time::sleep(CHUNK_INTERVAL * 25).await;

    assert_eq!(
        deltas.load(Ordering::SeqCst),
        at_the_stop,
        "no piece of the answer may arrive after the stop"
    );
}
