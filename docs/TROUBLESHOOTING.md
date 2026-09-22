# Troubleshooting log

One place to check before losing an hour to something another agent already hit. Append an entry every
time a non-obvious bug, footgun or environment quirk costs real time to diagnose — not every error, only
the ones a future agent (or Elise, on another machine) would otherwise rediscover from scratch. Newest
entry at the top. Keep each entry to what the next reader needs: what broke, why, and the fix, in English,
with the exact command or code where it matters.

---

## A small model wrote the same block for three and a half minutes and no timeout stopped it

**Found:** 22 September 2026, trying `qwen2.5:0.5b` (alias `qwen-nano`) as a chat profile fast enough for the
Windows workstation. The answer began in French, then drifted into a loop: a bullet reading
`Document fictif : Fictif - Dossier de spécialistes`, eight sub-bullets reading
`Détails de l'adjudication : N/A - N/A - N/A`, a heading `[Dokumentation] [Début]`, and the same block
again, indefinitely. Nothing on screen could stop it.

The register (metadata only, so this is all it holds) is precise about the scale:

```text
model_alias "qwen-nano"  completion_chars 10836  completion_tokens null  duration_ms 210369  outcome "completed"
model_alias "assistant-turbo"  completion_chars 4178  completion_tokens 1310  duration_ms 110392  outcome "completed"
```

Same question, same prompt hash. Three things combined, and only the third is about the model:

1. **Nothing in the chain caps the length of an answer.** `max_tokens` is optional in
   `apps/server/.../api/schemas.py`, the desktop client never sends it, so `num_predict` is never set in
   `providers/ollama.py` and the runtime generates without a ceiling. The **input** is capped in three
   places — 24 000 chars in `gateway.rs`, 48 000 in the gateway, 6 000 of evidence in `retrieval.rs`. The
   **output** is capped nowhere. Filling the context window does not end it either: Ollama starts
   `llama-server` with `--context-shift`, so the window slides and generation continues.
2. **A read timeout cannot catch a loop.** `LLM_REQUEST_TIMEOUT_SECONDS` becomes
   `httpx.Timeout(180, connect=10)`, which is the longest allowed gap **between chunks**, not a total. A
   model emitting a token every second forever never trips it: the run above lasted 210 s, longer than the
   180 s "timeout", and was recorded as completed. Only `CHAT_TIMEOUT` (300 s, a total, in `gateway.rs`)
   eventually ends it — five minutes of watching garbage accumulate.
3. **0.5B is below the floor for this job.** With 1 400 tokens of French context the model lost the thread,
   lost the language (`Dokumentation` is German), and invented vocabulary absent from every document
   (`adjudication` is procurement). Ollama's defaults (`repeat_penalty` 1.1 over `repeat_last_n` 64) cannot
   break a loop whose period is a ten-line block, far wider than that window. The system prompt already
   says *"You keep answers short and plain"*; a model this size cannot follow it, and no prompt wording
   fixes that.

**Fixed, in two parts.** A stop under the answer being written, which keeps the text as written, so she is
never again watching something she cannot end (`docs/CHAT-UX-ASSESSMENT.md` item 2). And the missing bound:
`MAX_OUTPUT_TOKENS`, default 2048, applied in `capped_output_tokens` in `api/chat.py` whether or not the
caller asked for a limit — a caller may ask for less, never for more, and `-1` (which means "unbounded" to
llama.cpp and to Ollama) is read as asking for nothing. So an unbounded answer is no longer something the
gateway can be talked into. Tests: `apps/server/tests/test_output_cap.py`.

**Still open, smaller:** the register calls this a success. `_chunks` in `api/chat.py` initialises
`outcome = "completed"` and only overwrites it on a `GatewayError`, so an answer cut off by a disconnect —
including every use of the new stop — is logged as completed with `completion_tokens: null`. A capped answer
is also reported with `finish_reason: "stop"` rather than `"length"`, since `GenerationChunk` does not carry
the runtime's reason for stopping. Both are honesty in metadata, neither changes what she sees.

**And keep sub-1B weights out of `MODEL_ALIASES`:** an alias offered in the settings is a promise that it
works.

## `cargo` is "not found" in a Cursor terminal although it is installed and on the PATH

**Found:** 22 September 2026, running `pnpm tauri dev` after a session that had used `cargo test` fine.

```text
failed to run 'cargo metadata' command to get workspace directory:
failed to run command cargo metadata --no-deps --format-version 1: program not found
```

Rust was installed and `C:\Users\<user>\.cargo\bin` was already on the **persisted user PATH**. The stale
environment belongs to the **editor process**, not to the terminal: Cursor was started before rustup added
that entry, so every terminal it spawns inherits the old PATH. Opening a new terminal in the same window
changes nothing, which is what makes this look like a broken installation rather than a stale variable.

**Do not reinstall Rust.** Check first, from any shell:
`[Environment]::GetEnvironmentVariable("Path","User") -split ';' | Select-String cargo`. If the entry is
there, **restart Cursor**. For the current session only, prepend it by hand — `cmd`:
`set PATH=%USERPROFILE%\.cargo\bin;%PATH%`, PowerShell: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"`.
Same trap for any tool installed while the editor was open (`uv`, `pnpm`, `rustup` itself).

## A `tiny_http` streaming stand-in sends nothing for seconds, however often its reader is called

**Found:** 21 September 2026, writing the test that proves a stopped question really stops
(`apps/desktop/src-tauri/tests/chat_cancellation.rs`).

The test streams an endless fake answer and asserts that no further delta arrives after the stop. The
first version failed on the assertion *before* that one — `the stand-in gateway must have been streaming
before the stop` — with zero deltas received, even though the reader had been called around ten times.

`tiny_http::Response::new(..., data, None, None)` streams a reader chunked, but it copies through an
8 KiB buffer and fills it before writing a chunk. A reader returning one 45-byte SSE line per call is
therefore not slow, it is **silent**: about 180 calls, so nearly four seconds at 20 ms each, before the
client sees a single byte. Nothing in the failure points at buffering, and raising the timeouts only makes
the test slower.

**Fix:** the reader fills the whole buffer it was handed, so one call becomes one chunk. It keeps an
offset into the repeating line, because a buffer boundary must not cut an SSE line where the client cannot
read it back. Same shape for any future streaming stand-in: think in chunks the size of the buffer, not in
protocol lines.

## A `MODEL_ALIASES` entry without the exact Ollama tag fails as a generic "the model did not answer correctly"

**Found:** 21 September 2026, adding a `cabinet-turbo` profile for faster local testing.

`.env`'s `MODEL_ALIASES=...,cabinet-turbo=qwen3` pointed at the bare name `qwen3`, which Ollama treats
as an implicit `qwen3:latest`. The pulled weight was actually tagged `qwen3:8b` (`ollama pull qwen3:8b`,
confirmed in `docker compose exec ollama ollama list`). `resolve_model_alias`
(`apps/server/src/assistant_cabinet_server/core/aliases.py`) does not check that the runtime name it
returns actually exists in Ollama — it only checks that the *alias* is in the allow-list — so the
mismatch is invisible at the gateway and only surfaces when Ollama itself can't find the model, which
the client renders as the generic `chat.answerFailed` banner.

**Fix:** the alias's right-hand side must be the **exact** tag from `ollama list` (`qwen3:8b`, not
`qwen3`). After editing `.env`, restart only the server container to pick it up —
`docker compose up -d server`; Ollama and its pulled weights are untouched.

**Same mistake, same fix, 21 September 2026:** `cabinet-super=smollm2` after `ollama pull
smollm2:1.7b` — the pulled tag is `smollm2:1.7b`, not bare `smollm2`. Whenever a new alias is added
to `MODEL_ALIASES`, copy the tag **character-for-character** from `docker compose exec ollama ollama
list`, including the size suffix, and restart only `server` afterwards.

## A renamed alias used to brick the client until someone edited `settings.json` by hand

**Found:** 21 September 2026, renaming every alias from `cabinet-*` to `assistant-*`.

Two names are **hardcoded** — but only as the very first value a fresh install ever sees, not as
something a rename affects: `DEFAULT_MODEL_ALIAS` / `DEFAULT_EMBEDDING_ALIAS` constants in
`apps/desktop/src-tauri/src/settings.rs` seed `settings.json` on the very first launch. After that,
the client only ever reads its **own persisted `modelAlias`/`embeddingAlias`**
(`%APPDATA%\com.assistantcabinetai.desktop\settings.json`) — never `.env`'s `DEFAULT_MODEL_ALIAS`,
which only matters server-side when a request arrives with no alias at all, which the desktop client
never does. So:

- Renaming `cabinet-chat` → `assistant-chat` in `MODEL_ALIASES` left every existing installation
  with a `modelAlias`/`embeddingAlias` that no longer resolves. `embeddingAlias` has **no Settings
  UI** at all, so indexing failed with `model_alias_not_allowed` and the error told the user to "choose
  another one in Settings" — advice with no way to follow it.
- Editing `.env`'s `DEFAULT_MODEL_ALIAS` after the fact changed nothing visible, because the
  client's own stale, already-persisted value took priority over it — the Settings dropdown
  displaying the alphabetically-first option was a rendering artefact of a `<select>` whose `value`
  matched none of its `<option>`s, not a real change of state.

**Fix:** `/health` now also reports `default_model_alias` and `embedding_alias`
(`apps/server/src/assistant_cabinet_server/api/{schemas,health}.py`), and `App.tsx` self-heals: on
every health check, if the persisted `modelAlias` is not in the current `aliases` list, or
`embeddingAlias` no longer matches the server's current one, the client silently adopts the
server's current value and saves it. A future alias rename now fixes itself on the next health
check instead of requiring a manual `settings.json` edit — and it means the embedding model behind
`assistant-embed` (e.g. swapping in `bge-m3` to bench it) can change freely as long as the **alias
name** stays the same; only renaming the alias itself needs this self-heal.

## Renaming the `MODEL_ALIASES` prefix leaves `DEFAULT_MODEL_ALIAS` / `DEFAULT_EMBEDDING_ALIAS` stale

**Found:** 21 September 2026, after renaming every alias from `cabinet-*` to `assistant-*` in
`.env`'s `MODEL_ALIASES`.

`DEFAULT_MODEL_ALIAS` and `DEFAULT_EMBEDDING_ALIAS` (`apps/server/src/assistant_cabinet_server/core/config.py`)
are **separate** settings, read independently of `MODEL_ALIASES`; renaming the aliases inside the map
does not rename the two defaults that point at them by name. `DEFAULT_EMBEDDING_ALIAS` was not even
set in `.env`, so it silently used the code default `cabinet-embed`, which then matched nothing.
Symptom: `/health`'s `aliases` list kept showing the embedding alias (see the entry below for why it
should not), because `Settings.chat_aliases` excludes whatever `default_embedding_alias` says, and
that no longer matched any real alias.

**Fix:** whenever the alias prefix or names change, update `DEFAULT_MODEL_ALIAS` **and**
`DEFAULT_EMBEDDING_ALIAS` in `.env` to match, then `docker compose up -d server`. There is no
validation today that catches a default pointing at a non-existent alias — it just quietly fails to
resolve (`model_alias_not_allowed`) the first time something relies on the default.

## "Généré par cabinet-chat" shown after switching to cabinet-rapide in Settings

**Found:** 20 September 2026, human test after the model-alias `passage_noun` rebuild: switching the
active profile in Settings and re-asking the same question still labelled the answer with the
*previous* alias.

`useChat`'s `send` callback (`apps/desktop/src/state/useChat.ts`) stamps every answer with the
`modelAlias` prop it closes over, but that prop was missing from the `useCallback` dependency array.
React kept the memoised closure from an earlier render — the one capturing the old alias — until some
*other* dependency (`hasWorkFolder`, `pending`, …) changed and forced a new closure to be built. The
actual request sent to the gateway was unaffected (Rust reads the current alias fresh from settings at
call time), so only the on-screen "Généré par …" credit line was wrong, not the model actually used.

**Fix:** add `modelAlias` to the dependency array of the `send` `useCallback`. One line.

## Tesseract's bare `tsv` argument is a config *file* we never bundled, not an output-format flag

**Found:** 20 September 2026, after fixing sidecar discovery, OCR still recognised nothing.

Sidecar discovery (previous entry) was fixed, and the process started, but every real page still
came back as unreadable. Running the exact command by hand showed the actual error, which never
reaches the calling process's stdout/stderr in a way the Rust code surfaces (only a non-zero exit
status):

```text
tesseract - - -l fra --tessdata-dir resources\tessdata tsv
read_params_file: Can't open tsv
```

The trailing `tsv` on a Tesseract command line is not an output-format switch - it is the *name of
a config file* Tesseract looks up at `<tessdata-dir>/configs/tsv` (or `TESSDATA_PREFIX/configs`).
We only bundle `resources/tessdata/fra.traineddata` (`resources/README.md`), never the `configs/`
tree that ships with a full Tesseract install, so the lookup fails and Tesseract exits non-zero
before producing any output - which the caller cannot distinguish from a genuinely unreadable
image.

**Fix:** the config file's entire content is one line, `tessedit_create_tsv 1`. Set that variable
directly on the command line with `-c tessedit_create_tsv=1` instead of the bare `tsv` argument
(`apps/desktop/src-tauri/src/ocr/tesseract.rs`) - same TSV stdout, no extra bundled resource
needed. Verified against the real bundled sidecar with
`cargo test --lib -- --ignored recognises_a_real_bundled_prescription_scan` (same file), which
`scripts/fetch-ocr-resources.ps1` must have staged first.

**Confirmed fixed:** 20 September 2026, human re-test on `fixtures/gp-sandbox/` — scanned files are
recognised, the OCR-sourced answers cite the right page, and the index summary shows 0 unreadable
files where it previously showed 5.

## `tauri dev` does not see Tesseract copies left in `src-tauri/`

**Found:** 20 September 2026, human test of OCR wiring (Sprint 2.5 Day 3).

Indexing reported scanned files as unreadable (`5 illisible(s)`) with no OCR summary line. The
engine had been copied into `apps/desktop/src-tauri/` (`tesseract.exe` plus its DLLs), which is
the crate root, but `tauri dev` runs `target/debug/<app>.exe`. Sidecar discovery only looked at
`resource_dir` and the executable's parent, so the copies were invisible. This is not a Windows
execute-bit / `chmod` problem.

**Fix:** `sidecar_search_roots` in `apps/desktop/src-tauri/src/commands.rs` walks a few ancestors
of the executable (and the current directory) so a copy in `src-tauri/` or `src-tauri/binaries/`
is found during `tauri dev`. `TesseractProvider` also prepends the binary directory and
`resources/tesseract` to `PATH` for that process, because Windows still will not load DLLs from a
folder that is merely nearby. After a rebuild, delete the work folder's `index.sqlite3` and
index again — unchanged files are skipped and would keep the empty extraction.

## `tauri-build`'s build script validates `externalBin`/`resources` on *every* `cargo build`, not only `cargo tauri build`

**Found:** 20 September 2026, wiring the Tesseract sidecar (Sprint 2.5).

Declaring `bundle.externalBin` or `bundle.resources` in `tauri.conf.json` before the referenced files
exist breaks `cargo build` and `cargo test --lib` immediately, with an error like:

```text
resource path `binaries\tesseract-x86_64-pc-windows-msvc.exe` doesn't exist
```

This is not limited to `cargo tauri build` or to bundling — `tauri-build`'s `build.rs` reads
`tauri.conf.json` and checks every declared path on any `cargo` invocation that touches the crate. A
config change that looks purely packaging-related can break plain development and CI.

**Fix / rule:** never commit an `externalBin`/`resources` entry for a file that is not committed or not
guaranteed to exist on every machine that will run `cargo build`/`cargo test` (including CI). If the
files are fetched by a script rather than committed (see the next entry), wire the config in the same
change that also updates CI to run that fetch script first — never before. See
`apps/desktop/src-tauri/binaries/README.md` for the concrete example.

## Model weights and bundled third-party engine binaries do not go in git, even as "resources"

**Found:** 20 September 2026, same session.

`AGENTS.md`'s git rule forbids committing model weights, and `.gitignore` already excludes
`models/*.gguf` for that reason. It is tempting to treat a bundled OCR engine's traineddata, DLLs and
shared library as ordinary "resources" that ship with the app and therefore belong in the repo next to
`tauri.conf.json`. They do not: `fra.traineddata` is a model weight in the same sense, and the engine
binaries are the same class of large, non-authored, third-party blob. Nearly staged ~150 MB of binaries
into a commit before catching this.

**Fix / rule:** anything fetched from a third party rather than authored in this repo — a model weight,
a compiled engine, a prebuilt library — is gitignored and fetched by a script
(`scripts/fetch-ocr-resources.ps1` is the first example) run once per machine, the same pattern already
used for Ollama's own weights (`.ollama/`, `models/*.gguf`).

## Windows DLL search order: a spawned sidecar only searches its own directory, not `resources/`

**Found:** 20 September 2026, same session.

A Tauri sidecar's runtime DLLs must sit in the **same directory as the sidecar executable** once
bundled. Windows resolves a spawned process's DLL dependencies by searching that process's own directory
(plus `PATH` and the system directories) — it does not know or care about Tauri's `resources/` tree.
Bundling the DLLs under `resources/tesseract/` produced a sidecar that failed to start
(`STATUS_DLL_NOT_FOUND`, exit code `-1073741515`) with no readable error message, because the OS loader
error never reaches the spawning process's stdout/stderr.

**Fix:** map the sidecar's own dependency DLLs to the bundle root (`"./"` in `tauri.conf.json`'s
`bundle.resources`), not to a subfolder. A library the *application* loads directly through an explicit
path (like `pdfium.dll`, loaded via `Pdfium::bind_to_library`) has no such constraint and can live
anywhere under `resources/`.

---

## How to use this file

- New entry at the top, dated, with a short heading naming the symptom or the wrong assumption — not
  the fix — so a search for the error text finds it.
- State what actually happened (the exact error text helps), why, and the concrete fix or rule that
  prevents it. Link to the file where the real fix lives rather than duplicating code here.
- This file is versioned (`docs/`), like the rest of the English specification: read `AGENTS.md`.
