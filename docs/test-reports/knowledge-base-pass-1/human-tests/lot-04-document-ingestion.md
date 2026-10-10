# Human test - lot 4, Entities from documents

Written for the owner, who runs it. English, with the French strings of the interface quoted as data.

| | |
| --- | --- |
| Branch to be on | `feat/kb-document-ingestion` |
| Build | Part A needs no application: `cargo test` from `apps/desktop/src-tauri`. Part B: `cd apps/desktop` then `pnpm tauri dev` |
| Services | Part A: none. Part B: `docker compose up -d` from the repository root (the helper script can do it), chat and embedding models loaded. Step B3 stops it on purpose |
| Duration | Part A about 5 minutes; Part B about 35 minutes (B7 and B10 are optional, 8 minutes together); the smoke test about 10 minutes |
| Visible in the application? | **Partly.** On screen: the new lines under the Documents card's analysis summary (« n personnes et organisations repérées ... »), the new progress label while documents analysed earlier are read again, and the two new choices in « Réglages ». The names themselves are not shown anywhere in the interface yet (the management dialog and the picker are lots 7 and 8); you read them with the read-only script `show_kb_entities.py` |

**Nothing in this test asks you to edit a file by hand.** Every change to a file is done by one script,
`lot04_helper.py`; every setting you change is a choice in « Réglages ».

## What this proves, and what it does not

It proves that **pressing Analyser** (and only that: nothing runs at start-up or in the background) teaches the knowledge
base the names in your documents: who is named, in which file, how sure the reading is; that pressing it again changes
nothing; that a changed or a deleted file takes its associations with it; that an index analysed **before** the knowledge
base existed is filled in by an Analyse, from the text the index already holds, **with the AI machine stopped**; that the
« Désactivée » choice leaves no trace; that the health module has a Swiss option (AVS / AHV number); and that an e-mail
address, a social security number, an AVS number or an IBAN is never written as itself in the knowledge base.

It does not prove that the names are *right* in general: the extractor is deterministic and makes mistakes on a
title-cased phrase (defect KBD-11: two of the ten names found in the pilot's documents). Nothing uses the names to answer
a question yet (lot 6), so an answer cannot be better or worse because of this lot; that is what Part C checks.

The agent ran the whole scenario of Parts A and B as automated tests (`tests/knowledge_documents.rs`) and ran every command
of the helper script on a temporary copy of the fixtures; **it did not open the application window.** What you see on
screen in Parts B and C is yours to confirm.

## Preparation

1. Close the application. In a terminal at the repository root (Python is the one that runs `show_kb_tables.py`):

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot04_helper.py backup
python docs\test-reports\knowledge-base-pass-1\human-tests\lot04_helper.py status
```

   `backup` copies your index, your settings and the identifier key (if there is one) to `C:\Users\<you>\kb-backup-lot-04`.
   `status` shows what the script sees; check that « Documents folder » is `...\Docs\Test`. To go back at the very end:
   close the application and run the same command with `restore`.
2. The fictional fixtures of the first human pass are in the Documents folder (as in the earlier lots): the files of
   `docs/test-reports/human-acceptance-pass-1/fixtures/documents` in `C:\Users\<you>\AssistantCabinetAI\Docs\Test`. Do not
   use real files.
3. The three scripts you will use, always from the repository root, in the folder
   `docs\test-reports\knowledge-base-pass-1\human-tests\`:
   - `lot04_helper.py <command>` - changes files for you (list of commands: run it with no argument);
   - `show_kb_tables.py` - row **counts** of the index and of every `kb_` table;
   - `show_kb_entities.py` - the **names** found, the files that mention them and how each was found. For the fictional
     fixtures only: a list of the names found in a practice's real files is personal data.

## Part A - what a pass learns from the pilot's documents (no application)

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| A1 | From `apps/desktop/src-tauri`: `cargo test --test knowledge_documents print_what_a_pass_learns -- --ignored --nocapture` | A `summary:` line: `entities_detected: 10, matched_existing: 0, created_new: 10, candidates: 4, identifiers: 2, refreshed_sources: 0, errors: 0`, then 12 lines. People: `Alice Exemple` (2026/mars/neurologie.pdf), `Antoine Martin` (convention-remplacement-dr-martin.docx), `Camille Exemple` (bail and convention), `Hugo Exemple` (janvier neurologie and the CPAM letter), `Jean Fontaine` (bail). Organisations: `CABINET DE NEUROLOGIE` (both neurologie files, active), `CPAM du Rhone` and `Cabinet Exemple` (candidates). Identifiers: `DV-2026-0117` (the quote) and `health_insurance_number 8c7628` (the fictitious social security number of the CPAM letter, shown as a label and six digits of its hash; the six digits differ on your machine). Candidates that are wrong (defect KBD-11): `Conseil de l'Ordre` (an organisation filed as a person) and `Neuf Ans` (a phrase of the lease) | A different count with no edit of yours, a name you can see in a document that is missing, a failure |
| A2 | **Judge it.** Read the 12 lines as the person who will live with them | Your judgement. Write what you think in the Findings table | |
| A3 | `cargo test --test knowledge_documents` and `cargo test --test knowledge_packs` | `test result: ok. 43 passed ... 3 ignored` and `ok. 18 passed` | A failure |

## Part B - in the application

Start `pnpm tauri dev` (interface in French) with the services up (`lot04_helper.py start-ai` if they are not). The
buttons are the Documents card's « Analyser » and « Reset », and « Réglages » at the top right. After each Analyse, read
the lines under the card; then run the script named in the step.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| B1 | On the Documents card press « Reset » and confirm. Then `python ...\show_kb_tables.py` | The folder reads as never analysed. In the script: `chunks 0`, `kb_entities 0`, and no `documents` line under « Sources by folder » (a `data` line may remain if you analysed the Data folder earlier) | Entities left after a Reset |
| B2 | **The off choice.** « Réglages » -> « Base de connaissances » -> **« Désactivée »**, close « Réglages ». Press « Analyser ». Then `show_kb_tables.py` and `lot04_helper.py status` | No line about people or organisations under the card. In the script `chunks` is not 0 (the documents are indexed) and `kb_entities`, `kb_aliases`, `kb_mentions`, `kb_sources` are all **0**: not one knowledge row was written. `status` says « Identifier key : absent » | A knowledge line, a row in `kb_sources`, a key file |
| B3 | **An index analysed before the knowledge base, with the AI stopped.** « Réglages » -> « Base de connaissances » -> **« Activée »**, close. Run `lot04_helper.py stop-ai`. Press « Analyser » (the documents did not change). Watch the card while it runs, then read the lines. Then `show_kb_entities.py` | While it runs the bar is labelled « Lecture des noms dans les documents déjà analysés ». No error banner, although the AI is stopped. Under the card: `10 personnes et organisations repérées, dont 0 déjà connues.`, `2 identifiants repérés (adresses e-mail, numéros de facture, IBAN...).` and `6 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA.` The script lists the **same entities as Part A**, and `status` now says « Identifier key : present » | A grey « Analyser » button, an error banner, anything that needed the AI machine |
| B4 | `lot04_helper.py start-ai`, wait for the models. Press « Analyser » twice. Compare `show_kb_tables.py` before and after | No line about people (nothing changed). Every count in the script is the same before and after, and `kb_meta` has the same values in `show_kb_entities.py` | A new line under the card, a count that moved |
| B5 | **A fresh pass.** « Reset » and confirm, then « Analyser » | `10 personnes et organisations repérées, dont 0 déjà connues.` and `2 identifiants repérés (...)`. **No** « relus » line: these documents were analysed by this very pass. The script lists the entities of Part A | Different counts; a line saying documents « n'ont pas pu être entièrement analysés » |
| B6 | **A changed file.** `lot04_helper.py edit-convention` (it replaces `Antoine Martin` by `Benoit Lambert`, twice, in `convention-remplacement-dr-martin.docx`; it keeps the original). Press « Analyser ». Then `show_kb_entities.py` | The summary counts one document analysed, and the knowledge line counts only the names of **that** document: `3 personnes et organisations repérées, dont 2 déjà connues.` (`Camille Exemple` and `Conseil de l'Ordre` were known, `Benoit Lambert` is new). In the script `Antoine Martin` is **gone**, `Benoit Lambert` is there, mentioned in the convention only; `Camille Exemple` is still mentioned in the bail **and** the convention. Then `lot04_helper.py restore-convention` and « Analyser » to put it back | `Antoine Martin` still listed; a name of another file lost |
| B7 | *(optional)* **A deleted file.** `lot04_helper.py hide-cpam` (moves `courrier-cpam-radiation.pdf` out of the folder). « Analyser ». `show_kb_entities.py`. Then `lot04_helper.py restore-cpam` and « Analyser » | After the removal the summary says the document was removed and forgotten. In the script `CPAM du Rhone` and the `health_insurance_number` entity are gone, `Hugo Exemple` is still there (the janvier neurologie letter names him as well). After the restore all three are back | `Hugo Exemple` lost with the first file |
| B8 | **An identifier is not written as itself.** `lot04_helper.py add-identifiers` (creates `essai-identifiants.txt` with an invented e-mail address and IBAN). « Analyser ». Then `show_kb_entities.py`, then `show_kb_entities.py --look-for claire.martin@exemple.fr --look-for "FR76 3000 6000 0112 3456 7890 189"` | The first lists `identifier/email   email xxxxxx` and `identifier/iban   iban xxxxxx` (a kind and six hexadecimal digits, never the address or the number). The second says, for each text, `knowledge base tables (kb_*): not found` and `stored document text (chunks): found in 1 chunk(s)`: the knowledge base holds a hash, while **the document's own text is, as ever, in the index** (written in `docs/PRIVACY-AND-SECURITY.md`) | `FOUND` for the knowledge base tables |
| B9 | **The Swiss option.** `lot04_helper.py add-swiss` (creates `essai-suisse.txt` with an invented AVS number and Swiss IBAN). « Analyser ». `show_kb_entities.py` -> no Swiss number is listed (the module is on « France »). Then « Réglages » -> « Module santé » -> **« Santé : Suisse (numéro AVS, IBAN et téléphone suisses) »**, close, « Analyser » again. `show_kb_entities.py`, then `show_kb_entities.py --look-for 756.1234.5678.97` | With « France »: no entity for the Swiss file's numbers. After choosing « Suisse » and analysing, the line `N documents déjà analysés relus pour y repérer les noms` appears (the vocabulary changed, so the documents are read again with no new request). The Swiss module **replaces** the French one, it is not added to it: the script lists a `health_insurance_number` and an `iban` for `essai-suisse.txt`, **and the French social security number of the CPAM letter is no longer listed** (the Swiss module does not read it), and `CPAM du Rhone` is no longer an organisation. The `--look-for` answer: `kb_*: not found`, `chunks: found in 1 chunk(s)`. Then choose « Santé : France » again and « Analyser »: the Swiss entities disappear and the French ones come back | An AVS number read with « France »; a French number still listed with « Suisse »; the AVS number or the IBAN found in a `kb_` table |
| B10 | *(optional)* **A lost key.** Close the application. `lot04_helper.py delete-key`. Start the application, « Analyser ». `show_kb_entities.py` | The e-mail and the IBAN of B8 are still listed, each **once**, with **different** six-digit labels from B8 (the old hashes were collected), and `status` says the key is present again | Two entities for the same address |

When you are done: `lot04_helper.py remove-identifiers`, `remove-swiss`, then « Analyser » so the folder is as before; make sure
« Base de connaissances » reads « Activée » and « Module santé » reads « Santé : France » (the defaults); close the application
and, if you want your index and settings back exactly as they were, `lot04_helper.py restore`.

## Part C - smoke test, the product as before

The lot touches the pass every Analyse runs, so what must hold is that every answer is the same as before the lot.

| # | Do | Expected |
| --- | --- | --- |
| S0 | Start the application. Press « Analyser » on the Documents card, then on the Data card | Both cards behave as before; no error banner |
| S1 | All documents selected. Ask `Combien de fichiers au total ?` | 6, answered « sans l'IA » |
| S2 | All documents selected. Ask `Que dit le courrier de la CPAM concernant la radiation ?` | Hugo Exemple struck off the general scheme from 1 February 2026; the sources list `courrier-cpam-radiation.pdf`, page 1 |
| S3 | All documents selected. Ask `Quelle est la durée du bail du cabinet d'après le contrat ?` | Nine years, 1 April 2024 to 31 March 2033 |
| S4 | Only the invoices workbook selected, no document. Ask `Quel est le maximum de montant ?` | 620 |
| S5 | The quote `devis-imprimante-medsupply.pdf` and the invoices selected. Ask `Le montant total facturé par MedSupply correspond-il à ce qui est indiqué dans le devis signé ?` | 1 450 (table) against 1 200,00 HT (quote), both sides cited |
| S6 | Only `bail-cabinet-2024.pdf` selected. Ask `Que dit bail-cabinet-2024.pdf ?` | An answer from that file only |
| S7 | Nothing selected. Ask `Rédige un e-mail de relance pour un fournisseur dont la facture n'est pas réglée.` | An answer labelled « Réponse sans vos documents » |
| S8 | On each card press « Reset » and confirm, then « Analyser » again | Both folders are analysed again as in a first pass; the Documents card adds its knowledge lines |

Use the model you normally test with (`ministral-3:3b` was the best of the eight small models compared on 9 October). If a
model's sentence is wrong while the sources and the figures are right, that is the model, not this lot (defects KBD-03 and
KBD-06). If any answer differs from a run on `kb/integration` before this lot, note the question and the answer; do not
guess at the cause.

## Results table

| Step | Expected | Observed | OK? | Screenshot |
| --- | --- | --- | --- | --- |
| A1 | | | | |
| A2 | | | | |
| A3 | | | | |
| B1 | | | | |
| B2 | | | | |
| B3 | | | | |
| B4 | | | | |
| B5 | | | | |
| B6 | | | | |
| B7 | | | | |
| B8 | | | | |
| B9 | | | | |
| B10 | | | | |
| S0 | | | | |
| S1 | | | | |
| S2 | | | | |
| S3 | | | | |
| S4 | | | | |
| S5 | | | | |
| S6 | | | | |
| S7 | | | | |
| S8 | | | | |

## Findings

A bug found here is fixed on `feat/kb-document-ingestion` (a new commit, the pull request updated) before the merge block is
used. A name you think should be found, or should not, is a finding for the open questions of
`lots/lot-04-document-ingestion.md` (and probably for defect KBD-11), not necessarily a bug: the reading is deliberately
cautious. An idea goes to the open questions.

| # | What was seen | Bug, edit or idea | Fixed in |
| --- | --- | --- | --- |
| | | | |
