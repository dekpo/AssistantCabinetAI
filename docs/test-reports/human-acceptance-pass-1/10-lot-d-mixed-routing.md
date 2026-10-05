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
