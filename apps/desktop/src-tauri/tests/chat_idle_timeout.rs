//! An answer is bounded by silence, not by how long it takes.
//!
//! The client used to put a five-minute deadline on the whole request. On the practice's 2019
//! workstation a larger model spends minutes reading a long prompt and minutes more writing the
//! answer, so that deadline killed answers that were arriving perfectly well and threw away text
//! the practitioner was in the middle of reading. What has to be bounded is a model that has
//! stopped saying anything, which is a different measurement.
//!
//! Two stand-in gateways, and the difference between them is the whole point: one streams for
//! far longer than the bound and must survive; the other goes quiet and must not.

use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use assistant_cabinet_ai_lib::gateway::{ChatTurn, GatewayClient};

const LINE: &[u8] = b"data: {\"choices\":[{\"delta\":{\"content\":\"mot \"}}]}\n\n";
const CHUNK_INTERVAL: Duration = Duration::from_millis(20);
const IDLE_TIMEOUT: Duration = Duration::from_millis(300);

/// Fill the whole buffer with repeated SSE lines.
///
/// tiny_http copies through an 8 KiB buffer, so a reader that hands back one 45-byte line at a
/// time is not slow - it is silent until it has been read two hundred times. Filling the buffer
/// is what makes each read a piece the client actually sees.
fn fill(buffer: &mut [u8], offset: &mut usize) -> usize {
    let mut written = 0;
    while written < buffer.len() {
        let remaining = &LINE[*offset..];
        let length = remaining.len().min(buffer.len() - written);
        buffer[written..written + length].copy_from_slice(&remaining[..length]);
        written += length;
        *offset = (*offset + length) % LINE.len();
    }
    written
}

/// Text for ever, in small regular pieces. No gap is longer than `CHUNK_INTERVAL`.
#[derive(Default)]
struct SteadyAnswer {
    offset: usize,
}

impl Read for SteadyAnswer {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        std::thread::sleep(CHUNK_INTERVAL);
        Ok(fill(buffer, &mut self.offset))
    }
}

/// A few pieces, then nothing at all: a model that has hung, or a machine swapping itself to
/// death, after writing part of an answer. The part it wrote is what the interface has to keep.
struct QuietAfterAWhile {
    offset: usize,
    bursts_left: usize,
}

impl Default for QuietAfterAWhile {
    fn default() -> Self {
        Self {
            offset: 0,
            bursts_left: 5,
        }
    }
}

impl Read for QuietAfterAWhile {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        std::thread::sleep(CHUNK_INTERVAL);
        if self.bursts_left > 0 {
            self.bursts_left -= 1;
            return Ok(fill(buffer, &mut self.offset));
        }
        // Long enough that the client's bound is what ends the wait, not this.
        std::thread::sleep(Duration::from_secs(30));
        Ok(0)
    }
}

fn start_gateway<R>(body: impl Fn() -> R + Send + 'static) -> String
where
    R: Read + Send + 'static,
{
    let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").expect("binds a free port"));
    let url = format!("http://{}", server.server_addr());

    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let mut sent = String::new();
            let _ = request.as_reader().read_to_string(&mut sent);
            let response = tiny_http::Response::new(
                tiny_http::StatusCode(200),
                vec![tiny_http::Header::from_bytes(
                    &b"Content-Type"[..],
                    &b"text/event-stream"[..],
                )
                .expect("a valid header")],
                body(),
                None,
                None,
            );
            let _ = request.respond(response);
        }
    });

    url
}

fn one_question() -> [ChatTurn; 1] {
    [ChatTurn {
        role: "user".to_string(),
        content: "Resume ce courrier.".to_string(),
    }]
}

#[tokio::test]
async fn an_answer_that_keeps_arriving_outlives_the_bound_many_times_over() {
    let server_url = start_gateway(SteadyAnswer::default);
    let gateway = GatewayClient::new().expect("builds a client");
    let deltas = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&deltas);
    let turns = one_question();

    // Five times the idle bound. Under a total deadline this would have been cut off long before
    // the test's own timer; under an inactivity bound it is still going, which is what `Elapsed`
    // here means.
    let still_running = tokio::time::timeout(
        IDLE_TIMEOUT * 5,
        gateway.chat(
            &server_url,
            "cabinet-chat",
            None,
            &turns,
            IDLE_TIMEOUT,
            |_| {
                counted.fetch_add(1, Ordering::SeqCst);
            },
        ),
    )
    .await
    .is_err();

    assert!(
        still_running,
        "an answer that is still arriving must not be cut off"
    );
    assert!(
        deltas.load(Ordering::SeqCst) > 0,
        "the stand-in gateway must have been streaming"
    );
}

#[tokio::test]
async fn an_answer_that_goes_quiet_is_abandoned_and_says_why() {
    let server_url = start_gateway(QuietAfterAWhile::default);
    let gateway = GatewayClient::new().expect("builds a client");
    let deltas = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&deltas);
    let turns = one_question();

    let started = std::time::Instant::now();
    let failure = gateway
        .chat(
            &server_url,
            "cabinet-chat",
            None,
            &turns,
            IDLE_TIMEOUT,
            |_| {
                counted.fetch_add(1, Ordering::SeqCst);
            },
        )
        .await
        .expect_err("silence must end the answer");

    assert_eq!(failure.code(), "server_timeout");
    assert!(
        deltas.load(Ordering::SeqCst) > 0,
        "the text that did arrive reached the caller, so the interface can keep it"
    );
    // The bound is what ended it, not the stand-in's own sleep.
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the wait must end on the idle bound"
    );
}

#[tokio::test]
async fn a_gateway_that_never_answers_at_all_is_bounded_too() {
    // No response head either: the same bound covers the wait for the first byte, which on a busy
    // machine is the model being queued rather than anything being wrong.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("binds a free port");
    let server_url = format!("http://{}", listener.local_addr().expect("an address"));
    std::thread::spawn(move || {
        // Accept and then say nothing, holding the connection open.
        for stream in listener.incoming() {
            std::thread::spawn(move || {
                let _held = stream;
                std::thread::sleep(Duration::from_secs(30));
            });
        }
    });

    let gateway = GatewayClient::new().expect("builds a client");
    let turns = one_question();
    let started = std::time::Instant::now();

    let failure = gateway
        .chat(
            &server_url,
            "cabinet-chat",
            None,
            &turns,
            IDLE_TIMEOUT,
            |_| {},
        )
        .await
        .expect_err("a gateway that never answers must not hang for ever");

    assert_eq!(failure.code(), "server_timeout");
    assert!(started.elapsed() < Duration::from_secs(10));
}
