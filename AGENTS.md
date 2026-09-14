# Agent instructions

This file is public. It is written in English.

Local AI for professional practices. Pilot: French GP **admin** (not clinical care). Inference stays local. Business retrieval stays on the workstation. The runtime is **Docker Compose** from the first prototype.

Owner-facing notes in `docs/` are French only (`docs/LANGUE.md`). Do not write Franglais.

## Before any change

1. Read `CONTRIBUTING.md`, `docs/LANGUE.md`, `docs/DOCKER.md`, `docs/PROTOTYPE-PLAN.md`, `docs/PILOT-GP-FRANCE.md`, `docs/CONFIDENTIALITE-ET-SECURITE.md`.
2. No cloud LLM, no telemetry, no real patient files, no Open WebUI Knowledge for medical documents.
3. File actions = plan + human approve. No destructive writes.
4. Do not install Open WebUI Computer (`cptr`) on a practice machine.
5. Official environment = Compose (`ollama` + `open-webui` on `127.0.0.1`). Not a native-only install.

## Git

Owner-only. Never commit or push. Suggest English branch names, Conventional Commit messages, and **full copy-paste git commands**. See `CONTRIBUTING.md`.

**Do not version internal notes.** Never stage `docs/` except `docs/user/`. Cadrage, audits, and interview files stay local.

## Language

- With the owner and in `docs/` framing: French, one language per file.
- Code, comments, README, CONTRIBUTING, this file, commits, branches: English.

## Out of scope for now

- Diagnosis, prescriptions, FSE / Vitale, DMP / MSSanté send.
- Native mobile app, cloud speech.
- Custom `/v1` gateway before `docker compose up` is the documented path.
