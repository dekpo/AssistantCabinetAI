# Document selection and conversation memory — sprint 2a.8

Decided by the owner on 27 September 2026, after a review of how the analysis scope, the chat paths and the
model context really behaved in the code. The table form of every decision is in `docs/DECISIONS.md`
("document selection and conversation memory"). This file keeps the reasoning, so that a later session can
tell a deliberate choice from an accident.

**Amended the same day.** The first cut of conversation memory (`## Conversation memory`, below) shipped a
real regression: real testing found the models hallucinating more, not less, once they could remember the
conversation. Traced and fixed the same day; the two rules that came out of it are load-bearing for
everything else in this file and are stated first, on purpose, rather than buried where the bug was.

## Grounding outranks memory

**A model that forgets after two exchanges but never states an unfounded fact is a success. A model that
remembers the whole conversation and invents one fact has failed at the one thing this product exists for.**
Memory is a convenience; grounding is the product. Whenever the two pull in different directions, grounding
wins, without exception and without asking the model to judge the trade-off itself.

What actually caused the regression, both found by reading the exact strings sent to the model rather than
guessing:

- **The honesty rule became conditional.** The base prompt used to say, unconditionally, "you never invent a
  fact; when the information you were given does not carry the answer, say so." Sprint 2a.8 rewrote it to
  "you never invent a fact **about the practice, its patients or its documents**... **when a question depends
  on the practice's documents** and the information does not carry the answer, say so." That second clause
  handed the model a judgement call - *does this question depend on the documents?* - that used to not exist.
  Once a question's connection to the documents was even slightly indirect (a short follow-up, a question the
  excerpts only partly covered), the model could decide "no" and answer from training data instead of saying
  it did not know. **Fixed:** the base rule (`BASE_SYSTEM_PROMPT` in `prompts.py`) is unconditional again. The
  licence to use general knowledge exists **only** in the instruction Rust sends on the one path it has
  already determined has no document (`NO_DOCUMENTS_INSTRUCTION`) - never as something the model infers for
  itself on a path where documents **are** attached.
- **Grounding material sat several turns away from the point of generation.** The message order was
  `[system: base rules] [system: retrieval instruction + this question's excerpts] [...history...] [user:
  question]`. Structurally correct, but for a small local model (1–3B), a distant system turn is read with
  less weight than the turns immediately before generation starts - "lost in the middle" is a documented
  property of exactly this model class, not a guess. The longer the remembered conversation, the further the
  excerpts drifted from the question, and the more the model leaned on the *tone* of prior answers - or
  reused a citation from a previous turn's excerpts - instead of re-checking what was actually retrieved for
  *this* question. **Fixed:** `commands::Writer::write` no longer sends a leading system turn for the
  grounding material. Whatever grounds this specific question - the tier's instruction, plus its excerpts or
  data - is folded into one turn immediately before the question, *after* every past exchange, so it is
  always the last thing the model reads before it starts writing, however long the conversation has gotten.
  The gateway's own system turn (the tier-independent safety rules) still comes first, once, and is
  unaffected by how much history follows it.

### The grounding priority chain

Which material a question is answered from is decided by Rust, deterministically, from what is actually
selected - never guessed by the model from the question's wording. Three tiers, evaluated in this order:

1. **A document is selected** → `retrieval::RETRIEVAL_INSTRUCTION`, held to its excerpts, unconditionally.
2. **No document is selected, but tabular data is** → the tabular engine alone (`tabular_answer`), with
   **no model at all** (since the tabular UI session, 28 September 2026). `AnalysisScope` carries the two
   selections side by side (`mode` for documents, `dataMode` for workbooks) and `AnalysisScope::tier` picks
   the tier from them before any file is read. A question the engine answers returns a computed value or a
   structural fact, citing its sheet; one it cannot answer returns a *nudge* naming the workbook's real
   columns and the operations that do work - never an open-chat fallback. `tabular::escalation::TABULAR_INSTRUCTION`
   is kept, unsent, for the case below where both are selected. Both selected at once is refused
   (`documents_and_tables_together`) until that case is designed.
3. **Neither is selected** → `conversation::NO_DOCUMENTS_INSTRUCTION`. Meant to be rare - once tier 2 exists,
   this is reached only when she has attached neither a document nor a table - and still cautious even then:
   general knowledge is a last resort the instruction explicitly bounds ("stay strictly factual: never
   invent a specific name, date, amount or other detail you are not certain of"), not a return to an
   unconstrained chat assistant. It does trust one thing beyond the current turn: whatever the user has
   already stated earlier in the conversation, because that came from her, not from the model.

A conversation moving between tiers - she answers two questions with a document selected, then unticks it -
is normal and each question is graded independently; nothing here tries to keep a conversation in one tier.

## Profession-neutral model-facing text

**Every string sent to the model - the base prompt, the retrieval instruction, the no-documents instruction,
and anything added later - must read the same for a doctor, a lawyer, a notary or an accountant.** The pilot
is a French GP's practice (`AGENTS.md`), and that stays true of the *product* and its *docs*, which are
allowed to describe the current pilot in plain terms. It must not be true of what is sent to the model: a
prompt that says "the practice's patients" bakes a medical assumption into the one part of the system a
future non-medical profession cannot configure around.

Found and corrected on 27 September 2026: `BASE_SYSTEM_PROMPT` said "the practice, its patients", "clinical,
diagnostic or prescribing advice"; `RETRIEVAL_INSTRUCTION` said "the practice's own documents";
`NO_DOCUMENTS_INSTRUCTION` said "the practice's documents" and "the practice's files" twice. All four are
now written in terms of **the user** and **their documents/files**, with no mention of patients, doctors,
practitioners, or the word "practice" itself. The one deliberate exception: `BASE_SYSTEM_PROMPT`'s
professional-advice rule now reads "clinical, legal, financial or otherwise" - naming a clinical example
*alongside* legal and financial ones is what makes the sentence read as multi-profession rather than
medical-first; dropping it to a bare "professional advice of any kind" would have been just as compliant but
less protective, since a concrete anchor is what a small model pattern-matches against.

**This is a standing rule for every session and every agent that touches a prompt in this codebase, not a
one-time cleanup.** `retrieval.rs` and `conversation.rs` each carry a test
(`..._stays_neutral_about_who_the_user_is`) that fails the build if `patient`, `doctor`, `practitioner`,
`practice` or `gp` reappears in the instruction constant it guards; `apps/server/tests/test_prompts.py`
carries the equivalent for `BASE_SYSTEM_PROMPT`. Any new model-facing constant - a future `TABULAR_INSTRUCTION`
included - should get the same guard alongside it, not rely on someone remembering to check by eye.

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

- **Tier 3's instruction** (`## The grounding priority chain`, above), sent in place of the retrieval
  instruction: no document is attached; general knowledge is allowed but bounded - never invent a specific
  detail, say plainly when unsure, trust what the user has already said in this conversation; never claim to
  have read, seen or checked the user's files; if the question needs a specific document, say in one sentence
  that none is selected and that one can be ticked in the documents list. No embedding call is made, so this
  answer is also the fastest.
- **A line under the answer:** "Réponse sans vos documents." Nothing more.
- The deterministic answers above still come first: a question about the folder is never sent to the model.

### Models that recite their rules

Small models often end an answer by quoting their instructions ("as an assistant I cannot give professional
advice…"). To her that reads as a malfunction. What is done about it, and what was undone:

- **Tried and reverted, 27 September 2026:** narrowing the base prompt's honesty rule to "questions that
  depend on the documents" did stop some recited disclaimers, but it did so by giving the model discretion
  over when grounding was required, and that discretion is what caused the hallucination regression this
  file opens with. The base rule is unconditional again (`## Grounding outranks memory`); reciting rules is
  addressed by the two items below instead, which do not touch how strict the honesty rule is.
- One line in the base prompt: the rules shape the answer, they are not part of it; never quote, list or
  mention them unless the request is something they forbid.
- Instructions stay short. Small models echo long lists of rules.
- **Not done:** deleting sentences from an answer after generation. That changes what the model said, which
  is worse than the problem.
- **Still to do:** a small, re-runnable check against the real models that flags meta phrases ("mes
  instructions", "en tant qu'assistant", an unprompted professional-advice disclaimer), run whenever a model
  or a prompt changes. It needs the real models, so it belongs with the measuring script, on the server
  device.

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
- **A budget, never a wall.** This question's own turn - its tier instruction, its excerpts or data, and the
  question - is reserved first and always kept whole; what is left goes to history, the last exchange first,
  older ones after, each exchange kept or dropped whole, never split. A long conversation therefore forgets
  gently and never blocks - "shorten your question" disappears - and never trims the one turn that actually
  answers the question to make room for older ones.
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

The case of tables and **no** documents is tier 2 of `## The grounding priority chain`, above - built, and
answered by the engine alone, with no model. The case below, both selected, is the one the chain does not
cover on its own and needs the design that follows; until then it is refused explicitly
(`documents_and_tables_together`) rather than routed to one side.

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
