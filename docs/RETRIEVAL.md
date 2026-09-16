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
  ├─ Extractor, and OCR later
  ├─ Full-text index plus vectors (SQLite)
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
work folder → parsing (PDF / DOCX / TXT) → chunking → embeddings
→ local index → retrieval → relevant context → gateway → LLM → answer + sources
```

Every stage sits behind a replaceable interface. Business code must not know which vector store or which
embedding model is in use, so that tomorrow's model can replace today's without touching the application.

**v0 choices, deliberately boring.** Extraction, chunking and the index live in the Rust core of the client.
The index is SQLite: full-text search for lexical matching, plus stored vectors compared in memory. A work
folder holds tens of files, not millions, so brute-force cosine is fast enough and avoids a vector-database
dependency for the prototype. Chunks keep file, page and section so that a citation can point at them.

**Embeddings.** First implementation is a no-store call to the gateway: the server computes vectors and
discards them, storing nothing. A local ONNX computation with a small model can replace it later behind the
same interface, which would also make indexing work with the server switched off. Since the Mac mini sits at
the practice, excerpts sent for embedding never leave the practice network.

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

Tests that keep this true: retrieval quality, source attribution, refusal beyond the sources, and an
isolation test proving the server retains nothing after a request.

## What we refuse

- Uploading a case folder into Open WebUI so that everyone benefits.
- A central Chroma or PGVector of case files.
- Server backups that would contain excerpts or prompts.

Server-side retrieval is only ever reconsidered for shared **non-sensitive** corpora such as templates or a
quality checklist, never for case files.
