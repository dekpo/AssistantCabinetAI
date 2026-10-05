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

| Run | Model | Window | Variant | Answer (short) | Verdict |
| --- | --- | --- | --- | --- | --- |
| (to fill in) | | | | | |

## Updated status

| ID | Status after the replay |
| --- | --- |
| BUG-07 | Closed, confirmed live (Q28 on four models; Q1, Q3 unchanged) |
| BUG-08 | Confirmed live for the deterministic and short answers; scroll-up during a stream and a long stream still to confirm |
| BUG-10 | Display side coded and unit-tested; not exercised live (no model echoed the name) |
| BUG-13 remainder, UX-4, UX-5 | Closed (reported) |
| UX-1 | Closed, confirmed live |
| BUG-18 | Confirmed live for outcome (b); the "kept" line not observed |
| BUG-11 | Not reproduced; BUG-21 fixed, proof recipe above |
| BUG-20, BUG-21 | New, fixed in the working tree |
| BUG-09 | C-b coded; measurement pending |
