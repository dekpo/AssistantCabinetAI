# Local chat (Docker Compose)

This is the official way to run the prototype: Ollama + Open WebUI on localhost. Do not install Open WebUI Computer on a practice machine. Do not paste real patient files into Knowledge.

## Requirements

- Docker Desktop (Windows or macOS)
- This repository, plus a local `.env` copied from `.env.example`

## First start

1. Start Docker Desktop and wait until it is running.
2. From the repository root:

```text
copy .env.example .env
```

On macOS or Linux use `cp .env.example .env`. Edit `WEBUI_SECRET_KEY` in `.env` to a long random string.

3. Start the stack:

```text
docker compose up -d
```

4. Pull the first model (see `models/LICENSES.md`):

```text
docker compose exec ollama ollama pull mistral
```

5. Open [http://127.0.0.1:3000](http://127.0.0.1:3000). Create the first account (that user is the admin). Then set `ENABLE_SIGNUP=false` in `.env` and run `docker compose up -d` again.

## What is kept where

| Kind | Where | Survives `up` / `down` / `down -v` | New machine |
| --- | --- | --- | --- |
| Flags (follow-ups, tags, titles, locale) | `compose.yaml` and `.env` | Yes | Same files |
| Admin account, chats, UI prompts | `data/open-webui/` (host folder, not git) | Yes, even with `-v`. Lost if you delete `data/` | Recreate from `docs` prompts |
| Model weights | `data/ollama/` (host folder, not git) | Same | Pull again |

`ENABLE_PERSISTENT_CONFIG=false` means Compose wins after a restart. If a setting must stick and travel, put it in `compose.yaml`.

Snapshot this machine (account + chats + models):

```text
.\scripts\backup-local-data.ps1
```

Restore:

```text
.\scripts\restore-local-data.ps1 backups\<folder-name>
```

## Rules for this prototype

- Chat only. Do not upload case files into Open WebUI Knowledge.
- Use the fictional files under `fixtures/gp-sandbox/` only.
- After the first admin exists, keep sign-up closed.
- Ollama is published on `127.0.0.1` only. Do not publish it on the LAN.

## Stop

```text
docker compose down
```

Host folder `data/` is not removed. `docker compose down -v` also leaves `data/` in place. Do not delete `data/` unless you intend to wipe the local account and models.
