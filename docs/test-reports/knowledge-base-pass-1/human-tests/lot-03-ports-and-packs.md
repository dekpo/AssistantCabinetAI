# Human test - lot 3, Ports, packs and resolver

Written for the owner, who runs it. English, with the French strings of the interface quoted as data.

| | |
| --- | --- |
| Branch to be on | `feat/kb-ports-and-packs` |
| Build | Part A needs no application: `cargo test` from `apps/desktop/src-tauri`. Part B: `cd apps/desktop` then `pnpm tauri dev` |
| Services | Part A: none. Part B: `docker compose up -d` from the repository root, chat and embedding models loaded, as for any pass |
| Duration | Part A about 15 minutes; Part B about 10 minutes (the smoke test) |
| Visible in the application? | **No.** This lot adds the vocabulary files, the interfaces and the rules that decide whether a name is one the knowledge base already holds. Nothing reads them yet (lot 4 does), so no screen changes. Part A is what you can read and edit; Part B only proves the product still behaves as before |

## What this proves, and what it does not

It proves that the vocabulary the knowledge base will read names with (the lexicon packs) loads, in both languages and
in every combination, and that the resolver answers the way the plan says in the situations that matter: a name in any
spelling, a surname shared by two people, a name outside the files you selected, a name you deleted, a misspelling.
The expected answers are a JSON file you can read and edit, like the table of sounds of lot 2.

It does not prove anything about documents: no file is read, no name is stored, nothing is shown in the interface.
The agent ran everything in Part A itself; **it did not open the application window**, so Part B is yours to confirm.

## Preparation

1. Part A: a terminal in `apps/desktop/src-tauri`. Nothing to back up.
2. Part B only: close the application and back up what a build may alter (PowerShell):

```text
$backup = "$env:USERPROFILE\kb-backup-lot-03"
New-Item -ItemType Directory -Force $backup
Copy-Item "$env:LOCALAPPDATA\com.assistantcabinetai.desktop\index.sqlite3" $backup
Copy-Item "$env:APPDATA\com.assistantcabinetai.desktop\settings.json" $backup
```

   To go back: close the application and copy both files back.
3. Part B only: the fictional fixtures of the first human pass, as in lots 0 and 1:
   `docs/test-reports/human-acceptance-pass-1/fixtures/documents` in `C:\Users\<you>\AssistantCabinetAI\Docs\Test` and
   `.../fixtures/data` in `C:\Users\<you>\AssistantCabinetAI\Data\Test`. Do not use real files.

## Part A - the packs and the resolver (no application)

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| A1 | `cargo test --test knowledge_packs print_the_summary_of_every_pack -- --nocapture --test-threads=1` | One line per pack and language, then one "(all)" line per language. For the lexicon as shipped: `base fr-FR` has 12 titles, 2 weak titles, 15 particles, 57 stop-words, 5 identifier schemes, 18 column rules, 3 relations, 7 roles; `health`, `legal` and `accounting` add up to 4 titles, 9 to 12 stop-words, 1 or 2 schemes, 3 or 4 column rules and 14 or 15 detection terms each. "(all)" reads `9 schemes, 29 column rules, 4 relations, 7 roles` in both languages, with a different hash for French and English, then a line `loading and validating every pack (fr-FR): ... ms each, debug build` (about 40 ms on the development PC; for orientation only). The test passes | A different count with no edit of yours, a missing line, a failure |
| A2 | Read `apps/desktop/src-tauri/resources/knowledge/base/fr-FR.json`. Is the vocabulary right for a French practice? The stop-words (months, "facture", "devis", "oui", "non"), the titles, the organisation markers, the header words (`nom`, `client`, `ville`, `téléphone`...) | Your judgement. This is the starting vocabulary, not a finished lexicon: every word you add or remove is a one-line edit. Note anything you would change in the Findings table | A French sentence in a list (the test in A5 refuses it) |
| A3 | Add a word of your own to `stop_words` of `base/fr-FR.json`, for example `"relance"`, then run `cargo test --test knowledge_packs` | All 18 tests pass, and A1 now shows 58 stop-words for `base fr-FR` (take the word out again to return to 57) | A failure caused by a plain word |
| A4 | Break a pack on purpose. In `resources/knowledge/packs/health/fr-FR.json`, add the line `"surprise": 1,` right under `"locale": "fr-FR",`. Run `cargo test --test knowledge_packs every_pack_loads_in_every_locale_alone_and_together`. Then remove the line | The test **fails** with `health fr-FR: KnowledgePackInvalid { pack: "health", path: "line 5 column 12" }` (the line of your edit; the column follows your indentation). After you remove the line it passes again | A pass (the typo went unnoticed), or a failure that names the wrong pack |
| A5 | Put a sentence in a list. In `base/fr-FR.json`, add to `stop_words` the entry `"Ceci est une phrase qui ne doit pas être là."`. Run `cargo test --test knowledge_packs a_pack_holds_words_and_phrases_never_a_sentence`. Remove it | The test **fails** with `baser-FR.json stop_words[0]: 10 words is a sentence, not a word: "Ceci est une phrase..."` | A pass |
| A6 | `cargo test --test knowledge_resolver the_resolver_cases_hold -- --nocapture` | A table of 28 rows (16 questions, 12 names read in a file), every row ending in `yes`, and the test passes. Read the rows against the note column of `tests/fixtures/knowledge/resolver-cases.json` | A `NO` row |
| A7 | **Read the answers as the person who will live with them.** Five rows decide how the knowledge base will behave; say whether each is what you want (details in the table below this one) | Your judgement; write it in the Findings table | |
| A8 | Add a row of your own to `resolver-cases.json` (in `cases`), for example `{ "mode": "query", "text": "Dr Moreau", "selection": [1], "outcome": "existing", "who": ["Hélène Moreau"] }`, and run A6 again. Then change its `outcome` to `ambiguous` | With the right outcome the table shows 29 rows, all `yes`. With the wrong one the test **fails** and names the row (`"Dr Moreau" ("query"): answer existing`) | A pass on a wrong row |
| A9 | `cargo test --test knowledge_packs --test knowledge_resolver` | `test result: ok. 18 passed` and `ok. 46 passed` | A failure |

The five rows of A7 (question -> what the resolver answers):

| Situation (the invented knowledge base: Jean and Marie Dupont, Pierre and Paul Martin, Hélène Moreau) | Answer | Question for you |
| --- | --- | --- |
| In a question, files 1 and 2 selected, you write `Dupont` | **ambiguous**, Jean and Marie: the product asks which | Right? |
| In a question, only file 1 selected, you write `Dupont` (Marie is in file 2) | **existing**, Jean: a surname is enough when exactly one person answers inside what you selected, and Marie is never mentioned | Right? Or should it always ask? |
| In a question you write `J. Dupont` | **possible match**, Jean: "did you mean", never assumed | Right? |
| In a document being analysed, `J. Dupont` appears and Marie Dupont is also in that document | **new entity** (a separate, unconfirmed name): the knowledge base does not guess who `J.` is | Right? |
| In a document being analysed, `Dr Martin` appears and two Martins are known | **nothing recorded**: no guess, no new alias | Right? It means a document that only says `Dr Martin` will not be found when you ask about Pierre Martin. The alternative is to record it at low confidence |

## Part B - smoke test, the product as before

Build and start (`pnpm tauri dev`), interface in French, Documents folder `...\Docs\Test`, Data folder `...\Data\Test`.
This lot adds a dependency (the pattern engine), one error code and some catalogue entries, so what must hold is that the
application starts and every answer is the same as before.

| # | Do | Expected |
| --- | --- | --- |
| S0 | Start the application. Press **Analyser** on the Documents card, then on the Data card | Both cards behave as before; no error banner |
| S1 | All documents selected. Ask `Combien de fichiers au total ?` | 6, answered "sans l'IA" |
| S2 | All documents selected. Ask `Que dit le courrier de la CPAM concernant la radiation ?` | Hugo Exemple struck off the general scheme from 1 February 2026; the sources list `courrier-cpam-radiation.pdf`, page 1 |
| S3 | All documents selected. Ask `Quelle est la durée du bail du cabinet d'après le contrat ?` | Nine years, 1 April 2024 to 31 March 2033 |
| S4 | Only the invoices workbook selected, no document. Ask `Quel est le maximum de montant ?` | 620 |
| S5 | The quote `devis-imprimante-medsupply.pdf` and the invoices selected. Ask `Le montant total facturé par MedSupply correspond-il à ce qui est indiqué dans le devis signé ?` | 1 450 (table) against 1 200,00 HT (quote), both sides cited |
| S6 | Only `bail-cabinet-2024.pdf` selected. Ask `Que dit bail-cabinet-2024.pdf ?` | An answer from that file only |
| S7 | Nothing selected. Ask `Rédige un e-mail de relance pour un fournisseur dont la facture n'est pas réglée.` | An answer labelled "Réponse sans vos documents" |
| S8 | On each card press **Réinitialiser** and confirm, then **Analyser** again | Both folders are analysed again as in a first pass |

Use the model you normally test with (`ministral-3:3b` was the best of the eight small models compared on 9 October). If
a model's sentence is wrong while the sources and the figures are right, that is the model, not this lot (defects
KBD-03 and KBD-06). If any answer differs from a run on `kb/integration` before this lot, note the question and the
answer; do not guess at the cause.

## Results table

| Step | Expected | Observed | OK? | Screenshot |
| --- | --- | --- | --- | --- |
| A1 | | | | |
| A2 | | | | |
| A3 | | | | |
| A4 | | | | |
| A5 | | | | |
| A6 | | | | |
| A7 | | | | |
| A8 | | | | |
| A9 | | | | |
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

A bug found here is fixed on `feat/kb-ports-and-packs` (a new commit, the pull request updated) before the merge block is
used. A word you want in or out of a pack is an edit, not a bug: make it, run A3, and say so. An idea goes to the open
questions of `lots/lot-03-ports-and-packs.md`.

| # | What was seen | Bug, edit or idea | Fixed in |
| --- | --- | --- | --- |
| | | | |
