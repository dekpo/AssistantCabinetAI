# Context budget: the excerpts follow the model's window

Decided and built on 13 October 2026, on the owner's instruction, during the Knowledge Base human
test. Branch: `fix/kb-context-budget`.

## The defect

With about 200 documents in the work folder, answers missed files. Reading the code showed that the
amount of text a question may send to the model was fixed in five constants, whatever the model and
whatever window the server device had configured:

| Constant | Value | Effect |
|---|---|---|
| `retrieval::MAX_EVIDENCE_CHUNKS` | 6 | an ordinary question cites six passages at most |
| `retrieval::MAX_EVIDENCE_CHARS` | 6,000 | and at most six thousand characters of them |
| `retrieval::MAX_PER_DOCUMENT_CHARS` | 12,000 | a question about **every** document stops after about a dozen files |
| `mixed_answer::MIXED_DOCUMENT_SHARE_CHARS` | 4,000 | the documents' share of a documents-and-tables answer |
| `gateway::MAX_CONTEXT_CHARS` | 24,000 | the whole prompt, so a window above about 10,000 tokens was memory for nothing |

`docs/DECISIONS.md` already said so ("a larger window is memory for nothing until that cap and the
gateway's own are raised in a separate, measured change"). The server device publishes 12,288 tokens
for `ministral-3:3b`; the desktop could not use more than about 10,000 of them. On the question that
covers every document, the 12,000-character budget meant about ten to twelve files out of 200; the
application reports its coverage on that question (`EvidenceCoverage`) but could do nothing about it.

## The change

The limits are no longer constants chosen once. They are a function of the window the server
publishes for the model chosen for the question.

- `conversation::ContextBudget::scaled_over_baseline(baseline)`: `baseline` multiplied by
  `prompt_chars / prompt_chars at the default window (18,432)`, never below `baseline`.
- `retrieval::RetrievalLimits::for_budget(&budget)` applies it to the three retrieval constants,
  which stay as the **baseline** (`RetrievalLimits::BASELINE`). `mixed_answer` applies it to its share.
- A model at or below the default window (8,192 tokens) therefore reads **exactly what it always
  read**. Nothing changes for the small models the fixtures and the earlier tests were measured on.
- `ContextBudget::prompt_chars` no longer stops at 24,000. It is the window minus the answer's
  reserve, at 3 characters per token, held under **two** ceilings: the cap the gateway publishes
  (`max_context_chars` in `/health`, less 2,000 characters for the gateway's own prompt) and a
  200,000-character safety ceiling in Rust (`gateway::MAX_CONTEXT_CHARS`, no longer the working limit).
- The server publishes `max_context_chars` in `/health`, and its default `MAX_CONTEXT_CHARS` goes
  from 48,000 to 200,000: it is a guard against a runaway request, not the working limit, which is
  the window. A gateway that does not publish the field (older than this change) is assumed to cap at
  48,000, so a new desktop never provokes a refusal from an old server.
- No new setting and no new interface: the one knob stays `MODEL_CONTEXT_WINDOWS` on the server
  device, which the project already treats as the per-model configuration.

What a window gives (answer reserve 2,048 tokens; passages of about 1,000 characters):

| Window (tokens) | Prompt (chars) | Passages | Characters of passages | Characters, one passage per document | Files covered by a question about every document |
|---|---|---|---|---|---|
| 8,192 or less | 18,432 | 6 | 6,000 | 12,000 | about 12 |
| 12,288 | 30,720 | 10 | 10,000 | 20,000 | about 20 |
| 16,384 | 43,008 | 14 | 14,000 | 28,000 | about 28 |
| 20,480 | 55,296 | 18 | 18,000 | 36,000 | about 36 |
| 32,768 | 92,160 | 30 | 30,000 | 60,000 | about 60 |
| 49,152 | 141,312 | 46 | 46,000 | 92,000 | about 92 |
| 65,536 | 190,464 | 62 | 62,000 | 124,000 | about 124 |
| above 65,536 | 200,000 (safety ceiling) | 65 | 65,000 | 130,000 | about 130 |

The unit tests that pin these figures: `retrieval::tests::a_larger_window_covers_more_documents_of_a_question_about_every_document`
(12 files at the default window, 36 at 20,480) and `...cites_more_passages_of_an_ordinary_question`
(6, then 18), `conversation::tests` for the budget itself.

## How far to raise the window (and what it costs)

The window is allocated in memory by the runtime for every request, used or not. For a 3B model the
key-value cache is of the order of **0.1 MB per token** (an estimate from the model's published
architecture, to be checked with `ollama ps` and `nvidia-smi` on the server device):

| Window | Cache, order of magnitude | Reading |
|---|---|---|
| 12,288 | 1.3 GB | what the server device publishes today |
| 20,480 | 2 GB | |
| 32,768 | 3.5 GB | first value to measure |
| 65,536 | 7 GB | needs the model, the cache and the OS inside the graphics memory |
| 131,072 | 14 GB | beyond the 200,000-character safety ceiling anyway |

Three limits, in the order they bite:

1. **Memory of the server device.** The model plus the cache must fit in the graphics memory. When
   they do not, the runtime spills into the processor and the main memory and the answer becomes
   several times slower, or the model fails to load. `ministral-3:3b` announces about 256,000 tokens,
   so the model is never what stops it.
2. **Time before the first word.** The prompt is read before the first word is written. That time
   grows at least in proportion to the prompt, and faster on a very long one. A 60,000-character prompt
   is about 17,000 tokens; the 124,000 of a 65,536 window about 35,000. The existing diagnostics
   record it per question (`generation_first_token_ms`, `chunks_selected`, `estimated_input_chars`
   in `retrieval-timings.jsonl`, see `docs/OPERATIONS.md`).
3. **Quality.** A 3B model reads a long list of passages less well than a short one: it forgets the
   middle and may follow the instruction less closely. This has **not been measured** here, and it is
   what the test below must show. A larger window is not an improvement until it is shown to be one.

The window of this PC does not matter for any of it: the PC runs the client (the local index search
costs milliseconds whatever the number of passages). The model runs on the server device.

**Recommendation.** Measure 12,288 (today), 20,480 and 32,768 on the same questions, keep the
largest value whose answers are both better and fast enough, and do not go above 65,536: the
200,000-character ceiling would be reached anyway, and the quality of a 3B model on that much text
is the doubtful part, not the hardware.

## What this does not solve

A question about **every** document sends one passage per file. For 1,000 files that is about a
million characters, some 300,000 tokens: more than any model of the shelf can read in one request,
and far more than is useful. Raising the window delays the problem; it does not remove it. The honest
answers for a folder of that size are the knowledge base's targeting (name a person, a date, a
category and the search reads only the files that match), and, later, a pass that reads the files in
batches and merges the partial answers. That pass is heavy, so under the owner's rule on silent work
it would run **only inside an Analyse started by hand**. It is not part of this change.

## Things to know

- Above 8,192 tokens `conversation::max_exchanges_with_sources` returns no cap, so a model with a
  large window also remembers more earlier exchanges beside the sources. That cap was added after BUG-09
  (a small model repeating an invented figure from an earlier answer instead of reading the
  excerpts). It was already off at 12,288, so this change does not alter it; if the symptom comes back
  at a large window, the cap has to be raised in steps, not removed.
- Tests that call `retrieval::search*`, `targeting::search_documents` and the other public entry
  points still run with the baseline limits (`RetrievalLimits::BASELINE`); only the two calls in
  `commands.rs` (`targeting::search_documents_within`) pass the model's limits.
- Statements elsewhere in `docs/DECISIONS.md` that describe the 24,000-character ceiling (the memory
  budget row, the mixed tier's budget row, "the useful ceiling today") are superseded by this file.

## Putting it in service

On the server device (`192.168.1.83`), after pulling this branch: set `ministral-3:3b=<tokens>` in
`MODEL_CONTEXT_WINDOWS` in its `.env`, then `docker compose up -d --build server` (the new field in
`/health` needs the new image; `restart` does not read `.env`). Check
`curl http://192.168.1.83:8080/health`: `context_windows` and `max_context_chars`. On the PC, start the
application from this branch (`pnpm tauri dev` in `apps/desktop`); it reads the windows by itself.

To go back: put the previous value in `.env` and `docker compose up -d server`; the desktop then
sends what a window of that size allows. To remove the change altogether, switch to the previous
branch.

## Measuring that it helps (for the test report)

Same questions, same folder, same analysed index, three windows (`12288`, `20480`, `32768`). For each
question note:

- the coverage line shown with the answer to a question about every document (documents covered out
  of documents indexed; computed from the evidence and the inventory, never by the model);
- the number of passages cited;
- the time to the first word and the total time (`retrieval-timings.jsonl`, or a stopwatch);
- whether the files that were missed before are now named, and whether the answer got worse in any
  other way (a wrong figure, a file attributed wrongly, an instruction ignored).

Improvement means: more files named, no new error, and a time the owner accepts. Anything else is
a cost without a gain and the window goes back.

## Measured on the tower (13 October 2026)

The numbers the change was judged on, from the application's own timing log
(`retrieval-timings.jsonl`) during the final human test. `ministral-3:3b` on the LAN tower
(`192.168.1.83`). The excerpts only are counted; the instruction adds a few hundred characters.

| Window | Passages sent | Characters | First word | Whole answer |
|---|---|---|---|---|
| 12,288 (before) | 6 | 2,000 to 5,800 | 20 to 29 s | 24 to 49 s |
| 12,288 | 1 to 2 (a narrowed or named-file question) | 120 to 670 | 16 to 20 s | 19 to 24 s |
| 32,768 | 30 | 20,624 | 89 to 94 s | 105 to 112 s |
| 32,768 | 29 | 28,998 | 82 to 83 s | 92 to 97 s |
| 32,768 | 33 | 32,847 | 91 s | 95 to 96 s |
| 32,768 | 37 | 36,720 | 112 s | 117 s |
| 65,536 | 62 | 46,847 | failed after 21 s (HTTP 500) | |
| 49,152 | | | "extremely slow" (not measured) | |

What it says:

- A first word takes **16 to 18 s even for a 120-character prompt**: a floor that comes from the
  machine, not from the amount of text. The cause is not in the log (to check on the tower:
  `ollama ps`, column PROCESSOR; `nvidia-smi`).
- Beyond the floor the prompt costs about **90 tokens per second** (3 characters per token).
- At 32,768, 29 to 37 passages came from **four or five distinct files**: the extra passages are
  further pages of the same long file, not further files.
- The improvement in **answers** at a large window was not measured (the replay of the 19 sentences
  of step E2 was not done).

**Setting recommended on this tower until the search sends less text:**
`MODEL_CONTEXT_WINDOWS=ministral-3:3b=12288` (10 passages), or 16384 (14 passages) at most. Written
as a comment beside the variable in `.env.example`. Do not use 32768 or more on this machine. The
Mac mini must be measured again (`apps/server/scripts/measure_context.py`); these figures describe
the tower only.

Why a larger window is the wrong lever for most questions: see
`docs/test-reports/knowledge-base-pass-1/07-record-study-fixes-and-freeze.md`, sections 4 to 8. A
question whose answer is a set of files is answered better by asking the index for every file that
holds the word (a word search of the existing index found 105 of the 109 expected files for six
such questions, where six passages found 36), and a question about a person needs one to six
passages, not thirty.

## Reusing this change without the knowledge base

The change is one commit, `03fd4ef` ("feat: size the excerpts sent to the model from its published
context window"), 15 files, 532 insertions. It is kept reachable by the tag `kb-context-budget-change`
(see the git section of the study above). It does **not** need the knowledge base to work: the budget
(`conversation::ContextBudget::scaled_over_baseline`, `retrieval::RetrievalLimits`, the server's
`max_context_chars`) is independent of it.

Checked on 13 October 2026 by a dry run on `main` (a throw-away working folder, removed after):
`git cherry-pick -n 03fd4ef` applies cleanly to `.env.example`, `conversation.rs`, `gateway.rs`,
`mixed_answer.rs`, the three server files, `compose.yaml`, `OPERATIONS.md`, `SELECTION-AND-MEMORY.md`
and this file, and stops on **three** files:

| File | Why | What to do |
|---|---|---|
| `knowledge/targeting.rs` | the file does not exist on `main` | drop that part: it only passes `RetrievalLimits` through `search_documents_within` |
| `retrieval.rs` | the knowledge base added code around the same functions | keep the `RetrievalLimits` struct and `for_budget`, and make the search functions take it |
| `commands.rs` | the two calls that pass the limits sit inside the knowledge base's search | pass `RetrievalLimits::for_budget(&writer.budget)` to the search call of the document path |

The server half (`/health` publishes `max_context_chars`, default 200,000) has no conflict. Tests
that pin the figures: `retrieval::tests::a_larger_window_covers_more_documents_of_a_question_about_every_document`,
`...cites_more_passages_of_an_ordinary_question`, `conversation::tests`, and
`apps/server/tests/test_context_window.py`. If the knowledge base is rewritten, port those tests
first: they are the specification of this change.
