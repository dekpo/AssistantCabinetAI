# 03 - Bug register (HAP-1)

Every entry below was found in the human pass of 3 October 2026. **No code was changed to produce this
register.** Root causes were established by reading the code and, where the product shows it, by the
"Compris comme" interpretation line; a cause marked *hypothesis* is not proven.

Severity: **S1** crash or hang; **S2** wrong answer presented as a fact; **S3** misleading or degraded
behaviour; **S4** cosmetic or wording.
Effort: **XS** (a few lines), **S** (a function plus tests), **M** (a design decision plus tests), **L**
(new capability).

| ID | Severity | Effort | Title | Seen in |
| --- | --- | --- | --- | --- |
| BUG-01 | S1 | XS (+S) | Model-written query plan can panic the command; spinner forever, Stop dead | Q18 run 3, S31 |
| BUG-02 | S2 | S-M | Tabular classifier invents filters from incidental words and digits | Q14, Q16, Q19c |
| BUG-03 | S2 | M | The data-only router answers a mixed question with the table half only | Q23 |
| BUG-04 | S3 | XS | "Demander à l'IA" does nothing when documents and tables are both selected | Q23, Q24 |
| BUG-05 | S2 | S | A sum picks the longest column name in the question, a text column | Q24 |
| BUG-06 | S3 | M | Entity linking and the mixed tier are unreachable with natural phrasing | Q23, Q24, Q26 |
| BUG-07 | S2 | S-M | The folder-question router hijacks a general-knowledge question | Q28 |
| BUG-08 | S3 | S-M | The conversation view does not scroll to the end of an answer | Q3, Q4, Q8-d, Q21, Q27, Q28 |
| BUG-09 | S2 | M | Conversation memory overrides the document scope (small models repeat an earlier invented fact) | Q8-e vs Q8-f |
| BUG-10 | S3 | S | The internal block name `WORK_FOLDER_CONTEXT` reaches the user through the model | Q8-d, S15 |
| BUG-11 | S3 | XS | A gateway-to-Ollama timeout is reported as a generic model failure | Q8-b, S11 |
| BUG-12 | S3 | S | The model is consulted for 17 s to 1m25s for questions already settled by the data | Q19, Q20, Q21 |
| BUG-13 | S4 | XS-S | Nudge wording: misleading advice, generic columns, unnamed column, model text shown as the user's | Q19, Q21, Q24 |
| BUG-14 | S3 | L | The query-plan vocabulary has no per-group threshold ("any supplier above N") | Q19 |
| BUG-15 | S4 | S | The engine asks which workbook although only one holds the named column | Q9 |
| BUG-17 | S2 | S | French "plus de N" / "moins de N" never becomes a filter; the unfiltered count is answered (added 4 Oct, lot B retest) | Q19 |
| BUG-18 | S3 | S | "Demander à l'IA" on a computed answer silently returns the same answer when the model cannot do better (added 4 Oct) | Q19 conv 2, 3 |
| BUG-19 | S4 | M | A threshold written in words ("cinq mille") is refused, not read (added 4 Oct, B2 replay) | Q19 variant |
| BUG-20 | S4 | XS | A small model writes the English word "excerpts" in a French answer (added 4 Oct) | Q8-d |
| BUG-21 | S3 | XS | `compose.yaml` did not forward `LLM_REQUEST_TIMEOUT_SECONDS` (added 4 Oct) | BUG-11 proof |
| UX-1 | - | S | Disambiguation choices are not clickable | Q4, Q9 |
| UX-2 | S4 | XS | Error banner overflows the sidebar height | Q26, S43 |
| UX-3 | S4 | XS | Stray `extrait.` and `[extrait 2]` markers in model text | Q8-e, Q8-f |
| UX-5 | S4 | XS | A single day is displayed "date entre X et X" (added 4 Oct) | `Combien de factures le 23/01/2026 ?` |

---

## BUG-01 - A model-written plan can panic the command (S1)

**Evidence.** Q18 run 3 (S31): the question `Combien de factures pour le fournisseur MedSupply ?` asked while
the *appointments* workbook (columns `date`, `salle`, `patient`, `duree_min`) was the only one ticked. The
spinner "Recherche" stayed for ever, **Stop did nothing**, the app had to be closed. Terminal:

```text
thread 'tokio-rt-worker' (28372) panicked at src\tabular\query_plan.rs:489:13:
internal error: entered unreachable code: eq/in are resolved against real data, not reached here
[ELIFECYCLE] Command failed with exit code -1.
```

**Cause (read from the code).**

- `tabular::query_plan::resolve_one_filter` (about line 335) treats a filter with op `eq` or `in` as
  "needs real data". It resolves the value only when the sheet *and* the column exist
  (`find_column`). Otherwise it falls through the comment "No sheet, or no such column: pass the raw
  value through so `engine::execute` can refuse", and calls `comparison_from(filter.op, ...)`.
- `comparison_from` (line 481) starts with
  `PlanFilterOp::Eq | PlanFilterOp::In => unreachable!("eq/in are resolved against real data, not reached here")`.
  So the very case the comment prepares for (a column the model invented, here `fournisseur` in a workbook
  that has no such column) ends in a panic. The model's plan was `{"column": "fournisseur", "op": "eq", "value": "MedSupply"}`.
- The panic happens inside the Tauri async command's task. The command future never completes, so the
  webview's `invoke` never resolves. `cancel_chat` only flips a flag that `until_stopped` reads from inside the
  same future, which no longer exists, hence Stop cannot end it.
- It is non-deterministic: the first, accidental run of the same question (rdv-mars ticked) got a harmless
  answer from `gemma2:2b`; a different plan from the model hit the panic. Any model can produce it.

**Impact.** The worst class of defect for a professional tool: the application freezes on a plausible
question and a wrong model output. Reachable in production with every model.

**Candidate fixes (not applied).**

1. Replace the `unreachable!` path: for `eq`/`in` with no resolvable column, return the plan as
   `PlanResolution::Unsupported` (or build `Comparison::Equals` and let `engine::execute` refuse with
   `ColumnNotFound`, as the comment already intends). Add a unit test with exactly this plan.
2. Defence in depth: run the model-assisted path so that a panic becomes an error result (for example by
   isolating the task and mapping a join error to a machine code), and make the webview time out a command
   that returns nothing.
3. Audit the crate for other `unreachable!`, `unwrap()` and `expect()` on paths fed by model output
   (`query_plan.rs`, `tabular_answer.rs`, `mixed_answer.rs`).

**Re-test.** Q18 with rdv-mars ticked on both models; a replay of the plan above against each workbook.

---

## BUG-02 - The classifier invents filters from incidental words and digits (S2)

**Evidence.**

- Q14 (S26): `Combien de rendez-vous entre le 09/03/2026 et le 15/03/2026 ?` -> 0, interpreted as
  `date entre 2026-03-09 et 2026-03-15 et date en 2026 et duree_min = 15 et date en 2026`. Truth: 4.
- Q16 (S28): `Quelle salle a le moins de duree_min ?` -> `Salle 2, 30`, interpreted as `patient = Martin A.`
  Truth: Salle 3, 50.
- Q19 run 3 (S35): `... plus de 5000 euros ...` -> `Nombre de lignes : 0 ... Compris comme : date en 5000.`

**Cause.** `tabular_answer::detect_filters` (line 1060) walks every residual word (a word of the question not
consumed by the recognised operation) and tries to anchor it on data:

1. *Equality against any cell word.* The loop (about line 1226) folds the word and looks it up in
   `column_value_words`, which indexes every word of every cell of every non-date column, **including
   numeric columns and one-letter words**. The French verb "a" equals the word "A" of the cell
   `Martin A` (Q16). The number 15 equals the cell `15` of `duree_min` (Q14).
2. *The date-range block marks only the word "entre" as consumed*, not the digit tokens of the two dates.
   The six tokens `09 03 2026 15 03 2026` stay residual and are then re-read: `2026` becomes a `Year` filter
   (twice, no de-duplication for year filters), `15` becomes `duree_min = 15`.
3. *Any four-digit number is a year* (line 1294 `word.chars().count() == 4` and `parse::<i32>` in
   1000..=9999): 5000 becomes year 5000 (Q19c).
4. All filters are combined with AND, so one spurious filter empties the result or changes the group.

The "Compris comme" line is the product's own audit trail and it shows each error, but the first line of
the answer ("Nombre de lignes : 0", "Salle 2, avec 30") is stated as a computed fact.

**Impact.** Silent wrong numbers from the deterministic engine, the most trusted part of the product. The
reference suite (106 cases) did not catch them because none of its fixtures has a cell word equal to a
common French word, and none combines a numeric cell value with a date range.

**Candidate fixes (not applied).**

- Mark the tokens consumed by a date or a date range as consumed; do the same for tokens that formed a
  comparison number.
- Ignore stop-words and one- or two-character tokens in the equality scan; add `a`, `est`, `de`, ... to the
  pack's filler list (data, per the language contract: `tabular-questions/fr-FR.json`, which sessions 9-11
  say is not to be widened as a side effect; this is a deliberate decision to take).
- Do not try equality against numeric columns unless the question names that column or uses an explicit
  comparison; accept a year only inside a plausible range or inside the date column's observed range.
- De-duplicate identical filters.
- Product guard: when the interpreted filters leave **0 rows**, say so as a refusal ("no row matches these
  conditions: ...") instead of presenting "0" as the answer.

**Re-test.** Q14, Q16, Q19c; add rows to `tabular_reference_cases.json` for each (an appointment fixture
with a patient named like a common word, a date range, a numeric cell equal to a day number).

---

## BUG-03 - The data-only router answers a mixed question with the table half only (S2)

**Evidence.** Q23 (S40): a comparison between the table and "le devis signé" is answered with the table sum
(1 450) and the line "les documents sélectionnés n'étaient pas nécessaires". The quote says 1 200,00 HT.

**Cause.** `commands::sourced_answer`, `GroundingTier::DocumentsAndTables` arm (line 719), always calls
`mixed_data_only_tier` first. `tabular_answer::prepare_if_data_only` (line 236) returns `Some` for any
question for which `prepare` produced `PendingAnswer::Done`. The "nothing left over" precondition of
session 15 only rejects a *capitalised unmatched residual word* (the `FilterDetection::ValueNotFound`
path). Words such as `correspond-il`, `indiqué`, `devis`, `signé` are lowercase and not filter-like, so
they are ignored: the classifier recognised "montant" + "MedSupply" and the rest of the sentence is silently
dropped. The product then claims the documents were not needed.

**Impact.** The central promise of session 15 ("only when the question is clearly and only about the
data") is not enforced; the most natural mixed questions (comparisons with a document) are answered as if
they were data-only.

**Candidate fixes (not applied).**

- Tighten the precondition: any residual word that is not filler and not consumed (lowercase included), or
  any word that retrieval knows (the folder router already uses a corpus-word port for this,
  `folder_questions::CorpusWords`), sends the question to the mixed tier.
- Add document-reference vocabulary to the pack as *disqualifiers* of the data-only route
  (`devis`, `contrat`, `lettre`, `courrier`, "dans le document"), the same denylist idea the folder router
  already uses for "read this to me" words.
- Or invert the default: with both kinds selected, always go to the mixed tier and let the table half answer
  alone with no gateway call when the document half has no evidence (the partial-answer path already exists).

**Re-test.** Q22 (must still be data-only, zero gateway call), Q23, Q24, Q26. Session 15's
`tests/mixed_selection_routing.rs` must stay green or be updated with an owner decision.

---

## BUG-04 - "Demander à l'IA" is a dead button on mixed selections (S3)

**Evidence.** Q23, Q24 (S40-S42): clicking it "blinks" the page and shows the same deterministic answer.

**Cause.** `ask_with_sources` receives `skip_deterministic`; `sourced_answer` destructures it (line 706),
but the `DocumentsAndTables` arm (line 719) does not read it: `mixed_data_only_tier(...)` is called with no
force flag and nothing branches to `mixed_tier`. The interface offers the button because the answer is a
tabular value or nudge (`tabularRetryable`), and tier 2 honours it (`tabular_tier(.., skip_deterministic)`).

**Candidate fix (not applied).** When `skip_deterministic` is true, skip `mixed_data_only_tier` and call
`mixed_tier` (or `tabular_answer::prepare(.., force_model: true)`), mirroring tier 2. A one-line branch plus a
test; it also gives the user an escape from BUG-03 and BUG-05 until they are fixed.

---

## BUG-05 - A sum picks the longest column named in the question (S2)

**Evidence.** Q24 (S42, S43): `Quel est le montant total pour le fournisseur ...` -> "Cette colonne ... ne
contient pas que des nombres : elle ne peut être ni additionnée ni comparée."

**Cause.** `tabular::question::find_column_name` (line 945) collects every column whose folded name occurs in
the question, sorts them by name length descending and keeps the **longest** (`fournisseur`, 11 letters,
beats `montant`, 7). `detect_operation` then builds `Operation::Sum { column: "fournisseur" }`, which the
engine correctly refuses as non-numeric. The same rule would pick the wrong column for any "total
`<text column>` ... `<numeric column>`" question.

**Candidate fix (not applied).** For sum, mean, median, min, max, largest row: when several columns match,
prefer the single numeric one; ask only if several numeric columns match. Name the refused column in the
nudge (BUG-13).

**Re-test.** Q24; add a reference case with two matched columns of different types.

---

## BUG-06 - The mixed tier and entity linking are unreachable with natural phrasing (S3)

**Evidence.** Of the four mixed-selection questions asked (Q23, Q24, Q26, and the earlier "publipostage"
question), only the publipostage one reached `mixed_answer::answer` (and it was wrongly classified
`not_asked_about`, see `docs/SESSION-DATA-17-Publipostage-Issue.md`, local file). Q23, Q24 and Q26 were all
consumed by the data-only router (BUG-03/05). The documented behaviours "correction appended", "document
part could not be written" and "entity linked from the excerpt" were therefore not observable.

**Cause.** Three gates in front of `mixed_answer::answer`: (1) the data-only router above; (2) in
`resolve_table`, entity linking runs only for `PendingAnswer::TryModel` with a recognised operation, i.e.
only after a *capitalised* residual word failed to match (`ValueNotFound`); "le fournisseur mentionné dans cette
lettre" has no such word, so the sum would run **unfiltered**; (3) a `NotRecognised` question is short-circuited to
`NotAskedAbout` without trying gap G (the publipostage root cause).

**Candidate fixes (not applied).** Fix BUG-03 and BUG-05 first; add referential phrases ("mentionné dans",
"indiqué dans", "cité dans cette lettre", "du document") as a trigger for entity linking; drop the
`NotAskedAbout` shortcut for unrecognised questions (publipostage issue, direction 1). Then re-run Q23, Q24,
Q26 and add them as automated tests through the real classifier (the current `tests/mixed_answer.rs`
bypasses the router, which is how this gap went unseen).

---

## BUG-07 - The folder router hijacks a general-knowledge question (S2)

**Evidence.** Q28 (S48-S51): both models, `Quel est le délai légal de conservation des dossiers médicaux en
France ?` -> `Aucun document sélectionné pour cette conversation.`, no model call.

**Cause.** `folder_questions::route` (line 231) decides by pack vocabulary: "quel" is a *list* operation word,
"dossiers" is a *folders* subject word, and the other words (`délai`, `légal`, `conservation`, `médicaux`,
`France`) are neither content words nor words of the indexed corpus, so they do not disqualify the
deterministic route (the rule of `docs/DECISIONS.md`, "when a question is answered from the folder": intent +
subject, no content word, no unknown word that the documents contain). With nothing selected,
`route_in_selection` then turns any deterministic folder answer into `NothingSelected`.

**Candidate fixes (not applied).** Limit the route to short questions or to few unknown words (a real folder
question rarely has five unrelated words); treat "dossier médical/patient" as a content subject; require the
subject word to be the grammatical object of the intent ("liste des dossiers") rather than anywhere in the
sentence. Trade-off recorded in `docs/DECISIONS.md` (the earlier strict "every word in the pack" rule failed on
ordinary French), so this needs the reference questions of `tests/work_folder_inventory.rs` as a safety net.

**Re-test.** Q28 and all of Q1-Q4; add the HAP-1 question as a negative case.

---

## BUG-08 - The view does not scroll to the end of an answer (S3)

**Evidence.** Cut-off answers in S03, S05, S12, S14, S37, S46, S48, S50 (the arrow "go to latest message" is
visible in several): deterministic answers (instant) and slow streamed answers (minutes).

**Cause (hypothesis, not proven).** `MessageList.tsx` (lines 77-96): an effect on `entries` scrolls to the
bottom for a new turn, or while the ref `following` is true. `following` is updated **only** by the `scroll`
event (`onScroll`). When the answer arrives (one large block for a deterministic answer; footer lines such
as "Généré par ... en ...", Sources and action buttons added by later renders for a model answer) the
container grows by more than the 24 px tolerance (`BOTTOM_TOLERANCE_PX` in `lib/scroll.ts`). A scroll
event dispatched after that growth, including the one caused by the programmatic `scrollIntoView` of the
placeholder turn, evaluates "not at the bottom", sets `following` to false, and the effect then skips the
scroll for every following render. Slow streams and the extra footer renders make the window long.

**Candidate fixes (not applied).** Decide "following" from user intent only (wheel, keyboard, pointer on
the scrollbar) rather than from every scroll event; or re-scroll after layout using a `ResizeObserver`
on the message list while `following` was true at the *previous* frame; always scroll once when an
answer completes. Reproduce first with a log of scroll events and `following` transitions.

---

## BUG-09 - Conversation memory overrides the document scope (S2)

**Evidence.** Q8-e (S16) versus Q8-f (S17): same model (`gemma2:2b`), same selection (quote unticked), same
question; with the earlier wrong answer in memory the model repeats "1 000,00 euros HT" and adds `[extrait 2]`; with
the conversation cleared it correctly says no document mentions it.

**Cause.** `lib/history.ts::conversationHistory` sends every answered question/answer pair of the conversation,
whatever selection was active when it was answered; Rust then keeps what fits the model's budget
(`conversation::fit_history`, newest first). A 2B model weighs a previous assistant turn that *sounds like the
requested answer* more than the instruction to answer only from the excerpts (`retrieval::RETRIEVAL_INSTRUCTION`),
which sits in the last turn, exactly as `docs/SELECTION-AND-MEMORY.md` warns. The first wrong figure (Q8-a)
was itself a hallucination, and memory then made it sticky.

**Candidate mitigations (not applied; any prompt change needs the owner's approval, see 05-analysis-notes.md).**

- Tag each remembered exchange with the selection it was answered under and drop, or demote to a short
  summary, the exchanges answered under a different selection.
- Send fewer exchanges (for example one) to models of 3B parameters or less.
- Do not remember answers that carried a document citation when the next question has a different
  selection.
- A numeric safety net for tier 1 similar to `mixed_answer::verify_numbers`: a figure in the answer that
  appears in none of the excerpts gets an appended notice. Q8-a, Q8-e would both have been flagged.

---

## BUG-10 - `WORK_FOLDER_CONTEXT` reaches the user (S3)

**Evidence.** Q8-d (S15): `Aucun document dans le WORK_FOLDER_CONTEXT ne mentionne le prix de l'imprimante MedSupply.`

**Cause.** `work_folder_context::build_system_turn` (called for every tier-1 question) sends the retrieval
instruction, the `WORK_FOLDER_KNOWLEDGE_CONTRACT` and the block rendered by `to_prompt_block()`, which starts
with the literal header `WORK_FOLDER_CONTEXT` (`work_folder_context.rs` line 118). The contract itself names the
block ("The WORK_FOLDER_CONTEXT block below is authoritative for filesystem facts", line 174). A model asked "where
did you look?" repeats the only name it was given. It is not a security issue (the block holds counts, relative
paths and states, no text, no hash).

**Candidate fixes (not applied, owner approval required).**

1. Rename the header and the contract's reference to a neutral, user-meaningful term ("the documents folder
   facts"); lowest risk, but any label can be echoed.
2. Send the block only when the question needs filesystem facts (a named file, a count, a per-document
   question); a plain content question such as Q8 does not need it, which also shortens the prompt (the
   2B model's 1m22s-1m52s).
3. Add one short sentence "never mention these block names" (the previous experience with long rule lists, 27
   September 2026, argues against it).
4. Not recommended: rewriting the answer afterwards (`docs/SELECTION-AND-MEMORY.md` forbids editing what the
   model wrote).

The same family of leaks should be audited for the Data folder: today tier 2 sends no such block, the mixed tier
sends the labels `Document excerpts` / `Table results` (user-meaningful), and the hidden interpreter sends a
`Schema:` JSON that the user never sees. A grep of model-facing constants for upper-case identifiers is a cheap
guard test.

**Re-test.** Q8-d with both models, repeated ten times (a leak is probabilistic); then the whole Q1-Q8 block to
detect a hallucination regression.

---

## BUG-11 - A gateway timeout looks like a model failure (S3)

**Evidence.** Q8-b (S11): `ministral-3:3b` right after a switch from `gemma2:2b`:
"Cela n'a pas fonctionné. Le modèle n'a pas répondu correctement. Réessayez." Third attempt, same model
and question, worked in 3m05s.

**Cause.** The message is the catalogue text for the gateway code `provider_error`. In
`apps/server/.../providers/ollama.py` an `httpx.TimeoutException` becomes
`GatewayError(provider_error, 504, data={"reason": "timeout"})`; the client is created with
`timeout=httpx.Timeout(request_timeout_seconds, connect=10.0)`, default **180 s**
(`LLM_REQUEST_TIMEOUT_SECONDS`, `core/config.py` line 23). The first request after a model switch makes Ollama
unload one model, load the other and process the prompt before the first token; on this CPU that exceeded 180 s
of silence. The owner's intuition (a timeout) is right, but the setting that failed is the **server's**, not the
client's `answerIdleTimeoutSeconds` (600 s was already in force for the third run; it did not matter for b).

**Candidate fixes (not applied).** Show a message that says what happened ("the model took too long to
start; try again, it is loaded now"); keep the `reason` in the error data and add a catalogue entry per
reason; document `LLM_REQUEST_TIMEOUT_SECONDS` for CPU deployments in `docs/OPERATIONS.md`; consider warming
the chosen model when the user picks it (Ollama `keep_alive`/an empty generate).

---

## BUG-12 - Useless model time on settled questions (S3)

**Evidence.** Q20 (40 s), Q21 (17 s and 1m12s), Q19 (40 s and 1m25s) each ended in the same nudge a
model-free run gives, after a long "Recherche".

**Cause.** Gap G (`tabular_answer::answer_sync`): an unrecognised question, or a residual word that matches
no value, always tries the model before the nudge. For a *named sheet that does not exist* (Q20) and a value
with no close match (Q21) the engine already knows the answer is a refusal. The product does say it asked
the model and how long it took (good), but the user waits first.

**Candidate fixes (not applied).** Skip the model when the question names a sheet or column the workbook
lacks; when the model is tried, show "interpretation in progress" with a working Stop (BUG-01); consider
making the model attempt opt-in through the existing "Demander à l'IA" button for the cases above.

---

## BUG-13 - Nudge wording (S4)

**Evidence and fixes (all copy, in the React catalogues; Rust only supplies the data).**

1. After a missing *value* (Q21) the second time: "nommez exactement l'une des colonnes ci-dessus", wrong
   advice; and it refers to "votre question précédente", which was a different question.
2. The generic "Ses colonnes sont : date, fournisseur, montant" is shown to a user who just named `fournisseur`
   (Q19 run 2): the product cannot say why it failed.
3. The non-numeric refusal (Q24) does not name the column it refused to add.
4. Q19 run 1: the quoted « liste des fournisseurs » is the value written by the model's plan, displayed as if
   the user had typed it; show the user's words, or say "the interpretation proposed by the model".

---

## BUG-14 - No per-group threshold in the query-plan vocabulary (S3, capability)

**Evidence.** Q19 in every form. "Has any single supplier exceeded 5 000?" is a group total compared with a
number (HAVING). `QueryPlan` (`filters`, `group_by`, `aggregate`, `sort`, `limit`) and
`query_plan::build_operation` can express "total per supplier, largest first", but not "keep groups whose total is
over N". With `cinq mille` no plan exists; with `5000` the classifier misreads the number (BUG-02).

**Candidate directions.** (a) accept the limit and make the refusal honest and useful ("I can list the totals per
supplier; compare them with 5 000 yourself" with a button that asks Q12); (b) add a `having` clause to the plan and
the engine (`Operation::GroupSum` + a threshold) behind tests; (c) answer it as two engine steps
(largest group, then compare). Effort L for (b).

---

## BUG-15 - Which workbook? (S4)

**Evidence.** Q9 (S18): both workbooks ticked, `Quelle est la somme des montant ?`: the engine asks which one.
Only `factures-fournisseurs-2026.xlsx` has a column `montant`.

**Candidate fix.** In `tabular_answer::pick_target`, when the question names no file but names a column, keep the
selected workbooks that have that column; ask only if several remain. Keep asking when none or several do.

---

## BUG-16 (added 4 October 2026) - Mixed answer prose is unchecked and the computed figure is hidden (S2)

Found in the lot A retest ([06-retest-lot-a.md](06-retest-lot-a.md), A2): `gemma2:2b` answered "the total
billed corresponds to the quote" (table 1 450, quote 1 200,00 HT) in two of four runs. The table figure is kept
in the answer data but not displayed beside the model's prose, and numeric verification flags only a number that
contradicts the table. Candidate fixes in 06. Related: OBS-7 (a small model can answer in English), UX-4 (pin the
connection banner), and BUG-13's remaining items (Q24 column name, "Quel salle").

## BUG-17 (added 4 October 2026) - French "plus de N" / "moins de N" never becomes a filter (S2)

Found in the lot B retest ([07-retest-lot-b.md](07-retest-lot-b.md), Q19). `Est-ce qu'on a dépensé plus de 5000 euros
avec un seul fournisseur ce trimestre ?` and, in isolation, `Les factures de plus de 500 euros ?` both return
`Count(8)` (every row) with `filters: []`.

**Cause (reproduced by running the engine, read in the code).**

- `tabular/question.rs`, `detect_operation`: "plus"/"moins" followed by a number is read as an implicit `Count`,
  "with the comparison itself resolved afterward as a filter" by `tabular_answer::detect_filters`.
- `residual_words` removes every word of the pack before `detect_filters` runs, and "plus"/"moins" are *also*
  superlative vocabulary (`groups.most`, `groups.least`), so they never reach the residual. `detect_filters`
  looks for its comparison words only in the residual, so no `GreaterThan`/`LessThan` is built, and the number is
  silently dropped.
- With nothing left over, no model escalation fires either: the unfiltered count is a finished answer.
- English works because "over" is not group vocabulary. French "supérieur(e)(s)" works; "plus de" and "moins de",
  the usual phrasing, never have since session 11. Lot B did not cause it: removing the invented year (BUG-02)
  only removed the line that used to betray it.

**Candidate fix.** In `detect_filters`, look for a comparison word in the *question's own tokens* rather than the
residual, accepting only the shape "comparison word, optional filler or one-letter connector, number"
("plus de 500", "plus que 500", "supérieur à 500"), so a superlative ("le plus de montant en 2026") is never read
as a threshold; then consume the number from the residual. If a comparison word and a number are present and no
numeric column can be targeted, refuse or escalate rather than answer unfiltered. Tests in French and English,
plus a check that "Quelle salle a le plus de duree_min ?" stays a group ranking.

## BUG-18 (added 4 October 2026) - A forced model attempt that fails leaves no trace (S3)

Q19 conversations 2 and 3 (the "Demander à l'IA" button with `gemma2:2b` and `ministral-3:3b`) show the exact
text of conversation 1. `answer_sync` with `force_model` returns `PendingAnswer::TryModel { fallback: computed }`;
when the model's plan is unusable `resolve` returns the fallback, and `with_model_attempt` decorates only a
`Nudge` (it carries `model_attempt`). A computed `Value` carries no such field, so the user cannot tell whether
the model was asked, failed or was ignored. Candidate fix: carry the attempt (model alias, duration) on a
`Value` as well and render a one-line note ("Le modèle n'a pas pu proposer une lecture plus précise ; la
réponse calculée est conservée"), user-visible text only, no model-facing change. Related: BUG-12.

## OBS-8 (added 4 October 2026) - The structural route ignores the rest of the question

`Que contient la colonne détails de la facture de 2026-01-22 ?` is answered "Colonnes de la feuille Factures :
date, fournisseur, montant." The words "contient"/"colonne" route it to the structural path, which never looks at
the leftover words ("détails", the date). Coherent here, but the same family as BUG-17 (part of the sentence
dropped without saying so). It matters for lot D, whose data-only precondition assumes that a structural answer
means "clearly and only about the data".

**Status 4 October 2026 (lot B2, replayed by the owner, see 07):** BUG-17 closed and confirmed live; the Q19
group-threshold refusal confirmed live; BUG-12 closed for a value with no close match (Alfa, live) and open for a
missing sheet; BUG-18 fixed in code, not yet seen live.

## BUG-19 (added 4 October 2026) - A threshold written in words is not read (S4)

`Est-ce qu'on a dépensé plus de cinq mille euros avec un seul fournisseur ... ?` is refused ("Je ne peux pas
répondre directement ...") with both models: the classifier reads digits only and the model plan cannot express it
(OBS-6). The refusal is honest. A fix would parse number words in the locale pack (the pack already has words
for one to ten, `numbers`); nothing in lots C, D or E depends on it.

## UX-5 (added 4 October 2026) - One day shown as a range

A single written date is filtered as the range of that day and displayed "date entre 2026-01-23 et 2026-01-23".
Copy-only fix: when the two ends are equal, show "date = 2026-01-23" (a new catalogue key in both languages).

## UX-1 - Clickable disambiguation (proposal)

See 05-analysis-notes.md section 5 for the recommended design: render each candidate as a button that sends a
new question with the file name appended ("Que dit 2026/mars/neurologie.pdf ?"), reusing the existing send path.

## UX-2 - Banner overflow

With both folder lists expanded, the red connection banner under the Data folder card makes the sidebar taller
than the window and a second page scrollbar appears (S43). Cosmetic: make the sidebar the scroll container or
anchor the banner.

## UX-3 - Stray markers in model text

`extrait.` at the end of a refusal (S17), `[extrait 2]` / `extrait[1]:` prefixes (S07, S16): the model copies the
excerpt labels the product gave it. Fixing the label format in the evidence block is a model-facing change
(owner approval).

---

## Model behaviour observations (not code defects)

| Id | Observation | Where |
| --- | --- | --- |
| OBS-1 | `gemma2:2b` returns a pasted quotation instead of an explanation | Q5 |
| OBS-2 | `gemma2:2b` adds a claim absent from the document ("durée du bail") and drops two facts | Q6 |
| OBS-3 | `gemma2:2b` writes a figure absent from the document even with the right excerpt in context | Q8-a |
| OBS-4 | `gemma2:2b` answers a drafting request by asking for a document: it applied the "needs a specific document" sentence of the no-documents instruction to a request that needs none | Q27 |
| OBS-5 | `ministral-3:3b` is more accurate and follows the contract better, at 1.5x to 3x the latency on this CPU | Q8-c, Q27 |
| OBS-6 | Neither model could translate Q19 into an executable plan | Q19 |

## Test-design issues (not product defects)

- Q20: the fixtures have no `Calculs` sheet, no `cumul` column and no formula; the formula-refusal scenario
  needs a formula workbook.
- Q21: the dataset has no "Alpha", so the close-value suggestion could not be exercised.
- Q22 was not run; Q25 is not a user action; Q26 reused the Q24 phrasing and could not reach the mixed tier.
- The owner's screenshot numbers shifted by up to two between Q10 and Q18 (see `screenshots/INDEX.md`).

---

## Status after lot C-a (4 October 2026, coded, awaiting live replay)

BUG-07, BUG-08 (design fix, cause not reproduced), BUG-10 (display side), BUG-13 remainder, UX-1, UX-4 and UX-5
are implemented on `fix/lot-c-interface-and-routing`; see [08-lot-c-plan.md](08-lot-c-plan.md) for the change per
item and the replay steps. BUG-09 (memory) waits for the owner's approval of the C-b numbers; BUG-18 and BUG-11
await a live proof.

## Added after the lot C-a replay (4 October 2026)

See [09-retest-lot-c-a.md](09-retest-lot-c-a.md): BUG-20 (English "excerpts" shown to a French reader, fixed on the
display side), BUG-21 (the timeout variable was never forwarded by Compose, fixed), OBS-9 to OBS-11. BUG-07, UX-1,
UX-4, UX-5 and the BUG-13 remainder are closed; BUG-09 has its C-b change coded and awaits the measurement.

## Added after the C-b measurement (5 October 2026)

See "C-b measurement results" in [09-retest-lot-c-a.md](09-retest-lot-c-a.md): OBS-12 (better after re-indexing, stale
index suspected), OBS-13, OBS-14, and the hypothesis BUG-22 (evidence not sized to the model's window; to confirm in
the Ollama logs). BUG-11 and BUG-21 are closed.

## Status after lot D (5 October 2026, coded, awaiting live replay)

BUG-03 and BUG-06 (the data-only router swallowing mixed questions; entity linking unreachable for a question that
points at a document) and BUG-16 (the computed figure hidden beside the model's prose) are implemented on
`fix/lot-d-mixed-routing`: see [10-lot-d-mixed-routing.md](10-lot-d-mixed-routing.md). OBS-8 and the publipostage
message are deferred (lot E needs the row lookup first).
