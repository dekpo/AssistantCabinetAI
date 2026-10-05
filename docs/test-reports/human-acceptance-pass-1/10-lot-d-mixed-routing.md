# 10 - Lot D: the mixed selection does what it says (5 October 2026)

Lot D of [04-improvement-plan.md](04-improvement-plan.md): the documents-and-tables tier. Branch to create before
committing: `fix/lot-d-mixed-routing`, stacked on `fix/lot-c2-history-priority`. Coded and tested; **awaiting the
owner's live replay**. No prompt string, instruction constant or turn order was changed.

## What the owner decided (4 October 2026) and how it is applied

| Decision | Application |
| --- | --- |
| With both kinds ticked, a question about the data alone is answered at once, with no model, unless it points at a document or asks for something to be written; everything else goes to the mixed tier | `prepare_if_data_only` now refuses to answer a question that **points at a document** (lot B2's gate, kept) **or carries a writing verb** (new, `question::has_writing_intent`). The words are data in the locale packs (`writing_intents`, `document_references`); nothing is hard-coded |
| The order in which files are cited in the sentence is not the router's business | The roles come from the file kinds: the table feeds the facts, the documents feed the text, the model writes. No sentence-order parser was added; see the reasoning in [08-lot-c-plan.md](08-lot-c-plan.md), "Looking ahead" |

## What changed

| Finding | Change | Tests |
| --- | --- | --- |
| BUG-06 / Q24: a question that points at a document gave no usable figure | `tabular_answer::prepare_for_mixed` (the entry point `commands::mixed_tier` now calls) hands a document-pointing question back unfinished (`TryModel`, `skip_model`) instead of a finished whole-table value; `mixed_answer` then ties the document's entity to a row **before** any figure is shown. The supplier a quote names gives **that supplier's** total (1 450 for MedSupply in the fixtures), shown with its filter ("Compris comme : fournisseur = MedSupply") | `tests/mixed_answer.rs`: the named supplier's total in French and English; nothing linked when nothing is named |
| Entity linking was fragile | `mixed_answer::entity_link`: (1) only **text** columns are linked, so a number written in a document ("650 euros") never selects the row whose amount is 650; (2) a **whole value** found as a phrase in the excerpts wins over a word it shares with another value ("FOURNITURES MEDICALES" no longer links to "Fournitures Dupont"); two different values named in the excerpts link nothing, rather than choosing one; the older first-word rule remains only when no whole value is found | four new tests in `tests/mixed_answer.rs` (number, two suppliers, shared word, named supplier) |
| BUG-16: the computed figure was hidden beside a model's prose | `MessageList`: a mixed answer always shows the engine's figure in its own block ("Calculé dans vos données (par le moteur, pas par l'IA)"), with the filters it was computed under, under the model's text. Not repeated when the table's sentence is already the whole answer | vitest and type checks; visual replay |
| Writing requests were swallowed by the data-only router | the writing-verb gate above | `tests/tabular_hap1.rs` (French and English; a plain data question stays instant) |

Behaviour kept on purpose: `skip_model` is now honoured by the mixed tier. A proper noun that matches no value and has no close one is refused at once there too, as in tier 2 (BUG-12); the previous behaviour, asking a model for a plan that could not find a value either, ended in the same refusal after a wait.

## Not done in lot D, and why

| Item | Why |
| --- | --- |
| Removing the "this question had nothing for your tables" line on the publipostage question (`Génère le courrier de la commande CMD-2026-002 ...`) | The message is wrong there because the question **does** concern a row, but the product has no way to find a row by identifier yet. Fixing the sentence without the capability would only move the lie. It goes with the row lookup in lot E (`docs/SESSION-DATA-17-Publipostage-Issue.md`, local) |
| OBS-8 (the structural route ignores the rest of the sentence) | A structural fact has no evidence form in the table block of the mixed tier (`table_evidence_text` handles values only); making the data-only precondition stricter for structural answers needs that first. Not a wrong number; documented |
| The numeric safety net for tier 1 (a figure in no excerpt gets a notice) | Needs the owner's approval (interface line). The C-b measurement showed a case (`llama3.2:3b`, "80 euros") it would have flagged. Open question to the owner |

## Replay list (exact steps)

Selection "B": `devis-imprimante-medsupply.pdf` and `factures-fournisseurs-2026.xlsx` ticked.

1. **Q24**: `Quel est le montant total pour le fournisseur mentionné dans cette lettre ?` Expected: under the model's text, the block "Calculé dans vos données (par le moteur, pas par l'IA)" reading "Somme de montant : **1 450**. Calculé sur 3 lignes. Compris comme : fournisseur = MedSupply." The model's prose should now be able to quote it. Never 2 215.
2. **Q23**: `Le montant total facturé par MedSupply correspond-il à ce qui est indiqué dans le devis signé ?` Expected unchanged (1 450 against 1 200,00 HT), now with the computed block under the prose (BUG-16).
3. **A letter that names no supplier**: `Quel est le montant total pour le fournisseur mentionné dans le courrier de la CPAM ?` Expected: the notice "Le total de vos tables n'a pas pu être rattaché ..." and no total. Select the CPAM letter and the invoices only (untick the quote), so that no excerpt names a supplier.
4. **Q26**: the Q24 question with the gateway stopped (`docker compose stop server`, then start it again). Expected: the computed block (1 450) and the line saying the document part could not be written; no crash.
5. **Instant answer kept**: both kinds ticked, `Quelle est la somme des montant ?` Expected: "Somme de montant : 2 215" at once, with no model, and the line saying the documents were not needed.
6. **A request to write**: both kinds ticked, `Rédige un résumé de la somme des montant.` Expected: the model answers (it is not the instant sentence), with the computed block "Somme de montant : 2 215".
7. **Tables only, no regression**: invoices alone, `Quelle est la somme des montant ?` Expected: instant, unchanged.
8. **No regression**: rows 1, 6, 7, 9, 11, 12 of the B2 list in [07-retest-lot-b.md](07-retest-lot-b.md), and the C-a replay list of [08-lot-c-plan.md](08-lot-c-plan.md).

Limits to keep in mind while replaying: the quote must be among the excerpts retrieved for the question (retrieval is the model-independent part: if another document is retrieved first and names a supplier of the table, that one is linked); and the owner's `.env` windows (2 048 / 4 096) cap how much a small model receives (`09-retest-lot-c-a.md`, BUG-22 hypothesis).

---

## Replay results (owner, 5 October 2026, `gemma2:2b`, `.env` windows kept)

The owner replayed with `gemma2:2b` on the assumption that the other models are not better, and copied the
engine's sub-answers and the notes under each answer by hand, because "Copy the conversation" left them out.

| # | Check | Verdict | What was seen |
| --- | --- | --- | --- |
| 1 | Q23 | **PASS-WITH-ISSUES** | "1 450,00 euros. Le montant du devis et de la facturation ne correspondent pas." (right verdict), and the new computed block "Somme de montant : 1 450. Calculé sur 3 lignes. Compris comme : fournisseur = MedSupply." But a **false correction** line: "La table indique 1 450, et non 450,00." (BUG-23) |
| 2 | Q24 | **PASS** for the figure, with the same false correction | "1 450,00 euros." and the computed block with `fournisseur = MedSupply`: the document's entity is tied to its row, and the whole-table 2 215 is gone. First live proof of the lot D linking |
| 3 | Supplier named in the CPAM letter | **PASS-WITH-ISSUES** | No total, an honest refusal, but about the acronym: "« CPAM » ne correspond à aucune valeur réelle ..." instead of the "not linked" notice (BUG-25). The folder-answer label under it was also wrong (BUG-24) |
| 4 | Q23 with the gateway stopped (Q26) | **PASS-WITH-ISSUES** | The computed figure and "L'IA n'a pas pu être contactée pour rédiger la partie document." are right. The label read "Réponse établie depuis votre dossier de documents, sans l'IA" although the answer comes from the data folder (BUG-24) |
| 5 | Both ticked, `Quelle est la somme des montant ?` | **PASS** | 2 215 at once, "calculée depuis votre dossier des données, sans l'IA" and "les documents sélectionnés n'étaient pas nécessaires" |
| 6 | Both ticked, `Rédige un résumé de la somme des montant.` | **PASS-WITH-ISSUES** | The model wrote "La somme des montants est de 2 215 euros." with the computed block 2 215, but the false correction "La table indique 2 215, et non 215." (BUG-23) |

### Findings

| ID | Severity | Title and cause |
| --- | --- | --- |
| BUG-23 | S2 | **A false numeric correction under correct answers.** `mixed_answer::scan_numbers` split "1 450,00" at the space and read "450,00" as a claim contradicting the table's 1 450; "2 215" became "215". Three of three replayed answers showed it. The code comment had put space-grouped thousands "out of scope"; with a French interface they are the normal way to write an amount |
| BUG-24 | S4 | The line under a table answer given inside a mixed question (degraded, or a refusal) named the **documents** folder |
| BUG-25 | S3 | A capitalised word that **designates a document** ("CPAM" in "the CPAM letter") was taken for a table value that matches nothing, which ended the answer in a refusal about an acronym |
| UX-6 | S4 | "Copy the conversation" left out the engine's sub-answers and the notes under each answer (owner request: copy everything except the lists of sources) |

## D2: what was changed after the replay (same branch)

| Finding | Change | Tests |
| --- | --- | --- |
| BUG-23 | New `number_check` module: a number is read whole (a group of exactly three digits after one to three, separated by a space, no-break or narrow no-break space, belongs to it; "1,500" and "1.500" keep both readings; full European and English formats read the same). The mixed tier's `verify_numbers` uses it and compares values, so "1 450,00", "1 450" and "1450" are the same figure | 10 unit tests in `number_check.rs`; two integration tests in `tests/mixed_answer.rs` (the table's own value written four ways is never corrected; a wrong amount written the same way is corrected once, as one number) |
| Numeric safety net for document answers (approved by the owner on 5 October) | `AskAnswer.unverified_numbers`: after a document answer, every figure of two digits or more that is in none of the excerpts the answer was written from, and not in the question, is reported under the answer ("Le chiffre « 80 » ne figure dans aucun des extraits utilisés ni dans votre question : vérifiez-le dans vos documents"). Citation markers like [1] are ignored; the answer is never edited. The no-documents tier is not checked (nothing to check against) | `number_check` tests; `annotations.test.ts` |
| BUG-24 | The line under a degraded or refused table answer in a mixed question now names the data folder | `annotations.test.ts` |
| BUG-25 | `prepare_for_mixed` receives the selected documents' paths; a word of a document's name is not reported as a missing table value | `tests/mixed_answer.rs` (the acronym with and without the document name) |
| UX-6 | `lib/annotations.ts` is the one source of the lines written under an answer. The conversation view renders them and "Copy the conversation" copies them: the text as shown (internal names replaced), the engine's figure, then every note; the lists of sources are left out | `annotations.test.ts` (the sources are not copied) |

Suites after D2: cargo 478 lib plus the integration suites, vitest 314, `tsc`, no warning.

Limits: the safety net compares written numbers; it cannot see that "1 200" was said of the wrong thing (the lot C-b case where the total was given as the printer's price) and it flags a legitimate derived figure (a difference the model computed) as unsupported, by design: the reader is asked to check it.

### Replay list for D2

1. Q23, Q24 and the writing request again with `gemma2:2b`: **no** "La table indique ... et non ..." line when the model's amount is the table's, written in any way.
2. The CPAM question (CPAM letter and invoices ticked): the "Le total de vos tables n'a pas pu être rattaché ..." notice, no total, no refusal about "CPAM".
3. Q26 (gateway stopped): the label under the answer reads "calculée depuis votre dossier des données, sans l'IA".
4. Q8 with the quote ticked, three times with `llama3.2:3b`: when the model writes a price that is in no excerpt (for example 80), the line "Le chiffre « 80 » ne figure dans aucun des extraits ..." appears under the answer; when it states only the quote's 1 200,00, nothing appears.
5. "Copier la conversation" after a mixed answer: the computed block, the "Généré par" line and the notes are in the clipboard, and no "Sources".

### D2 replay results (owner, 5 October 2026, `gemma2:2b`, then other small models)

| # | Check | Verdict | What was seen |
| --- | --- | --- | --- |
| 1 | Q23, Q24, the writing request | **PASS** | The computed blocks show 1 450, 1 450 and 2 215; **no false correction** under any of the three (BUG-23 confirmed fixed live). Limit seen again, not a defect: for the writing request gemma wrote "Le montant total de la commande est de 1 200,00 euros HT" (the quote's figure) next to the computed 2 215; that figure is in an excerpt, so nothing is flagged, and the block lets the reader see the difference |
| 2 | The CPAM letter | **PASS** for the notice, with a finding | The "n'a pas pu être rattaché" notice appears and the refusal about "CPAM" is gone (BUG-25 confirmed). But the model wrote "Le montant total est de 1 200,00 euros HT. extrait de [1]" and **nothing flagged it**: with no table figure shown, the mixed tier checked no figure at all (BUG-26, fixed below) |
| 3 | Q26, gateway stopped | **PASS** | The computed figure, the "partie document" line and the label "calculée depuis votre dossier des données" (BUG-24 confirmed fixed live) |
| 4 | Numeric safety net, `gemma2:2b` "3 imprimantes" | **Not triggered** | It answered "1 200,00 euros HT." (a figure of the quote), so nothing to flag |
| 5 | Numeric safety net, "prix de l'imprimante" ×3 with `llama3.2:3b` (by "Régénérer") | **Not triggered** | Three correct refusals or extracts, no invented price |
| 6 | The same question with `granite3.1-moe:3b`, `gemma3:1b`, `llama3.2:1b`, `qwen2.5:1.5b` | **Not triggered** | All quote the 1 200,00 total. `llama3.2:1b` writes "Le prix de l'imprimante est de 1 200,00 euros HT" (the total given as the printer's price: a figure of the quote, so not catchable by a number check); the others state it as the total |
| 7 | Copy the conversation | **PASS-WITH-ISSUES** | It copies everything wanted (computed block, "Généré par", notes, no sources) but with many blank lines (UX-7, fixed below) |

The numeric safety net for document answers is therefore verified by unit tests only; the owner could not make a model invent a figure in this pass and will stay watchful.

### Findings and fixes after the D2 replay

| ID | Title | Change |
| --- | --- | --- |
| BUG-26 | A mixed answer with **no table figure shown** states a figure from nowhere and nothing says so (CPAM question) | The mixed tier now holds such an answer to the excerpts and the question, like a document answer (`MixedAnswer.unverified_numbers`, same notice) |
| UX-7 | "Copy the conversation" has too many blank lines | One blank line between a question-and-answer pair and the next, none inside a turn; a question and its answer follow each other on consecutive lines (`formatConversation`) |
