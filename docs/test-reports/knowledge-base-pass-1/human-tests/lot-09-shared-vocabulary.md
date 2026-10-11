# Human test - lot 9, Suggestions and the sheet of a name

Written for the owner, who runs it. English, with the French strings of the interface quoted as data. **Every command is written in full and can be pasted as it is** (`.cursor/rules/kb-programme.mdc`, "Exact commands in a human test" and "A human test is a script, not a list").

| | |
| --- | --- |
| Branch to be on | `feat/kb-shared-vocabulary` |
| Build | From `apps\desktop`, `pnpm tauri dev` |
| Services | **The AI machine must be up**: four steps send a question to the model (C2, C4, D6, the smoke test) and the analyses embed the documents. The address in « Réglages » must answer. With the tower of the local network, nothing to do; on the local machine, `docker compose up -d` from the repository root |
| Duration | Preparation 8 minutes; Part A 2; Part B 4; Part C 8; Part D 10; Part E 8; Part F 4; Part G 2; smoke test 5; Part H (optional) 2; final restore 2. About 55 minutes. You can stop after any part: each one ends with a stated state, and the restore at the end works from anywhere |
| Visible in the application? | **Partly.** Visible: the questions to try (in the empty conversation and at the bottom of the Documents card), the button that shows the names, the sheet of a name (from a chip and from the `@` list). **Not visible:** the ranked names for speech, the repair of a transcript and the outbound guard are Rust services with no screen; see "What stays invisible" |

## Status

**First run by the owner, 12 October 2026.** Matched: Preparation, A, B, C, D, E (the content of the sheet), G (`OK`), and the answer of every suggestion she clicked. Findings, **all product defects of this lot, fixed in the branch**: (1) the small **i** was invisible on the highlighted row of the `@` list and on the chip (same colours as the background); it now always looks like a hovered button, with a border, in both themes; (2) **the blocks disappeared at the bottom of the Documents card after a second « Analyser »** (so « Masquer les noms » did nothing there, step H): the block was drawn inside the analysis summary, which is empty after a pass that changed nothing and sits inside a disclosure; it is now drawn at the end of the card, whatever the summary says; (3) **in « Réglages » the two folder cards were squashed**: the sheet's style rules used the class name `card`, which the folder cards already use; the sheet now has its own names (`entity-card`); (4) wording: the title is now **« Suggestions »** and the line under it **« Questions suggérées à partir de vos fichiers »** (the promise « elle a une réponse » was dropped: nothing is affirmed that cannot be proved). **Re-run needed:** Preparation, then A, B (B1 and the new B3), E1, E2, F (the new F0), S1, S3 and H. Parts C, D and G are unchanged.

## What this proves, and what it does not

It proves, in the running application, that **the suggested questions you click have an answer** (you click them and read what comes back; nothing here says that every question the product could suggest has one); that **names are hidden in the suggestions until you ask to see them** and that your click lasts the session only; that **the list follows the selection** (all documents, three documents, a spreadsheet, two spreadsheets); that **a name is never written into a suggestion when the setting hides it**; that **the sheet of a name lists only the files of your selection and only counts the others**; and that **no file of either folder is touched** (a script compares every file, byte for byte, before and after).

It does not prove that the model answers a document question well: the steps judge the *route* (which files the sources name) and not the sentences. It does not prove anything about speech, the repair of a transcript or the outbound guard: nothing in the window calls them.

**Two limits you will meet, both recorded as defects and not fixed in this lot.** (1) With documents **and two spreadsheets** selected, a question that names no column is answered « Quel classeur ? » by the existing product, even when it is about the documents (KBD-21): so, in that selection, no question about the documents is suggested (step D7). (2) A person written with a title in a column (`Dr Jean Dupont`) is read as « 0 rows » by the existing product when other doctors of the column are written `Dr ...` too (KBD-24): the suggestions say `Jean Dupont`, which works.

**What the agent ran, and what only you can see.** The agent ran every automated suite (numbers in the lot report), every helper command of this protocol in order on temporary copies of the settings and the folders (`ACAI_CONFIG_DIR`, `ACAI_DATA_DIR`, `ACAI_BACKUP_DIR`), and every suggestion of every selection below through the same Rust code the window calls, then through the real classifier and the real folder router, in French and in English: the lists and the figures come from there (`cargo test --test knowledge_surfaces_walk -- --ignored --nocapture`). **The agent did not open the application window and clicked nothing**: every label and every click is unverified on screen until you run it. The sentences of the model's answers are not predicted.

## How to read this protocol

- **One line = one command**, typed in the order given, from `C:\Users\elise\Documents\CURSOR\AssistantCabinetAI`.
- Each part begins with **« You start with »** and ends with **« You leave with »**. If your state is not the one written, stop and say so: do not guess.
- **Nothing is undone before the very last step**, « Final restore ». Part H is optional, comes last, and brings its own undo.
- After a tick, **wait about two seconds**: the list is looked for half a second after the selection stops changing.
- « The list at the bottom of the Documents card » means the block « Suggestions » at the **bottom of the card « Dossier des documents »**, below the list of files. « The list in the conversation » is the same block in the middle of the empty conversation, which disappears once a question has been asked.

## The commands you will use (copy them)

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot09_helper.py status
python docs\test-reports\knowledge-base-pass-1\human-tests\lot09_helper.py fingerprint
python docs\test-reports\knowledge-base-pass-1\human-tests\lot09_helper.py verify-files
python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_entities.py
```

`lot09_helper.py` changes files and settings (close the application first, it refuses otherwise; `fingerprint`, `verify-files` and `status` may run with the application open). `show_kb_entities.py` prints the names the knowledge base holds, read-only.

## The documents and the spreadsheets

`lot09_helper.py prepare` writes the thirty-one documents of lot 8 in a folder `kb-lot09` beside your Documents folder, and two spreadsheets in a folder `kb-lot09-data` beside your Data folder (all invented):

- **Dr Jean Dupont**: `dupont-bilan-2026.txt`, `dupont-lettre-confrere.txt`, `dupont-horaires.txt` (and `rappel-m-dupont.txt`, which only says « M. Dupont »). **Mrs Claire Martin**: `martin-controle.txt`, `martin-ordonnance.txt`. **Dr Paul Martin**, **Dr Pierre Martin**: one document each. Twenty-two notes and `note-jeannot.txt`.
- **`rdv.csv`** (five appointments in March 2026): columns `date`, `patient`, `medecin`, `duree_min`, `montant`. The `medecin` column holds `Dr Jean Dupont` (three rows), `Dr Paul Martin`, `Dr Pierre Martin`; `montant` totals 160 for Dr Jean Dupont. Patients: Claire Martin, Alice Archer, Emma Evans.
- **`factures.csv`** (three invoices): `date`, `fournisseur`, `montant`. `MedSupply` totals 1070, `PapeterieDuParc` 180.

## Preparation

**You start with:** the application closed, your own settings (your Documents folder **and** your Data folder chosen), the AI machine up.

1. One line at a time:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot09_helper.py backup
python docs\test-reports\knowledge-base-pass-1\human-tests\lot09_helper.py status
python docs\test-reports\knowledge-base-pass-1\human-tests\lot09_helper.py prepare
```

   `backup` copies your index, settings and identifier key to `C:\Users\elise\kb-backup-lot-09`. `prepare` makes `kb-lot09` and `kb-lot09-data`, writes the invented files in them, points the Documents folder and the Data folder at them, sets the mode to `suggest`, ticks « Masquer les noms dans les suggestions » and switches the timing log on. Your own files are never touched; `restore` puts your folders back.
2. Start the application:

```text
cd apps\desktop
pnpm tauri dev
```

3. **Empty the knowledge base of what your own earlier analyses taught it.** On the card « Dossier des documents », click **« Reset »**, then **« Reset »** in the window « Effacer l'analyse ? ». Do the same on the card « Dossier des données » (« Effacer l'analyse des données ? »). **Expected:** both cards say that nothing is analysed.
4. On the card « Dossier des documents », click **« Analyser »**. **Expected:** thirty-one documents analysed, and the line **« 6 personnes et organisations repérées, dont 0 déjà connue. »**
5. On the card « Dossier des données », click **« Analyser »**. **Expected:** `rdv.csv` and `factures.csv` green, and among the lines of the summary **« 8 personnes et organisations repérées, dont 4 déjà connues. »** and **« 3 colonnes de tableur reconnues (personnes, organisations, identifiants...) dans 2 tableurs lus. »**
6. One line at a time:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_entities.py
python docs\test-reports\knowledge-base-pass-1\human-tests\lot09_helper.py fingerprint
```

   **Expected from the first:** active people `Alice Archer`, `Claire Martin`, `Emma Evans`, `Jean Dupont`, `Paul Martin`, `Pierre Martin`; active organisations `MedSupply`, `PapeterieDuParc`; two guesses (candidates) `cabinet des Lilas` and `cabinet du Parc`. **The second prints** `remembered 33 files of ...kb-lot09 and ...kb-lot09-data`.

**You leave with:** the application running on `kb-lot09` and `kb-lot09-data`, mode `suggest`, « Masquer les noms dans les suggestions » ticked, both folders analysed, **nothing ticked** in either card, an empty conversation, the 33 files remembered.

## Part A - Nothing is selected, nothing is suggested

**You start with:** what Preparation left.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| A1 | Look at the Documents card and at the Data card. Read the line that says what is used | « Documents utilisés : aucun » and « Données utilisées : aucune » | Another selection |
| A2 | Look at the conversation (the middle of the window) | The welcome text only: **« Posez une question »** and the two lines under it. **No « Suggestions »** | A list of questions with nothing selected |
| A3 | Look at the bottom of the Documents card | Nothing more: no « Suggestions » | A list |

**You leave with:** nothing ticked, an empty conversation, no list anywhere.

## Part B - All the documents, names hidden

**You start with:** what Part A left.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| B1 | On the Documents card open **« Documents utilisés »** and tick **« Tous les documents »**. Wait two seconds | « Documents utilisés : tous ». **In the conversation**, under the welcome text: the title **« Suggestions »**, the line **« Questions suggérées à partir de vos fichiers »**, **one** button **« Combien de documents ai-je ? »**, then **« Les noms sont masqués : l'écran peut être vu par d'autres personnes. »** and a button **« Afficher les suggestions avec des noms »**. **At the bottom of the Documents card**: the same block, shorter (the title, the same button, the note and the button; no intro line) | A name (Dupont, Martin) in either block; more than one question; no block |
| B2 | In the conversation, click **« Combien de documents ai-je ? »** | The sentence appears as your message and the answer arrives **at once** (no waiting for the AI): a sentence that gives **31** (« 31 documents ont été analysés. »), with the line **« Réponse établie depuis votre dossier de documents, sans l'IA. »**. The block in the conversation is gone (the conversation is no longer empty); the block at the bottom of the Documents card stays | A wait of several seconds; another number; a model name under the answer |

| B3 | On the Documents card click **« Analyser »** again (nothing changed, it ends at once) | The block « Suggestions » is **still** at the bottom of the card, with its one button « Combien de documents ai-je ? » and the note. *(Before the fix it vanished here.)* | The block gone |

**You leave with:** « Documents utilisés : tous », one question and its answer in the conversation, names hidden, the block at the bottom of the Documents card showing one button.

## Part C - Show the names, then change the selection

**You start with:** what Part B left.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| C1 | At the bottom of the Documents card, click **« Afficher les suggestions avec des noms »** | After about a second the block lists **two** buttons, in this order: **« Que disent mes documents à propos de Jean Dupont ? »** and **« Combien de documents ai-je ? »**. The note and the button are replaced by **« Masquer à nouveau les noms »** | Another name; the block unchanged |
| C2 | Click **« Que disent mes documents à propos de Jean Dupont ? »** | The sentence is sent as your question. **The AI answers** (about 25 seconds on the tower of the network). The answer speaks of Dr Jean Dupont (the blood test, the letter to a colleague or the opening hours). Under it, **« Sources »** holds at least one `dupont-…` file. No « Quel classeur ? », no error | « Quel classeur ? »; an answer about someone else; an error |
| C3 | On the Documents card open « Documents utilisés », **untick « Tous les documents »**, then tick `martin-controle.txt`, `martin-ordonnance.txt` and `note-01.txt`. Wait two seconds | « Documents utilisés : 3 fichiers ». The block lists **three** buttons, in this order: **« Que disent mes documents à propos de Claire Martin ? »**, **« Résume chaque document. »**, **« Combien de documents ai-je ? »** | Jean Dupont still offered; a button missing |
| C4 | Click **« Résume chaque document. »** | The AI answers with a summary **of each of the three documents**; **« Sources »** lists exactly those three files | Another file in the sources; only one document summarised |
| C5 | Click **« Combien de documents ai-je ? »** | At once, no AI: a sentence that gives **3** (« 3 documents ont été analysés. ») with « Réponse établie depuis votre dossier de documents, sans l'IA. » | 31 |
| C6 | Click **« Masquer à nouveau les noms »**. Then click **« Afficher les suggestions avec des noms »** | After the first click the block lists **two** buttons, **« Résume chaque document. »** and **« Combien de documents ai-je ? »**, then the note and the button « Afficher… » again. After the second, the three buttons of C3 are back | A name while hidden |

**You leave with:** `martin-controle.txt`, `martin-ordonnance.txt` and `note-01.txt` ticked, no spreadsheet, **names shown**, a conversation holding the steps B2, C2, C4, C5.

## Part D - With spreadsheets

**You start with:** what Part C left.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| D1 | On the Documents card open « Documents utilisés » and **tick « Tous les documents »** (« Documents utilisés : tous »). On the Data card open **« Données utilisées »** and tick **only `rdv.csv`**. Wait two seconds | « Données utilisées : 1 fichier ». The block lists **six** buttons, in this order: **« Que disent mes documents à propos de Jean Dupont ? »**, **« Combien de lignes pour Jean Dupont ? »**, **« Quelle est la somme de montant pour Jean Dupont ? »**, **« Combien de lignes pour Jean Dupont en mars 2026 ? »**, **« Que disent mes documents et mes tableaux à propos de Jean Dupont ? »**, **« Quelle est la somme de montant par patient ? »**. **No `Dr`** in any of them | « Dr Jean Dupont » in a sentence; fewer than six; « Combien de documents ai-je ? » (it is not offered with a spreadsheet selected) |
| D2 | Click **« Combien de lignes pour Jean Dupont ? »** | At once, no AI: **« Nombre de lignes : 3. »**, with `rdv.csv` among the sources and the line **« Réponse calculée depuis votre dossier des données, sans l'IA. »** | 0; a question « Quel classeur ? » |
| D3 | Click **« Quelle est la somme de montant pour Jean Dupont ? »** | **« Somme de montant : 160 »** (it may be written 160,00) | 0; a refusal |
| D4 | Click **« Combien de lignes pour Jean Dupont en mars 2026 ? »** | **« Nombre de lignes : 3. »**, with a line that says how the question was understood (the month and the year) | 0 |
| D5 | Click **« Quelle est la somme de montant par patient ? »** | **« Totaux de montant, par groupe : »** then **Alice Archer 120**, **Claire Martin 100**, **Emma Evans 80** (in some order) | Other totals |
| D6 | Click **« Que disent mes documents et mes tableaux à propos de Jean Dupont ? »** | The AI answers (25 to 60 seconds). It speaks of Dr Jean Dupont. No « Quel classeur ? » | « Quel classeur ? » |
| D7 | On the Data card **tick `factures.csv` as well** (« Données utilisées : 2 fichiers »). Wait two seconds | The block lists **four** buttons, in this order: **« Combien de lignes pour Jean Dupont dans rdv.csv ? »**, **« Quelle est la somme de montant pour Jean Dupont dans rdv.csv ? »**, **« Combien de lignes pour Jean Dupont en mars 2026 dans rdv.csv ? »**, **« Quelle est la somme de montant par fournisseur dans factures.csv ? »**. **No question about the documents** (KBD-21: it would earn « Quel classeur ? ») | A « Que disent mes documents ... » button |
| D8 | Click **« Quelle est la somme de montant par fournisseur dans factures.csv ? »**, then **« Combien de lignes pour Jean Dupont dans rdv.csv ? »** | First: **« Totaux de montant, par groupe : »** **MedSupply 1070**, **PapeterieDuParc 180**. Second: **« Nombre de lignes : 3. »** | A question about which workbook |
| D9 | Click **« Masquer à nouveau les noms »** | The block lists **one** button, **« Quelle est la somme de montant par fournisseur dans factures.csv ? »**, then the note and the button « Afficher les suggestions avec des noms » | A name |

**You leave with:** « Documents utilisés : tous », « Données utilisées : 2 fichiers » (`rdv.csv`, `factures.csv`), names hidden, a conversation holding B2 to D8.

## Part E - The sheet of a name

**You start with:** what Part D left.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| E1 | In the box type `@Dup` | A list with one row: **« Jean Dupont · Personne · 4 fichiers »** (the three documents and `rdv.csv`), and at the right of the row a small round **« i »** with a light background and a border, **clearly visible even though the row is highlighted** (look at the dark theme too if you use it) | A second row; another count |
| E2 | Click the **« i »** of that row (**not** the name) | A window opens: title **« Fiche de Jean Dupont »**, under it **« Personne »**. **« Autres écritures »**: Dupont Jean · J. Dupont · Dupont · Docteur Dupont. **« Rôles »**: **« Prestataire (3 mentions) »**. **« Dans les documents choisis »**: `dupont-bilan-2026.txt : 2 mentions, page 1`, `dupont-horaires.txt : 1 mention, page 1`, `dupont-lettre-confrere.txt : 2 mentions, page 1`. **« Dans les tableaux choisis »**: `rdv.csv, feuille rdv, colonne medecin : 3 lignes`. **« Liens »**: « Claire Martin suivi par Jean Dupont » and « Alice Archer suivi par Jean Dupont ». No « Hors de la sélection ». The box still reads `@Dup` and **no chip** was added | `rappel-m-dupont.txt` listed; a chip added; a machine word (`served_by`, `provider`) |
| E3 | Click **« Fermer »**. Click in the box and press **Enter** | The name is chosen: the box reads `Jean Dupont` and a **chip « Jean Dupont »** shows above the box, with an **« i »** (round, light, bordered in the green of the chip) and a **×** | An invisible « i » |
| E4 | Click the **« i »** of the chip | The same window as in E2. Click **« Fermer »** | A different content |
| E5 | Click the **×** of the chip (« Retirer Jean Dupont »), then empty the box (Ctrl+A, Delete) | No chip, empty box | |
| E6 | Data card: **untick `rdv.csv` and `factures.csv`** (« Données utilisées : aucune »). Documents card: **untick « Tous les documents »** and tick **`dupont-bilan-2026.txt` and `note-01.txt`** (« Documents utilisés : 2 fichiers »). In the box type `@Dup`, then click the **« i »** of the row **« Jean Dupont · Personne · 1 fichier »** | The window: **« Dans les documents choisis »** holds only `dupont-bilan-2026.txt : 2 mentions, page 1`. **No « Rôles », no « Dans les tableaux choisis », no « Liens »**. A line: **« Hors de la sélection : 2 dans les documents, 1 dans les tableaux. La sélection n'est pas changée. »** The names `dupont-horaires.txt`, `dupont-lettre-confrere.txt` and `rdv.csv` appear **nowhere** in the window. Click « Fermer », empty the box | A file outside the selection named |

**You leave with:** `dupont-bilan-2026.txt` and `note-01.txt` ticked (« Documents utilisés : 2 fichiers »), no spreadsheet (« Données utilisées : aucune »), names hidden, an empty box, no chip, no window open.

## Part F - The mode, off and back

**You start with:** what Part E left.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| F0 | Click **« Réglages »** and scroll to the bottom | The two cards **« Dossier des documents »** and **« Dossier des données »** show in full (title, folder, the four buttons, the counts), **not squashed** into a small scrolling strip | Two flat cards with a scrollbar |
| F1 | Click **« Fermer »**, then look at the bottom of the Documents card | The block lists **two** buttons, **« Résume chaque document. »** and **« Combien de documents ai-je ? »**, then the note and the button « Afficher les suggestions avec des noms » | |
| F2 | Click **« Réglages »**. In the group **Connaissances**, choose **Ciblage intelligent** **« Désactivé »**, click **« Fermer »**. Wait two seconds. Type `@Dup` in the box, then empty it | The block at the bottom of the Documents card has **gone**. The `@` gives **no list** | The block stays |
| F3 | **« Réglages »**, **Ciblage intelligent** **« Suggestions seulement »**, **« Fermer »**. Wait two seconds | The block is back with the same two buttons | The block does not come back |

**You leave with:** mode `suggest`, `dupont-bilan-2026.txt` and `note-01.txt` ticked, names hidden, the block showing two buttons.

## Part G - No file was touched

**You start with:** what Part F left.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| G1 | Run `python docs\test-reports\knowledge-base-pass-1\human-tests\lot09_helper.py verify-files` | `33 files now, 33 remembered` then **`OK: every file is byte for byte the same, none added, none removed`** | CHANGED, REMOVED or ADDED |

**You leave with:** the same state, 33 files verified.

## Smoke test - the product as it was

**You start with:** what Part G left.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| S1 | Documents card: tick **« Tous les documents »**. Click **« Analyser »** | Ends quickly (nothing changed): « 31 documents » already analysed, no error. At the bottom of the card the block « Suggestions » lists **« Combien de documents ai-je ? »** (names hidden) and the note | An error; a long re-analysis; no block |
| S2 | In the box type `Que dit le Docteur Jean Dupont du bilan sanguin ?` and press Enter | The AI answers about the blood test; **« Sources »** holds `dupont-bilan-2026.txt` | An error; « Quel classeur ? » |
| S3 | Data card: tick **only `rdv.csv`**. Wait two seconds: the block lists **« Quelle est la somme de montant par patient ? »**. In the box type `Combien de lignes pour Jean Dupont ?` and press Enter | **« Nombre de lignes : 3. »** | 0 |

**You leave with:** « Documents utilisés : tous », `rdv.csv` ticked, the conversation longer by two turns, mode `suggest`.

## Part H (optional) - Names always shown

**You start with:** what the smoke test left. **This part undoes itself.**

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| H1 | **« Réglages »**, **untick** « Masquer les noms dans les suggestions », « Fermer ». Wait two seconds | The block at the bottom of the Documents card lists the six questions that name Jean Dupont at once (all documents and `rdv.csv` are ticked), with **no** note about hidden names and **no** button « Afficher les suggestions avec des noms » (and no « Masquer à nouveau »: nothing was revealed by a click) | A note about hidden names |
| H2 | **Undo:** **« Réglages »**, **tick** « Masquer les noms dans les suggestions », « Fermer ». Wait two seconds | The questions that name Jean Dupont are gone; the note and the button « Afficher... » are back | |

**You leave with:** « Masquer les noms dans les suggestions » ticked again, as Preparation set it.

## Final restore

**You start with:** the application running.

1. Close the application (close the window; stop `pnpm tauri dev` with Ctrl+C in its terminal).
2. One line:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot09_helper.py restore
```

   **Expected:** `restored index.sqlite3`, `restored settings.json`, and `removed knowledge-identifier.key` only if you had none. The folders `kb-lot09` and `kb-lot09-data` stay on disk; delete them if you wish.

**You leave with:** your own index, settings and folders, as before the test.

## What stays invisible

| Service | Why nothing in the window shows it | What to look at instead |
| --- | --- | --- |
| Ranked names for speech and autocomplete (`knowledge::surface`) | Nothing calls it yet; the `@` list is lot 7's | `cargo test --test knowledge_surfaces -- --nocapture` prints nothing; the tests `speech_hints_are_capped_in_names_and_in_tokens` and `a_title_is_spoken_by_the_packs_and_a_typed_pronunciation_wins` are the proof |
| Repair of a transcript (`knowledge::repair`) | Waits for the speech work | The contract file `apps\desktop\src-tauri\tests\fixtures\knowledge\repair-fr.json` and the test `the_repair_contract_file_holds` |
| Outbound guard (`knowledge::guard`) | Waits for the connectors; no command returns anything to the window on purpose | The tests that begin with `the_guard_` and the ignored benchmark `the_guard_scans_two_kilobytes_against_ten_thousand_names_in_under_five_milliseconds` |

## Results table (empty, for you to fill)

| Step | Expected | Observed | OK? | Screenshot |
| --- | --- | --- | --- | --- |
| Preparation 3 to 6 | | | | |
| A1 to A3 | | | | |
| B1, B2 | | | | |
| C1 to C6 | | | | |
| D1 to D9 | | | | |
| E1 to E6 | | | | |
| F1 to F3 | | | | |
| G1 | | | | |
| S1 to S3 | | | | |
| H1, H2 (optional) | | | | |

## Findings

A bug found here is fixed in the lot branch (a new commit, the same pull request) before the merge. An idea goes to the lot report's open questions. A deviation you report is a finding about this protocol first: the agent says which of the two was wrong and gives the short ordered re-run it now needs.
