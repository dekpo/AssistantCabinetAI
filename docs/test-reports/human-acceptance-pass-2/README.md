# HAP-2 - Human acceptance pass 2: the complete protocol, in one file

**What this is.** The second full manual test of Assistant Cabinet AI, written so that **one person can run it
from this file alone**, without opening any other document: setup, test data and its verified answers, every
question to type, the expected result, and a place to tick the verdict. It reuses every question of the first pass
(HAP-1, 3 October 2026, 28 questions) and adds what was built afterwards (lots A to E: thresholds, row lookup,
mixed documents-and-tables answers, the mail merge, the interface fixes). Run it after any significant change; the
verdicts of two runs on the same fixtures are comparable.

**Who can run it.** The owner, or any tester given this file and the test folder. Nothing here needs a real
patient file: **every document and table is fictional** (`AGENTS.md`, rules 2 and 6). Never run it on real files.

**How long.** About 25 minutes for the sections that use no model (D, E, G1-G6), then about 40 minutes per model for
the sections that do (B, C, F, G7 and after, H). Use at least two models (the smallest and the best one available).

**How to use it.** Copy this file to `results-<your-name>-<date>.md` next to it, tick `PASS` or `FAIL` in each row,
and write what you saw in the *Note* column when it is not the expected result (copy the exact text of the answer).
The last section tells you how to total it up.

Conventions: `Type exactly` is what you type in the chat, character for character. "No model" means the answer
must appear at once, with the line *"... sans l'IA"* under it and no "Généré par ..." line. A tick box is `☐`.

---

## 0. Before you start

### 0.1 The machine and the run (fill in)

| Item | Value |
| --- | --- |
| Tester and date | |
| Commit tested (`git rev-parse --short HEAD`) | |
| Operating system | |
| Interface language (Settings) | fr-FR for everything except test H6 |
| Models tested (the names shown in the model selector at the bottom right of the chat). From 7 October 2026 a pass that ends in a choice of model uses only the free-licence families of `docs/MODELS.md`; models outside them (Gemma 2/3, Llama, MedGemma) may still be run as witnesses but are not candidates | |
| Context window of each model (the line `MODEL_CONTEXT_WINDOWS` of the `.env` file; default 8192) | |
| `LLM_REQUEST_TIMEOUT_SECONDS` in `.env` (default 180) | |

### 0.2 Start the program

1. In a terminal at the repository root: `docker compose up -d`, then `docker compose ps`. The two services
   `server` and `ollama` must read `healthy`. (Open WebUI is not needed.)
2. Check the models exist: `docker compose exec ollama ollama list` must list the models you will test.
3. In a second terminal: `cd apps/desktop`, then `pnpm tauri dev`. Keep this terminal open and **copy its output
   if anything hangs** before closing the app.
4. The red banner "IA indisponible" must not be shown. If it is, stop here and fix the server first.

### 0.3 Create the test folders (once)

The two test folders live **outside** `Documents`, `OneDrive` and `iCloud` (the program refuses a synced folder).
In PowerShell, from the repository root:

```powershell
$base = "$env:USERPROFILE\AssistantCabinetAI-HAP2"
$fx   = "docs\test-reports\human-acceptance-pass-1\fixtures"
New-Item -ItemType Directory -Force "$base\Docs", "$base\Data" | Out-Null
Copy-Item -Recurse -Force "$fx\documents\*" "$base\Docs"
Copy-Item -Force "$fx\data\*" "$base\Data"
Get-ChildItem -Recurse "$base" | Select-Object FullName
```

You must see, under `Docs`: `bail-cabinet-2024.pdf`, `convention-remplacement-dr-martin.docx`,
`courrier-cpam-radiation.pdf`, `devis-imprimante-medsupply.pdf`, `2026\janvier\neurologie.pdf`,
`2026\mars\neurologie.pdf` (**6 files**); under `Data`: `factures-fournisseurs-2026.xlsx` and
`rdv-mars-2026.xlsx` (**2 workbooks**). Nothing else yet: the mail-merge files are added in section G.

In the program: Settings → choose `...\AssistantCabinetAI-HAP2\Docs` as the documents folder and
`...\AssistantCabinetAI-HAP2\Data` as the data folder; press **Analyse** in both cards. Every file must show a green
dot (analysed), 0 unreadable.

### 0.4 What the test data says (so you can check every answer here)

**Documents** (all fictional, "document fictif, usage de test uniquement")

| File | What matters |
| --- | --- |
| `bail-cabinet-2024.pdf` | Professional lease: nine years, from 1 April 2024 to 31 March 2033. Annual rent 18 400 euros |
| `convention-remplacement-dr-martin.docx` | Dr Antoine Martin replaces Dr Camille Exemple from Monday 6 July 2026 to Friday 31 July 2026 inclusive. Retrocession of 80 % of the fees collected; the other 20 % goes to the titular doctor (for the premises, equipment and patients). Signed in Lyon on 2 June 2026, in three originals. **Nothing about the lease** |
| `courrier-cpam-radiation.pdf` | The health fund informs that M. Hugo Exemple was struck off the general scheme from 1 February 2026 (new employer); paper forms dated after that go to the new fund |
| `devis-imprimante-medsupply.pdf` | Quote DV-2026-0117 of 8 January 2026 from MedSupply. One lump sum: **1 200,00 euros HT** for printer + 4 cartridges + 10 reams + miscellaneous supplies. **No separate printer price.** Complementary orders are invoiced separately |
| `2026/janvier/neurologie.pdf` | Letter about M. Hugo Exemple, consultation of 14 January 2026 (tension headaches, no imaging needed) |
| `2026/mars/neurologie.pdf` | Letter about Mme Alice Exemple, consultation of 9 March 2026 (paresthesia of the arms for three months, an electromyogram is proposed). **Same file name as the previous one, on purpose** |

**`factures-fournisseurs-2026.xlsx`**, sheet `Factures`, 8 rows

| date | fournisseur | montant |
| --- | --- | --- |
| 2026-01-10 | MedSupply | 450 |
| 2026-01-22 | Fournitures Dupont | 180 |
| 2026-02-05 | MedSupply | 620 |
| 2026-02-14 | Papeterie Lefevre | 95 |
| 2026-02-28 | Fournitures Dupont | 210 |
| 2026-03-03 | MedSupply | 380 |
| 2026-03-11 | Papeterie Lefevre | 120 |
| 2026-03-19 | Fournitures Dupont | 160 |

Totals: **all 2 215**; maximum line **620**; **MedSupply 1 450** (3 rows), **Fournitures Dupont 550** (3 rows),
**Papeterie Lefevre 215** (2 rows). Lines above 400: **2** (450, 620); above 500: **1** (620); above 200: **4**
(450, 620, 210, 380); below 200: **4** (180, 95, 120, 160). No invoice is dated 23 January.

**`rdv-mars-2026.xlsx`**, sheet `RDV`, 10 rows

| date | salle | patient | duree_min |
| --- | --- | --- | --- |
| 2026-03-02 | Salle 1 | Dupont J | 20 |
| 2026-03-03 | Salle 2 | Martin A | 30 |
| 2026-03-04 | Salle 1 | Bernard L | 15 |
| 2026-03-05 | Salle 3 | Petit M | 25 |
| 2026-03-09 | Salle 1 | Dupont J | 20 |
| 2026-03-10 | Salle 2 | Girard P | 35 |
| 2026-03-11 | Salle 1 | Roux S | 25 |
| 2026-03-12 | Salle 3 | Fontaine R | 25 |
| 2026-03-16 | Salle 1 | Lambert T | 20 |
| 2026-03-20 | Salle 2 | Simon V | 30 |

**Mean of duree_min 24,5**; Mondays **3** (2, 9, 16 March); appointments from 9 to 15 March **4**; rows per room
**5 / 3 / 2** (Salle 1 / 2 / 3); minutes per room: **Salle 1 = 100, Salle 2 = 95, Salle 3 = 50**.

### 0.5 Rules for the whole pass

1. **One section at a time.** Press *Effacer la conversation* at the start of each section, and wherever a test says
   "new conversation": the conversation is sent back to the model with each question and can change its answer.
2. **The selection is part of the test.** Set the two lists exactly as the block heading says ("Documents utilisés",
   "Données utilisées" in the sidebar). Ticking two workbooks when a test needs one makes the program, correctly, ask
   which.
3. **Check every number against section 0.4.** "The application answered" is not "the answer is right".
4. **Model tests:** run each with each model; write the model name and the time shown under the answer ("Généré par
   ... en ...") in the *Note* column when it differs between models.
5. **Do not scroll by hand** before judging whether the end of an answer is visible (test A5).
6. When something hangs: copy the terminal output of `pnpm tauri dev`, then close.

---

## A. Questions about the documents folder (no model)

**Selection:** Documents: all 6 files. Data: nothing. *(New conversation.)*

| # | Type exactly | Expected | HAP-1 | Result | Note |
| --- | --- | --- | --- | --- | --- |
| A1 | `Combien de fichiers au total ?` | "Votre dossier de documents contient 6 fichiers." No model. | PASS | ☐ PASS ☐ FAIL | |
| A2 | `Combien de fichiers PDF ?` | 5 files with the extension .pdf. No model | PASS | ☐ PASS ☐ FAIL | |
| A3 | `Peux-tu me donner la liste des fichiers ?` | The six names, each "analysé": 2026/janvier/neurologie.pdf, 2026/mars/neurologie.pdf, bail-cabinet-2024.pdf, convention-remplacement-dr-martin.docx, courrier-cpam-radiation.pdf, devis-imprimante-medsupply.pdf | PASS | ☐ PASS ☐ FAIL | |
| A4 | `Que dit neurologie.pdf ?` | "Plusieurs fichiers s'appellent neurologie.pdf. Lequel voulez-vous ?" with **one button per file** (`2026/janvier/neurologie.pdf`, `2026/mars/neurologie.pdf`). Never a choice made for you | PASS (list, retyped by hand) | ☐ PASS ☐ FAIL | |
| A4b | Click the button `2026/mars/neurologie.pdf` | Your question is replaced in place by one naming that path, and the answer is about Mme Alice Exemple, consultation of 9 March 2026, paresthesia, electromyogram proposed. No retyping | new | ☐ PASS ☐ FAIL | |
| A5 | (no typing) Look at A1 to A4 | The **end of every answer is visible** without scrolling by hand; the arrow "go to latest message" is not needed | FAIL | ☐ PASS ☐ FAIL | |

## B. Questions about the content of the documents (model)

**Selection:** Documents: all 6. Data: nothing. Run each test with **each model**. *(New conversation per model.)*

| # | Type exactly | Expected | HAP-1 | Result | Note |
| --- | --- | --- | --- | --- | --- |
| B1 | `Que dit le courrier de la CPAM concernant la radiation ?` | M. Hugo Exemple struck off the general scheme from 1 February 2026 because of a new employer; later paper forms go to the new fund. Cites `courrier-cpam-radiation.pdf`. Pasting the whole letter instead of answering is a weak result: note it | PASS | ☐ PASS ☐ FAIL | |
| B2 | `Résume la convention de remplacement signée avec le Dr Martin.` | 6 to 31 July 2026; 80 % to the replacement, 20 % to the titular; signed 2 June 2026 (a missing signature date is a minor omission). **Must not mention the lease** | PASS-WITH-ISSUES | ☐ PASS ☐ FAIL | |
| B3 | `Quelle est la durée du bail du cabinet d'après le contrat ?` | Nine years (1 April 2024 to 31 March 2033) | PASS | ☐ PASS ☐ FAIL | |
| B4 | `Quel est le prix de l'imprimante dans le devis MedSupply ?` | The quote is selected: the **total** 1 200,00 euros HT, and that the printer alone is not priced. **Never another figure.** If the answer states a figure that is not in the quote (for example 80), a line must appear under it: *"Le chiffre « 80 » ne figure dans aucun des extraits utilisés ni dans votre question : vérifiez-le dans vos documents."* | FAIL | ☐ PASS ☐ FAIL | |
| B5 | New conversation, **untick** `devis-imprimante-medsupply.pdf`, same question as B4 | A refusal: no document mentions the quote or the price. **No figure at all.** The words "extrait(s)" appear, never the English "excerpt(s)"; never `WORK_FOLDER_CONTEXT` | PASS-WITH-ISSUES | ☐ PASS ☐ FAIL | |
| B6 | Follow-up: new conversation, all 6 ticked. Type B1, then `Et la convention de remplacement ?` | The follow-up is understood (the answer is about the replacement agreement). The quality of the text depends on the model; note a non-answer that only names the file | new | ☐ PASS ☐ FAIL | |

**B7 - Memory must not beat the sources (do it 3 times per model; it is long but it is the test of the
conversation-memory rule).** For each run:

1. New conversation, all 6 documents ticked. Type `Quel est le prix de l'imprimante dans le devis MedSupply ?` and
   note the answer (variant **a**).
2. In the **same** conversation, untick the quote and type the same question again. Note the answer (variant **e**).
3. *Effacer la conversation*, quote still unticked, type the same question once more. Note the answer (variant **f**).

| Run | Model | (e) states a figure that is in none of the selected documents? | (e) equals (f)? | Result | Note |
| --- | --- | --- | --- | --- | --- |
| 1 | | ☐ no (good) ☐ yes | ☐ yes ☐ no | ☐ PASS ☐ FAIL | |
| 2 | | ☐ no (good) ☐ yes | ☐ yes ☐ no | ☐ PASS ☐ FAIL | |
| 3 | | ☐ no (good) ☐ yes | ☐ yes ☐ no | ☐ PASS ☐ FAIL | |

*Pass:* in (e) the answer never repeats a figure the unticked quote contained or that a model invented in (a); (e) and
(f) are both refusals. HAP-1: (e) FAIL, (f) PASS with one model.

## C. Nothing selected: bounded general help (model)

**Selection:** untick everything (Documents: none, Data: none). *(New conversation.)*

| # | Type exactly | Expected | HAP-1 | Result | Note |
| --- | --- | --- | --- | --- | --- |
| C1 | `Rédige un e-mail de relance pour un fournisseur dont la facture n'est pas réglée.` | A drafted e-mail, with the line "Réponse sans vos documents" under it. A model that refuses to draft is a weak result: note it | gemma FAIL-ish / larger PASS | ☐ PASS ☐ FAIL | |
| C2 | `Quel est le délai légal de conservation des dossiers médicaux en France ?` | The question **reaches the model** (an answer, with "Réponse sans vos documents" and "Généré par ..."), and does **not** say "Aucun document sélectionné". The legal period itself is the model's own knowledge and differs between models: **record each answer, do not trust it** | FAIL | ☐ PASS ☐ FAIL | |

## D. The invoices workbook alone (no model unless stated)

**Selection:** Documents: none. Data: tick **only** `factures-fournisseurs-2026.xlsx`. *(New conversation.)*

| # | Type exactly | Expected | HAP-1 | Result | Note |
| --- | --- | --- | --- | --- | --- |
| D1 | `Quel est le maximum de montant ?` | 620 | PASS | ☐ PASS ☐ FAIL | |
| D2 | `Quelle est la somme des montant ?` | "Somme de montant : 2 215", calculated over 8 lignes | PASS | ☐ PASS ☐ FAIL | |
| D3 | `Quel fournisseur a le plus de montant ?` | MedSupply, 1 450, with the note that it is a total per supplier, not the largest line | PASS | ☐ PASS ☐ FAIL | |
| D4 | `Quel est le total de montant par fournisseur ?` | MedSupply 1 450, Fournitures Dupont 550, Papeterie Lefevre 215 | PASS | ☐ PASS ☐ FAIL | |
| D5 | `Combien de factures pour le fournisseur MedSupply ?` | 3 | PASS | ☐ PASS ☐ FAIL | |
| D6 | `Combien de factures de plus de 400 euros ?` | **2**, "Compris comme : montant > 400" | FAIL (wrong filter) | ☐ PASS ☐ FAIL | |
| D7 | `Combien de factures de plus de 500 euros ?` | **1**, "montant > 500" | - | ☐ PASS ☐ FAIL | |
| D8 | `Combien de factures de moins de 200 euros ?` | **4**, "montant < 200" | - | ☐ PASS ☐ FAIL | |
| D9 | `Combien de factures de plus de 5 000 euros ?` | **0**, "montant > 5 000", then the note "Aucune ligne ne correspond à ces critères. Vérifiez la ligne « Compris comme » ..." | FAIL (invented year) | ☐ PASS ☐ FAIL | |
| D10 | `Combien de factures de plus de 400 euros pour le fournisseur MedSupply ?` | **2** (450 and 620), "fournisseur = MedSupply" and "montant > 400" | - | ☐ PASS ☐ FAIL | |
| D11 | `Est-ce qu'on a dépensé plus de 5000 euros avec un seul fournisseur ce trimestre ?` | "Aucun fournisseur n'a un total de montant supérieur à 5 000. Le plus haut total est **1 450** (MedSupply)", with the note that it is the total per supplier. **Not** "Nombre de lignes : 8" | FAIL x4 | ☐ PASS ☐ FAIL | |
| D12 | `Y a-t-il un fournisseur avec plus de 500 euros ?` | Totals above 500 (2 groupes): MedSupply 1 450, Fournitures Dupont 550 | - | ☐ PASS ☐ FAIL | |
| D13 | `Y a-t-il un fournisseur avec moins de 300 euros ?` | Totals below 300 (1 groupe): Papeterie Lefevre 215 | - | ☐ PASS ☐ FAIL | |
| D14 | `Combien de factures pour Alfa ?` | « Alfa » ne correspond à aucune valeur réelle de ...; the columns are listed; **at once, with no "a été interrogé" line** | PASS-WITH-ISSUES (17 s to 1 min 12 s of model time) | ☐ PASS ☐ FAIL | |
| D14b | Ask D14 a second time, in a row | The same refusal plus the line "Votre question précédente n'a pas pu être calculée non plus : vérifiez que la valeur est écrite exactement comme dans le fichier." | - | ☐ PASS ☐ FAIL | |
| D15 | `Quelle est la somme de cumul dans Calculs ?` | A refusal: that sheet does not exist; the real columns are named (date, fournisseur, montant). The model may be asked first: note the time | PASS-WITH-ISSUES | ☐ PASS ☐ FAIL | |
| D16 | `Quelle est la somme de fournisseur ?` | A refusal that **names the column**: "Colonne concernée : fournisseur." | FAIL | ☐ PASS ☐ FAIL | |
| D17 | `Combien de factures le 23/01/2026 ?` | **0**, "Compris comme : date = 2026-01-23" (not "entre ... et ..."), then the "Aucune ligne ne correspond" note | - | ☐ PASS ☐ FAIL | |
| D17b | `Combien de factures le 22/01/2026 ?` | **1** (Fournitures Dupont, 180), "date = 2026-01-22", "Calculé sur 1 ligne" | - | ☐ PASS ☐ FAIL | |
| D18 | `Quelle est la somme des montant ?` then click **Demander à l'IA** under the answer | Two good outcomes: the value stays and a line says the model was asked and the computed answer is kept; **or** the value is shown as interpreted by the model. Never a silent identical answer. 2 215 in every case | FAIL (button did nothing) | ☐ PASS ☐ FAIL | |

## E. The appointments workbook, and both workbooks (no model)

**Selection:** Documents: none. Data: tick **only** `rdv-mars-2026.xlsx`. *(New conversation.)*

| # | Type exactly | Expected | HAP-1 | Result | Note |
| --- | --- | --- | --- | --- | --- |
| E1 | `Combien de rendez-vous le lundi ?` | 3 | PASS | ☐ PASS ☐ FAIL | |
| E2 | `Combien de rendez-vous entre le 09/03/2026 et le 15/03/2026 ?` | **4**, with "Compris comme : date entre 2026-03-09 et 2026-03-15" and **nothing else** (no year, no number) | FAIL (0) | ☐ PASS ☐ FAIL | |
| E3 | `Quelle est la moyenne de duree_min ?` | 24,5 over 10 lignes | PASS | ☐ PASS ☐ FAIL | |
| E4 | `Quelle salle a le moins de duree_min ?` | **Salle 3, 50**; ranking Salle 3 50, Salle 2 95, Salle 1 100; "total par salle" note | FAIL (Salle 2, 30) | ☐ PASS ☐ FAIL | |
| E5 | `Quelle salle a le plus de duree_min ?` | Salle 1, 100 (still a ranking, not a threshold) | - | ☐ PASS ☐ FAIL | |
| E6 | `Combien de rendez-vous par salle ?` | Salle 1: 5, Salle 2: 3, Salle 3: 2 | PASS | ☐ PASS ☐ FAIL | |

**Selection:** Data: tick **both** workbooks. *(New conversation.)*

| # | Type exactly | Expected | HAP-1 | Result | Note |
| --- | --- | --- | --- | --- | --- |
| E7 | `Quelle est la somme des montant ?` | **2 215 at once**, with no "which workbook" question (only the invoices have a column `montant`) | PASS (it asked) | ☐ PASS ☐ FAIL | |
| E8 | `Combien de lignes ?` | The program asks which workbook, with **one button per workbook** | PASS (retype) | ☐ PASS ☐ FAIL | |
| E8b | Click the button of `factures-fournisseurs-2026.xlsx` | The question is replaced by one naming that workbook and the engine answers for it (8 lignes) | new | ☐ PASS ☐ FAIL | |

## F. Documents and tables together (the mixed tier)

**Selection:** Documents: tick **only** `devis-imprimante-medsupply.pdf`. Data: tick **only**
`factures-fournisseurs-2026.xlsx`. *(New conversation. F2 to F4 need a model: run them with each model.)*

| # | Type exactly | Expected | HAP-1 | Result | Note |
| --- | --- | --- | --- | --- | --- |
| F1 | `Quelle est la somme des montant ?` | **2 215 at once, no model**, with the lines "calculée depuis votre dossier des données, sans l'IA" and "les documents sélectionnés n'étaient pas nécessaires" | NOT RUN | ☐ PASS ☐ FAIL | |
| F2 | `Le montant total facturé par MedSupply correspond-il à ce qui est indiqué dans le devis signé ?` | The model answers (the question reaches it; "Demander à l'IA" is not needed). Under its text, the block **"Calculé dans vos données (par le moteur, pas par l'IA)"**: "Somme de montant : 1 450. Calculé sur 3 lignes. Compris comme : fournisseur = MedSupply." The verdict should be that they do **not** match (1 450 against 1 200,00 HT). **No line "La table indique 1 450, et non 450,00"** when the model writes 1 450,00 or 1 450 | FAIL (table half only) | ☐ PASS ☐ FAIL | |
| F3 | `Quel est le montant total pour le fournisseur mentionné dans cette lettre ?` | The block shows **1 450** with "Compris comme : fournisseur = MedSupply" (the supplier named in the quote is linked to its rows). **Never 2 215** | FAIL | ☐ PASS ☐ FAIL | |
| F4 | `Rédige un résumé de la somme des montant.` | The model writes a text; the block shows "Somme de montant : 2 215. Calculé sur 8 lignes." and no false correction line | - | ☐ PASS ☐ FAIL | |

**Selection:** Documents: tick **only** `courrier-cpam-radiation.pdf`. Data: `factures-fournisseurs-2026.xlsx`.
*(New conversation.)*

| # | Type exactly | Expected | HAP-1 | Result | Note |
| --- | --- | --- | --- | --- | --- |
| F5 | `Quel est le montant total pour le fournisseur mentionné dans le courrier de la CPAM ?` | **No total.** The notice "Le total de vos tables n'a pas pu être rattaché à ce que dit le document (par exemple le fournisseur cité) : aucun total global n'est affiché ...". **No** refusal about the word "CPAM". If the model states a figure that is in the letter nowhere, the line "Le chiffre « ... » ne figure dans aucun des extraits ..." appears | - | ☐ PASS ☐ FAIL | |

**Selection:** back to quote + invoices as for F1. **Stop the server:** in a terminal at the repository root,
`docker compose stop server`. *(New conversation.)*

| # | Type exactly | Expected | HAP-1 | Result | Note |
| --- | --- | --- | --- | --- | --- |
| F6 | `Le montant total facturé par MedSupply correspond-il à ce qui est indiqué dans le devis signé ?` | The table part answers without the server: "Somme de montant : 1 450 ... Compris comme : fournisseur = MedSupply", the label "calculée depuis votre dossier **des données**, sans l'IA" and the line "L'IA n'a pas pu être contactée pour rédiger la partie document." The red banner stays visible at the bottom of the sidebar even with both lists open | NOT REACHED | ☐ PASS ☐ FAIL | |

Then restart it: `docker compose start server`, and wait until the banner goes.

**Copy the conversation (no typing).** Go back to a conversation that contains F2, F3 and F4 (redo them if needed),
click **Copier la conversation** and paste into Notepad.

| # | Check | Expected | Result | Note |
| --- | --- | --- | --- | --- |
| F7 | The pasted text | Each answer is followed by its block "Calculé dans vos données ...", its "Généré par ..." line and its notes; **no list of "Sources"**; **no empty line inside an answer**; **one empty line between** one question-and-answer pair and the next | ☐ PASS ☐ FAIL | |
| F8 | The **Copier** button under one single answer | What is pasted reads like what is on screen (no internal name such as `WORK_FOLDER_CONTEXT`) | ☐ PASS ☐ FAIL | |

## G. The mail merge (publipostage)

**Add the test files** (PowerShell, repository root), then press **Analyse** in both cards:

```powershell
$base = "$env:USERPROFILE\AssistantCabinetAI-HAP2"
$p = "docs\test-reports\human-acceptance-pass-1\fixtures\publipostage"
Copy-Item -Force "$p\donnees_publipostage.xlsx" "$base\Data"
Copy-Item -Force "$p\modele_lettre.docx", "$p\modele_lettre.txt" "$base\Docs"
```

**The orders** (`donnees_publipostage.xlsx`, 6 lines, one per article)

| ID_Client | Civilite | Prenom | Nom | No_Commande | Article | Quantite | Prix_Unitaire | Total_Ligne |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 101 | M. | Jean | Dupont | CMD-2026-001 | Ordinateur Portable | 1 | 899 | 899 |
| 101 | M. | Jean | Dupont | CMD-2026-001 | Souris Sans Fil | 1 | 25.5 | 25.5 |
| 102 | Mme | Sophie | Martin | CMD-2026-002 | Smartphone 5G | 1 | 649 | 649 |
| 103 | M. | Thomas | Robert | CMD-2026-003 | Livre de Cuisine | 2 | 24.9 | 49.8 |
| 103 | M. | Thomas | Robert | CMD-2026-003 | Tablier en Coton | 1 | 15 | 15 |
| 103 | M. | Thomas | Robert | CMD-2026-003 | Set de Couteaux | 1 | 45 | 45 |

(The column `Email` follows `Nom`.) Sum of `Total_Ligne` for CMD-2026-001: **924,5**; for the whole table:
**1 683,3**. **Three orders, six lines.**

**The templates.** `modele_lettre.docx` holds the fields `«No_Commande»`, `«Civilite»`, `«Prenom»`, `«Nom»` and, in
a table row, `«Article»`, `«Quantite»`, `«Prix_Unitaire»`, `«Total_Ligne»`: **every name is exactly a column**.
`modele_lettre.txt` holds `[No_Commande]`, `[Civilite]`, `[Prenom]`, `[Nom]`, `[Article]`, `[Quantite]`, and the two
abbreviations **`[Prix_U]`** and **`[Total]`**, which are **not** column names (they begin `Prix_Unitaire` and
`Total_Ligne`).

### G-1. Finding a row (no model)

**Selection:** Documents: none. Data: tick **only** `donnees_publipostage.xlsx`. *(New conversation.)*

| # | Type exactly | Expected | Result | Note |
| --- | --- | --- | --- | --- |
| G1 | `Quelle est la commande CMD-2026-002 ?` | "1 ligne retenue, d'après No_Commande", one row: Sophie Martin (Mme), Smartphone 5G, 1, 649, 649; "Compris comme : No_Commande = CMD-2026-002"; "sans l'IA" | ☐ PASS ☐ FAIL | |
| G2 | `Quelle est la commande CMD-2026-001 ?` | "2 lignes retenues": Ordinateur Portable 899 and Souris Sans Fil 25.5 | ☐ PASS ☐ FAIL | |
| G3 | `Quelle est la commande CMD-2026-003 ?` | "3 lignes retenues": Livre de Cuisine (2, 24.9, 49.8), Tablier en Coton (15), Set de Couteaux (45) | ☐ PASS ☐ FAIL | |
| G4 | `Quelle est la commande CMD-2026-009 ?` | « CMD-2026-009 » ne correspond à aucune valeur réelle ...; "Valeurs proches trouvées : CMD-2026-001, CMD-2026-002, CMD-2026-003"; **at once, no model line**; never a row | ☐ PASS ☐ FAIL | |
| G5 | `Quelle est la somme de Total_Ligne pour la commande CMD-2026-001 ?` | **924,5**, calculée sur 2 lignes, "No_Commande = CMD-2026-001" | ☐ PASS ☐ FAIL | |
| G6 | `Quelle est la somme de Total_Ligne ?` | **1 683,3**, calculée sur 6 lignes | ☐ PASS ☐ FAIL | |

### G-2. Generating letters (no model: none is ever called)

**Selection:** Documents: tick **only** `modele_lettre.docx`. Data: only `donnees_publipostage.xlsx`. In the
file manager, check that **no `Generated` folder exists yet** inside `...\AssistantCabinetAI-HAP2\Docs`.
*(New conversation.)*

| # | Type exactly / do | Expected | Result | Note |
| --- | --- | --- | --- | --- |
| G7 | `Génère le courrier de la commande CMD-2026-002 en reprenant les données du client et en les insérant dans la lettre correspondante.` | **A card "Courrier à générer"**, not a text answer: "Données : donnees_publipostage.xlsx, No_Commande = CMD-2026-002"; Modèle `modele_lettre.docx`; "Seulement CMD-2026-002" selected; the eight fields each point at the column of the same name, **no "Confirmer" button**; the preview reads "Confirmation de votre commande n° CMD-2026-002", "Bonjour Mme Sophie Martin" and a table row Smartphone 5G \| 1 \| 649 \| 649; the line "Aucune IA n'a lu vos données ..." under it. **Nothing is written yet** (still no `Generated` folder) | ☐ PASS ☐ FAIL | |
| G8 | Press **Générer 1 courrier(s)** | "1 courrier(s) créé(s) ..." and the path `Generated/modele_lettre-CMD-2026-002.docx` with a **Voir** button that shows the file. Open it in Word: the letter is filled with one table row. `modele_lettre.docx` is **unchanged** (same modification time) | ☐ PASS ☐ FAIL | |
| G9 | Type G7 again and generate | A second file `modele_lettre-CMD-2026-002-2.docx`; the first one is unchanged (never overwritten) | ☐ PASS ☐ FAIL | |
| G10 | `Génère le courrier de la commande CMD-2026-001` then generate | The preview shows two lines (Ordinateur Portable, Souris Sans Fil); the Word letter has two table rows | ☐ PASS ☐ FAIL | |
| G11 | Also tick `modele_lettre.txt`. Type G7 again. In the card, choose `modele_lettre.txt` as the template | `Prix_U` and `Total` show **Confirmer**; the line "À confirmer avant de générer : Prix_U, Total."; the generate button is **disabled**; the preview still shows `[Prix_U]` and `[Total]` as written. Press Confirmer on both: the preview fills them (649 and 649) and the button is enabled. Generate: a `.txt` file is created in `Generated` | ☐ PASS ☐ FAIL | |
| G12 | Untick the `.txt`. Type `Génère un courrier pour chaque commande` | A card "Données : toutes les lignes de ..."; "Un courrier par : chaque ligne" announces **6** letters. Choose **No_Commande** in the list: **3** letters. Generate: `modele_lettre-CMD-2026-001.docx`, `-002`, `-003` appear; the 001 letter has **two** table rows, the 003 letter has Thomas Robert and **three** rows | ☐ PASS ☐ FAIL | |
| G13 | Settings → "Dossier des courriers générés": type `Courriers générés`, leave the field. Then generate one letter (G7 + Générer) | The field shows `Courriers-generes`; the letter goes in `Courriers-generes/` (in the documents folder), not in `Generated`. Put the field back to `Generated` afterwards | ☐ PASS ☐ FAIL | |
| G14 | Open the log: PowerShell `Get-Content "$env:LOCALAPPDATA\com.assistantcabinetai.desktop\generated-files.jsonl" -Tail 12` | One line per letter written: time, template, data file, key, output name. **No name of a person, no amount, no article** | ☐ PASS ☐ FAIL | |
| G15 | Untick `modele_lettre.docx` (no template left). Type G7 again | **No card.** The answer comes from the mixed tier, with the block "Calculé dans vos données" showing the row of CMD-2026-002, and **not** the sentence "Cette question ne portait sur aucune de vos tables" | ☐ PASS ☐ FAIL | |

## H. Interface and system

| # | What to do | Expected | Result | Note |
| --- | --- | --- | --- | --- |
| H1 | Ask any question that makes a model write at least 15 lines (B1 with a small model). While it streams, scroll **up** with the wheel; then scroll back to the bottom | While you read higher up, the view does not drag you down. Back at the bottom it follows the text again. The end of the finished answer is visible | ☐ PASS ☐ FAIL | |
| H2 | Over all the model tests above | You **never** saw `WORK_FOLDER_CONTEXT`, `Table results`, `Document excerpts` or the English word "excerpt(s)" in an answer (shown as "Dossier des documents", "Dossier des données", "extrait(s)") | ☐ PASS ☐ FAIL | |
| H3 | Open both folder lists in the sidebar, then `docker compose stop server` | The red banner stays visible (it sticks to the bottom of the sidebar); the sidebar scrolls, not the page. Restart the server | ☐ PASS ☐ FAIL | |
| H4 | Timeout message. In `.env` set `LLM_REQUEST_TIMEOUT_SECONDS=5`, run `docker compose up -d server`, then ask a question with a model that is **not loaded yet** (change model first) | The sentence "Le modèle a mis trop de temps à démarrer ..." instead of the generic failure. **Put the value back** (and `docker compose up -d server`) afterwards | ☐ PASS ☐ FAIL | |
| H5 | The truncation check of appendix 1 after sections B and F | No `truncated = 1`; the largest prompt is below the window of the model | ☐ PASS ☐ FAIL | |
| H6 | Settings → language **English**. Select the 6 documents and type `How many files in total?` | "Your documents folder holds 6 files." and the labels of the interface in English. Put the language back to French | ☐ PASS ☐ FAIL | |

## I. Open observations to record (no pass or fail)

Write what you see; these are known open points, kept so that any change shows up.

| # | Observation | What to type / look at | What it is today (HAP-1 + fixes) | Your observation |
| --- | --- | --- | --- | --- |
| I1 | A threshold **written in words** | Selection as D. `Est-ce qu'on a dépensé plus de cinq mille euros avec un seul fournisseur ce trimestre ?` | An honest refusal: "Je ne peux pas répondre directement à cette question pour ...", with the columns and two examples. The engine reads digits only, and a small model cannot turn it into a plan. **Never a number.** Record any change | |
| I2 | The **example sentence** in the help message | Trigger any refusal (D15) and read the last lines: "Par exemple : « Quelle est la somme de ... ? »" and "Ou : « Quel(le) ... a le plus de ... ? »" | For the invoices: `montant` and `fournisseur`, good. For `donnees_publipostage.xlsx` the group example is `Civilite` (two values), a poor example: a column with more distinct values would be better. Record the examples you get on each workbook | |
| I3 | A small model answering **in English** or pasting an unrelated excerpt | Sections B and F, small models | Known (`gemma2:2b`): English refusals, a pasted excerpt before "no information". Count them | |
| I4 | Figures caught by the check "Le chiffre « ... » ne figure ..." | B4, B5, F4, F5 | It never fired in the owner's replays (no model invented a figure). List every time it does, and every invented figure it missed | |
| I5 | Time per answer | The "Généré par ... en ..." lines | Seconds to a minute and a half on the development PC, per model. Record it | |
| I6 | Total of a **model-assisted** reading | D18 with each model | The model proposes a column reading in 40 s to 1 min 20 s. Record it | |

## J. Totalling up

1. Count the ticks of each section. `FAIL` rows are the findings: for each, copy the exact answer and the model.
2. **A regression** is a row whose HAP-1 column says PASS and which is now `FAIL`; list them first.
3. **A fixed finding** is a row whose HAP-1 column says FAIL or NOT RUN and which is now `PASS`.
4. Rows marked "new" have no HAP-1 verdict: they are the baseline of the next pass.
5. Section I is not scored: copy your observations into the report.

| Section | Tests | PASS | FAIL | Of which regressions |
| --- | --- | --- | --- | --- |
| A | 6 | | | |
| B | 6 + 3 runs | | | |
| C | 2 | | | |
| D | 20 | | | |
| E | 9 | | | |
| F | 8 | | | |
| G | 15 | | | |
| H | 6 | | | |

---

## Appendix 1 - Checking that the model's window is not truncating the prompt

Why: if the prompt (instruction, excerpts, remembered answers) is longer than the model's window, the runtime cuts the
beginning **without telling the application**, and the answers get strange (English, an unrelated excerpt, a refusal
after a long wait). Ollama says it in its own log.

1. **Reproduce.** In the program, ask the question that looked wrong (for example B1 or F2).
2. **Read the log, right away,** at the repository root:

   ```powershell
   docker compose logs ollama --since 30m | Select-String "truncated = 1"
   docker compose logs ollama --since 30m | Select-String "stop processing"
   ```

   (bash: `docker compose logs ollama --since 30m | grep "truncated = 1"`). Each question the model answers leaves a
   line such as `slot release: id 0 | task 0 | stop processing: n_tokens = 853, truncated = 0`.
   - **`truncated = 0` on every line:** nothing was cut. `n_tokens` is the prompt plus the answer, in tokens; it must
     stay clearly below the window of the model.
   - **A line with `truncated = 1`:** the prompt was cut. Raise the window of that model in `.env`
     (`MODEL_CONTEXT_WINDOWS`, the comments above it say how), run `docker compose up -d server`, and ask again.
3. **Check the window really applied:** `docker compose logs ollama --since 30m | Select-String "n_ctx_slot"` shows
   the window Ollama gave the model it just loaded (4096, 8192 ...). It must equal the value in `.env`, unless the model
   itself supports less.
4. A line `requested context size too large for model ... n_ctx_train=2048` is the embedding model
   (`nomic-embed-text`, trained at 2048) being asked for a larger window: harmless, it uses 2048.
5. The log only goes back to the creation of the container; `docker compose up -d --force-recreate` or `down`
   empties it. Run the check **in the same session** as the test.

*State on 7 October 2026 (owner's machine, container up since 5 October):* 88 answers in the log, **none truncated**,
the largest prompt 933 tokens, windows seen 2048, 4096 and 8192.

## Appendix 2 - Test again after changing the models or the windows

Changed `MODEL_CONTEXT_WINDOWS`, the model list or the machine (for example the Mac mini)? Repeat at least: B1 to B5,
**B7** (three runs per model), F2 to F4, then appendix 1. A window above 8192 also lifts the program's cap on
remembered exchanges (above 8192 no cap; 8192 and below, 2 exchanges, 1 at 4096 and below), which B7 exists to check.
