# Retrieval — the files never live on the AI server

## Decision

Client-side retrieval is possible and it is the project default. The inference server must not become a
parallel document store. Each workstation, or each Windows profile, keeps its own documents and its own
index. The server runs the model, holds a queue, and keeps a request register with no document bodies.

Open WebUI "Knowledge", which uploads files to the server, is **forbidden for business documents**. It stays
acceptable for non-sensitive text such as empty letter templates or an internal procedure already public
within the practice.

## What the server sees anyway

"Not stored" is not "never seen". To answer, the model must receive the question plus the relevant excerpts
for the duration of the request, in memory. That is unavoidable when inference is centralised.

The contract is that those excerpts are **ephemeral**: no disk write, no server-side history, no knowledge
base, encrypted swap, process memory only. After the answer, the server forgets.

## Split of responsibilities

```text
Workstation
  ├─ Files (local disk, or an already-authorised share)
  ├─ Extractor, including local OCR for scans and images
  ├─ Full-text index plus vectors (SQLite)
  ├─ Workbook inventory and the deterministic tabular engine (SQLite)
  ├─ Conversation history (local, encrypted)
  └─ Sends to the gateway: question + top-k excerpts + hashes

AI server
  ├─ Stateless LLM (no-store)
  ├─ Request register: id, user, timestamp, model, tokens, excerpt count,
  │   file hashes, status — NOT the text
  └─ No index, no file copies, no full prompts
```

Traceability — knowing that a case produced twelve requests — comes from that register plus the local
history. To resume a thread, the client resends the context it holds.

## Pipeline

```text
work folder → parsing (PDF / DOCX / TXT / MD, plus OCR for scans, JPEG, PNG) → chunking → embeddings
→ local index → retrieval → relevant context → gateway → LLM → answer + sources
```

Every stage sits behind a replaceable interface. Business code must not know which vector store or which
embedding model is in use, so that tomorrow's model can replace today's without touching the application.

**v0 choices, deliberately boring.** Extraction, chunking and the index live in the Rust core of the client.
The index is SQLite: full-text search for lexical matching, plus stored vectors compared in memory. A work
folder holds tens of files, not millions, so brute-force cosine is fast enough and avoids a vector-database
dependency for the prototype. Chunks keep file, page and section so that a citation can point at them.

**The lexical bonus is held to the scope.** The full-text search only decides which chunks earn a small bonus
(0.2) on top of their vector score, and it keeps six places. Run over the whole index and filtered afterwards, a
file outside the user's selection that repeats the question's words takes all six places and the match the
selection really holds goes without its bonus. So for a named file, a chosen set of files and the current folder
the full-text query itself is restricted to those files' paths (`IndexStore::search_lexical_in`, a join on
`chunks` with the path set passed as one JSON value, `bm25` unchanged); only "the whole index" keeps the
unrestricted search. "Each document" runs that query once for its whole list of files, not once per file. Ranking,
the caps, the four-character minimum for a query term and the order in which "each document" takes files are
**unchanged**: those change what the user gets and are decided with measurements
(`docs/DECISIONS.md`, "Settled by KB lot 0").

**Embeddings.** First implementation is a no-store call to the gateway: the server computes vectors and
discards them, storing nothing. A local ONNX computation with a small model can replace it later behind the
same interface, which would also make indexing work with the server switched off. Since the Mac mini sits at
the practice, excerpts sent for embedding never leave the practice network.

`POST /v1/embeddings` is OpenAI-compatible: `input` is one text or a batch, and the answer is one vector
per input in the order they were sent. Three rules matter more than the wire shape.

- The embedding weights are reached through their own alias out of the same allow-list, so which model
  builds the vectors is configuration. Changing it means **re-indexing**: vectors from two models cannot
  be compared, and mixing them silently degrades every result.
- The batch is capped on count and on total characters, in the gateway as well as in the client, because
  the gateway does not trust a caller.
- A batch that comes back with the wrong number of vectors, or with vectors of different lengths, is
  refused rather than stored. Accepting a short batch would bind chunk 2 to the vector of chunk 3, and
  the index would then cite the wrong passage for as long as it lives.

The register records the alias, how many passages, how many characters, how many vectors, the dimensions
and one SHA-256 over the batch. Not a passage.

**How indexing sends it, and what it can and cannot do.** A document is embedded in requests of at most 16
chunks and 20 000 characters, in chunk order, and its chunks enter the index only when **every** vector exists: a file that fails
halfway leaves nothing behind and is retried by the next pass. Each request has a deadline (120 s, 240 s for the first of a pass)
and one retry on a deadline or a 502 / 503 / 504. There is **no cap on the size of a file**; the cost is linear, about 0.7 s per
1 000-character chunk on the CPU-only reference stack, so a 42-page text PDF takes about 80 s to analyse and a thousand-page one
takes about half an hour, shown by a bar that moves by batch. When one file cannot be embedded (a deadline passing twice, an error
of the runtime's own) the pass goes on, lists the file with its reason and retries it next time; when the server is unreachable or
the model is down, or three files fail in a row, the pass stops with that error. Nothing about this changes what is stored or
what a citation can point at. Decision and numbers: `docs/DECISIONS.md` (embedding timeout fix) and `docs/TROUBLESHOOTING.md`.

**What the logs show.** Every embeddings request leaves one `embedding` event in the gateway's log (metadata only: `request_id`,
`actor`, `started_at`, `duration_ms`, `model_alias`, `input_count`, `input_chars`, `inputs_sha256`, `vector_count`, `dimensions`,
`prompt_tokens`, `outcome`). A healthy pass over a long document is a run of lines with `input_count` <= 16 and `outcome`
`completed`, whose `input_count` adds up to the chunks the file produced; a failed request has the gateway's error code as its
`outcome`. An unchanged folder produces none. How to read them: `docs/OPERATIONS.md`, "Watching an Analyse".

**OCR is part of parsing, and it stays on the workstation.** A page whose text layer is usable is read
natively and never rasterised; a page without one is rendered to a bitmap in memory and handed to a
local engine behind the `OcrProvider` port. The detection is per page, so a letter that mixes a
born-digital covering page with a scanned appendix keeps both. Nothing about this reaches the gateway:
the server receives excerpts, exactly as it does for a native PDF, and has no way to tell the two apart.
Cloud OCR is forbidden on the same grounds as cloud inference, and recognised text is never written
outside the local index — which rules out the usual OCR command-line habit of writing the result into a
file beside the input. Recognised text carries `derivation: Recognised { engine, confidence }` rather
than `Extracted`, and a page the engine could not read produces no chunk at all, so a failed recognition
has no path to a citation. Design and engine choice: `docs/SPRINT-2.5-ASSESSMENT.md`.

## The work folder inventory answers filesystem questions, retrieval does not

Retrieval returns passages, so it can say what a document states and cannot say how many documents
there are. Asking it anyway is how a folder of fifteen files gets described as a folder of six: the
model counts the files its excerpts came from. Since sprint 2a.5 those questions are answered from a
deterministic inventory of the work folder instead, with no gateway call at all - file counts, names,
extensions, paths, the folder structure, and whether one named file was read and by which method.

Two consequences for this pipeline. A question that names a file is resolved first, and retrieval is
then **constrained to that file**, which removes the cross-document contamination that made "what does
neurologie.pdf say?" answerable from a different letter of the same name. And when the model is needed,
the system turn carries the filesystem context beside the excerpts, with a contract forbidding either to
stand in for the other. Design: `docs/WORK-FOLDER-INVENTORY.md`.

Since sprint 2a.7 a conversation can also be **scoped** to files she chose (`AnalysisScope`, `docs/DECISIONS.md`).
Retrieval is then held to exactly those files (`RetrievalScope::Files`): a set with one member behaves like a
named file, and the caps are unchanged, so a narrower scope means better evidence and never more of it. Sources
under an answer are listed once per file with their pages; the excerpts sent to the model and its citations are
not affected by how they are listed.

Since sprint 2a.8 (`docs/SELECTION-AND-MEMORY.md`) a selection with **no document chosen** - the default - is not
searched at all: the question is answered without excerpts, with no embedding call, and the interface says so
under the answer. A selection whose chosen files are all gone or changed is still refused (`scope_unavailable`).
The whole folder ("tous") is searched through `RetrievalScope::CurrentFolder`, which keeps only chunks of files
present in the current folder, so a previous folder's passages cannot be cited before the next Analyse.

A short follow-up question (eight words or fewer) is searched together with the previous question, so "and for
the second one?" finds what the conversation is about. The router and the file-name resolver still read the
current question only.

## Tabular data is a second pipeline, not a special case of the first

Spreadsheets are **not** flattened into text chunks so that they resemble PDFs. Chunking a schedule
destroys exactly the structure that makes it answerable, and embedding it invites the model to guess a
total it cannot count. They get their own path:

```text
work folder → parsing (CSV / XLSX) → workbook inventory → deterministic lookup
→ verified answer                                    (no model, no embedding)
→ or structured evidence → gateway → LLM → answer + sources   (only when necessary)
```

The inventory is built once per file version, keyed by relative path and SHA-256, and persisted locally.
It holds structure — sheets, dimensions, columns, inferred types, row counts, header row, date and
numeric and categorical columns, formula presence — and the aggregate facts computed in a **full pass
over every row**, never a sample. A group total computed from the first two thousand rows of a large
export is wrong in a way nobody notices. No copy of the source file is kept.

Deterministic questions are answered from that inventory with no inference at all: row counts, column
names, distinct values, sums, minima and maxima, group totals, the largest single row, filters and
sorts. A group **total** and the largest **single row** are different facts and are always labelled as
such. When the engine cannot establish an answer it returns `NOT_DETERMINISTICALLY_ANSWERABLE` together
with what it does hold — the available columns — rather than a guess.

Formula workbooks are treated conservatively. A formula-heavy file is inventoried and disclosed, and
refused for factual answers; a cached formula result is never reported as a verified fact. v0 builds no
Excel calculation engine.

Escalation to the model carries the compact aggregate card, never rows, and the model's citations are
verified against that evidence afterwards.

## Partitioning

| Level | Mechanism |
| --- | --- |
| Between users | Index and history in the Windows profile; per-person API keys |
| Between cases | Separate local collections (one folder, one index) |
| Between professions or practices | No shared index; the server has nothing to mix |
| Shared workstation | Distinct Windows accounts, otherwise partitioning is fiction |

A network share does not force indexing on the AI server. Each client indexes what its account may already
read. No new repository is created.

## Implications

- The first indexing of a large folder happens on the workstation and costs CPU time. That is acceptable and
  safer.
- Changing PC means losing the index and the history unless the **workstation** is backed up.
- The gateway must cap excerpt size (for example 8–16k tokens) so a client cannot send a whole folder "just
  in case".
- Open WebUI stores chats and uploads by default: disabled, or reserved for the owner away from real files.
- A second user cannot query the first user's files through the server, because the server has no index.
  That is intended.

## Reliability

The product prefers saying it did not find enough information over generating an unsupported answer. Answers
cite filename, page, section and the relevant passage. Amounts, dates and identifiers are copied from the
source, never reformulated from memory. If extraction fails, classification is blocked rather than guessed.

OCR does not weaken that rule; it is resolved one stage earlier. A page the engine read with too little
confidence does not enter the index, so it cannot be retrieved and cannot become a confident sentence.
Uncertainty is handled at ingestion, where it is still a measurable number, rather than at generation,
where it is not.

Tests that keep this true: retrieval quality, source attribution, refusal beyond the sources, and an
isolation test proving the server retains nothing after a request.

**Tabular reference set.** The deterministic tabular path is measured by a bilingual set of questions,
each asked once in French and once in English, over fictional workbooks written at test time
(`apps/desktop/src-tauri/tests/common/tabular_fixtures.rs`, which carries the hand arithmetic behind
every expected value - never a value produced by the engine). The cases are data, in
`apps/desktop/src-tauri/tests/tabular_reference_cases.json`; `tests/tabular_reference.rs` analyses the
Data Folder and asks each one through tier 2, as the app does, and prints passing and known failures
per gap. A `pass` case must get its expected answer exactly. A `known_failure` case is a gap already
named (A to E from the tabular continuation plan, plus H, found by the set itself), with the session
expected to fix it and, when today's answer is a silently wrong value rather than a refusal, that
wrong value as `current_answer`; it must **not** match its expectation, and its `current_answer` must
still be what comes back. The promotion rule: a fix is done when its known failures turn green and
nothing green turns red. A known failure that starts passing fails the run until its status is set to
`pass`, so no fix goes unrecorded; a `current_answer` that changes without the case passing fails it
too, because the bug changed shape.

## What we refuse

- Uploading a case folder into Open WebUI so that everyone benefits.
- A central Chroma or PGVector of case files.
- Server backups that would contain excerpts or prompts.

Server-side retrieval is only ever reconsidered for shared **non-sensitive** corpora such as templates or a
quality checklist, never for case files.
