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

Indexing needs a second model, pulled once and never asked to answer anything:

```powershell
docker compose exec ollama ollama pull nomic-embed-text
```

It is served under its own alias, `cabinet-embed`, from the same `MODEL_ALIASES` line, and
`DEFAULT_EMBEDDING_ALIAS` says which alias `/v1/embeddings` uses when a client does not name one.
Replacing it is one edit and a restart, **plus a re-index of every work folder**: vectors built by two
different models cannot be compared. `MAX_EMBEDDING_INPUTS` and `MAX_EMBEDDING_CHARS` bound one batch.

`MAX_OUTPUT_TOKENS` (default 2048, about 6 500 French characters) bounds the other direction: how long
an answer may be, whether or not the caller asked for a limit. It is a safety property rather than a
tuning knob — `LLM_REQUEST_TIMEOUT_SECONDS` is the longest allowed gap **between** chunks, so it cannot
end a model that loops steadily, and before this cap existed one wrote the same block for three and a
half minutes (`docs/TROUBLESHOOTING.md`, 22 September 2026). A related piece of hygiene: an alias in
`MODEL_ALIASES` is a promise that the model works, so weights under about 1B do not belong there
however fast they are.

On a CPU-only server the first request after switching models can stay silent for more than the default
180 s of `LLM_REQUEST_TIMEOUT_SECONDS` (Ollama unloads one model, loads the other, then reads the prompt before
the first token). The gateway then answers `provider_error` with `reason: "timeout"`, which the desktop shows as
"the model took too long to start, try again" (`docs/test-reports/human-acceptance-pass-1/`, BUG-11). The
second attempt normally succeeds because the model is loaded. Raise the value on slow machines (for example
600): add `LLM_REQUEST_TIMEOUT_SECONDS=600` to the `.env` file next to `compose.yaml` and run
`docker compose up -d server` (until 4 October 2026 `compose.yaml` did not forward this variable, so setting it
in `.env` changed nothing; `.env.example` lists it now). To check that the desktop explains a timeout, set it to
`5` the same way, ask any question, then put the value back; the desktop's own `answerIdleTimeoutSeconds` is a separate, client-side
setting and does not govern this one.

### Watching an Analyse

When an analysis is slow, stalls or fails, watch both containers while it runs (repository root, before pressing Analyse; the
logs only go back to the creation of the container):

```powershell
docker compose ps
docker compose logs -f --tail=0 server     # one terminal: the gateway, one line per embeddings request
docker compose logs -f --tail=0 ollama     # a second one: the runtime
docker stats --no-stream assistant-cabinet-ollama    # CPU while a request runs
```

(bash: the same.) An `embedding` line looks like this, with no text in it:

```text
embedding {"duration_ms": 9664, "input_chars": 14884, "input_count": 16, "inputs_sha256": "86b0...", "model_alias": "assistant-embed",
           "outcome": "completed", "prompt_tokens": ..., "request_id": "embd-...", "vector_count": 16, ...}
```

- `input_count` <= 16 and `input_chars` <= 20 000 on every line; their sum over a pass is the number of chunks of the files read.
  One line with a large `input_count` means the client is an older build.
- `duration_ms` is what to compare with the client's 120 s deadline (240 s for the first request of a pass). On the development PC a
  16-chunk request takes 8 to 17 s. **Time one on the Mac mini before relying on those numbers:**

  ```powershell
  # from the repository root, with the stack up; sends 16 invented passages, never a document
  python -c "import json,urllib.request as u;b=json.dumps({'model':'assistant-embed','input':['Le comite relit le calendrier des reunions de la semaine. '*20]*16}).encode();import time;t=time.time();u.urlopen(u.Request('http://127.0.0.1:8080/v1/embeddings',b,{'Content-Type':'application/json'}),timeout=300).read();print(round(time.time()-t,1),'s')"
  ```

- `outcome` is `completed`, or the gateway's error code: `provider_error` (with a `reason` in the response, not in the log),
  `provider_unreachable` when the Ollama container is down or was cut during the request. Two consecutive failing lines for the
  same `input_count` are the attempt and its one retry.
- No `embedding` line at all after pressing Analyse: the pass did not reach the gateway, or nothing needed embedding. Look at the
  summary in the app: unchanged files are skipped without a request, a scan is read by OCR on the workstation first (up to 20 s
  a page), and a gateway that is off is `server_unreachable` in the sidebar.
- A client that gave up does not stop the gateway: a request that ran past the client's deadline still ends with `completed` in the
  log, a few seconds after the sidebar reported a failure. That mismatch is how the 7 October 2026 case showed itself.
- In `ollama`: `[GIN] ... | 200 | 9.6s | POST "/api/embed"` is one batch; `starting llama-server` just before the first one is the
  model loading, and should appear once per pass, not before every batch (`OLLAMA_MAX_LOADED_MODELS=1` evicts it whenever a chat
  model is used in between).
- To check that no text reached the logs, search them for a distinctive sentence of the document:
  `docker compose logs server | grep -c "<sentence>"` must print `0`.

`DEFAULT_CONTEXT_WINDOW` (default 8192 tokens) and `MODEL_CONTEXT_WINDOWS` (`alias=tokens` pairs, for the
aliases that need another value) set how much a model may read in one request - the conversation's memory
included. The gateway passes it on every request (`num_ctx`), caps it at what the model itself supports, and
publishes each chat alias's window in `/health`, where the desktop app reads it to decide how many past
exchanges fit (`docs/SELECTION-AND-MEMORY.md`). Before 27 September 2026 it was left to Ollama's default, which
silently dropped the oldest messages. A larger window is slower and needs more memory, so the value is measured
on the server device rather than guessed:

```powershell
cd apps/server
uv run python scripts/measure_context.py --url http://127.0.0.1:8080 --alias cabinet-chat
```

It sends synthetic conversations only - never a document - with 0, 1, 2 and 4 remembered exchanges, and prints
the time to answer and the prompt size the runtime reports.

Open WebUI now goes through the gateway (`OPENAI_API_BASE_URL`), with `ENABLE_OLLAMA_API=false`, so
the workbench sees the same alias catalogue as the practice window. The gateway does not check API
keys yet; per-person keys are sprint 4.

## Diagnosing the tabular hidden interpreter against a real model

Session 14 (`docs/DECISIONS.md`, "the hidden interpreter session" and its manual validation pass
entries) added a second gateway-facing script, alongside `measure_context.py`:

```powershell
cd apps/server
uv run python scripts/probe_query_plan.py --url http://127.0.0.1:8080 --alias gemma2:2b
```

Sends the **exact** instruction and schema `tabular::query_plan::build_schema_message` builds
(copied verbatim into the script, over a small fictional fixture it also carries) to a real model,
non-streaming, and prints the raw reply - no document, no real workbook, nothing from `fixtures/`.
`--question <key>` runs one case instead of all six (`--help` lists them); the reply is exactly
what `tabular::query_plan::parse_response`/`resolve` would be given, so a reply that looks wrong
here is the same one the product would have received.

**Use this whenever a tabular question that should reach the model-assisted path instead nudges,
or computes a number that looks wrong, and a real gateway is reachable.** It answers the question
"did the model write a bad plan, or did Rust misread a good one" directly, without needing to
reproduce the conversation in the app first. Reading its output against
`tabular::query_plan::QueryPlan`'s own fields (`src/tabular/query_plan.rs`) usually shows which:
a reply that is not valid JSON, or whose shape does not match the plan at all, is the model;
a reply that looks like a sensible plan but still nudged is more likely Rust's - and every
`query_plan.rs` test named `a_real_<model>_reply_...` started from exactly this script's output,
captured verbatim, kept as a permanent regression test once the gap it found was fixed. Follow
that pattern: a new finding from this script is worth its own such test, not only a fix.

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

**The step-by-step runbook for a separate server device** (recorded as pending on 27 September 2026,
written on 1 October 2026 ahead of Sprint 4) is `docs/DEPLOYMENT.md`. Today the gateway, Ollama and
Open WebUI still run on the same machine as the desktop app in development; that document covers
the target shape - a **server device** (the Mac mini first, other hardware later for other
cabinets) separate from each **user device** (the desktop app, its index and OCR) - the beta
rollout checklist, and what "no inference port exposed on the LAN" becomes once the gateway alone
is published on the practice network, behind a firewall rule and a per-person access key
(Sprint 4, still to build). Tracked in `docs/ROADMAP.md`, Sprint 4.

## Where the desktop program keeps its own files

Outside the documents and data folders, in the application's local data folder (`%LOCALAPPDATA%\com.assistantcabinetai.desktop\` on
Windows, `~/Library/Application Support/com.assistantcabinetai.desktop/` on macOS; it is a hidden folder on Windows): the local index
(`index.sqlite3`, `workbooks.sqlite3`, `tabular.sqlite3`), the log of file renames (`renamed-files.jsonl`, original and new name of every
file the clean-names pass renamed) and the log of generated letters (`generated-files.jsonl`: time, template, data file, key and output name
of every letter written; never a cell value). With `writeTimingLog` switched on in `settings.json` (off by default), also
`retrieval-timings.jsonl` (one line per question) and `analysis-timings.jsonl` (one line per Analyse pass): a timestamp, milliseconds, counts
and machine codes, never a question, a file name or a passage; delete them when a measurement is done. `settings.json` is in the configuration folder instead (`%APPDATA%\com.assistantcabinetai.desktop\` on
Windows). None of them holds the text of a document.
