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

## Rules for this prototype

- Chat only. Do not upload case files into Open WebUI Knowledge.
- Use the fictional files under `fixtures/gp-sandbox/` only.
- After the first admin exists, keep sign-up closed.
- Ollama is published on `127.0.0.1` only. Do not publish it on the LAN.

## Stop

```text
docker compose down
```

Volumes keep models and the Open WebUI database. To remove them as well: `docker compose down -v` (this deletes local models and the admin account).
