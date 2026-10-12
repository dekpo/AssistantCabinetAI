//! KB lot 11 - the same library, on the real stack: the embedding model of the gateway, the bundled Tesseract and
//! pdfium for the scans, and the generation model for a few answers.
//!
//! `#[ignore]`: it needs the AI gateway running and takes minutes to hours. It replays what `sourced_answer`
//! does with a question (inventory, selection, router, knowledge step, embedding of the question, search) without
//! the window, and measures what the fake-vector run cannot:
//!
//! - the cost of an Analyse by stage on real files, OCR included, as the library grows from tier to tier;
//! - what OCR really reads of a scanned letter (words and names that survive);
//! - for every question, in the three modes: the passages the search really cites, the files the reduced set
//!   would have kept, and the time of each stage. The release gate reads the first: **the files the answer cites
//!   with the base off, found in the candidate set**;
//! - on a few questions, the time of a whole answer through the generation model, in the three modes.
//!
//! ```text
//! cargo test --release --test knowledge_validation_real -- --ignored --nocapture
//!   ACAI_GATEWAY=http://127.0.0.1:8080  ACAI_EMBED_ALIAS=assistant-embed  ACAI_MODEL=ministral-3:3b
//!   ACAI_TIERS=30,100,200  ACAI_GENERATE=0 (answers per mode)  ACAI_SANDBOX_DIR=<library>  ACAI_VALIDATION_OUT=<folder>
//! ```
//!
//! The application's own data is never touched: the index lives in a temporary folder.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use assistant_cabinet_ai_lib::gateway::{ChatTurn, GatewayClient};
use assistant_cabinet_ai_lib::knowledge::normalize::TitleSet;
use assistant_cabinet_ai_lib::knowledge::targeting::{QueryVocabulary, TargetingRequest};
use assistant_cabinet_ai_lib::knowledge::KnowledgeMode;
use assistant_cabinet_ai_lib::ocr::tesseract::TesseractProvider;
use assistant_cabinet_ai_lib::ocr::OcrProvider;
use assistant_cabinet_ai_lib::raster::{self, PageRasterizer};
use assistant_cabinet_ai_lib::retrieval;
use assistant_cabinet_ai_lib::work_folder_context::{self, ContextView};

use common::validation::*;

const PACKS: [&str; 2] = ["health-fr", "health-ch"];

fn env(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

fn manifest(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}

/// The bundled OCR engine and page rasteriser, when they are staged (`scripts/fetch-ocr-resources.ps1`).
fn real_ocr() -> (
    Option<TesseractProvider>,
    Option<&'static raster::Rasterizer>,
) {
    let binary = std::env::var("ACAI_TESSERACT")
        .map(PathBuf::from)
        .ok()
        .or_else(|| {
            [
                "binaries/tesseract-x86_64-pc-windows-msvc.exe",
                "binaries/tesseract-aarch64-apple-darwin",
                "binaries/tesseract-x86_64-apple-darwin",
                "tesseract.exe",
            ]
            .iter()
            .map(|p| manifest(p))
            .find(|p| p.exists())
        });
    let tessdata = std::env::var("ACAI_TESSDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| manifest("resources/tessdata"));
    let engine = binary.and_then(|b| TesseractProvider::new(b, tessdata).ok());
    let library = std::env::var("ACAI_PDFIUM")
        .map(PathBuf::from)
        .ok()
        .or_else(|| {
            [
                "resources/pdfium/pdfium.dll",
                "resources/pdfium/libpdfium.dylib",
                "resources/pdfium/libpdfium.so",
            ]
            .iter()
            .map(|p| manifest(p))
            .find(|p| p.exists())
        });
    let rasterizer = library.and_then(|l| raster::shared(&l));
    (engine, rasterizer)
}

fn words(text: &str) -> BTreeSet<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3)
        .map(str::to_string)
        .collect()
}

fn squash(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn fold_accents(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    text.nfd()
        .filter(|c| !c.is_ascii_punctuation() && !('\u{300}'..='\u{36f}').contains(c))
        .collect()
}

/// What OCR really read of each scanned document: the share of the page's words that survive, and the names and
/// numbers that survive verbatim (with accents, and without).
fn ocr_quality(lib: &Library, truth: &Truth) -> String {
    let mut out = String::from("### What OCR read of the scanned documents\n\n| Document | Format | Words kept | Names and numbers kept (exact) | Kept without accents | OCR text empty |\n| --- | --- | --- | --- | --- | --- |\n");
    let (mut w_all, mut w_hit, mut m_all, mut m_exact, mut m_fold) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    for doc in truth
        .docs_in(lib.tier)
        .filter(|d| d.ocr && d.product_reads())
    {
        let chunks = lib
            .lab
            .index
            .chunk_texts_for_document(&doc.path)
            .unwrap_or_default();
        let read: String = chunks
            .iter()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let expected = doc.ocr_pages.join("\n");
        let (truth_words, read_words) = (words(&expected), words(&read));
        let kept = truth_words.intersection(&read_words).count();
        let squashed = squash(&read);
        let folded = fold_accents(&squashed);
        let mentions: Vec<&Mention> = doc.mentions.iter().filter(|m| m.place == "text").collect();
        let exact = mentions
            .iter()
            .filter(|m| squashed.contains(&squash(&m.text)))
            .count();
        let loose = mentions
            .iter()
            .filter(|m| folded.contains(&fold_accents(&squash(&m.text))))
            .count();
        out.push_str(&format!(
            "| {} | {} | {} | {} of {} | {} | {} |\n",
            doc.path,
            doc.format,
            pct(kept, truth_words.len()),
            exact,
            mentions.len(),
            loose,
            read.trim().is_empty()
        ));
        w_all += truth_words.len();
        w_hit += kept;
        m_all += mentions.len();
        m_exact += exact;
        m_fold += loose;
    }
    out.push_str(&format!(
        "\nWords kept: **{}**. Names and numbers kept verbatim: **{}** ({} of {}); without accents: {}.\n",
        pct(w_hit, w_all), pct(m_exact, m_all), m_exact, m_all, pct(m_fold, m_all)
    ));
    out
}

#[derive(Default, Clone)]
struct Row {
    id: String,
    category: String,
    mode: &'static str,
    route: &'static str,
    selected: usize,
    candidates: usize,
    searched: usize,
    chunks_considered: usize,
    evidence_chunks: usize,
    evidence_chars: usize,
    cited: Vec<String>,
    candidate_paths: Vec<String>,
    applied: bool,
    would_apply: bool,
    widened: bool,
    skip: String,
    capped: Vec<String>,
    embed_ms: f64,
    knowledge_ms: f64,
    search_ms: f64,
    relevant: BTreeSet<String>,
    risk: String,
}

fn mode_name(mode: KnowledgeMode) -> &'static str {
    match mode {
        KnowledgeMode::Off => "off",
        KnowledgeMode::Suggest => "suggest",
        KnowledgeMode::Auto => "auto",
    }
}

fn tsv(rows: &[Row]) -> String {
    let mut out = String::from("id\tcategory\tmode\troute\tselected\tcandidates\tsearched\tchunks\tevidence_chunks\tevidence_chars\tapplied\twould_apply\twidened\tskip\tembed_ms\tknowledge_ms\tsearch_ms\tcited\n");
    for r in rows {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.1}\t{:.1}\t{:.1}\t{}\n",
            r.id,
            r.category,
            r.mode,
            r.route,
            r.selected,
            r.candidates,
            r.searched,
            r.chunks_considered,
            r.evidence_chunks,
            r.evidence_chars,
            r.applied,
            r.would_apply,
            r.widened,
            r.skip,
            r.embed_ms,
            r.knowledge_ms,
            r.search_ms,
            r.cited.join("|")
        ));
    }
    out
}

fn summarise(rows: &[Row], tier: usize) -> String {
    let mut out = format!("### Real run, tier {tier}\n\n");
    let by_id = |mode: &str| -> BTreeMap<&str, &Row> {
        rows.iter()
            .filter(|r| r.mode == mode)
            .map(|r| (r.id.as_str(), r))
            .collect()
    };
    let (off, suggest, auto) = (by_id("off"), by_id("suggest"), by_id("auto"));

    // --- the gate. `off` cites six passages whatever the library holds, so on a small library it also cites
    // files that have nothing to do with the question (filler). A reduced set drops them on purpose. What a
    // reduction must not lose is a file that holds the answer, so the figure is read twice: on every cited
    // file ("raw", the shadow measure of the diagnostics) and on the cited files the generator knows hold the
    // answer ("relevant").
    let (mut raw_all, mut raw_hit, mut would_apply_questions) = (0usize, 0usize, 0usize);
    let (mut rel_all, mut rel_hit, mut filler_dropped, mut filler_kept) =
        (0usize, 0usize, 0usize, 0usize);
    let mut by_reason: BTreeMap<&str, usize> = BTreeMap::new();
    let mut miss_lines: Vec<String> = Vec::new();
    let mut by_category: BTreeMap<String, (usize, usize, usize, usize)> = BTreeMap::new();
    let mut questions_with_a_loss = 0usize;
    for (id, s) in &suggest {
        let Some(base) = off.get(id) else { continue };
        if !s.would_apply || base.cited.is_empty() {
            continue;
        }
        would_apply_questions += 1;
        let kept: BTreeSet<&str> = s.candidate_paths.iter().map(String::as_str).collect();
        let mut lost_here = false;
        for path in &base.cited {
            let is_relevant = base.relevant.contains(path);
            let cell = by_category.entry(s.category.clone()).or_default();
            raw_all += 1;
            if kept.contains(path.as_str()) {
                raw_hit += 1;
            }
            if is_relevant {
                rel_all += 1;
                cell.1 += 1;
                if kept.contains(path.as_str()) {
                    rel_hit += 1;
                    cell.0 += 1;
                } else {
                    lost_here = true;
                    let why = if s.capped.contains(path) {
                        "capped"
                    } else if s.risk.contains("KBD-19") {
                        "carry_over"
                    } else {
                        "not_linked_to_the_name"
                    };
                    *by_reason.entry(why).or_default() += 1;
                    miss_lines.push(format!("- {id} [{}] a file that holds the answer, cited with the base off, not in the candidate set: {path} ({why})", s.category));
                }
            } else if kept.contains(path.as_str()) {
                filler_kept += 1;
                cell.3 += 1;
            } else {
                filler_dropped += 1;
                cell.2 += 1;
            }
        }
        questions_with_a_loss += usize::from(lost_here);
    }
    out.push_str(&format!(
        "**Recall of the candidate set, on the files that hold the answer** (cited with the base off AND known to hold the answer): **{}** ({rel_hit} of {rel_all}), over {would_apply_questions} questions the base would reduce; {questions_with_a_loss} of them lost at least one such file. Lost, by reason: {by_reason:?}\n\n",
        pct(rel_hit, rel_all)
    ));
    out.push_str(&format!(
        "Raw shadow measure (every cited file, filler included): {} ({raw_hit} of {raw_all}). Filler the reduced set dropped: {filler_dropped}; filler it kept: {filler_kept}.\n\n",
        pct(raw_hit, raw_all)
    ));
    out.push_str("| Category | Relevant cited files | Kept by the candidate set | Recall | Filler dropped | Filler kept |\n| --- | --- | --- | --- | --- | --- |\n");
    for (category, (hit, all, dropped, kept_filler)) in &by_category {
        out.push_str(&format!(
            "| {category} | {all} | {hit} | {} | {dropped} | {kept_filler} |\n",
            pct(*hit, *all)
        ));
    }

    // --- the answers' sources, base off against `auto`
    let (
        mut overlap_all,
        mut overlap_hit,
        mut top1_same,
        mut compared,
        mut applied_n,
        mut widened_n,
    ) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    let (mut good_off, mut cited_off_n, mut good_auto, mut cited_auto_n, mut with_relevant) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    let (mut hit_off, mut hit_auto) = (0usize, 0usize);
    for (id, a) in &auto {
        let Some(base) = off.get(id) else { continue };
        if base.route == "folder" || base.cited.is_empty() {
            continue;
        }
        compared += 1;
        applied_n += usize::from(a.applied);
        widened_n += usize::from(a.widened);
        if a.applied {
            overlap_all += base.cited.len();
            overlap_hit += base.cited.iter().filter(|p| a.cited.contains(p)).count();
            top1_same += usize::from(base.cited.first() == a.cited.first());
        }
        if !base.relevant.is_empty() {
            with_relevant += 1;
            good_off += base
                .cited
                .iter()
                .filter(|p| base.relevant.contains(*p))
                .count();
            cited_off_n += base.cited.len();
            good_auto += a
                .cited
                .iter()
                .filter(|p| base.relevant.contains(*p))
                .count();
            cited_auto_n += a.cited.len();
            hit_off += usize::from(base.cited.iter().any(|p| base.relevant.contains(p)));
            hit_auto += usize::from(a.cited.iter().any(|p| base.relevant.contains(p)));
        }
    }
    out.push_str(&format!(
        "
Questions that search the documents: {compared}. Reduced in `auto`: {applied_n}. Searched again on the whole selection after a reduction (widened): {widened_n}.

"
    ));
    out.push_str(&format!(
        "- The `auto` answer still cites the sources the base-off answer cites (on the {applied_n} reduced questions): **{}** ({overlap_hit} of {overlap_all}). Same first source: {top1_same} of {applied_n}.
",
        pct(overlap_hit, overlap_all)
    ));
    out.push_str(&format!(
        "- Questions with a known answer in the library: {with_relevant}. Share of cited files that really hold the answer: base off **{}**, `auto` **{}**. At least one right source: off {hit_off}, auto {hit_auto}.
",
        pct(good_off, cited_off_n), pct(good_auto, cited_auto_n)
    ));

    // --- timings
    let med = |mode: &str, f: &dyn Fn(&Row) -> f64| -> String {
        let v: Vec<f64> = rows
            .iter()
            .filter(|r| r.mode == mode && r.route != "folder")
            .map(f)
            .collect();
        format!(
            "{} / {}",
            fmt1(median(v.clone())),
            fmt1(percentile(v, 0.95))
        )
    };
    out.push_str("\n| Mode | Embedding of the question, median / p95 ms | Knowledge lookup, median / p95 ms | Search, median / p95 ms | Chunks scored, median | Files searched, median | Passages in the prompt, median | Characters of passages, median / p95 |\n| --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for mode in ["off", "suggest", "auto"] {
        let pick = |f: &dyn Fn(&Row) -> f64| -> Vec<f64> {
            rows.iter()
                .filter(|r| r.mode == mode && r.route != "folder")
                .map(f)
                .collect()
        };
        let chars = pick(&|r| r.evidence_chars as f64);
        out.push_str(&format!(
            "| {mode} | {} | {} | {} | {} | {} | {} | {} / {} |\n",
            med(mode, &|r| r.embed_ms),
            med(mode, &|r| r.knowledge_ms),
            med(mode, &|r| r.search_ms),
            fmt1(median(pick(&|r| r.chunks_considered as f64))),
            fmt1(median(pick(&|r| r.searched as f64))),
            fmt1(median(pick(&|r| r.evidence_chunks as f64))),
            fmt1(median(chars.clone())),
            fmt1(percentile(chars, 0.95))
        ));
    }
    out.push_str("\nLost sources (first 60):\n");
    for line in miss_lines.iter().take(60) {
        out.push_str(&format!("{line}\n"));
    }
    out
}

fn fmt1(v: Option<f64>) -> String {
    v.map(|v| format!("{v:.1}")).unwrap_or_else(|| "-".into())
}

#[tokio::test]
#[ignore]
async fn real_run() {
    let truth = Truth::load();
    let url = env("ACAI_GATEWAY", "http://127.0.0.1:8080");
    let alias = env("ACAI_EMBED_ALIAS", "assistant-embed");
    let model = env("ACAI_MODEL", "ministral-3:3b");
    let generate: usize = env("ACAI_GENERATE", "0").parse().unwrap_or(0);
    let tiers: Vec<usize> = env("ACAI_TIERS", "30,100")
        .split(',')
        .filter_map(|t| t.trim().parse().ok())
        .collect();

    let client = GatewayClient::new().expect("a client");
    let health = client
        .health(&url)
        .await
        .expect("the gateway answers; start it with docker compose up -d");
    println!(
        "gateway {url}: {} / chat aliases {:?}",
        health.status, health.aliases
    );

    let (tesseract, pdfium) = real_ocr();
    println!(
        "OCR engine: {}, page rasteriser: {}",
        tesseract
            .as_ref()
            .map(|t| format!("{} {}", t.id(), t.version()))
            .unwrap_or_else(|| "NOT AVAILABLE".into()),
        pdfium.is_some()
    );
    let stack = Stack {
        real: Some(Real {
            client: GatewayClient::new().expect("a client"),
            url: url.clone(),
            alias: alias.clone(),
        }),
        ocr: tesseract.as_ref().map(|t| t as &dyn OcrProvider),
        rasterizer: pdfium.map(|r| r as &dyn PageRasterizer),
    };

    let vocabulary = QueryVocabulary::load("fr-FR", &PACKS).expect("loads the vocabulary");
    let titles: TitleSet = vocabulary.titles.clone();
    let _ = titles;
    let mut library: Option<Library> = None;
    for wanted in tiers {
        let tier = truth.tier_size(wanted);
        match library.as_mut() {
            None => library = Some(library_on(&truth, tier, &PACKS, &stack).await),
            Some(lib) => lib.grow_to(&truth, tier, &stack).await,
        }
        let lib = library.as_ref().expect("a library");
        let mut report = String::new();
        report.push_str(&format!("## Tier {tier}\n\n### Analyse\n\n"));
        for (t, ms, line) in &lib.passes {
            report.push_str(&format!("- to {t} documents: {ms} ms. {line}\n"));
        }
        report.push_str(&format!(
            "\nDocuments read by the product at this tier: {}.\n\n",
            lib.product_files.len()
        ));
        report.push_str(&ocr_quality(lib, &truth));
        println!("{report}");

        // --- the questions
        let replay = Replay::new(lib, &vocabulary);
        let connection = lib.lab.side();
        let _ = connection;
        let mut rows: Vec<Row> = Vec::new();
        let mut answers: Vec<String> = Vec::new();
        let mut generated = 0usize;
        for q in truth
            .questions
            .iter()
            .filter(|q| q.min_tier <= tier && q.category != "data")
        {
            let (paths, with_workbooks) = {
                let paths = match &q.selection.documents {
                    Pick::Word(_) => None,
                    Pick::Paths { paths } => Some(
                        paths
                            .iter()
                            .filter(|p| lib.product_files.contains(p))
                            .cloned()
                            .collect::<Vec<_>>(),
                    ),
                };
                (
                    paths,
                    !matches!(&q.selection.workbooks, Pick::Word(w) if w == "none"),
                )
            };
            let request = TargetingRequest::default();
            let first = replay.ask(
                &q.text,
                KnowledgeMode::Off,
                &request,
                paths.as_deref(),
                with_workbooks,
            );
            if matches!(first.route, RouteKind::Folder | RouteKind::NothingSelected) {
                rows.push(Row {
                    id: q.id.clone(),
                    category: q.category.clone(),
                    mode: "off",
                    route: "folder",
                    ..Row::default()
                });
                continue;
            }
            let embed_started = Instant::now();
            let vectors = client
                .embed(&url, &alias, std::slice::from_ref(&q.text))
                .await;
            let embed_ms = embed_started.elapsed().as_secs_f64() * 1000.0;
            let Ok(vectors) = vectors else {
                println!("embedding failed for {}", q.id);
                continue;
            };
            let embedding = vectors.into_iter().next().unwrap_or_default();
            let relevant: BTreeSet<String> = q.relevant_readable.iter().cloned().collect();
            for mode in [
                KnowledgeMode::Off,
                KnowledgeMode::Suggest,
                KnowledgeMode::Auto,
            ] {
                let asked = replay.ask(&q.text, mode, &request, paths.as_deref(), with_workbooks);
                let started = Instant::now();
                let searched = replay.search(&asked, &q.text, &embedding);
                let search_ms = started.elapsed().as_secs_f64() * 1000.0;
                let (cited, chunks, widened, evidence_chunks, evidence_chars) = match &searched {
                    Some(s) => {
                        let mut seen = Vec::new();
                        for e in &s.outcome.evidence {
                            if !seen.contains(&e.relative_path) {
                                seen.push(e.relative_path.clone());
                            }
                        }
                        let chars: usize = s
                            .outcome
                            .evidence
                            .iter()
                            .map(|e| e.text.chars().count())
                            .sum();
                        (
                            seen,
                            s.outcome.chunks_considered,
                            s.widened,
                            s.outcome.evidence.len(),
                            chars,
                        )
                    }
                    None => (Vec::new(), 0, false, 0, 0),
                };
                let plan = asked.plan.as_ref();
                // in suggest the candidates are a shadow; in auto they are what runs
                let candidate_paths: Vec<String> = plan
                    .map(|p| p.reduction.documents.candidates.clone())
                    .unwrap_or_default();
                let capped = plan
                    .map(|p| p.reduction.documents.capped.clone())
                    .unwrap_or_default();
                rows.push(Row {
                    id: q.id.clone(),
                    category: q.category.clone(),
                    mode: mode_name(mode),
                    route: asked.route.code(),
                    selected: asked.selected.len(),
                    candidates: candidate_paths.len(),
                    searched: asked.searched().len(),
                    chunks_considered: chunks,
                    evidence_chunks,
                    evidence_chars,
                    cited: cited.clone(),
                    candidate_paths: candidate_paths.clone(),
                    applied: asked.applied(),
                    would_apply: plan.is_some_and(|p| p.reduction.documents.has_candidates()),
                    widened,
                    skip: asked.skip_code(),
                    capped,
                    embed_ms,
                    knowledge_ms: plan.map(|p| p.knowledge_ms as f64).unwrap_or(0.0),
                    search_ms,
                    relevant: relevant.clone(),
                    risk: q.risk.clone(),
                });
            }
            // the answer itself, on a few questions, in each mode
            if generated < generate
                && matches!(
                    q.category.as_str(),
                    "person"
                        | "organization"
                        | "identifier"
                        | "long_document"
                        | "ocr"
                        | "practitioner"
                )
            {
                generated += 1;
                for mode in [
                    KnowledgeMode::Off,
                    KnowledgeMode::Suggest,
                    KnowledgeMode::Auto,
                ] {
                    let asked =
                        replay.ask(&q.text, mode, &request, paths.as_deref(), with_workbooks);
                    let Some(searched) = replay.search(&asked, &q.text, &embedding) else {
                        continue;
                    };
                    let full = assistant_cabinet_ai_lib::inventory::WorkFolderInventory::discover(
                        lib.lab.work_path(),
                        Some(&lib.lab.index),
                    )
                    .expect("inventory");
                    let context = work_folder_context::build(&full, &ContextView::Summary);
                    let grounding = format!(
                        "{}\n{}",
                        work_folder_context::build_system_turn(
                            retrieval::RETRIEVAL_INSTRUCTION,
                            &context
                        ),
                        retrieval::format_evidence(&searched.outcome.evidence)
                    );
                    let turns = vec![ChatTurn {
                        role: "user".into(),
                        content: format!("{grounding}\n\nQuestion: {}", q.text),
                    }];
                    let started = Instant::now();
                    let first_token = std::sync::OnceLock::<f64>::new();
                    let answer = client
                        .chat(
                            &url,
                            &model,
                            Some("fr-FR"),
                            &turns,
                            Duration::from_secs(300),
                            |_| {
                                first_token
                                    .get_or_init(|| started.elapsed().as_secs_f64() * 1000.0);
                            },
                        )
                        .await;
                    let total = started.elapsed().as_secs_f64() * 1000.0;
                    let text = answer.unwrap_or_else(|e| format!("(error {e:?})"));
                    let has_fact = q
                        .expect
                        .contains
                        .iter()
                        .all(|f| squash(&text).contains(&squash(f)));
                    answers.push(format!(
                        "{{\"id\":\"{}\",\"mode\":\"{}\",\"first_token_ms\":{:.0},\"total_ms\":{:.0},\"prompt_chars\":{},\"sources\":{},\"fact_found\":{},\"answer\":{}}}",
                        q.id, mode_name(mode), first_token.get().copied().unwrap_or(0.0), total, grounding.chars().count(),
                        searched.outcome.evidence.len(), has_fact, serde_json::to_string(&text).unwrap_or_default()
                    ));
                    println!(
                        "answer {} {}: first token {:.0} ms, total {:.0} ms, prompt {} chars",
                        q.id,
                        mode_name(mode),
                        first_token.get().copied().unwrap_or(0.0),
                        total,
                        grounding.chars().count()
                    );
                }
            }
        }
        let summary = summarise(&rows, tier);
        println!("{summary}");
        report.push_str(&summary);
        write_report(&format!("real-tier-{tier}.md"), &report);
        write_report(&format!("real-tier-{tier}.tsv"), &tsv(&rows));
        if !answers.is_empty() {
            write_report(
                &format!("real-answers-tier-{tier}.jsonl"),
                &answers.join("\n"),
            );
        }
    }
}

/// What the real OCR engine reads of every scanned document of the library, by how badly the page was scanned:
/// the share of the words that survive, the names and numbers that survive verbatim (with accents, and without),
/// and the time per file. No gateway needed. `ACAI_SANDBOX_DIR` points at a larger library.
#[test]
#[ignore]
fn ocr_on_every_scan() {
    use assistant_cabinet_ai_lib::discovery::DiscoveredFile;
    use assistant_cabinet_ai_lib::extraction::extract;

    let truth = Truth::load();
    let (tesseract, pdfium) = real_ocr();
    let tesseract =
        tesseract.expect("the bundled Tesseract is staged (scripts/fetch-ocr-resources.ps1)");
    let rasterizer = pdfium.expect("the bundled pdfium is staged");
    let mut out = String::from("### What the real OCR engine read of every scan, by how badly the page was scanned\n\n| Level | Document | Format | Seconds | Words kept | Names and numbers kept (exact) | Kept without accents |\n| --- | --- | --- | --- | --- | --- | --- |\n");
    #[allow(clippy::type_complexity)]
    let mut by_level: BTreeMap<i64, (usize, usize, usize, usize, usize, usize, f64)> =
        BTreeMap::new();
    let raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(truth.root.join("documents.json")).unwrap())
            .unwrap();
    let levels: BTreeMap<String, i64> = raw
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|d| Some((d["path"].as_str()?.to_string(), d["ocr_level"].as_i64()?)))
        .collect();
    for doc in truth.docs.iter().filter(|d| d.ocr && d.product_reads()) {
        let path = truth.root.join("documents").join(&doc.path);
        let file = DiscoveredFile {
            relative_path: doc.path.clone(),
            absolute_path: path.display().to_string(),
            extension: doc.extension.clone(),
            size_bytes: 0,
            modified_at: None,
        };
        let started = Instant::now();
        let extracted =
            extract(&file, Some(&tesseract), Some(rasterizer), "fr-FR").expect("extracts");
        let seconds = started.elapsed().as_secs_f64();
        let read: String = extracted
            .pages
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let expected = doc.ocr_pages.join("\n");
        let (truth_words, read_words) = (words(&expected), words(&read));
        let kept = truth_words.intersection(&read_words).count();
        let squashed = squash(&read);
        let folded = fold_accents(&squashed);
        let mentions: Vec<&Mention> = doc.mentions.iter().filter(|m| m.place == "text").collect();
        let exact = mentions
            .iter()
            .filter(|m| squashed.contains(&squash(&m.text)))
            .count();
        let loose = mentions
            .iter()
            .filter(|m| folded.contains(&fold_accents(&squash(&m.text))))
            .count();
        let level = levels.get(&doc.path).copied().unwrap_or(-1);
        out.push_str(&format!(
            "| {level} | {} | {} | {seconds:.1} | {} | {exact} of {} | {loose} |\n",
            doc.path,
            doc.format,
            pct(kept, truth_words.len()),
            mentions.len()
        ));
        let cell = by_level.entry(level).or_default();
        cell.0 += 1;
        cell.1 += truth_words.len();
        cell.2 += kept;
        cell.3 += mentions.len();
        cell.4 += exact;
        cell.5 += loose;
        cell.6 += seconds;
    }
    out.push_str("\n| Level | Scans | Words kept | Names and numbers kept (exact) | Without accents | Seconds per scan |\n| --- | --- | --- | --- | --- | --- |\n");
    for (level, (n, words_all, words_hit, m_all, m_exact, m_fold, seconds)) in &by_level {
        out.push_str(&format!(
            "| {level} | {n} | {} | {} ({m_exact} of {m_all}) | {} | {:.1} |\n",
            pct(*words_hit, *words_all),
            pct(*m_exact, *m_all),
            pct(*m_fold, *m_all),
            seconds / *n as f64
        ));
    }
    println!("{out}");
    write_report("ocr.md", &out);
}
