# 07 - Retest after lot B (4 October 2026)

Lot B of [04-improvement-plan.md](04-improvement-plan.md), branch `fix/lot-b-tabular-engine-truth`, stacked on
lot A. The owner replayed the lot B checks on the real stack and sent the conversations; every answer below was
then checked against the fixtures in [fixtures/README.md](fixtures/README.md) and, for the failures, against the
code. The French sentences are test data.

## What lot B changed

| Finding | Change |
| --- | --- |
| BUG-02 (cheap part) | `tabular_answer::detect_filters`: the digit groups of a written date are consumed by that date; a one-letter word is never a cell value (the verb "a" no longer equals the patient "Martin A"); a number is a year only inside 1900..2100; one written date filters exactly that day; identical filters are pushed once |
| BUG-05 | `tabular/question.rs`: an aggregate ("total", "somme") reads the *measure* column the question names, not the longest column name in it (a text column) |
| BUG-15 | `tabular_answer::pick_target`: when several workbooks are ticked and the question names a column, only the workbooks that have that column stay candidates; none named still asks which workbook |
| BUG-13 (part) | The example sentence for a group question is gender-free ("Quel(le) fournisseur a le plus de montant ?") |
| New | A filtered value computed over 0 rows says "No row matches these criteria" and points at the "Compris comme" line (`tabularAnswer.noRowMatched`) |
| Q24 gate | `prepare_if_data_only` leaves a question that points at a document ("... mentioned in this letter") to the mixed tier; the phrases are data in `resources/tabular-questions/*.json` (`document_references`) |
| Tests | New `tests/tabular_hap1.rs` replays the two HAP-1 workbooks in French and English (Q14 = 4, Q16 = Salle 3 / 50, sum 2 215, Q9 workbook choice, the answers that passed in HAP-1) |

Deferred on purpose: BUG-12 (contradicts decision D6), UX-1 (clickable choices), BUG-16 (displaying the computed
figure in a mixed answer).

## Verdicts

| Check | Verdict | One line |
| --- | --- | --- |
| Q9, both workbooks ticked | **PASS** | 2 215 at once, no workbook question |
| Q14, appointments only | **PASS** | 4, with "Compris comme : date entre 2026-03-09 et 2026-03-15"; with both workbooks ticked the engine still asks which one (see B1) |
| Q16 | **PASS** | Salle 3 / 50, ranking 50, 95, 100 is exact |
| Q19 with "5000" | **FAIL** | A wrong "Nombre de lignes : 8" with no filter line; the button changes nothing (BUG-17, BUG-18) |
| Q20, Q21 | **PASS** | Refusals are immediate and the new gender-free example shows |
| Q24 | **FAIL (exit criterion not met)** | Reaches the models now, but neither answer is the expected 1 450 (BUG-06, BUG-16; lot D) |
| Q23 | **PASS with `ministral-3:3b`, PASS-WITH-ISSUES with `gemma2:2b`** | Both reach the model; only one gives a verdict |
| Filter that finds nothing | **NOT EXERCISED** | The question chosen was routed to the structural path, so the new 0-row note did not run (B4) |

### Q9 - `Quelle est la somme des montant ?` (both workbooks ticked)

```text
Assistant: Somme de montant : **2 215**. Calculé sur 8 lignes.
```

Ground truth 450+180+620+95+210+380+120+160 = 2 215. Correct, and no "which workbook" question: only the invoices
workbook has `montant`.

### Q14 - `Combien de rendez-vous entre le 09/03/2026 et le 15/03/2026 ?` (appointments only)

```text
Assistant: Nombre de lignes : **4**. Calculé sur 4 lignes. Compris comme : date entre 2026-03-09 et 2026-03-15.
```

Rows on 9, 10, 11, 12 March = 4. Correct, and the single "Compris comme" clause is the proof that no spurious
filter is left. Owner remark: with both workbooks ticked the engine asks which file; she had to untick the
invoices. This is the designed behaviour for a question that names no column (both workbooks have a `date`
column, nothing in the sentence points at either), see B1.

### Q16 - `Quelle salle a le moins de duree_min ?`

Salle 3 = 25+25 = 50; Salle 2 = 30+35+30 = 95; Salle 1 = 20+15+20+25+20 = 100. The answer lists exactly that.
Correct.

### Q19 - `Est-ce qu'on a dépensé plus de 5000 euros avec un seul fournisseur ce trimestre ?`

Three conversations: the engine's immediate answer, then "Demander à l'IA" with `gemma2:2b`, then with
`ministral-3:3b`. All three:

```text
Assistant: Nombre de lignes : **8**. Calculé sur 8 lignes.
```

**Wrong, and worse than HAP-1.** The question asks whether a supplier's total exceeds 5 000; the invoices
total 2 215, so the correct answer is "no" (or an honest "I cannot compute a per-supplier threshold", BUG-14).
In HAP-1 the product wrongly said "Nombre de lignes : 0, Compris comme : date en 5000", which at least showed
the invented filter. The lot B fix removed the invented year, and the product now answers a different question
(how many rows) with **no filter line at all**, so nothing hints that "plus de 5000" was dropped.

Root cause, established by running the question through the engine (see BUG-17):

1. `classify_with` reads "plus" + a number as an implicit count (`tabular/question.rs`, the "No operation word"
   branch), promising that "the comparison itself is resolved afterward as a filter".
2. But `residual_words` removes every word of the pack, and in French "plus" and "moins" are also vocabulary of
   the superlative group questions (`groups.most`, `groups.least`). They are therefore consumed before
   `detect_filters` looks for a comparison word in the residual, and no `GreaterThan` filter is ever built.
3. Nothing is left that fails to match real data, so no model escalation either: the unfiltered count is
   returned as a finished answer.

Reproduced in isolation, no model involved: `Les factures de plus de 500 euros ?` returns `Count(8)`, every row,
with `filters: []`. The English "over 500" works (the unit test `a_comparison_word_beside_a_number_filters_the_
single_numeric_column`) because "over" is not group vocabulary. This is **not caused by lot B**: the French
comparison has never worked; lot B only removed the accidental year that used to mask it.

Second finding, BUG-18: "Demander à l'IA" on a computed answer asks the model and, when the model cannot do
better, **returns the same deterministic answer without a word** (`PendingAnswer::TryModel { fallback: computed }`,
and `with_model_attempt` decorates only a `Nudge`). Conversations 2 and 3 are therefore indistinguishable from
conversation 1: the user cannot tell whether the model was asked, failed or was ignored.

### Q20 and Q21

```text
Vous: Quelle est la somme de cumul dans Calculs ?
Assistant: Cette feuille n'existe pas dans factures-fournisseurs-2026.xlsx. Ses colonnes sont : date, fournisseur, montant. ...
          Ou : « Quel(le) fournisseur a le plus de montant ? »
Vous: Combien de factures pour Alfa ?
Assistant: « Alfa » ne correspond à aucune valeur réelle de factures-fournisseurs-2026.xlsx. ...
          Votre question précédente n'a pas pu être calculée non plus : vérifiez que la valeur est écrite exactement comme dans le fichier.
```

Both are honest and fast, and the example now reads "Quel(le)". The repeat line on the second answer is the
designed behaviour (the previous answer was also a refusal).

### Q24 - `Quel est le montant total pour le fournisseur mentionné dans cette lettre ?` (quote + invoices ticked)

Ground truth: the letter (the quote) names MedSupply; MedSupply's invoices total **1 450**.

| Conv | Model | Answer | Verdict |
| --- | --- | --- | --- |
| 1 | `gemma2:2b` | "1 200,00 euros HT." | **Wrong**: the quote's amount, no table figure, no mention of the invoices (BUG-16) |
| 2 | `ministral-3:3b` | "Extrait [1] : montant total du devis = 1 200,0 € HT. Table résultats : montant total des factures = **2 215 €** (hors devis initial). Le montant total pour le fournisseur Medsupply n'est pas précisé dans les documents fournis. ..." | **Wrong and misleading**: 2 215 is the total of **every** supplier, labelled "montant total des factures"; "(hors devis initial)" is invented; and the final sentence says the figure for MedSupply is unavailable although the table gives it (1 450) |

The owner's reading is "model quality is decisive". That is true for conv 1 and for the prose of conv 2, but
**not for the 2 215**: that number was computed by the engine, not written by the model. The mixed tier asks the
engine the question as written, which contains no value word ("le fournisseur mentionné dans cette lettre"), so
the engine sums the whole column. The supplier named in the document is only linked to the table after a
`ValueNotFound` (`mixed_answer::entity_link`), which never occurs here. This is BUG-06 and it belongs to lot D;
no model, however good, can write 1 450 from a block that says 2 215. The Q24 exit criterion of lot B
("1 450 or an explicit refusal that names the column") is therefore **not met**, and it was never reachable by
lot B alone: lot B only moves the question out of the data-only router.

Note on what lot B did improve: before the gate the same question was answered by the data-only router with the
deterministic sentence "Somme de montant : 2 215", with no model and no caveat. It now at least goes through the
model, which can (conv 2) say that the supplier-specific figure is missing.

### Q23 - `Le montant total facturé par MedSupply correspond-il à ce qui est indiqué dans le devis signé ?`

| Conv | Model | Answer | Verdict |
| --- | --- | --- | --- |
| 1 | `gemma2:2b` | "Le montant total facturé est de 1 450 euros. [1] Le montant du devis est de 1 200,00 euros." | Both figures right, **no verdict** to the yes/no question, stray `[1]` |
| 2 | `ministral-3:3b` | "Non, le montant total facturé par MedSupply (1 450 €) ne correspond pas au montant du devis signé (1 200 € HT). Le devis initial (DV-2026-0117) couvre uniquement la commande initiale de janvier. Les factures supplémentaires pour l'année ne sont pas incluses dans ce devis." | **Correct**, and every claim is in the quote text ("Ce devis ne couvre que la commande initiale de janvier ...") |

Both reached the model directly (lot A's BUG-04 fix, and the router now lets the question through). 1 450 vs
1 200 is the right comparison; the improvement over HAP-1 and lot A is confirmed.

### B4 - `Que contient la colonne détails de la facture de 2026-01-22 ?` (invoices only)

```text
Assistant: Colonnes de la feuille Factures : date, fournisseur, montant.
```

The owner meant this as a filter that finds nothing. It did not exercise the lot B 0-row note: the words
"contient" and "colonne" route the question to the **structural** path ("what are the columns"), which answers
without reading the rest of the sentence. The answer is coherent (it shows that `détails` is not a column) but it
silently ignores "de la facture de 2026-01-22". Same family as BUG-17: a part of the question is dropped and
nothing says so (OBS-8). The 0-row note must be replayed with a question that reaches the engine (see the replay
list at the end).

## New findings

| ID | Severity | Effort | Title |
| --- | --- | --- | --- |
| BUG-17 | S2 | S | French "plus de N" / "moins de N" is never turned into a filter: the product answers the unfiltered count as if it were the answer (Q19) |
| BUG-18 | S3 | S | "Demander à l'IA" on a computed answer returns the same answer silently when the model cannot do better (Q19 conv 2, 3) |
| OBS-8 | - | - | The structural route ignores the rest of the question ("colonne détails de la facture de 2026-01-22") |

Entries are detailed in [03-bug-register.md](03-bug-register.md).

## Updated bug status

| ID | Status after lot B |
| --- | --- |
| BUG-02 | Cheap part fixed and confirmed live (Q14 = 4, Q16). The comparison part is **not** fixed: see BUG-17 |
| BUG-05 | Fixed in code and unit-tested; Q24 could not confirm it live because the question now goes to the mixed tier (the sum itself is covered by `tests/tabular_hap1.rs`) |
| BUG-15 | Fixed and confirmed live (Q9). Remainder: a question naming no column still asks which workbook (Q14 with both ticked); UX-1 is the fix |
| BUG-13 | Gender example confirmed live (Q20). Still open: the refused column is not named (Q24), generic column list |
| BUG-06 | Still open (Q24): lot D |
| BUG-16 | Still open (Q24 conv 1, Q23 conv 1): lot D |
| BUG-12, UX-1 | Not started |
| BUG-17, BUG-18, OBS-8 | New |

## Fixes applied after the retest (lot B2, same branch, 4 October 2026)

The owner decided to fix before committing. Four owner decisions were taken first: BUG-12 yes (refuse at once),
Q19 refuse when a group is named, Q24 withhold the whole-table total until lot D, scope = BUG-17 plus BUG-18.

| Finding | Change | Tests |
| --- | --- | --- |
| BUG-17 | `detect_filters` reads "comparison word, connectors, number" from the question's own words; thousands written with a space ("5 000") are one number; "au moins", "pas plus de", "1,500" are refused, never dropped; an unresolved threshold beside other filters is no longer lost | `tests/tabular_hap1.rs`: thresholds fr/en, thousands, never-dropped, superlative stays a ranking |
| Q19 | `group_threshold_not_supported` (new reason, both catalogues): a plain count under a row threshold with a named text column and no value | `tests/tabular_hap1.rs`, `tabularAnswer.test.ts` |
| BUG-12 | `skip_model` on `TryModel`: a name with no close value is refused at once; D6 narrowed, recorded in `DECISIONS.md` | `tests/tabular_hap1.rs` (no model attempt) |
| BUG-18 | `Value.modelAttempt`; the line "model asked, computed answer kept" | `tests/tabular_hap1.rs` |
| Q24 interim | `MixedPartUnavailable::NotLinked`: no whole-table total for a question that points at a document | `tests/mixed_answer.rs` |

Limits to know: BUG-12 does not cover a missing *sheet* (Q20), which only the model's plan reveals; and Q24 still
needs lot D to produce 1 450 (the entity named in the letter must be tied to a row for a question that names no
value); today it says the total could not be tied.

## Replay list after the B2 fixes

| # | Selection | Question | Expected |
| --- | --- | --- | --- |
| 1 | invoices | `Combien de factures de plus de 400 euros ?` | 2, "Compris comme : montant > 400" |
| 2 | invoices | `Combien de factures de plus de 500 euros ?` / `de moins de 200 euros ?` | 1 / 4, each with its "Compris comme" line |
| 3 | invoices | `Combien de factures de plus de 5 000 euros ?` | 0 rows, "Compris comme : montant > 5000" and the "Aucune ligne ne correspond" note |
| 4 | invoices | Q19 sentence (`... plus de 5000 euros avec un seul fournisseur ce trimestre ?`) | the refusal "compare un seuil au total d'un groupe", at once, no model line |
| 5 | invoices | `Combien de factures d'au moins 400 euros ?` | a refusal or an interpretation by the model, never a count shown without its filter |
| 6 | invoices | `Combien de factures pour Alfa ?` | the refusal at once, **no** "a été interrogé" line (BUG-12) |
| 7 | invoices | `Quelle est la somme des montant ?` then "Demander à l'IA" | the same value, plus "... n'a pas proposé de lecture plus précise ... la réponse calculée ci-dessus est conservée" (BUG-18) |
| 8 | quote + invoices | Q24 sentence | no 2 215 anywhere; the notice "Le total de vos tables n'a pas pu être rattaché ..."; the prose comes from the document only |
| 9 | quote + invoices | Q23 sentence | still a combined answer, 1 450 against 1 200 (no regression) |
| 10 | invoices | `Combien de factures le 23/01/2026 ?` | 0, the "Compris comme" line and the "Aucune ligne" note |
| 11 | appointments | Q14, Q16, `Quelle salle a le plus de duree_min ?` | 4 / Salle 3, 50 / a ranking led by Salle 1 (100) |
| 12 | both | Q9 | 2 215 without asking which workbook |

---

## Replay after the B2 fixes (4 October 2026): results

Run by the owner on the real stack (`pnpm tauri dev`). Selection A = `factures-fournisseurs-2026.xlsx` only;
selection B = the invoices workbook and `devis-imprimante-medsupply.pdf`. Every answer below was checked against
the fixtures ([fixtures/README.md](fixtures/README.md)). Models: `gemma2:2b` and `ministral-3:3b`, as noted.

### Build warning found by the owner, and fixed

`pnpm tauri dev` printed `warning: variable does not need to be mutable` at `src/mixed_answer.rs:169`. Cause: the
Q24 interim edit shadowed `table_unavailable` with a second `let`, so the first binding's `mut` became useless.
Not present before B2. Fixed (`mut` removed); `cargo build` and `cargo build --tests` now print no warning.

### Verdicts

| # | Sel. | Question | Verdict | What was seen |
| --- | --- | --- | --- | --- |
| 1 | A | `... de plus de 400 euros ?` | **PASS** | 2, "Compris comme : montant > 400" (450, 620) |
| 2 | A | `... de plus de 500 euros ?` | **PASS** | 1, "montant > 500" (620) |
| 3 | A | `... de plus de 200 euros ?` | **PASS** | 4 (450, 620, 210, 380) |
| 4 | A | `... de plus de 5 000 euros ?` and `de plus de 5000 euros ?` | **PASS** | 0 rows, "montant > 5 000" and the new "Aucune ligne ne correspond" note, for both spellings. First live proof of the zero-row note and of the thousands space |
| 5 | A | `... plus de cinq mille euros avec un seul fournisseur ...` (both models) | **PASS-WITH-ISSUES** | An honest refusal ("Je ne peux pas répondre directement ...") with the columns and examples. Spelled-out numbers are not read (BUG-19, below) |
| 6 | A | Q19 sentence with digits (`... plus de 5000 euros avec un seul fournisseur ...`) | **PASS** | The group-threshold refusal at once, identical for both models (no model involved). First live proof of `group_threshold_not_supported`. The refusal is followed by the generic "Essayez de demander une somme ..." block (accepted) |
| 7 | A | `Combien de factures pour Alfa ?` | **PASS** | Refused at once, no "a été interrogé" line (BUG-12 for a value with no close match) |
| 8 | A | `Combien de factures le 23/01/2026 ?` | **PASS** | 0 rows with "date entre 2026-01-23 et 2026-01-23" and the zero-row note. Wording of a single day is clumsy (UX-5) |
| 9 | B | Q24, `gemma2:2b` | **PASS-WITH-ISSUES** | "1 200,00 euros HT." The 2 215 is gone. The answer is the quote's amount; the expected 1 450 is not produced (needs lot D) |
| 10 | B | Q24, `ministral-3:3b` | **PASS-WITH-ISSUES** | "1 200,00 € HT (extrait [1])." Same remark; short and cited |
| 11 | B | Q23, `gemma2:2b` | **PASS** (terse) | "1 200,00 euros HT. 1450 euros. Non." Both figures and the verdict are right; the wording is poor (model quality) |
| 12 | B | Q23, `ministral-3:3b` | **PASS** | Quote 1 200,00 HT, invoices 1 450, "Non ... ne correspond pas" |
| 13 | both | Q9 | **PASS** | 2 215, no workbook question, with both and with the invoices alone |
| 14 | appointments | Q14 | **PASS** | 4, one "Compris comme" clause |
| 15 | appointments | Q16 | **PASS** | Salle 3, 50; ranking 50, 95, 100 |

No regression against the HAP-1 PASS entries (Q9, Q14, Q16, Q23 with `ministral-3:3b`). Not observed in the
paste and to confirm visually: the Q24 notice "Le total de vos tables n'a pas pu être rattaché ..." (the owner
pasted the answer text only), and the absence of any model line on Alfa.

### Q24 after B2, honestly

Both answers state the quote's total (1 200,00 HT). That is a defensible reading of "the amount for the supplier
mentioned in this letter" but it is not the protocol's expected 1 450 (the supplier's invoices). The point of the
B2 change was to remove the wrong figure (the whole table's 2 215): done. The expected figure needs the document's
entity (MedSupply) tied to a table row for a question that names no value, which is lot D (BUG-06).

### New findings

| ID | Severity | Effort | Title |
| --- | --- | --- | --- |
| BUG-19 | S4 | M | A threshold written in words ("cinq mille") is not read; the product refuses honestly (the model cannot plan it either, OBS-6). Nothing in the next lots depends on it |
| UX-5 | S4 | XS | A single day is displayed as "date entre 2026-01-23 et 2026-01-23"; "date = ..." reads better (copy only) |

### Status of the lot B and B2 findings after the replay

| ID | Status |
| --- | --- |
| BUG-02 | Closed: Q14, Q16, Q19 digits confirmed live |
| BUG-05, BUG-15 | Closed (Q9 live; the sum itself in `tests/tabular_hap1.rs`) |
| BUG-17 | Closed, confirmed live (400, 500, 200, 5 000) |
| BUG-18 | Fixed in code and unit-tested; the "model asked, answer kept" line was **not exercised live** (no "Demander à l'IA" on a computed value in this replay) |
| BUG-12 | Closed for a value with no close match (Alfa, live). Open for a missing sheet (Q20) |
| BUG-14 | Refused honestly (live); the capability itself is lot E |
| Q24 | Partly improved; the expected figure needs lot D |
| BUG-13 | Gender example confirmed live; the refused column is still not named |
| UX-1 | Not started |
| BUG-19, UX-5 | New, not blocking |

### Do the remaining issues block the next lots?

No. See [04-improvement-plan.md](04-improvement-plan.md), "Triage after the lot B2 replay". Nothing in lot C
(interface, memory, folder router, scroll) reads the tabular filter code or the mixed-tier table block; lot D owns
Q24 and the data-only precondition; BUG-19 and UX-5 are independent copy or capability items.
