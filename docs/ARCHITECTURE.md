# Architecture

Merges the earlier French notes on the target architecture, agent runtimes and client bridges.

## Overview

```text
Usual software (Word, document management, Explorer, VS Code / Continue, ...)
  └─ adapters (pane, add-in, OpenAI-compatible configuration)
Assistant Cabinet AI (native window: work folder + plan + chat + local index)
Open WebUI (owner workbench, no sensitive files)
           │
           │  LAN + TLS
           ▼
Gateway — apps/server (Python, FastAPI, Pydantic)
  ├─ Auth (per-person keys, later LDAP / Entra)
  ├─ Model aliases, queue, quotas
  ├─ no-store (never writes excerpts or prompts)
  ├─ Tool catalogue and plan validation
  └─ Request register (metadata only)
           │
           ▼
Ollama (on the server's 127.0.0.1)  ←  Hugging Face / Ollama library weights
```

Ollama and Open WebUI are an inference substrate and a UI. They run in **Docker Compose**, the same
definition on the development PC and on the Mac mini. The **product** is the gateway, the native client and
the approved plans. Business retrieval lives on the workstation, not on the server.
See `docs/OPERATIONS.md`, `docs/RETRIEVAL.md`, `docs/CLIENT.md`.

## Layers

| Layer | Responsibility | Starting technology |
| --- | --- | --- |
| Environment | Pinned versions, Windows to Mac portability | Docker Compose (`docs/OPERATIONS.md`) |
| Inference | Chat, embeddings without retention | Ollama behind the gateway |
| Gateway | Auth, aliases, quotas, no-store, metadata audit, `AIProvider` | `apps/server`: Python, FastAPI, Pydantic |
| Admin UI | Accounts, demos, **non-sensitive** corpora | Open WebUI (owner workbench, frozen for v0) |
| Practice client | Chat, local index, plans, history | `apps/desktop`: Tauri 2, React, TypeScript |
| Retrieval | Full-text index plus vectors **on the client** | SQLite in the Rust core (`docs/RETRIEVAL.md`) |
| Extraction / OCR | Native PDF on the workstation; scans local **or** as an ephemeral job | Out of v0 for scans; `/v1/jobs` contract |
| File tools | list / extract / propose / apply | JSON catalogue plus an MCP façade |
| Security | VLAN, TLS, scopes, disk encryption | `docs/PRIVACY-AND-SECURITY.md` |

## Why a dedicated client and not only a chat

Chat matters, but it must not become the practice software. The client offers a pane over the folder
already open — drag and drop, plan, approval — **and** a chat. Other software connects through the same
gateway. Open WebUI is the owner's workbench, never the doctor's screen.

## Contracts to freeze early

1. OpenAI-compatible API (`/v1`) for every client.
2. A versioned tool catalogue (`tools/v1`).
3. An action-plan format (dry run, hashes, reason, revocable id).
4. Metadata audit (actor, timestamp, hashes, decision) — never the confidential content.
5. A no-store policy plus a path allow-list.
6. Model aliases (`cabinet-chat`, `cabinet-rapide`, ...): allow-list only, licence record mandatory. No open
   model store, no clinical-care weights in the pilot. See `docs/MODELS.md`.
7. One language variable, `locale`, owned by the client. See `docs/LANGUAGE-AND-LOCALE.md`.

## Runtimes are adapters, never the home of business logic

Tooling (Open WebUI Tools, MCP, Hermes, OpenClaw, whatever comes next) will change faster than the need. We
anticipate by owning the contracts, not by betting on a harness.

| Family | Examples | Value | Risk | Posture |
| --- | --- | --- | --- | --- |
| OpenAI-compatible API | Ollama, llama.cpp, vLLM | Inference portability | Dialect differences on tools and vision | **Primary port from v1** |
| MCP | File and IDE servers | Tool standard | Young spec, uneven servers | **Tool port** once the catalogue exists |
| Self-hosted UI | Open WebUI | Admin, demos | Persistence by default, UI lock-in | UI adapter, **not** the product |
| All-in-one agents | Hermes, OpenClaw | Memory, skills, fast demos | Broad surface, sometimes too many disk rights | Optional proof of concept **behind** our tools |
| Editor plugins | Continue, Cline, Aider | Technical profiles | Third-party telemetry, wide scope | External clients, `chat` scope only |

Design rules that follow:

- **Hexagonal.** The domain (plan, approval, audit, policy) imports no vendor SDK.
- **Two stable ports.** LLM is OpenAI-compatible; tools are our schemas, and MCP is only a façade over them.
- **A plan is our own JSON.** If a runtime disappears, the manifest still applies.
- Runtime feature flags, so a harness can be tried without exposing it to the pilot.
- A runtime that cannot honour no-store is out of production.
- Before adding an "agent" dependency, ask whether it is an adapter or whether the business logic will end
  up living inside it. If the second, refuse.

Scenarios this survives: Ollama changes licence or API (point the gateway at llama.cpp or vLLM, same
aliases); Open WebUI breaks its Python tools (the client and MCP remain); MCP becomes the single standard
(expose the same catalogue, rewrite nothing); a much better model appears (change the `cabinet-chat` alias,
not the clients).

## Bridges to the usual software

Every client targets the same OpenAI-compatible base URL with a per-person key and an **alias** as the model
name, never a raw weight name.

```json
{ "provider": "openai", "model": "cabinet-chat",
  "apiBase": "https://ia.cabinet.local/v1", "apiKey": "sk-user" }
```

| Wave | Surface | Role |
| --- | --- | --- |
| v1 | Gateway plus the native window | Single contract, no browser for the practice |
| v1 | Open WebUI through the gateway | Admin and test bench, no sensitive knowledge |
| v2 | Explorer context menu, Word add-in | "Send to the assistant" without leaving the file |
| v2 | Connection sheet for Continue / Cline / Aider | Technical profiles |
| later | Outlook and document-management connectors | A connector is an adapter, not a new backend |
| reserved | `/v1/audio/*` and `/v1/jobs` | Local voice and LAN mobile |

Specific risks when connecting an IDE plugin: many extensions phone home, they send the content of open
files, and some resend the whole history every turn. Therefore: an allow-list of permitted tools, `chat`
scope only, never `apply` or `move` exposed to plugins, and context caps on the gateway.

Business code never addresses `http://ollama:11434`. It addresses the gateway.

## Isolation

There is no server-side index to partition, because the server does not hold the files. Partitioning is
Windows accounts plus local collections plus API keys. A network share is indexed **on the workstation that
already has access to it**.

## Work folder

Disk writes happen **only** inside a chosen work folder (allow-list), like a project root. Not the whole
Windows profile, not the whole Documents folder, and never Open WebUI Computer. Excerpts go to the server,
the plan is approved, deletions go to a dedicated trash folder. See `docs/CLIENT.md`.

## Portability

`LLM_BASE_URL`, aliases and paths are configuration. Development uses the PC with a small model; production
is the Mac mini's URL with more capable models. Sizing: `docs/HARDWARE.md`.
