# Agent instructions

This file is public. It is written in English.

Local AI for professional practices. Pilot: French GP **admin** (not clinical care). Inference stays local. Business retrieval stays on the workstation. The runtime is **Docker Compose** from the first prototype.

Owner-facing notes in `docs/` are French only (`docs/LANGUE.md`). Session handoff files (`docs/SESSION-*.md`) are **English**. Do not write Franglais. Model instructions live in `prompts/` (English in, French out). Open WebUI is an **English** workbench.

## Before any change

1. Read `CONTRIBUTING.md`, `docs/LANGUE.md`, `docs/DOCKER.md`, `docs/PROTOTYPE-PLAN.md`, `docs/PILOT-GP-FRANCE.md`, `docs/CONFIDENTIALITE-ET-SECURITE.md`. Versioned prompt bodies: `prompts/`. Open WebUI stays English (historical FR notes: `docs/TRADUCTION-OPEN-WEBUI.md`). Local stack data lives in `data/` (not git); backup notes: `docs/SAUVEGARDES-LOCALES.md`.
2. No cloud LLM, no telemetry, no real patient files, no Open WebUI Knowledge for medical documents.
3. File actions = plan + human approve. No destructive writes.
4. Do not install Open WebUI Computer (`cptr`) on a practice machine.
5. Official environment = Compose (`ollama` + `open-webui` on `127.0.0.1`). Not a native-only install.

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
- Custom `/v1` gateway before `docker compose up` is the documented path.
