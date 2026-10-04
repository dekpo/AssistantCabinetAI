# 08 - Plan for lot C (interface, routing, memory), prepared 4 October 2026

Source: the owner's decisions of 4 October 2026 ([04-improvement-plan.md](04-improvement-plan.md), "Owner decisions
of 4 October 2026") and the findings of [03-bug-register.md](03-bug-register.md). Nothing in this file has been
implemented. **Anything that changes what a model reads waits for the owner's approval of the exact change**
(`AGENTS.md`; the 27 September 2026 regression, `docs/SELECTION-AND-MEMORY.md`, "Grounding outranks memory").

Lot C is split in two so that the safe work is not held back by the work that needs approval:

| Part | Branch (stacked on lot B2) | Touches what a model reads? |
| --- | --- | --- |
| **C-a** interface and routing | `fix/lot-c-interface-and-routing` | No |
| **C-b** conversation memory | `fix/lot-c2-history-priority` | **Yes: owner approval of the numbers first** |

C-b goes before lot D, because the mixed tier assembles its turn with the same history function
(`conversation::fit_history`): lot D's tests would otherwise be written against a context that C-b changes.

## C-a - interface and routing (no model-facing change)

### C-a1. Internal names never reach the user (BUG-10), display side only

Owner decision: the name shown must be the one the user knows from the application, and **the identifier in the
code does not change**.

What is shown today and what the user knows (catalogues `fr-FR.json` / `en-US.json`):

| Concept | User-facing name, French | User-facing name, English |
| --- | --- | --- |
| The documents folder | Dossier des documents | Documents folder |
| The data folder | Dossier des données | Data folder |

Audit of what can reach the user through a model (read in the code):

| Internal text a model reads | Where | Seen echoed? | Display name proposed |
| --- | --- | --- | --- |
| `WORK_FOLDER_CONTEXT` (also the contract's `WORK FOLDER KNOWLEDGE CONTRACT`) | `work_folder_context.rs` | Yes, Q8-d | Dossier des documents / Documents folder |
| `Table results` | `mixed_answer.rs`, block label | Yes, Q24 with `ministral-3:3b` ("Table résultats : ...") | Dossier des données / Data folder (or "vos données"; owner to pick) |
| `Document excerpts` | `mixed_answer.rs`, block label | Not yet | Dossier des documents / Documents folder |
| `Schema:` | hidden interpreter (D6) | No: its output is parsed, never shown | none |
| `[extrait N]`, `extrait.` | excerpt labels | Yes (UX-3) | left alone: see C-b |

The deterministic engine and the interface already say "dossier des données" and "dossier des documents"
everywhere (catalogue audit); no engine sentence uses an internal name.

Design:

- One pure function in `src/lib/displayNames.ts`: `localiseInternalNames(text, t)`, applied **at render time** to
  an assistant message, case-insensitive, whole identifier only, tolerant of the variants a model produces
  (`WORK_FOLDER_CONTEXT`, `WORK FOLDER CONTEXT`, `work_folder_context`, `Table results`, `Table résultats`).
- The stored message and the conversation history sent back to the model are **not** changed: the model keeps
  reading its own words, so memory and verification are unaffected.
- The replacement strings are catalogue keys in both languages (`names.documentsFolder`, `names.dataFolder`),
  changed together.
- A guard test lists every upper-case identifier in the model-facing constants and fails when one has no entry in
  the map, so a future block name cannot leak silently (the cheap guard proposed in BUG-10).
- Copy button: copies what is displayed.

**Deviation to record.** `docs/SELECTION-AND-MEMORY.md` and `docs/DECISIONS.md` (session 16, "What is never
rewritten") say the model's draft is never edited. This is a narrow, owner-approved exception: **a presentation-
layer substitution of internal identifiers only**, never a change to a sentence, a number or a citation. It goes
into `docs/DECISIONS.md` with the rule's wording kept for everything else. Known limit: it hides the name but
cannot stop the model from saying "in the documents folder there is nothing about X" when it should simply say the
documents do not carry the answer; that is a wording of the answer, not a leak.

Tests: vitest cases for each variant, a case that leaves ordinary text alone ("work folder" in a sentence), the
guard test; manual: Q8-d ten times with both models, the leak must never show an internal name.

### C-a2. The folder router no longer hijacks a general question (BUG-07, Q28)

`Quel est le délai légal de conservation des dossiers médicaux en France ?` was answered "Aucun document
sélectionné" without a model call, because "quel" (list) and "dossiers" (subject) matched and nothing
disqualified the route (`folder_questions::route`).

Options, in order of preference:

1. **A bound on the unknown words.** A real folder question ("liste les fichiers du dossier 2026") has at most one
   or two words that are neither pack vocabulary nor file names; a sentence with five unrelated words is not one.
   Keep the existing rule (`docs/DECISIONS.md`, "when a question is answered from the folder") and add the bound
   as data in the pack. Lowest risk, testable with negative cases.
2. Treat "dossier médical / patient" as a content subject (profession wording, and the previous strict rule failed
   on ordinary French): rejected.
3. Require the subject to be the grammatical object of the intent: too heavy for this vocabulary approach.

Tests: Q28 as a negative case in `tests/work_folder_inventory.rs`, plus every existing Q1-Q4 question as the
positive safety net, in French and English. Exit: Q28 reaches the model (or, with nothing selected, the
general-knowledge path), Q1-Q4 unchanged.

### C-a3. The view always shows the end of an answer (BUG-08)

Cause is a hypothesis (the `following` ref is cleared by a scroll event fired after the content grew past the 24 px
tolerance, `MessageList.tsx`, `lib/scroll.ts`). Plan: (1) reproduce with a temporary log of scroll events and
`following` transitions, on a deterministic answer and on a slow stream; (2) fix by deciding "following" from user
intent only (wheel, keyboard, scrollbar drag) and re-scrolling after layout with a `ResizeObserver` while the user
had been at the bottom; scroll once when an answer completes; (3) unit-test the decision function in
`lib/scroll.ts`. Exit: the end of the answer is visible in Q3, Q4, Q21, Q27, Q28 and a minute-long stream.

### C-a4. Clickable disambiguation (UX-1)

Design in [05-analysis-notes.md](05-analysis-notes.md) section 5: each candidate in a "which file" answer is a
button that sends a new question with the file name appended, through the existing send path. Q4 and Q9-style
"which workbook" answers become one click, which also removes the retyping the owner met in Q14 with both
workbooks ticked. The question text is built in TypeScript from catalogue strings; tests in vitest.

### C-a5. Small copy and display fixes

- UX-5: one day is shown "date = 2026-01-23", not "date entre X et X".
- BUG-13 remainder: the refusal for a non-numeric column names the column it refused to add (Q24 early form).
- UX-4: the red connection banner stays visible (sticky) when both folder lists are open.
- BUG-18: replay live ("Demander à l'IA" on a computed value) to see the new line.
- BUG-11: live proof with `LLM_REQUEST_TIMEOUT_SECONDS=5` (recipe in [06-retest-lot-a.md](06-retest-lot-a.md), A3).

### C-a replay list

Q8-d x10 (no internal name), Q28 (reaches the model), Q1-Q4 (unchanged), Q3/Q4/Q21/Q27/Q28 (end visible),
Q4 and Q9 (buttons), `Combien de factures le 23/01/2026 ?` ("date = ..."), Q24 with a non-numeric column,
banner with both lists open, BUG-18 and BUG-11 recipes, then the whole B2 replay list of
[07-retest-lot-b.md](07-retest-lot-b.md) to prove no regression.

## C-b - conversation memory yields to the sources (BUG-09), approval first

Owner decision, stated as formal: **the texts of the documents and the data of the tables outrank the memory of
the exchanges; a small model that can remember only one of the two must remember the sources.** This is the rule
already in force since 27 September 2026 ("Grounding outranks memory"); what is missing is a hard limit on how
much memory is sent.

What the code does today (read, not guessed):

- The turn order is already right: the grounding material is folded into the one turn immediately before the
  question, **after** every past exchange (`commands::Writer::write`). Not to be touched.
- `conversation::fit_history` keeps the newest whole exchanges that fit what the sources leave over
  (`ContextBudget::history_chars`: prompt budget minus the fixed turn minus a reserve). With the default 8 192-token
  window that leaves several thousand characters, so a long earlier answer that *sounds like* the answer to the new
  question is still sent, and a 2B model weighs it over the instruction (Q8-e).
- The tier-1 search query adds the previous question when the new one has eight words or fewer
  (`conversation::retrieval_query`): a second place where memory shapes retrieval.

Proposal (for approval; none of it edits a prompt string):

1. **A cap on exchanges, from the published window.** Send at most N past exchanges to a model whose published
   context window is W tokens: for example N = 1 when W <= 4 096, N = 2 when W <= 8 192, no cap above. W is
   already known per alias (`ModelBudgets`), so no model-size detection and no hard-coded model name. The numbers
   are a starting point, to be tuned by the measurement below.
2. **A cap on the size of a remembered answer.** A remembered assistant answer is cut to its first M characters
   (for example 400) when the turn carries sources: the gist survives, the invented detail does not.
3. **Sources first in the budget.** Unchanged in spirit, now explicit: the history only ever receives what the
   sources and the question leave over, under the caps above.
4. Optional, separate approval: a tier-1 numeric safety net like `mixed_answer::verify_numbers`: a figure in the
   answer that appears in none of the excerpts gets an appended notice (Q8-a and Q8-e would both have been
   flagged). It adds a line of interface text, no prompt change.

What I will **not** do without a further explicit yes: change `RETRIEVAL_INSTRUCTION`, `NO_DOCUMENTS_INSTRUCTION`,
`MIXED_INSTRUCTION`, the knowledge contract, the order of the turns, or the label format of the excerpts (UX-3's
`[extrait 2]` markers are a label change and wait for the same approval round).

Measurement protocol (before and after, same fixtures): Q1-Q8 once on both models (regression check, the 27
September lesson), Q8-e and Q8-f ten times each with `gemma2:2b` and `ministral-3:3b`, recording the verdict of
every run in a table in the retest report. "The same model, the same question and the same selection must not
depend on how long the conversation is" is the pass criterion. Per the owner, a failure on a 3B model that
disappears on a larger one is not a product defect, but a *fixed* design must be as good as possible on the smallest
one first ("who can do more can do less").

## Exit criteria of lot C

C-a: no internal name visible in ten Q8-d runs; Q28 reaches the model and Q1-Q4 unchanged; the end of each answer
visible; one-click choices for Q4 and Q9; B2 replay list still green; all suites green.
C-b: Q8-e behaves like Q8-f in ten runs per model; Q1-Q8 unchanged in verdict; suites green.

## Looking ahead: lot D and the router order (owner decision 3)

The owner asked whether, with documents and tables both selected, the router can follow the order in which the
user cites them ("select the patient's information in the table and write an absence certificate using template
Y"). Findings, to be confirmed in lot D:

- Word order is a weak signal ("write a certificate from the table" cites the document first but needs the data
  first). The robust order is **logical**: (1) find the facts in the table (a row lookup, not built yet: lot E),
  (2) retrieve the template or document, (3) the model writes from both. Which named file plays which role is read
  from the file type (spreadsheet or document), not from word order, so no ordering parser is needed.
- "Data only first" is therefore wrong for a question that mixes both (decision 4 of the plan, answered): a
  question with a document reference or a writing intent goes to the mixed tier. This is what the Q24 gate does
  for document references today; lot D generalises it.
- Open point for the owner: a question about the data alone (Q9: "somme des montants") answered instantly and with
  no model when both kinds are ticked (the owner liked this in the lot B replay), or always through the mixed tier
  (the owner's reading of decision 3: ticking both kinds means "cross the information")? The recommendation is the
  first for a question with no document reference and no writing intent, the mixed tier for everything else.

---

## Implementation status of C-a (4 October 2026, branch `fix/lot-c-interface-and-routing`)

Coded and covered by tests (vitest 305, cargo all green, no warning); **awaiting the owner's live replay**.
C-b (memory) is not started: it waits for the owner's approval of the numbers.

| Item | What was done | Tests |
| --- | --- | --- |
| C-a1 internal names | `src/lib/displayNames.ts`, applied at render time and to the copy button of an assistant message; catalogue keys `names.documentsFolder` / `names.dataFolder` in both languages. The stored message and the history are untouched | `displayNames.test.ts`: variants, ordinary text untouched, and a **guard** that fails when an upper-case identifier in the model-facing strings has no display name |
| C-a2 Q28 | `max_unknown_words` (2) in `resources/work-folder-questions/*.json`, read by `folder_questions::folder_answer`: more unrelated words than that and the question is not answered from the inventory | `tests/work_folder_inventory.rs`: Q28 in French and English as negative cases; proven to fail without the bound; all existing positive cases unchanged |
| C-a3 scroll | `lib/scroll.ts::nextFollowing`: the view stops following only for a scroll she caused (wheel, touch, scroll key, pointer within 600 ms); a `ResizeObserver` on each turn follows late growth; one more scroll when an answer finishes. **The cause was a hypothesis and was not reproduced with a log**: this is a design that removes the suspected mechanism, so the replay is the proof | `scroll.test.ts` for the decision function |
| C-a4 UX-1 | `lib/choices.ts` and buttons under a "which file?" answer (documents: ambiguous name; tables: ambiguous name or which workbook). A click replaces her question in place (the edit-and-resend path) with the path written into it | `choices.test.ts`, both languages |
| UX-5 | One day is shown "date = 2026-01-23" | `tabularAnswer.test.ts` |
| BUG-13 remainder | A refusal about a column's content names the column ("Colonne concernée : fournisseur.") | `tests/tabular_hap1.rs`, `tabularAnswer.test.ts` |
| UX-4 | The connection banner is sticky at the bottom of the sidebar | none (CSS); replay |

Not done in C-a: BUG-18 and BUG-11 live proofs (owner replay recipes below).

### C-a replay list (exact steps)

1. **Internal names.** Documents selected without the quote, ask `Quel est le prix de l'imprimante dans le devis MedSupply ?` several times with each model; whenever the answer names where it looked, it must read "Dossier des documents", never `WORK_FOLDER_CONTEXT`. With the quote and the invoices ticked, Q24 with `ministral-3:3b` must not show "Table results" (it shows "Dossier des données" if the model echoes it). The Copy button copies the displayed words.
2. **Q28.** Nothing selected: `Quel est le délai légal de conservation des dossiers médicaux en France ?` must reach the model (answer with the "no documents" notice) and not say "Aucun document sélectionné". Then Q1, Q2, Q3 of the protocol unchanged.
3. **Scroll.** Q3, Q4, Q21, Q27 and a long streamed answer: the end of the answer must be visible without scrolling by hand; scrolling up while an answer streams must keep you where you are; scrolling back to the bottom must resume the follow.
4. **Buttons.** All documents ticked, `Que dit neurologie.pdf ?` (Q4): one button per file; a click replaces the question and answers about that file. Both workbooks ticked, `Combien de lignes ?`: one button per workbook; a click answers for that workbook.
5. **One day.** Invoices only, `Combien de factures le 23/01/2026 ?`: "Compris comme : date = 2026-01-23".
6. **Refused column.** Invoices only, `Quelle est la somme de fournisseur ?`: the refusal names "Colonne concernée : fournisseur."
7. **Banner.** Both folder lists expanded, gateway stopped: the red banner stays visible at the bottom of the sidebar while the sidebar scrolls.
8. **BUG-18 (live proof).** Invoices only, ask `Quelle est la somme des montant ?` (answer: "Somme de montant : 2 215", computed). Under that answer press "Demander à l'IA". Two outcomes are correct: (a) the model proposes nothing usable: the same answer stays and a line says "... a été interrogé, pendant X, mais n'a pas proposé de lecture plus précise de cette question : la réponse calculée ci-dessus est conservée." (b) the model proposes a valid reading: the answer is shown as "interprétée par le modèle". It must never be a silent identical answer.
9. **BUG-11 (live proof, optional).** Start the server with `LLM_REQUEST_TIMEOUT_SECONDS=5`, ask any question with `ministral-3:3b`: the dedicated "trop de temps à démarrer" sentence must appear instead of the generic failure; restore the value afterwards.
10. **No regression.** The B2 replay list in [07-retest-lot-b.md](07-retest-lot-b.md), at least rows 1, 6, 7, 9, 11 and 12.
