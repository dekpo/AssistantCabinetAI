# Operations — local stack, Open WebUI flags, backups

The official environment is **Docker Compose**, from the first prototype. `compose.yaml` is the recipe —
"start the engine (Ollama) and the owner workbench (Open WebUI) together" — and `docker compose up` is its
start button. The prototype, and later the practice, must come up **the same way** on the Windows
workstation and on the Mac mini, so the unit of portability is that versioned file, not a hand-made Ollama
install. "When Compose answers" means the Open WebUI chat opens and a fictional sentence gets a reply; it is
not a separate piece of software named Compose.

Step-by-step walkthrough for the owner: `docs/user/local-compose.md`. Dated plan: `docs/ROADMAP.md`.
Data rules: `docs/PRIVACY-AND-SECURITY.md`. Layers: `docs/ARCHITECTURE.md`.

## Decisions

- Compose is the environment baseline from the first prototype.
- Services today: `server` + `ollama` + `open-webui`, internal network, ports published on `127.0.0.1`
  only.
- The gateway (`apps/server`, `/v1`) is **in the same** Compose file since 16 September 2026, not a
  parallel "Windows only" script.
- Host folders under `data/` (account, chats, prompts, model weights) live on the project disk, outside
  git. `docker compose down -v` does not erase them.
- Open WebUI settings live **in `compose.yaml`**, with `ENABLE_PERSISTENT_CONFIG=false`.
- Same repository, same files: `docker compose up` on Windows (Docker Desktop), then on macOS. The volume
  is never copied from one machine to the other.
- Open WebUI Computer is not installed in this Compose.

## The gateway service

Built from `apps/server`, published on `127.0.0.1:${SERVER_HOST_PORT:-8080}`. It has **no volume**:
the gateway writes nothing to disk, so giving it one would be giving it a reason to. Ollama is
reachable from it on the Compose network and from nowhere else.

```powershell
docker compose up -d --build server
curl.exe -s http://127.0.0.1:8080/health
```

`status` is `ok` when the runtime answers and at least one alias is configured; otherwise it is
`degraded` and `issues` names the machine codes. Aliases come from `MODEL_ALIASES`, written as
`alias=model` pairs because Compose cannot interpolate a default containing braces. Changing which
model answers is a change to that line and a restart, never a change to a client.

Open WebUI now goes through the gateway (`OPENAI_API_BASE_URL`), with `ENABLE_OLLAMA_API=false`, so
the workbench sees the same alias catalogue as the practice window. The gateway does not check API
keys yet; per-person keys are sprint 4.

## Why not a native-only install

An Ollama Windows install plus Open WebUI via `pip` works for a demo, but it does not port cleanly to the
Mac mini and it drifts per machine. Docker pins image versions and the network wiring. Temporary
exception: if Docker Desktop is not there yet, give the Docker install commands **then** the Compose. No
"definitive native stack".

## Constraints

- No inference port exposed on the LAN during the prototype.
- No cloud service in the Compose file (no OpenAI key "just in case").
- `.env.example` is English (public, in the repository).
- Service and image names stay the publishers' own.

## Open WebUI flags

**The source of truth is the repository files**, not clicks in the Administration page. Agents read this
file before touching Compose.

Working rule:

1. A setting to **keep and reproduce** goes to `compose.yaml` (or `.env.example`).
2. Administration is for **discovery**. Carry the result back into Compose.
3. `ENABLE_PERSISTENT_CONFIG=false`: after a restart, **Compose wins**.
4. Business instructions: paste the **Body** sections of `prompts/*.md`. Recreate them on a new machine.
5. Back up `data/` with `.\scripts\backup-local-data.ps1` → `backups/`.

### Decisions, 15–16 September 2026

| Setting | File | Why |
| --- | --- | --- |
| Follow-up questions **off** | `compose.yaml` | They invent the clinical next step. |
| Automatic tags **off** | `compose.yaml` | They come out in English, off-topic. |
| Chat titles in **French** | `compose.yaml` | Readable output: avoid "Holter Test Proposed". The title instruction itself stays English. |
| Interface locale `en-US` | `compose.yaml` | English owner workbench. The `fr-FR` overlay is **suspended**. |
| Starter suggestions | `compose.yaml` | **English** chips (Letter summary / Filing plan) that require a **French** answer. An empty list brings back the product's generic chips, so it is never left empty. |
| Sign-up | `.env` → `ENABLE_SIGNUP=false` | After the first account exists. |
| Citations without duplicates | `prompts/gp-letter-summary.md` | Not an Open WebUI button. |
| Filing plan = **table**, never tools | `prompts/gp-inbox-classify.md` | On 15 September 2026 the model invented `delete_calendar_event` / `rename_file` / `move_file`. Next trial: **English** body, tools **off** in that chat. A Compose flag only once the exact name is known. |

## Where host data lives

Settings (follow-ups, locale, titles) live in `compose.yaml`; they are **not** in `data/`. The work done in
Open WebUI (account, conversations, pasted instructions, model weights) lives in `data/` at the project
root. Not git. No network upload.

| Action | Account, chats, instructions, weights (`data/`) | Settings (Compose) |
| --- | --- | --- |
| Close the browser | Kept | Kept |
| `docker compose up -d` / `down` | Kept | Re-read from `compose.yaml` / `.env` |
| `docker compose down -v` | **Kept** (`data/` is not a named volume) | Same |
| Delete the `data/` folder by hand | **Lost** | Kept |
| Other workstation / Mac mini | Do not copy `data/` from the test PC | **The same**, through Compose |

Compose mounts **host folders** (`data/ollama`, `data/open-webui`) plus some invisible Docker volumes.
`down -v` removes only old named volumes. What can still destroy everything: deleting `data/` by hand, then
emptying the Windows recycle bin.

## Backup and restore

When a trial worked, from the project root:

```powershell
.\scripts\backup-local-data.ps1
```

That creates `backups/<date-time>/`, also outside git. To roll back:

```powershell
.\scripts\restore-local-data.ps1 backups\2026-09-15-160000
```

Replace the folder name with the real one. The script stops Compose while copying, then starts it again.

In addition, let Windows back up the project folder (file history) — **without** putting real patient
folders in it.

## A new machine (Mac mini, later)

Same repository, same `compose.yaml`, a new `.env`. Run `docker compose up -d`, pull the model, create the
account, recreate the instructions from `prompts/`. Do **not** carry the Windows `data/` over: it is a test
account, not a recipe.
