# Agent instructions

This file is public. It is written in English.

Local AI for professional practices. Pilot: French GP **admin** (not clinical care). Inference stays local. Business retrieval stays on the workstation. The runtime is **Docker Compose** from the first prototype.

Owner-facing notes in `docs/` are French only (`docs/LANGUE.md`). Do not write Franglais.

## Before any change

1. Read `CONTRIBUTING.md`, `docs/LANGUE.md`, `docs/DOCKER.md`, `docs/PROTOTYPE-PLAN.md`, `docs/PILOT-GP-FRANCE.md`, `docs/CONFIDENTIALITE-ET-SECURITE.md`. Open WebUI UI wording: `docs/TRADUCTION-OPEN-WEBUI.md`. Local stack data lives in `data/` (not git); backup notes: `docs/SAUVEGARDES-LOCALES.md`.
2. No cloud LLM, no telemetry, no real patient files, no Open WebUI Knowledge for medical documents.
3. File actions = plan + human approve. No destructive writes.
4. Do not install Open WebUI Computer (`cptr`) on a practice machine.
5. Official environment = Compose (`ollama` + `open-webui` on `127.0.0.1`). Not a native-only install.

## Git

Owner-only. Never commit or push. Suggest English branch names, Conventional Commit messages, and **full copy-paste git commands**. See `CONTRIBUTING.md`.

**Do not version internal notes.** Never stage `docs/` except `docs/user/`. Cadrage, audits, interview files, and `docs/SESSION-*.md` handoff files stay local.

When the owner asks for a prompt for **another Cursor tab**: write `docs/SESSION-<topic>.md` (French, not git). In chat, give the **path** and the **tab name** (the feature name, not “Prompt …”). Do not paste a long handoff as the only deliverable. She drags that file into the new chat.

## Language

- With the owner and in `docs/` framing: French, one language per file.
- Code, comments, README, CONTRIBUTING, this file, commits, branches: English.

## Out of scope for now

- Diagnosis, prescriptions, FSE / Vitale, DMP / MSSanté send.
- Ameli professional account (sick leave, occupational disease, work accident).
- Accounting module (e-invoicing and the accountant’s software stay theirs).
- Native mobile app, cloud speech.
- Custom `/v1` gateway before `docker compose up` is the documented path.
