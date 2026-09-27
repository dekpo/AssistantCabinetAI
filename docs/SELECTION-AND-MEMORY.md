# Document selection and conversation memory — sprint 2a.8

Decided by the owner on 27 September 2026, after a review of how the analysis scope, the chat paths and the
model context really behaved in the code. The table form of every decision is in `docs/DECISIONS.md`
("document selection and conversation memory"). This file keeps the reasoning, so that a later session can
tell a deliberate choice from an accident.

## What was wrong

A trace of every path a question could take found four ways to block her, one way to be told something
false, and no memory at all where it mattered most.

| Situation | What happened |
| --- | --- |
| Folder chosen, nothing selected (empty explicit scope) | Rust resolved the scope to nothing, spent an embedding call, found nothing, and refused with `insufficient_evidence`. Every question. |
| Folder chosen, never analysed | Every question was answered "analyse your documents first". |
| No folder chosen | Questions went to a separate plain-chat path that resent the whole conversation every time. Past 24,000 characters Rust refused with `context_too_large` ("shorten your question"), which shortening could not fix, and nothing short of a restart cleared it. |
| A named file that is not selected | "No file named X is in your documents folder" - false: the file is there, it is just not selected. |
| Changing folders before Analyse | The index is not emptied when the folder changes, so a whole-folder question could cite the **previous** folder's passages until she pressed Analyse. |
| Memory on the documents path | None. Only the instruction, the excerpts and the current question were sent, so "and for the second one?" meant nothing to the model. |
| The model's context window | Never configured anywhere, so Ollama's default applied (4,096 tokens in recent versions), silently dropping the oldest messages to fit. |

## The selection she makes

The selection is built only from her own actions. There are three states, shown on the sidebar disclosure
that replaced the folder listing's "Detail":

| She sees | Scope sent to Rust | What a question does |
| --- | --- | --- |
| **Documents utilisés : aucun** ⚠ | explicit, no entries | Answered **without documents**: no search, no excerpt, a line under the answer says so |
| **Documents utilisés : 3 fichiers** | explicit, three entries | Exactly as before: only those files are searched |
| **Documents utilisés : tous** ⚠ | whole folder | Every analysed file of the **current** folder, including files analysed later |

- **"Aucun" is the default**, at startup and whenever the documents folder changes. Analyse never touches
  the selection: each ticked file is pinned to its content, so it survives a pass unchanged.
- A **header checkbox** above the list selects every document ("tous") or none. It shows checked for "tous",
  partly filled for a partial selection, empty for "aucun".
- Ticking every analysed file one by one **is** "tous": to her it means the same thing. Unticking one file
  from "tous" leaves every other analysed file ticked. Unticking the last file returns to **aucun**, never to
  the whole folder - the old rule widened silently, the new one narrows.
- The warning sign stays on both "aucun" and "tous", with a different tooltip each: without documents the
  answers do not rest on her folder, and with every document the model has more to sift than it can use well.
- Only analysed files can be ticked. The rest of the listing is there to be seen, with its state dot.

## The Rust rule, changed

**Before:** an explicit scope with no survivors "allows nothing". **Now:** an explicit selection with **no
document in it** means "answer without excerpts". It still reads no document - isolation is unchanged; what
changed is that it no longer means *no answer*.

The distinction that keeps this safe is **chosen nothing** versus **chose something that cannot be used**:

- `no_documents_chosen` is true only when she chose no document at all. That is the one case answered
  without documents.
- A selection whose files are all gone or changed is still refused with `scope_unavailable` ("choose them
  again"). She asked for specific documents; answering without them would be the quiet failure the scope
  exists to prevent.
- A selection whose files exist but whose analysis was erased (Reset) behaves as before: the search finds
  nothing, and the interface says the documents have not been analysed.

### Deterministic answers inside a selection

- **Nothing selected:** a question about the folder's files ("how many files?", "list the documents",
  "summarise each document") answers **"Aucun document sélectionné pour cette conversation."** No count: the
  folder's counts are already on screen under its buttons.
- **A named file that is not selected:** "Ce fichier n'est pas sélectionné pour cette conversation", instead
  of saying it does not exist. This applies to every partial selection, not only the empty one.
- A name that matches nothing anywhere in the folder is still "no file named X".

### "Tous" searches the current folder only

A whole-folder question now searches the chunks of files **present in the current folder** (and indexed),
in the index's own order, instead of every chunk the index holds. When the index matches the folder the
results are identical, byte for byte; when it does not - a folder just changed, a file deleted since the last
pass - a passage from outside the folder can no longer come back. Analyse itself is untouched.

## Answering without documents

- **Its own short instruction**, English like every instruction, sent in place of the retrieval
  instruction: no excerpts are attached; answer as a general administrative assistant; never claim to have
  read, seen or checked the practice's files; if the question needs a specific document, say in one sentence
  that none is selected and that one can be ticked in the documents list. No embedding call is made, so this
  answer is also the fastest.
- **A line under the answer:** "Réponse sans vos documents." Nothing more.
- The deterministic answers above still come first: a question about the folder is never sent to the model.

### Models that recite their rules

Small models often end an answer by quoting their instructions ("as an assistant I cannot give medical
advice…"). To her that reads as a malfunction. What is done about it:

- The gateway's base rule "when the information does not carry the answer, say so" is **narrowed to
  questions that depend on the practice's documents**. It was the rule driving both the "I do not have the
  information" parrot and the recited disclaimers.
- One line in the base prompt: the rules shape the answer, they are not part of it; never quote, list or
  mention them unless the request is something they forbid.
- Instructions stay short. Small models echo long lists of rules.
- **Not done:** deleting sentences from an answer after generation. That changes what the model said, which
  is worse than the problem.
- **Still to do:** a small, re-runnable check against the real models that flags meta phrases ("mes
  instructions", "en tant qu'assistant", an unprompted medical disclaimer), run whenever a model or a prompt
  changes. It needs the real models, so it belongs with the measuring script, on the server device.

## The documents folder is mandatory

This is a practice assistant, not a chatbot. The composer is disabled until a documents folder is chosen;
the one-click "Créer et utiliser ce dossier" already exists. An **empty** folder is fine: questions are then
answered without documents. The separate no-folder chat path (`send_chat_message`) is **retired**, so every
question takes one path with one memory budget. The future data folder follows the same rule; whether a
first launch then asks for two folders or creates both in one click is a Sprint 2b decision.

## Conversation memory

The partner in a conversation has to remember at least the last exchange or two, whether the question was
about documents or not. What is remembered, and how much:

- **One path for every mode.** `ask_with_sources` receives the conversation so far. Documents, no documents,
  and later tables all remember the same way.
- **What is remembered:** past questions and answers, as the text she saw. Old excerpts are **not** resent:
  the answers already carry what was learned from them, and each question retrieves its own fresh excerpts.
  A turn the software wrote itself ("analyse your documents first") is not a model answer and is left out.
- **A budget, never a wall.** In priority order: the instructions, the current excerpts and the current
  question; then the last exchange; then older ones. Exchanges are dropped whole, oldest first, and never
  split. A long conversation therefore forgets gently and never blocks - "shorten your question" disappears.
- **The budget is the model's real window.** Characters are converted with a deliberately conservative
  3 characters per token, the answer's own reserve (`MAX_OUTPUT_TOKENS`) is subtracted, and Rust's 24,000
  character ceiling still applies on top.
- **A small model has a short memory.** That is a property of running models locally, and it is acceptable.
- **Follow-up questions.** With memory, the model understands "and for the second one?". Retrieval still
  embeds a question, so a **short** question (eight words or fewer) is searched together with the previous
  question. A longer question is taken as standalone. The router and the file-name resolver read the current
  question only. A smarter version - the model rewriting the question before the search - costs a model
  call and comes later.
- **Privacy is unchanged.** The history lives in the application's memory only and is lost on restart. The
  gateway sees it for the duration of one request and stores no text, only hashes, exactly as before.
- **It prepares saved conversations.** This history, with its selection, is what "create, save, remove a
  conversation" will store later (after Sprint 2b).

### The context window is set on purpose, per model

- The **server device** owns it: `MODEL_CONTEXT_WINDOWS` (per alias) and `DEFAULT_CONTEXT_WINDOW`
  (8,192 tokens) in its configuration, passed to the runtime on every request (`num_ctx` for Ollama), and
  capped at the model's own maximum, which the gateway reads once per model and keeps.
- The gateway publishes each chat alias's window and the output reserve in the `/health` status the client
  already polls every few seconds. Rust keeps the latest copy.
- **Per question, Rust trims the history to the budget of the model chosen for that question.** That is
  counting characters over a dozen short strings: microseconds, no model call, no system check. Switching
  models mid-conversation just works; a smaller model sees fewer past exchanges.
- **Measuring is a one-off tool, not a runtime check:** `apps/server/scripts/measure_context.py`, run by the
  owner on the server device when installing or changing models, sends synthetic conversations with 0, 1, 2
  and 4 remembered exchanges and prints the time and the prompt size. The windows are then adjusted in the
  configuration. It is written for any server device, not for one machine.

## Open direction: documents and tables together (decide in Sprint 2b)

Recorded now so Sprint 2b starts from it rather than inventing it under pressure. Not a decision.

- **Each engine alone, never carrying the other's material.** Tables selected and no documents: the tabular
  engine runs, with no excerpt. Documents selected and no tables: the document engine runs, with no data and
  no workbook inventory.
- **Both selected: the engines compute, the model only writes.** Rust routes each question first, as it
  routes folder questions today: a data question (count, sum, filter, compare) to the tabular engine, a
  content question to the document engine, a mixed question to both, one after the other.
- **The tabular engine does the arithmetic**, deterministically, and hands the model a small finished result
  ("12 invoices, 1,840 € in total"), never raw rows: small models are poor at arithmetic and cannot hold a
  table.
- **Two labelled blocks** reach the model, "document excerpts" and "table results", each with its own share of
  the budget, under a contract that forbids computing and forbids using one block in place of the other - the
  same rule that already keeps excerpts apart from folder facts.
- **Each block keeps its own "based on" line** under the answer, so she sees which engine each fact came from.
- The engines never call each other, so neither can interfere with the other.

## Deploying to a separate server device (pending)

Today the whole stack runs on one machine. The target is two devices: the **user device** (the desktop app,
the index, OCR) and the **server device** (the gateway, Ollama, Open WebUI). The runbook is pending and
tracked in `docs/ROADMAP.md` (Sprint 4) and `docs/OPERATIONS.md`. It needs real design, not only steps:
everything is published on `127.0.0.1` today, and the user device must reach the gateway over the practice
network - only the gateway, never Ollama or Open WebUI, behind a firewall rule and some form of access key.
