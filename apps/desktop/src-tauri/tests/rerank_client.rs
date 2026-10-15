//! Lot D of the retrieval-quality programme, the desktop side of the reranker: the gateway client asks
//! `POST /v1/rerank` for the order of the candidate passages, reads the answer by its indexes, says so when
//! the gateway has no reranker, and the passages are cut to the budget after being put in order.

use std::sync::{Arc, Mutex};

use assistant_cabinet_ai_lib::error::AppError;
use assistant_cabinet_ai_lib::extraction::PageOrigin;
use assistant_cabinet_ai_lib::gateway::GatewayClient;
use assistant_cabinet_ai_lib::retrieval::{cut_after_rerank, Evidence, RetrievalLimits};
use serde_json::{json, Value};

/// A gateway that answers `/v1/rerank` and `/health` from a script, and remembers the bodies it saw.
struct Fake {
    url: String,
    bodies: Arc<Mutex<Vec<Value>>>,
}

fn serve(status: u16, answer: Value) -> Fake {
    let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").expect("binds a free port"));
    let url = format!("http://{}", server.server_addr());
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&bodies);
    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let mut body = String::new();
            let _ = request.as_reader().read_to_string(&mut body);
            seen.lock()
                .unwrap()
                .push(serde_json::from_str(&body).unwrap_or(Value::Null));
            let response = tiny_http::Response::from_string(answer.to_string()).with_status_code(status);
            let _ = request.respond(response);
        }
    });
    Fake { url, bodies }
}

fn evidence(id: &str, text: &str) -> Evidence {
    Evidence {
        chunk_id: id.to_string(),
        relative_path: format!("{id}.pdf"),
        page_number: 1,
        ordinal: 1,
        last_page: 1,
        section: None,
        matches: Vec::new(),
        text: text.to_string(),
        score: 0.1,
        origin: PageOrigin::TextLayer,
        confidence: None,
    }
}

#[tokio::test]
async fn the_candidates_are_sent_with_the_question_and_the_order_is_read_back_by_index() {
    let gateway = serve(
        200,
        json!({
            "object": "list",
            "model": "assistant-rerank",
            "results": [
                { "index": 2, "relevance_score": 0.91 },
                { "index": 0, "relevance_score": 0.40 },
                { "index": 1, "relevance_score": 0.02 }
            ]
        }),
    );
    let client = GatewayClient::new().unwrap();
    let ranking = client
        .rerank(
            &gateway.url,
            "quelle franchise ?",
            &["a".to_string(), "b".to_string(), "c".to_string()],
        )
        .await
        .expect("the gateway ranks");
    assert_eq!(ranking.iter().map(|r| r.index).collect::<Vec<_>>(), vec![2, 0, 1]);
    assert!((ranking[0].score - 0.91).abs() < 1e-6);
    let sent = gateway.bodies.lock().unwrap()[0].clone();
    assert_eq!(sent["query"], "quelle franchise ?");
    assert_eq!(sent["documents"], json!(["a", "b", "c"]));
}

#[tokio::test]
async fn a_gateway_without_a_reranker_is_an_error_the_caller_can_degrade_from() {
    let gateway = serve(
        501,
        json!({ "error": { "code": "provider_capability_unsupported", "data": {} } }),
    );
    let client = GatewayClient::new().unwrap();
    let outcome = client
        .rerank(&gateway.url, "question", &["a".to_string()])
        .await;
    assert!(outcome.is_err(), "{outcome:?}");
}

#[tokio::test]
async fn a_ranking_that_names_a_missing_candidate_is_refused() {
    let gateway = serve(
        200,
        json!({ "results": [ { "index": 5, "relevance_score": 0.9 } ] }),
    );
    let client = GatewayClient::new().unwrap();
    let outcome = client
        .rerank(&gateway.url, "question", &["a".to_string(), "b".to_string()])
        .await;
    assert!(matches!(outcome, Err(AppError::ServerResponseInvalid)), "{outcome:?}");
}

#[tokio::test]
async fn the_health_of_the_gateway_says_whether_a_reranker_is_there() {
    let body = |available: bool| {
        json!({
            "status": "ok", "version": "0.1.0",
            "provider": { "name": "ollama", "reachable": true },
            "aliases": ["a"], "default_model_alias": "a", "embedding_alias": "e",
            "default_output_locale": "fr-FR", "output_locales": ["fr-FR"],
            "context_windows": {}, "max_output_tokens": 2048, "max_context_chars": 200000,
            "rerank_available": available, "max_rerank_documents": 64, "issues": []
        })
    };
    let client = GatewayClient::new().unwrap();
    let with = serve(200, body(true));
    let snapshot = client.health(&with.url).await.unwrap();
    assert!(snapshot.rerank_available);
    assert_eq!(snapshot.max_rerank_documents, Some(64));
    let without = serve(200, body(false));
    assert!(!client.health(&without.url).await.unwrap().rerank_available);
    // an older gateway does not say: no reranker
    let older = serve(
        200,
        json!({
            "status": "ok", "version": "0.1.0",
            "provider": { "reachable": true }, "aliases": [], "default_model_alias": "a",
            "embedding_alias": "e", "default_output_locale": "fr-FR", "output_locales": [],
            "issues": []
        }),
    );
    assert!(!client.health(&older.url).await.unwrap().rerank_available);
}

#[test]
fn the_best_ranked_passages_are_kept_within_the_budget_and_the_rest_are_dropped() {
    let candidates = vec![
        evidence("a", "first"),
        evidence("b", "second"),
        evidence("c", "third"),
        evidence("d", "fourth"),
    ];
    let limits = RetrievalLimits {
        max_chunks: 2,
        max_chars: 1_000,
        max_per_document_chars: 1_000,
    };
    let ranking = [(3usize, 0.9f32), (1, 0.8), (0, 0.5), (2, 0.1)];
    let kept = cut_after_rerank(candidates, &ranking, limits);
    assert_eq!(
        kept.iter().map(|e| e.chunk_id.as_str()).collect::<Vec<_>>(),
        vec!["d", "b"]
    );
    assert!((kept[0].score - 0.9).abs() < 1e-6, "the score shown is the reranker's");
}

#[test]
fn a_passage_that_would_overrun_the_characters_is_skipped_for_the_next_one() {
    let candidates = vec![
        evidence("a", &"x".repeat(900)),
        evidence("b", &"y".repeat(300)),
        evidence("c", &"z".repeat(100)),
    ];
    let limits = RetrievalLimits {
        max_chunks: 3,
        max_chars: 1_000,
        max_per_document_chars: 1_000,
    };
    let kept = cut_after_rerank(candidates, &[(0, 0.9), (1, 0.8), (2, 0.7)], limits);
    assert_eq!(
        kept.iter().map(|e| e.chunk_id.as_str()).collect::<Vec<_>>(),
        vec!["a", "c"],
        "b does not fit after a; c does"
    );
}

#[test]
fn candidates_the_ranking_leaves_out_follow_in_their_own_order_and_a_bad_index_is_ignored() {
    let candidates = vec![evidence("a", "one"), evidence("b", "two"), evidence("c", "three")];
    let limits = RetrievalLimits {
        max_chunks: 6,
        max_chars: 1_000,
        max_per_document_chars: 1_000,
    };
    let kept = cut_after_rerank(candidates, &[(2, 0.9), (9, 0.8)], limits);
    assert_eq!(
        kept.iter().map(|e| e.chunk_id.as_str()).collect::<Vec<_>>(),
        vec!["c", "a", "b"]
    );
}
