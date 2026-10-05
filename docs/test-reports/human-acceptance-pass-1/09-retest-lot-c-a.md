# 09 - Retest after lot C-a, and the C-b change (4 October 2026)

Lot C-a of [08-lot-c-plan.md](08-lot-c-plan.md), branch `fix/lot-c-interface-and-routing` (committed and pushed by
the owner). The owner replayed the C-a list on the real stack with `gemma2:2b`, `ministral-3:3b`, `llama3.2:3b`
and, for one question, `medgemma:4b`. Every answer below was checked against the fixtures
([fixtures/README.md](fixtures/README.md)) or the code. French sentences are test data.

## Verdicts

| # | Check | Verdict | What was seen |
| --- | --- | --- | --- |
| 1 | Q8-d, quote unticked, `gemma2:2b` | **PASS-WITH-ISSUES** | No invented price, but the answer is a pasted unrelated excerpt ("Extrait de [4] : CABINET DE NEUROLOGIE ...") followed by "Je n'arrive pas à répondre à votre question." (OBS-9: the pasted-quotation habit of OBS-1) |
| 2 | Q8-d, `ministral-3:3b` | **PASS-WITH-ISSUES** | Correct refusal ("Aucun document dans les excerpts ne mentionne un devis de MedSupply ..."), but the English word "excerpts" is shown to a French reader (BUG-20) |
| 3 | Q8-d, `llama3.2:3b` | **PASS-WITH-ISSUES** | Correct refusal, same English word (BUG-20) |
| 4 | Q28, nothing selected, four models | **PASS** (routing) | The question reaches the model on all four, with the line "Réponse sans vos documents." and "Généré par ... en N s" (30 s, 1m6s, 42 s, 1m34s). It no longer says "Aucun document sélectionné". The factual content is the models' own (see OBS-10) |
| 5 | Q1, Q3 | **PASS** | 6 files; the list of 6 with their state, unchanged |
| 6 | Scroll: Q3, Q4, Q21, Q27 | **PASS** | The end of the answer is visible. Not reported: scrolling up during a stream, and a minute-long stream (only the deterministic and short answers were observed) |
| 7 | Buttons, Q4 `Que dit neurologie.pdf ?` | **PASS** | One button per file; a click edits the original question in place and answers about that file |
| 8 | Buttons, `Combien de lignes ?` with both workbooks | **PASS** | One button per workbook; a click runs the deterministic engine |
| 9 | One day, `Combien de factures le 23/01/2026 ?` | **PASS** | "date = 2026-01-23" |
| 10 | Refused column, `Quelle est la somme de fournisseur ?` | **PASS (reported)** | Reported as working; no transcript pasted |
| 11 | Banner always visible | **PASS** | |
| 12 | BUG-18, "Demander à l'IA" on `Quelle est la somme des montant ?` | **PASS, outcome (b)** | With `llama3.2:3b` (50 s), `gemma2:2b` (38 s) and `ministral-3:3b` (1m21s) the model produced a valid reading: "Somme de montant : 2 215. Calculé sur 8 lignes. La question elle-même a été comprise par le modèle avant que le moteur ne calcule cette valeur ..." with the "Généré par" line. The figure is the engine's (2 215, correct). Outcome (a), the new "answer kept" line, was **not observed** (it is unit-tested only) |
| 13 | BUG-11 live proof | **NOT REPRODUCED** | The owner could not find where to set `LLM_REQUEST_TIMEOUT_SECONDS`. Cause found: BUG-21 below |
| 14 | `WORK_FOLDER_CONTEXT` display name | **NOT EXERCISED** | No model echoed it in these runs; covered by unit tests and the guard only |

No regression found in the rows replayed. The other rows of the B2 list were not replayed in this pass.

### Q28, the four answers

Question: `Quel est le délai légal de conservation des dossiers médicaux en France ?` (nothing selected).

| Model | Answer | Time |
| --- | --- | --- |
| `gemma2:2b` | "... est de 10 ans." | 30 s |
| `ministral-3:3b` | A three-case list: current records 10 years after the last consultation, specific records 30 years, deceased patients 10 years; "Vérifiez les règles locales ..." | 1m6s |
| `llama3.2:3b` | "Je ne connais pas la loi spécifique ... consulter une source officielle" | 42 s |
| `medgemma:4b` | "... est de 10 ans." | 1m34s |

The four answers disagree with one another. To the agent's knowledge the period usually cited for medical records
is 20 years, which none of them states (to be verified against the Code de la santé publique before anyone relies
on it). The honest answer here is the one of `llama3.2:3b`. This is the model's own knowledge, not the product's: it
is exactly what the line "Réponse sans vos documents." is there to say.

## Owner direction recorded (not for the current sprint)

Letting the models answer from their own knowledge when nothing is selected is **useful and must stay**: it shows
which knowledge is already in the models. The owner plans to connect the models later to other sources (APIs of legal
and regulatory texts, lists of medicines, various reference resources). Not in scope now; recorded in
`docs/PLATFORM-VISION.md`. The Q28 comparison above is the evidence that unsourced answers on a legal period are not
reliable across models, which is the argument for those future sources.

## New findings

| ID | Severity | Effort | Title |
| --- | --- | --- | --- |
| BUG-20 | S4 | XS | A small model writes the English word "excerpts" in a French answer ("dans les excerpts fournis"); the user does not know the word. **Fixed in the C-b change, display side**: "excerpt" is shown "extrait" and "excerpts" "extraits" in the French interface (capital kept), English untouched, stored text and history untouched. The owner asked for this and for it to be done on output only; it cannot affect answer quality because the model never sees the replacement |
| BUG-21 | S3 | XS | `compose.yaml` did not forward `LLM_REQUEST_TIMEOUT_SECONDS` to the server container, so the variable could not be set from `.env` at all (the setting is read by the gateway, `core/config.py`). **Fixed**: forwarded with the default 180, listed in `.env.example`, `docs/OPERATIONS.md` corrected (it told operators to "raise the value in the server environment" without a way to do it). The BUG-11 proof can now be run, see below |
| OBS-9 | - | - | `gemma2:2b` answers a refusal by pasting an unrelated excerpt (OBS-1 family) |
| OBS-10 | - | - | The no-documents answers of four models disagree on a legal period; honest answer only from `llama3.2:3b` |
| OBS-11 | - | - | `llama3.2:3b` is efficient on this machine (owner remark) and the most honest on Q28; to be tried on more questions at the end of the fixes (decision 4 of 4 October) |

## BUG-11 live proof, now possible

1. In the `.env` file next to `compose.yaml`, add the line `LLM_REQUEST_TIMEOUT_SECONDS=5`.
2. Run `docker compose up -d server` (the container is recreated with the new value).
3. In the desktop, ask any question that reaches a model, preferably right after changing model (for example with
   `ministral-3:3b`).
4. Expected: the sentence "Le modèle a mis trop de temps à démarrer ..." instead of the generic failure.
5. Put the value back to `180` (or delete the line) and run `docker compose up -d server` again.

## C-b: conversation memory yields to the sources (coded 4 October 2026)

The owner approved the starting numbers: **one past exchange** for a published context window of 4 096 tokens or
less, **two** up to 8 192, no cap above, and a remembered answer cut to **400 characters**, both only for a turn
that carries sources.

| Item | Change |
| --- | --- |
| `conversation::max_exchanges_with_sources` | window <= 4 096 -> 1, <= 8 192 -> 2, otherwise no cap. The window is already published per alias; no model name is used |
| `conversation::history_beside_sources` | keeps the newest N exchanges, cuts each remembered answer to 400 characters at a word boundary and marks the cut with "...". A question is never cut |
| Where it applies | The tier-1 grounded answer (`commands.rs`, before `Writer::write`) and the mixed tier (`mixed_answer::answer`). **Not** the no-documents tier, where the earlier exchanges are the only context there is (its instruction says what the user already said may be used) |
| What did not change | Every prompt string, the order of the turns (grounding last, after the history), the retrieval query, `fit_history` |
| Tests | 6 new unit tests in `conversation.rs`: 1 at 4 096, 2 at 8 192 and by default, no cap at 32 768, cut at a word boundary and marked, short answers and an empty history untouched, thresholds |

With the repository default (`DEFAULT_CONTEXT_WINDOW=8192`) every model gets two exchanges. To measure the one-
exchange case, set `MODEL_CONTEXT_WINDOWS=cabinet-chat=4096` in `.env` and restart the server.

### Measurement protocol (to run, then record in a table below)

Record every run: model, window, the answer, and the verdict.

1. **Regression, once per model.** Q1 to Q8 of [01-protocol.md](01-protocol.md). Verdicts must equal the lot C-a
   replay (rows 1-3 above for Q8-d).
2. **Q8-e versus Q8-f, ten times per model** (`gemma2:2b`, `ministral-3:3b`, `llama3.2:3b`):
   - *Q8-e*: new conversation, all documents ticked, ask `Quel est le prix de l'imprimante dans le devis MedSupply ?`
     (the quote is read; note the answer); then untick the quote **in the same conversation** and ask the same
     question again; record the second answer.
   - *Q8-f*: new conversation, quote unticked from the start, same question; record the answer.
   - Pass: in no run does Q8-e state a figure that is not in a selected document (no "1 000,00"), and the Q8-e
     verdict equals the Q8-f verdict.
3. **A normal follow-up still works.** With documents ticked: Q5, then `Et la convention de remplacement ?`, then a
   third question: the follow-up is understood (two exchanges remembered), a fourth question no longer remembers the
   first (by design).
4. Optional: repeat 2 with `MODEL_CONTEXT_WINDOWS=cabinet-chat=4096` (one exchange).

The results are in the next section, "C-b measurement results".

## Updated status

| ID | Status after the replay |
| --- | --- |
| BUG-07 | Closed, confirmed live (Q28 on four models; Q1, Q3 unchanged) |
| BUG-08 | Confirmed live for the deterministic and short answers; scroll-up during a stream and a long stream still to confirm |
| BUG-10 | Display side coded and unit-tested; not exercised live (no model echoed the name) |
| BUG-13 remainder, UX-4, UX-5 | Closed (reported) |
| UX-1 | Closed, confirmed live |
| BUG-18 | Confirmed live for outcome (b); the "kept" line not observed |
| BUG-11 | Closed: proved live by the owner once BUG-21 was fixed |
| BUG-20, BUG-21 | New, fixed in the working tree |
| BUG-09 | C-b coded; measured at windows <= 4 096 (no regression, effect not discriminated); a decisive run needs windows above 4 096 |

## C-b measurement results (owner, 4-5 October 2026)

### Configuration used

From the owner's `.env` (kept as is, by instruction): `LLM_REQUEST_TIMEOUT_SECONDS=300` (raised to try larger
models on this machine, also the proof that the variable is now forwarded, BUG-21) and
`MODEL_CONTEXT_WINDOWS=gemma2:2b=2048,llama3.2:3b=4096,ministral-3:3b=4096`; `MAX_OUTPUT_TOKENS=2048`;
`DEFAULT_MODEL_ALIAS=gemma2:2b`. The aliases are the model names themselves. Consequence for this measurement:
all three models are in the "at most one past exchange beside sources" class of C-b (windows <= 4 096), and the
history budget is already close to zero before C-b (see "What the measurement can and cannot show").

The owner ran Q8-e/Q8-f **three** times per model, not ten ("already long").

**BUG-11 proof**: validated by the owner ("tested and approved") with `LLM_REQUEST_TIMEOUT_SECONDS` set through
`.env`. BUG-11 and BUG-21 are closed.

### 1. Regression, Q1 to Q8 once per model (Q8 twice: quote ticked, then unticked)

Ground truth: [01-protocol.md](01-protocol.md) and [fixtures/README.md](fixtures/README.md).

| Q | `gemma2:2b` | `ministral-3:3b` | `llama3.2:3b` |
| --- | --- | --- | --- |
| Q1 `Combien de fichiers au total ?` | 6 PASS | 5 (see the note below) | 6 PASS |
| Q2 `... PDF ?` | 5 PASS | 4 (consistent with 5 files) | 5 PASS |
| Q3 list | 6 files PASS | 5 files (consistent) | 6 files PASS |
| Q4 `Que dit neurologie.pdf ?` | asks which, then the typed full path answers PASS | same PASS | same PASS |
| Q5 CPAM | **PASS-WITH-ISSUES**: the whole letter pasted with a stray `extrait[1]:`; the 1 February 2026 date is in it | **PASS**: date, person, cause, consequence in bullets | **PASS-WITH-ISSUES**: the letter pasted after "Extrait de [1] ..."; content right |
| Q6 convention | **PASS-WITH-ISSUES**: period, 80 % / 20 %, parties right; signature date 2 June 2026 missing; stray `extrait[1]`; no lease information | **PASS**: period, parties, 80 % / 20 %, Lyon 2 June 2026, no lease | **PASS-WITH-ISSUES**: period, 80 % / 20 % right; signature date missing |
| Q7 lease duration | **PASS** (nine years; the dates are missing; stray `extrait.`) | **PASS** (nine years, 1 April 2024 to 31 March 2033) | **PASS** (nine years and the dates) |
| Q8 with the quote | **PASS-WITH-ISSUES**: "1 200,00 euros HT. extrait [1]" does not say the printer alone is not priced | **PASS-WITH-ISSUES**: total and "the printer alone is not specified" right, but "(hors consommables)" is **invented** (the quote's total includes the consumables) | **PASS-WITH-ISSUES**: total right; "Le prix de l'imprimante est inclus dans le montant total" is an inference |
| Q8 quote unticked | **PASS-WITH-ISSUES**: refuses ("Le document ne contient pas d'information") after pasting an unrelated excerpt (OBS-9) | **PASS**: "Aucun extrait ne mentionne un devis MedSupply ..." | **PASS-WITH-ISSUES**: refuses, plus invented headings "Fonction de recherche :" and "Fichier : Aucun fichier spécifique" |

The `ministral-3:3b` conversation shows 5 files, 4 PDFs and a list without the quote: the quote was **not selected**
during Q1-Q3 in that conversation (the folder answers follow the selection), then selected for the first Q8. The
counts are correct for that selection; the three conversations did not start from the same selection.

Seen live: `ministral-3:3b` now says "Aucun **extrait**" (BUG-20 display fix confirmed). `llama3.2:3b` writes "les
extrait" (singular after a plural article) in several answers: the display keeps the number the model wrote, so a
grammar slip of the model shows through (observation OBS-13).

### 2. Q8-e against Q8-f, three runs per model

Each row: (a) the quote ticked, question asked; (b) quote unticked in the same conversation, same question
(= Q8-e); (c) a new conversation with the quote unticked from the start (= Q8-f).

| Model | Run | (a) quote ticked | (b) Q8-e, memory holds the earlier answer | (c) Q8-f |
| --- | --- | --- | --- | --- |
| `gemma2:2b` | 1 | "1 200,00 euros HT." | refusal after a pasted excerpt, **in English** | refusal after a pasted excerpt |
| `gemma2:2b` | 2 | "1 200,00 euros HT." | refusal after a pasted excerpt, **in English** | refusal after a pasted excerpt |
| `gemma2:2b` | 3 | "1 200,00 euros HT." | refusal after a pasted excerpt | refusal after a pasted excerpt, **in English** |
| `ministral-3:3b` | 1 | "Le montant total du devis : 1 200,00 euros HT." | "Il n'existe pas de devis MedSupply dans les fichiers disponibles." | "Je ne peux pas répondre ..." |
| `ministral-3:3b` | 2 | **"Le prix de l'imprimante laser multifonction A4 ... est de 1 200,00 euros HT."** (the total given as the printer's price) | refusal | refusal |
| `ministral-3:3b` | 3 | **same misattribution** | refusal | refusal |
| `llama3.2:3b` | 1 | **"Le prix de l'imprimante est de 80 euros (sans taxe)"**, then "environ 80 euros, mais il n'y a pas de preuve" (an **invented figure**, absent from the quote) | refusal | refusal |
| `llama3.2:3b` | 2 | "Montant total du devis : 1 200,00 euros HT." | refusal | refusal |
| `llama3.2:3b` | 3 | "Montant total du devis : 1 200,00 euros HT." | refusal | refusal |

**Pass criterion** (no figure absent from the selected documents in Q8-e, and Q8-e equal to Q8-f): **met in 9 of 9
pairs** for the unticked question. Gemma answered in English in 3 of its 6 refusals (OBS-7, a known weakness).

**Failures seen in step (a), where the quote is selected and no memory is involved** (so not C-b material, but real):
`llama3.2:3b` invented "80 euros" (run 1) and `ministral-3:3b` gave the total as the printer's price (runs 2 and 3).
The protocol's expected answer is "total 1 200,00 HT, the printer alone is not priced". Of the nine step (a) answers,
six give the total without a false claim (gemma 3, llama 2, ministral 1), though only the regression answers of
ministral and llama also say the printer alone is not priced; three are wrong (llama 1, ministral 2). A numeric
safety net (a figure in the answer that is in none of the excerpts gets an appended notice, the optional item 4 of
C-b, needing the owner's approval) would have flagged "80"; no code check can catch "the total given as the printer's
price": that is a model-reading problem.

### 3. A normal follow-up (`gemma2:2b`, window 2 048)

| Question | Answer | Verdict |
| --- | --- | --- |
| `Que dit le courrier de la CPAM concernant la radiation ?` | a fragment: `extrait[1], "Objet : radiation d'un assure du regime general"` | PASS-WITH-ISSUES (true, useless) |
| `Et la convention de remplacement ?` | "La convention de remplacement est disponible dans le document `convention-remplacement-dr-martin.docx` (page 1). extrait." | The follow-up was **understood** (the right file found) but no content given |
| `Résume la convention de remplacement signée avec le Dr Martin.` | the whole agreement text pasted | content right |
| `fait pareil pour le du bail du cabinet` | "Le bail du cabinet est mentionné dans [2] bail-cabinet-2024.pdf (page 1). extrait [2]" | understood ("fait pareil" -> the lease), no content |

The follow-ups are understood **through the search query** (`conversation::retrieval_query` adds the previous
question to a short question), not through the model's memory (at window 2 048 the history budget is zero). What
fails is the small model's answer, not the memory. Not tested: that a fourth question forgets the first, and a
follow-up with `ministral-3:3b` or `llama3.2:3b`.

### What the measurement can and cannot show

- **It cannot separate C-b from the budget.** `history_chars` = prompt budget - (this turn's instruction, excerpts,
  question) - 2 000 reserved; the prompt budget is (window - 2 048 output) x 3 characters: **0** at 2 048 tokens and
  6 144 at 4 096. A turn carrying a full excerpt block (up to 6 000 characters) leaves no room for history at either
  window. So with these `.env` values Q8-e could not carry the earlier answer to the model even without C-b, and the
  9 of 9 result is true but not discriminating for the new cap. C-b's cap and cut act where there is room: windows
  above 4 096, for example the repository default of 8 192. A decisive run needs those windows; the owner chose to
  keep the current values, so it is **not run**.
- **A second, more important suspicion about these windows (not proven).** The excerpts block alone can be 6 000
  characters (about 1 500-2 000 tokens), plus the instruction, the folder context, the gateway's own rules and the
  answer's 2 048-token reservation. At a 2 048-token window the prompt cannot fit. The window is sent to Ollama as
  the context size, and Ollama truncates an over-long prompt (from the start) without telling anyone. That would
  explain several gemma symptoms: answers in English, an unrelated excerpt pasted, a refusal after a long wait. To
  check: `docker compose logs ollama` and search for "truncating input prompt" while a Q8 runs. If it is confirmed,
  the cause is the configured window, not the model, and the evidence budget should follow the window (candidate
  BUG-22). The owner keeps the values; this note only records the risk.

### Owner observation recorded

For `llama3.2:3b`, answers were **better after deleting and re-indexing the files** (OBS-12). Cause unknown. Candidate
explanations to test: an index built before a change of extraction, chunking or embedding (vectors of two different
embedders are not comparable; the embedding alias is `assistant-embed`), or a partial earlier analysis. Candidate
safeguard: stamp the index with the embedding model and the chunking version and say when it is stale. Not
investigated yet.

### New findings

| ID | Severity | Effort | Title |
| --- | --- | --- | --- |
| OBS-12 | - | - | `llama3.2:3b` answers better after the files are deleted and re-indexed (owner); stale index suspected |
| OBS-13 | - | - | A model's own grammar slip ("les extrait") shows through the display replacement, which keeps the number the model wrote |
| OBS-14 | - | - | At step (a) of Q8, `llama3.2:3b` invented a printer price (80 euros) and `ministral-3:3b` twice gave the total as the printer's price; `ministral-3:3b` also invented "(hors consommables)" in the regression run |
| BUG-22 (hypothesis) | S2 | S-M | The evidence and instruction are not sized to the model's window; at 2 048 tokens the runtime may truncate the prompt silently. To confirm in the Ollama logs first |

### Verdict on C-b

Coded, unit-tested, and **no regression** found in the regression run (Q1-Q8) or in the 18 refusals of Q8-e/Q8-f. Its
**effect is not proven** by this run (see the limits above). It stays: it applies the owner's principle (sources
outrank memory) at the windows where it matters and costs nothing at the others.
