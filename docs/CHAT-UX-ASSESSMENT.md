# Chat UX assessment — the pilot's reading and writing surface

**Written 21 September 2026, after inspecting the repository with Sprint 2.5 (local OCR) delivered and the
chain holding end to end. No code has changed.** This document records feasibility only; nothing here is
scheduled, and nothing here may displace a milestone.

Read with `docs/ROADMAP.md` (dates and what each sprint owns), `docs/DECISIONS.md` (the "Client UI —
noted, not scheduled" table, which this document expands), `docs/ARCHITECTURE.md` (the boundary between
the webview and Rust), `docs/LANGUAGE-AND-LOCALE.md` and `docs/PRIVACY-AND-SECURITY.md`.

**Why it exists.** Sprint 3's GP workflows put the pilot in front of this chat panel for hours a day. A
workflow she cannot comfortably read, interrupt or correct is a workflow she will not adopt, however
correct its retrieval. The nine questions below were raised by the owner on 21 September; seven are
recorded here, and three smaller readability fixes were cut into their own session because they carry no
architectural decision at all.

---

## Summary

| # | Item | Verdict | Rough effort | Blocked on |
| --- | --- | --- | --- | --- |
| 1 | Render markdown in an answer | Feasible | ~half a day | Nothing |
| 1b | Download / source links **inside** markdown | **Refused as asked** — wrong carrier | — | Use structured events instead |
| 2 | Stop button during generation | Feasible, two layers | ~1 day | One product decision (below) |
| 3 | Copy and edit/repost a question | Feasible | ~1 day | Nothing |
| 4 | Copy and regenerate an answer | Feasible | ~half a day **if built with 3** | Nothing |
| 5 | Upload a file into the Work Folder | Feasible, largest | ~2–3 days | Three policy decisions (below) |
| 6 | Read an answer aloud (local TTS) | Feasible | ~1 day | A `docs/DECISIONS.md` amendment |
| 7 | Dictate a question (speech to text) | **Not now** | A sprint | Frozen until after 14 October |

Items 1 to 5 fit between Sprint 2.5 and Sprint 3 without touching either milestone date. Item 6 needs a
written decision first. Item 7 is a sprint of its own, comparable in size to Sprint 2.5.

---

## Already cut into a session — chat readability

Three things were separated out because they are view-only, carry no decision and take a few hours: the
user's bubble reads right-aligned and should read left; a long question cannot be collapsed and needs a
four-line clamp with an "Afficher plus" toggle; and there is no scroll-to-bottom control on a long
conversation. That last one also fixes a live bug — `MessageList.tsx:10-12` scrolls to the bottom on every
`entries` change, and streaming replaces that array on every delta, so scrolling up during an answer snaps
the reader back many times a second.

The handoff lives in the local (ungit) `docs/SESSION-CHAT-READABILITY.md`.

---

## 1. Markdown in an answer

**The observation is correct.** `MessageList.tsx:41` puts the answer straight into a paragraph, and
`.message__body` (`styles.css:245-249`) carries `white-space: pre-wrap`, so `**Patient**` and `-` bullets
reach the screen as literal characters. There is no markdown library anywhere: `apps/desktop/package.json`
has exactly three runtime dependencies (`@tauri-apps/api`, `react`, `react-dom`).

**Recommendation: `react-markdown`, not a hand-rolled renderer.** It builds a React element tree rather
than using `dangerouslySetInnerHTML`, so untrusted output never becomes HTML. That matters specifically
here: the model's input contains extracted document text, and `.cursor/rules/security-local-first.mdc`
requires that tools obey the schema and the allow-list, **never** the extracted text. A regex renderer
over streaming, syntactically incomplete markdown is a bug farm by comparison.

Three implementation facts found in the code:

- `<p className="message__body">` must become a `<div>`. Markdown emits `<ul>`, `<h3>` and `<p>`, which
  cannot legally nest inside a `<p>`; the browser silently restructures the DOM if they do.
- `white-space: pre-wrap` must be dropped for rendered markdown (it doubles every blank line) but **kept**
  for the user's bubble, which stays plain text.
- The CSP in `tauri.conf.json` (`default-src 'self'; img-src 'self' data:`) is already right for this: a
  markdown image pointing at an http URL simply fails to load, which is the desired behaviour.
- Streaming re-parses on every delta. Memoise per entry and only re-parse the one that is streaming.

### 1b. Why download and source links must not come from the markdown

**This is the one request to refuse in its asked form, and the reason is recorded history.**
`docs/DECISIONS.md` ("A failed trial worth remembering") records that a French filing prompt made the
model invent `rename_file`, `move_file` and `delete_calendar_event` calls, and that the fix was *"file
actions live in code rather than in model output."* A `[télécharger](file:///C:/...)` in an answer is a
file path chosen by model prose; a poisoned PDF could put any path there. It also would not work today:
`capabilities/default.json` grants only `core:default` and `dialog:allow-open`, and there is no `asset:`
protocol in the CSP.

**The correct shape already exists in this codebase.** `commands.rs:327` sends retrieval sources as a
*separate structured event* beside the text stream, and `MessageList` renders them as UI:

```text
ChatStreamEvent::Sources { sources: Vec<Evidence> }   ->  React renders the affordance
```

So the three sub-requests resolve differently:

| Sub-request | Answer |
| --- | --- |
| Link to the source of a statement | `Evidence` already carries `relativePath` and `pageNumber`. Add a `reveal_in_work_folder(relative_path)` command that joins against the **stored** work folder, re-runs `WorkFolderPolicy`, refuses `..` and symlinks, and opens through the OS shell. The path comes from Rust-produced evidence, never from prose. One command, one permission, one button — genuinely small |
| Produce a certificate or any artefact | Sprint 3's plan-and-approve flow (dry run, manifest, hashes, cap, trash, undo). The artefact lands in the Work Folder and the UI shows it as a reviewed result, not as a link in a sentence |
| Convert DOCX to PDF | Out of scope by name in both `.cursor/rules/v0-sprint.mdc` and `docs/ROADMAP.md`: *"DOCX to PDF and any other document conversion"* |

**Rule to carry forward:** markdown may carry *formatting*. Anything that identifies a file, a path or an
action travels on the structured channel beside the text, and Rust re-validates it.

---

## 2. Stop button during generation

Feasible. Swapping the "Envoyer" button to "Arrêter" while pending, and back on stop, is the right
interaction. **But a frontend-only version would be a trap, and that is the finding that matters.**

`useChat.send` awaits `invoke(...)`, and Tauri v2's `invoke` returns a plain Promise with no abort —
abandoning it does not stop the Rust future. `gateway.rs:160` streams with no cancellation check at all.
So flipping `pending` in React would hide the answer while the Mac mini keeps generating at full GPU for
another minute, and the orphaned stream would keep pushing deltas at a dead entry id.

The real version is four small pieces:

1. `Composer` gains `onStop`; `useChat` exposes `stop()`; two catalogue keys.
2. A `cancel_chat` command flips a `CancellationToken` held in `AppState`.
3. `gateway::chat` checks it each loop iteration and drops the `reqwest` response, closing the connection.
4. Nothing changes on the server. It is already structured to cope: `api/chat.py`'s generator has a
   `finally: record(...)` so the metadata register stays honest on a disconnect, and `providers/ollama.py`
   streams inside an `async with`, so cancellation propagates down to Ollama. Confirm once by hand.

**Two things that are decisions, not code:**

- **Keep or discard the partial answer?** `useChat.ts:115` discards on failure, deliberately, with the
  comment *"an incomplete summary is worse than none."* A user-initiated stop is not a failure, so keeping
  it is defensible — but it then needs a visible "interrupted" marker. A half-written summary of a
  specialist letter that *looks* complete is exactly the risk `chat.disclaimer` exists for. **Recommend:
  keep it, marked, with its own catalogue string.**
- **Stop must also cancel the embedding leg.** `ask_with_sources` calls `gateway.embed()` before the chat
  call, and that is what runs during the "Recherche" spinner. Cancelling only the chat leg leaves the
  button unresponsive for the first seconds, which is precisely the moment she wants it.

Tests: one vitest that `stop()` clears pending; one `cargo test` using the existing `tiny_http` dev
dependency, proving the stream loop exits on the token.

---

## 3. Copy and edit/repost a question

Already assessed in `docs/DECISIONS.md` as *"Feasible, small: `entries` already holds every user turn… a
per-turn 'edit' affordance in `MessageList` would populate the composer and truncate `entries` from that
turn on resend — no new IPC, no server change."* Confirmed, with two wrinkles that entry does not mention:

- The non-retrieval path rebuilds the conversation from `entries` **inside** `send` (`useChat.ts:101-104`),
  so truncation must happen *before* the send or the edited question goes out with the stale tail
  attached. In practice `useChat` needs a `resend(entryId, text)` rather than a reused `send`.
- An inline edit form (the ChatGPT shape the owner asked for) reads better than repopulating the composer,
  but duplicates the textarea's draft state inside `MessageList` — roughly 40 lines of local state. Not a
  problem, just not zero.

**Do not copy ChatGPT's version history** (`< 1/2 >` arrows over superseded questions). Truncate and
replace is simpler and is what the recorded decision already committed to.

**Clipboard needs the Tauri plugin, not `navigator.clipboard`.** `writeText` requires a secure context.
On Windows the webview is served from `http://tauri.localhost`, which Chromium treats as trustworthy; on
macOS it is a custom scheme where that is not guaranteed, and `AGENTS.md` makes both platforms mandatory.
So: `@tauri-apps/plugin-clipboard-manager` — one pnpm dependency, one Cargo dependency, one line in
`lib.rs`, one permission in `capabilities/default.json`. That also keeps clipboard access an explicitly
granted capability rather than an ambient browser API, which is how that file is written today
(*"Opening a folder dialog is the only file capability"*).

---

## 4. Copy and regenerate an answer

`docs/DECISIONS.md` already parks this as the "per-answer action row", with the timing the owner is now
proposing: *"worth adding once OCR (Sprint 2.5) is done, before the sprint-3 workflows"*, and notes that
copy *"sits closest to the existing export concept."*

- **Copy** — the same clipboard plugin as item 3. Trivial once that is wired.
- **Regenerate** — the same `useChat` restructuring as item 3, which is why they should be built together.
- **One caveat specific to this product:** regenerating re-runs `embed` → `retrieval::search` → `chat`, so
  the sources can legitimately differ between runs. The UI must clear and re-render them. Today sources
  are attached once on the `Sources` event; a regenerate that keeps the previous list would caption a new
  answer with an old citation, which is worse than no citation at all.

The ChatGPT "share" button has no equivalent here and is not wanted: nothing is transmitted from this
product (`AGENTS.md`, rule 3).

---

## 5. Upload a file into the Work Folder, and index it

The largest of the seven. `docs/DECISIONS.md` already assesses it as *"Feasible, larger"* and names the
two missing pieces correctly: a command that indexes one file instead of walking the folder, and a picker
that copies the file into the Work Folder first, *"because the allow-list only covers what is already
inside it."*

**The incremental engine genuinely exists.** `indexing.rs:38 run` already hashes each file and calls
`should_skip` per file, and `IndexStore::replace_document` is keyed by relative path. A one-file path is a
refactor of the loop body into `index_one(file)`, not new machinery — roughly 150 lines including its own
summary type.

**On extensibility, which was the owner's specific worry: the design is already right, and the upload
button must not break it.** `discovery.rs:12-14` states the boundary explicitly —

```text
/// Extensions this session's extractor understands. `.csv` and `.xlsx` are a later pipeline
/// (`docs/RETRIEVAL.md`), not this one, so they are deliberately absent here.
const SUPPORTED_EXTENSIONS: &[&str] = &["pdf", "docx", "txt", "md", "jpg", "jpeg", "png"];
```

and `docs/ARCHITECTURE.md` deliberately keeps a **second** pipeline (`TabularDataSource`, Sprint 2b)
rather than flattening a workbook into text, because a spreadsheet answer must be deterministic and
computable with the model switched off. So the upload button copies the file in and hands off to the same
ingestion entry point, which dispatches by extension. Sprint 2b then adds a branch *there* and the upload
button never changes. A button that knows how to process files is a button edited every sprint.

**No new permission for the picker.** `dialog:allow-open` already covers file selection as well as folder
selection. But the copy happens in Rust; the webview has no filesystem access and must not gain any.
Drag-and-drop from Explorer is possible through Tauri's drop events (which yield real paths) and is the
fiddlier half — ship the button first.

**Three policy decisions to settle before writing code:**

1. **Name collisions.** Overwriting a document already in her Work Folder is a destructive write, which
   `AGENTS.md` rule 3 forbids without explicit confirmation. Needs a suffix-rename or a confirm step.
2. **Source outside the allow-list.** She will pick files from Documents or OneDrive. Reading from there
   is fine — it is the **destination** that `WorkFolderPolicy` must re-validate, plus an explicit symlink
   refusal on the copy path (`discovery.rs` refuses symlinks when walking; a copy is a different path).
3. **Unsupported extension.** An `.xlsx` dropped today must be refused with a machine code, not copied in
   silently — otherwise it sits in the folder looking ingested while being invisible to the index.

---

## 6. Read an answer aloud (local text to speech)

**Technically feasible without any sidecar, which makes it far cheaper than OCR was.**
`window.speechSynthesis` is a Web API unaffected by the CSP, present in WebView2 on Windows (system SAPI
voices) and in WKWebView on macOS (backed by AVSpeechSynthesizer). Both mandatory platforms are covered
natively, which is the opposite of the dictation case below.

**Three cautions, the first of which is severe:**

- **On Windows, `getVoices()` includes Microsoft's *online* natural voices.** Selecting one sends the text
  to Microsoft's servers. The text being a summary of a patient letter, that is the single worst outcome
  available in this project. The mitigation is one line — filter to `voice.localService === true` — but it
  must be non-negotiable, covered by a test asserting the code never selects a non-local voice, and it
  must **refuse** with a localised message rather than fall back when no local voice matches the locale.
- **Voice availability is a machine-level fact.** Windows may ship no French voice if the language pack is
  absent. The feature must degrade with a machine code exactly as OCR does (`ocr_unavailable` → falls back
  to Sprint 2a behaviour), never fail silently.
- **`getVoices()` populates asynchronously.** Called once, it returns an empty array; you must wait for
  `voiceschanged`. A classic footgun and a `docs/TROUBLESHOOTING.md` entry when it bites.

Also: once item 1 lands, answers are markdown, and the synthesiser must receive the stripped plain text or
it reads the asterisks aloud.

**The blocker is documentary, not technical.** `docs/DECISIONS.md` freezes text to speech as
local-or-on-device-only under "Reserved contracts", and the Client UI note says read-aloud *"would need
the voice rules revisited for text already on screen rather than a live conversation."* That revision is
small and well-scoped — speaking text already rendered on the pilot's own screen is materially different
from a live voice conversation — but it belongs in `docs/DECISIONS.md` **before** the code, not after.

---

## 7. Dictate a question (speech to text)

**The one to refuse for now, for three independent reasons.** The first is the one that changes how to
think about the feature.

**The browser API the request hopes for is a cloud API.** Chromium's `webkitSpeechRecognition` does not
recognise speech locally; it streams the audio to Google's servers. `docs/DECISIONS.md` and
`.cursor/rules/architecture.mdc` rule 5 both forbid cloud speech to text outright. The easy path is the
forbidden path — which is the exact opposite of item 6, where the browser API really is local. **That
asymmetry is the thing to remember: local voices for speaking exist on both platforms; a local browser
recogniser does not exist at all.**

**It fails the cross-platform gate regardless.** `SpeechRecognition` is not exposed in WKWebView on macOS
and is not reliably enabled in WebView2. `AGENTS.md`: *"A library, runtime or engine available on only one
of them is disqualified however good it is."* This is the same trade-off that chose Tesseract over the
platform OCR engines — `docs/ROADMAP.md`: *"those are two engines with two accuracies and two failure
modes."*

**And it is frozen in writing.** `.cursor/rules/v0-sprint.mdc` freezes voice until 14 October 2026.
`docs/DECISIONS.md` reserves `POST /v1/audio/transcriptions` with handlers returning `501` until a
dedicated phase.

**The honest local design, for when it is scheduled**, and the owner guessed it correctly: a
`SpeechProvider` port beside `OcrProvider`, with `whisper.cpp` as a bundled sidecar, reusing the
sidecar-discovery pattern in `commands.rs` and the same degrade-to-unavailable behaviour. But note what
that pattern actually cost: `docs/DECISIONS.md` records that `bundle.externalBin` is **still not wired**
for Tesseract, and that macOS OCR resources are **"Not started"** — no fetch script, no verified dylib
set, and nothing in this repository has ever been compiled on macOS. Add a model file to bundle on top of
that. This is a sprint, not a feature.

---

## Decisions this document is waiting on

None of these is a coding question; each changes what gets built.

1. **Item 2** — does a stopped answer stay on screen, marked as interrupted, or disappear like a failed one?
2. **Item 5** — what happens when an uploaded file's name already exists in the Work Folder?
3. **Item 6** — is `docs/DECISIONS.md`'s voice rule amended to permit local-voice-only read-aloud of text
   already rendered on screen? Without that, item 6 does not start.
4. **Ordering** — items 1 to 5 fit before Sprint 3. Confirm that none of them is allowed to move the
   1 October start of the GP workflows.

## Cross-platform note

Everything in items 1 to 6 is either pure webview code or an existing Tauri plugin, so nothing here
repeats the OCR portability problem. The two places where a platform difference is real are the clipboard
(handled by using the plugin rather than `navigator.clipboard`) and the availability of a local voice
(item 6, handled by degrading with a machine code). Item 7 is the only one that would need a second
bundled sidecar on two operating systems.
