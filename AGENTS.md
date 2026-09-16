# Agent instructions

This file is public. It is written in English.

Local AI for professional practices. Pilot: French GP **admin** (not clinical care). Inference stays local. Business retrieval stays on the workstation. The runtime is **Docker Compose** from the first prototype.

Owner-facing notes in `docs/` are French only (`docs/LANGUE.md`). Session handoff files (`docs/SESSION-*.md`) are **English**. Do not write Franglais. Model instructions live in `prompts/` (English in, French out). Open WebUI is an **English** workbench.

## Current objective — v0 prototype

One goal: a reliable installable prototype on the pilot GP's workstation, with the AI server (Ollama behind the gateway) on a Mac mini **at her practice**. Two milestones: **30 September 2026** the chain holds end to end on fictional fixtures; **14 October 2026** she uses it on her own documents, with a Windows installer. Given a choice between one more impressive feature and making the existing flow more reliable, private, testable and replaceable, choose the latter. Do not rewrite what works.

```text
apps/desktop/            Tauri 2 + React + TypeScript. UI in TypeScript, never in Rust.
apps/desktop/src-tauri/  Rust: work folder, extraction, local index, applying approved plans.
apps/server/             Python + FastAPI + Pydantic. OpenAI-compatible /v1 plus product endpoints.
```

Tooling: `pnpm` (client), `uv` (server). Tests: `vitest`, `cargo test`, `pytest`. The server URL, the model and the work folder are configuration, never constants.

Boundaries: the webview never calls the server directly, it goes through Tauri commands, so the path allow-list, the context cap and no-store live in code rather than in a prompt. Business code imports no Ollama or Open WebUI client: `AIProvider` (`generate`, `embed`, `rerank`), the index and the embedder sit behind replaceable interfaces. The server writes no document text anywhere: no file, no database, no log, no debug trace.

Weekly, not at the end: tests for retrieval, source attribution, refusal to answer beyond the sources, and server data isolation. When the documents do not carry the answer, the product says so instead of generating one.

## Before any change

1. Read `CONTRIBUTING.md`, `docs/LANGUE.md`, `docs/DOCKER.md`, `docs/PROTOTYPE-PLAN.md`, `docs/PILOT-GP-FRANCE.md`, `docs/CONFIDENTIALITE-ET-SECURITE.md`. Versioned prompt bodies: `prompts/`. Open WebUI stays English (historical FR notes: `docs/TRADUCTION-OPEN-WEBUI.md`). Local stack data lives in `data/` (not git); backup notes: `docs/SAUVEGARDES-LOCALES.md`.
2. No cloud LLM, no telemetry, no real patient files, no Open WebUI Knowledge for medical documents.
3. File actions = plan + human approve. No destructive writes. Never transmit, delete, move or modify a professional document without explicit confirmation. "Export" means producing a validated artefact in the work folder or on the clipboard; the GP sends it herself from her own software.
4. Do not install Open WebUI Computer (`cptr`) on a practice machine.
5. Official environment = Compose (`server` + `ollama` + `open-webui`, published on `127.0.0.1`). Not a native-only install.
6. Real documents only after a written DPIA draft, a named data controller, and disk encryption on both machines. Until then: `fixtures/gp-sandbox/`.

## Git

Owner-only. Never commit or push. Suggest English branch names, Conventional Commit messages, and **full copy-paste git commands**. See `CONTRIBUTING.md`.

**Do not version internal notes.** Never stage `docs/` except `docs/user/`. Cadrage, audits, interview files, and `docs/SESSION-*.md` handoff files stay local.

When the owner asks for a prompt for **another Cursor tab**: write `docs/SESSION-<topic>.md` (**English**, not git). In chat, give the **path** and the **tab name** (the feature name, not “Prompt …”). Do not paste a long handoff as the only deliverable. She drags that file into the new chat.

## Language

- Instructions **to** the model (`prompts/`): English. Visible model **output** and proposed file names: French for the pilot (later: the firm’s working language).
- Open WebUI UI: English (owner workbench). Product UI (Assistant Cabinet AI): French first.
- With the owner and in `docs/` framing (except `docs/user/` and `SESSION-*`): French, one language per file.
- Code, comments, README, CONTRIBUTING, this file, commits, branches, `docs/SESSION-*.md`: English.

## Out of scope for now

- Diagnosis, prescriptions, FSE / Vitale, DMP / MSSanté send.
- Ameli professional account (sick leave, occupational disease, work accident).
- Accounting module (e-invoicing and the accountant’s software stay theirs).
- Native mobile app, cloud speech.
- Frozen for the v0 sprint: Open WebUI development, voice, vision, scan OCR (contract only), certificates, referral letters, mobile, app stores, RBAC, billing, analytics, large model catalogues, commercial packaging.
