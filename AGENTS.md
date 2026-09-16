# Agent instructions

Written in English, like the whole repository.

Local AI for professional practices. Pilot: French GP **admin** (not clinical care). Inference stays local. Business retrieval stays on the workstation. The runtime is **Docker Compose** from the first prototype.

**Everything written for a developer, an agent or a model is English:** code, tests, commits, `prompts/`, and all of `docs/`. **Everything a user reads is in their own language**, French for the pilot. That split is the product's language contract, not a preference — see `docs/LANGUAGE-AND-LOCALE.md`. Never write Franglais, and never mix two languages in one file.

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

1. Read `CONTRIBUTING.md`, `docs/BRIEF-V0-PROTOTYPE.md`, `docs/ROADMAP.md`, `docs/ARCHITECTURE.md`, `docs/DECISIONS.md`, `docs/LANGUAGE-AND-LOCALE.md`, `docs/PRIVACY-AND-SECURITY.md`. Then, as relevant: `docs/RETRIEVAL.md`, `docs/CLIENT.md`, `docs/PILOT-GP.md`, `docs/MODELS.md`, `docs/OPERATIONS.md`, `docs/HARDWARE.md`. Check `docs/DECISIONS.md` before proposing something that may already be settled. Versioned prompt bodies: `prompts/`. Local stack data lives in `data/` (not git).
2. No cloud LLM, no telemetry, no real patient files, no Open WebUI Knowledge for medical documents.
3. File actions = plan + human approve. No destructive writes. Never transmit, delete, move or modify a professional document without explicit confirmation. "Export" means producing a validated artefact in the work folder or on the clipboard; the GP sends it herself from her own software.
4. Do not install Open WebUI Computer (`cptr`) on a practice machine.
5. Official environment = Compose (`server` + `ollama` + `open-webui`, published on `127.0.0.1`). Not a native-only install.
6. Real documents only after a written DPIA draft, a named data controller, and disk encryption on both machines. Until then: `fixtures/gp-sandbox/`.

## Git

Owner-only. Never commit or push. Suggest English branch names, Conventional Commit messages, and **full copy-paste git commands**. See `CONTRIBUTING.md`.

`docs/` **is** the versioned English specification: stage it like any other source. Two exceptions stay local: `docs/private/` (personal context about the pilot, purchase logistics, commercial notes) and `docs/SESSION-*.md` (tab handoffs).

When the owner asks for a prompt for **another Cursor tab**: write `docs/SESSION-<topic>.md` (not git). In chat, give the **path** and the **tab name** (the feature name, not “Prompt …”). Do not paste a long handoff as the only deliverable. She drags that file into the new chat.

## Language

Full design: `docs/LANGUAGE-AND-LOCALE.md`. In short:

- One variable, `locale`, owned by the client. It drives interface strings, error text, and the output-language directive appended to the English system prompt.
- English: `prompts/` bodies and keys, all of `docs/`, code, comments, tests, commits, branches, README, CONTRIBUTING, this file, machine codes, JSON fields, audit fields. Open WebUI stays an English owner workbench.
- The user's language: everything a human reads in Assistant Cabinet AI — interface, model answers, proposed file names, error messages, disclaimers. French for the pilot.
- Rust and Python return **machine codes**, never user-facing prose. The UI localises them. A French string literal in `apps/server` or `apps/desktop/src-tauri` is a bug.

## Out of scope for now

- Diagnosis, prescriptions, FSE / Vitale, DMP / MSSanté send.
- Ameli professional account (sick leave, occupational disease, work accident).
- Accounting module (e-invoicing and the accountant’s software stay theirs).
- Native mobile app, cloud speech.
- Frozen for the v0 sprint: Open WebUI development, voice, vision, scan OCR (contract only), certificates, referral letters, mobile, app stores, RBAC, billing, analytics, large model catalogues, commercial packaging.
