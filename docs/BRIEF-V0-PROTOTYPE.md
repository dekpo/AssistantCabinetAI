# Brief — v0 four-week prototype (canonical direction)

**Recorded:** 16 September 2026. **Source:** project owner, verbatim.
**Status:** canonical direction for v0. Where an older note disagrees on **ordering or scope**, this brief wins.
**Deadlines.** The brief says four weeks; the owner's "end of the month" leaves two. Split into two
milestones, same technical chain, decided 16 September 2026:

- **30 September 2026** — the chain holds end to end on fictional fixtures: native window, gateway,
  local index, answer with sources, isolation test.
- **14 October 2026** — she uses it: the three GP flows, the Windows installer, deployment at her
  practice on her own documents.

Server for v0: Ollama behind the FastAPI gateway on a **Mac mini at her practice**.
Dated plan and risks: `docs/ROADMAP.md`.

Do not edit the verbatim section below. The dated schedule, acceptance criteria and risks live in
`docs/ROADMAP.md`.

## Hard constraints this brief does NOT override

The following were decided with the pilot GP and remain in force. They are safety and legal limits,
not sequencing preferences. See `docs/PRIVACY-AND-SECURITY.md`, `docs/PILOT-GP.md`, `docs/DECISIONS.md`.

1. No automatic transmission. No MSSanté send, no Apicrypt, no Ameli login, no write into Medilink.
   "Export" means: produce a validated artefact in the work folder or on the clipboard. The GP
   attaches and sends it herself, in her own software.
2. No real patient documents before a written AIPD draft, an identified data controller, and disk
   encryption on the workstation. Fictional fixtures until then (`fixtures/gp-sandbox/`).
3. No permanent delete. Dedicated trash folder only, inside the work folder allow-list.
4. No Open WebUI Computer on a practice machine.
5. Extracted document text is **data**, never instructions. Tools obey schema plus allow-list only.

---

## Verbatim brief

# Assistant Cabinet AI — 4-Week Prototype Direction

You are developing **Assistant Cabinet AI**, a privacy-first local AI application for professional practices handling confidential documents.

## PRIMARY OBJECTIVE

The immediate goal is NOT to build the complete product.

The goal is:

> **Within 4 weeks, produce a reliable installable prototype that can be deployed on a real GP's workstation and used with real administrative documents.**

The developer already has a clear understanding of the GP's main time-consuming workflows.

Prioritize execution over speculative features.

---

# 1. Fundamental architecture

The core principle is:

> **The workstation owns the data. The AI server owns the intelligence.**

Professional documents must remain on the user's workstation.

The AI server must NOT persistently store user documents.

Architecture:

```text
USER WORKSTATION
├── Tauri application
├── local documents
├── local RAG/index
└── local user context
          │
          │ question + retrieved context
          ▼
PRIVATE AI SERVER
├── FastAPI
├── AI Provider
├── model routing
└── Ollama / local LLM
          │
          ▼
       response
       + sources
```

The server may receive relevant retrieved chunks as ephemeral request context, but must not become a document repository.

---

# 2. Client technology

Use:

* **Tauri 2**
* **React**
* **TypeScript**

Use Rust only for native Tauri functionality where required.

Do NOT implement the UI in Rust.

The client must be a real native desktop application and must not depend on a browser being manually opened.

---

# 3. Server technology

Use:

* Python
* FastAPI
* Pydantic
* Ollama

Keep the AI backend provider-agnostic.

Do not hard-code business logic around a specific LLM.

Create an abstraction similar to:

```python
class AIProvider:
    generate(...)
    embed(...)
    rerank(...)
```

Ollama can be the first implementation.

---

# 4. Local RAG

RAG belongs to the workstation.

Initial pipeline:

```text
local folder
    ↓
PDF/DOCX/TXT parsing
    ↓
chunking
    ↓
embeddings
    ↓
local index
    ↓
retrieval
    ↓
relevant context
    ↓
private AI server
    ↓
LLM
    ↓
answer + sources
```

The RAG implementation must be replaceable.

Do not tightly couple application logic to one vector database or embedding model.

---

# 5. Reliability

The system must prefer:

> "I could not find sufficient information in the available documents."

over unsupported generation.

Answers should cite their sources whenever possible:

* filename;
* page;
* section;
* relevant passage.

Never invent document facts.

Create automated tests for:

* retrieval;
* source attribution;
* hallucination resistance;
* server data isolation.

---

# 6. Four-week implementation target

## WEEK 1 — Vertical slice

Implement:

* Tauri + React + TypeScript client;
* FastAPI server;
* Ollama integration;
* server health check;
* client/server configuration;
* basic chat;
* local folder selection.

Target:

```text
Tauri → FastAPI → Ollama → response
```

---

## WEEK 2 — Local RAG

Implement:

* PDF/DOCX/TXT ingestion;
* local indexing;
* local retrieval;
* relevant context transmission;
* source references.

Target:

```text
local documents
→ local retrieval
→ AI server
→ answer + sources
```

Add a test proving that user documents are not persistently stored on the AI server.

---

## WEEK 3 — GP workflows

Implement only the most important real workflows already identified with the GP.

Prioritize approximately 3–5 workflows such as:

* document summarization;
* information retrieval;
* multi-document comparison;
* structured extraction;
* document drafting/transformation.
Document intake → classification → standardized filename generation → local filing → summarization → human validation → export/transmission through the user's existing secure messaging system and/or Medilink.
Never automatically transmit, delete, move or modify a professional document without explicit user confirmation.
Do not add unrelated features.

---

## WEEK 4 — Real-world deployment

Prepare:

* Windows installer;
* configuration;
* error handling;
* logging without sensitive document content;
* offline/LAN testing;
* basic security checks;
* documentation for installation and use.

Deploy the prototype on a real GP workstation.

---

# 7. Explicitly defer

Do NOT prioritize during this 4-week prototype:

* Microsoft Store;
* cloud deployment;
* multi-tenant SaaS;
* autonomous agents;
* advanced voice;
* advanced vision;
* complex RBAC;
* billing;
* analytics;
* large model catalogs;
* commercial packaging.

These belong after the first real-world validation.

---

# 8. Engineering principle

Whenever choosing between:

A. adding another impressive AI feature

and

B. making the existing workflow more reliable, private, testable and replaceable,

choose B.

Avoid unnecessary rewrites.

Inspect the existing repository before modifying it.

Reuse existing components where appropriate.

Keep interfaces clean and implementation replaceable.

---

# 9. Definition of success

At the end of four weeks, the following must work:

```text
User opens Assistant Cabinet AI
        ↓
Selects a local folder
        ↓
Documents are indexed locally
        ↓
User asks a question
        ↓
Relevant information is retrieved locally
        ↓
Only necessary context is sent to the private AI server
        ↓
Local LLM generates the answer
        ↓
Answer provides document sources
        ↓
No user documents are persistently stored on AI server
```

The prototype must be usable by a real GP.

Do not optimize for completeness.

Optimize for:

**working software + privacy + reliability + real-world feedback.**

---

# 10. Long-term architectural rule

The current hardware, inference runtime and LLM are replaceable.

The durable product architecture must survive changes in:

* LLM;
* inference engine;
* embedding model;
* reranker;
* hardware;
* operating system;
* local AI runtime.

Build the platform so that tomorrow's model can replace today's model without rewriting the application.

> **Build the smallest complete private AI workflow now. Make it excellent later.**
