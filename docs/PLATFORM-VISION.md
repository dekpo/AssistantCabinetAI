# Platform vision

Long-term direction. **This file is not a plan.** What we are actually building, and when, is
`docs/ROADMAP.md`. Why the product exists and for whom is `docs/VISION.md`. The stable technical
boundaries are `docs/ARCHITECTURE.md`.

The rule this document exists to protect: the long-term vision must not contaminate the current sprint
with speculative infrastructure. Nothing below is implemented before the pilot holds. It is written down
so that today's decisions do not close tomorrow's doors, not so that tomorrow's features get built early.

## What the product is becoming

Not `desktop application + local model`. A **private AI platform for professional practices**: several
interchangeable inference engines, several document and data sources, several authenticated clients.

```text
Clients        Desktop (Windows, macOS)  ·  Mobile (later)  ·  Web (later)
                                   │
                            Private API
                                   │
Capabilities   Documents · Tabular data · Retrieval · Deterministic analysis
               AI orchestration · Models · Workflows · Devices · Policies
                                   │
Runtimes       Ollama today  ·  MLX / llama.cpp / vLLM / another private runtime later
```

Two properties hold at every stage, and everything else is negotiable.

**The workstation owns the sensitive corpus.** Documents, the retrieval index, the tabular inventory and
the chat history live on the professional's machine. Only the minimum selected evidence crosses the AI
boundary, and the server keeps a metadata register rather than the text. See
`docs/PRIVACY-AND-SECURITY.md` and `docs/RETRIEVAL.md`.

**The LLM is not the source of truth.** For anything that can be retrieved or computed locally, the
platform prefers deterministic computation, and it keeps a verified fact structurally distinct from a
model's reasoning. Changing the model must not change a spreadsheet answer. This is a boundary, not a
performance optimisation.

## Capability progression

| | Stage | Capabilities added |
| --- | --- | --- |
| **V0** | Current pilot, September–October 2026 | Tauri desktop, private FastAPI gateway, workstation retrieval over PDF/DOCX/TXT/MD and CSV/XLSX, deterministic tabular lookup, sourced answers, refusal when evidence is insufficient, privacy isolation, Windows deployment, one GP pilot. No mobile |
| **V1** | Platform foundation, after pilot validation | Stronger authentication, workspace and practice isolation, device identity, policy boundaries, richer audit metadata, a model registry, AI orchestration, improved retrieval, more formats, a mobile API surface, a first mobile client |
| **V1.5** | Multi-device | Mobile query with sourced answers, camera document capture, upload, notifications, secure device pairing, workstation actions under explicit authorisation |
| **V2** | Professional workflows | OCR, voice, a workflow engine, structured extraction, controlled automation, approval workflows, richer tabular analysis, additional professional domains |
| **V2.5** | Hybrid intelligence | Multiple AI runtimes, capability-aware model selection, on-device inference where appropriate, more sophisticated deterministic tools, an evaluation and benchmark pipeline |
| **V3** | Ecosystem | Windows, macOS, iOS, Android and web clients; multiple private AI nodes; multiple engines and models; an advanced policy engine; workflow orchestration; further verticals |

V0 is the only stage with dates. The others are an ordering, and each one waits on evidence from the one
before it.

## Boundaries that make the progression possible

Each row is something we do today at no extra cost, which prevents a rewrite later.

| Future change | What keeps it cheap |
| --- | --- |
| A second client (mobile, web) | Engine modules import no Tauri. Commands are request/response data. Provenance is self-describing, so a client that never saw the file can render the citation |
| A second inference runtime | `AIProvider` (`generate`, `embed`, `rerank`, `check_health`). Business code never addresses Ollama |
| A second index or vector store | `IndexStore` and `InventoryStore` traits. SQLite is an implementation, not a schema the application knows |
| Local ONNX embeddings instead of gateway embeddings | The `Embedder` port. Retrieval does not know where a vector came from |
| A new file format, or a database or API data source | `TextExtractor` and `TabularDataSource` adapters behind one `Source` model |
| A new profession | File discovery is extension-driven and contains no domain assumption. Column and naming hints are locale and profession **data**, never code |
| A new language | Interface catalogues, plus the locale-keyed question pattern pack. `docs/LANGUAGE-AND-LOCALE.md` |
| The LLM being unavailable | Deterministic tabular answers still work. This is a test, not an aspiration |

## What stays out, permanently or until stated otherwise

Cloud inference on business documents, cloud speech, telemetry, a server-side case index, a medical
records warehouse on the AI server, automatic transmission of anything to anyone, and irreversible file
actions without human approval. Open WebUI remains the owner's workbench: the platform must stay fully
functional without it. Open WebUI Computer never runs on a practice machine.

Frozen decisions that a future stage does not silently reopen: `docs/DECISIONS.md`.
