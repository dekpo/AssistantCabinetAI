# 11 - Plan for lot E: find a row, compare a group's total, fill a template (5 October 2026)

Nothing here is implemented. Lot E is the first lot that adds capabilities instead of repairing one, and the last
one writes a file, so it needs decisions from the owner (end of this file). Sources: the publipostage finding
(`docs/SESSION-DATA-17-Publipostage-Issue.md`, local), BUG-14, OBS-8, and the owner's own test files, copied here
as fictional fixtures in [fixtures/publipostage/](fixtures/publipostage/).

## What the test files show

| File | Content | What it implies |
| --- | --- | --- |
| `donnees_publipostage.csv` | `ID_Client, Civilite, Prenom, Nom, Email, No_Commande, Article, Quantite, Prix_Unitaire, Total_Ligne`; **one row per order line**: CMD-2026-001 and CMD-2026-003 have two lines, CMD-2026-002 has one | A lookup by `No_Commande` returns **one or several rows**; the client's fields repeat on every line, the line fields differ |
| `modele_lettre.txt` | Placeholders in square brackets: `[No_Commande]`, `[Civilite]`, `[Prenom]`, `[Nom]`, `[Article]`, `[Quantite]`, **`[Prix_U]`**, **`[Total]`**; one table row with the line fields | `[Prix_U]` and `[Total]` are **not** column names (`Prix_Unitaire`, `Total_Ligne`): a placeholder can be an abbreviation |
| `modele_lettre.docx` | Placeholders written with guillemets, each in one Word run: `«No_Commande»`, `«Civilite»`, `«Prenom»`, `«Nom»`, and in a Word table (header row plus one template row) `«Article»`, `«Quantite»`, `«Prix_Unitaire»`, `«Total_Ligne»`. A trailing note says Word's own mail merge cannot group several lines under one client | The docx names match the columns exactly; the grouping of several lines under one letter is exactly what this product would do |

Facts that frame the design:

- **The rows never go to a model.** Decided in session 16 (`docs/DECISIONS.md`: "what the model may never receive: a
  cell value, a distinct value, or a row"). A letter made of a client's name and address cannot be written by a model
  that is not allowed to read them. Filling the template is therefore a **deterministic local substitution** in Rust;
  no model is needed for it, which also makes it testable and identical on every machine.
- Layer 2 of the finding ("nothing can look up a row") is partly out of date: since session 11 the question's own words
  already anchor an `Equals` filter on a real cell value. What is missing is the *operation*: `CMD-2026-002` alone,
  with no verb the classifier knows, is `NotRecognised`.
- The crates to read and write a `.docx` (`zip`, `quick-xml`) are already dependencies (extraction reads it today).

## Steps, in dependency order

| Step | What | Depends on | Size |
| --- | --- | --- | --- |
| **E1** | The mixed tier stops saying "this question had nothing for your tables" when it did not even try: an unrecognised question with tables selected falls through like tier 2 does (honest message naming the real columns, the model's plan attempted when allowed). Layer 1 of the finding, a bug in session 16 | nothing | XS |
| **E2** | **Row lookup.** A value in the question that is found in real data (session 11's anchoring rules: exact, folded, never a guess, close values on a miss, a value in two columns is asked about) and **no operation word** returns the matching rows, shown as a table with the filter named. Several rows are normal (order lines) | E1 | S-M |
| **E3** | **A group's total against a threshold** (BUG-14, Q19: "any supplier above 5 000?"): "which suppliers have a total above N" answered from the grouped sums, replacing today's refusal; a yes/no question gets "yes, X" or "no, the highest total is Y". Independent of E2 | nothing | M |
| **E4a** | **Fill a template, text.** A `.txt`/`.md` template and the rows of E2 produce a filled letter | E2 | M |
| **E4b** | **Fill a template, Word.** Same for `.docx`: placeholders inside runs, one repeated table row per order line | E4a | M-L |

E1 and E3 can be done before the decisions below; E2, E4a and E4b need them.

## E4 in detail (needs the owner's decisions)

1. **Recognising the request.** The writing-verb gate of lot D already sends "génère / rédige / write / generate ..."
   to the tier that reads both sources. The fill capability adds: a document of the selection that contains
   placeholders **and** a row found in the data for an identifier in the question.
2. **Which template.** Exactly one selected document with placeholders: that one. Several: the existing "which one?"
   buttons (UX-1). The template is never guessed.
3. **Placeholders.** `«Name»`, `[Name]`, `{{Name}}`. A placeholder is matched to a column by folded name. One that
   matches no column (`[Prix_U]`) is **never filled silently**: the preview shows it as unresolved, with the closest
   column (by prefix) proposed ("`Prix_U` -> `Prix_Unitaire`?") and the user confirms or leaves it empty.
4. **Several rows.** The fields common to all rows (client) come from the first row; the template's table row (docx) or
   the line of the template (txt) is **repeated once per row**; a single-row order gives one line. If the lookup
   matches rows of different clients, nothing is filled and she is asked.
5. **Plan, then approve** (`AGENTS.md`, rule 3). A card in the chat shows: the template, the rows used, the mapping,
   a preview of the letter, and the file that would be written. Nothing is written until she presses Approve.
6. **The file.** A new file, never an overwrite (`-2`, `-3`), never next to a document it could be mistaken for:
   in a dedicated subfolder of the documents folder; the template is untouched; the action is logged with the
   template, the identifier and the output name; no row is logged. She sends the letter herself, from her own
   software ("export" means a validated artefact in the work folder or on the clipboard).
7. **What the model may still do.** Nothing in the fill. Writing the sentences *around* the data (a covering note) would
   be a second, separate step, and the model would then receive only the fields the owner agrees to send; out of
   scope for E.
8. **Both platforms.** Paths come from `app.path()`, no platform literal; the docx reader and writer use the same
   crates on Windows and macOS.

## Exit criteria

- E1: the publipostage question no longer says "nothing for your tables".
- E2: `Quelle est la commande CMD-2026-002 ?` shows the one line of Sophie Martin; `CMD-2026-001` shows two lines;
  an unknown number lists close values; a value in two columns is asked about.
- E3: Q19 (`... plus de 5000 euros avec un seul fournisseur ...`) is answered "no" with the highest total (1 450,
  MedSupply), and `plus de 1000` lists MedSupply.
- E4a/E4b: the owner's own question (`Génère le courrier de la commande CMD-2026-002 ...`) shows a preview of the
  letter for Sophie Martin with the chair, quantity 1, 189,00 and 189,00; after Approve a new file exists, the
  template is unchanged; CMD-2026-001 gives a letter with two lines; `[Prix_U]` is proposed, not silently filled;
  adversarial cases (placeholder with no column, two clients, a value that is in no row, an existing output name)
  have tests with these fixtures, in both languages.

## Decisions to take (owner)

1. **Order of work.** E1, then E2 and E3 (no new file written), then E4a and E4b; or E4 earlier.
2. **Formats.** Text first and Word second, or Word only (the owner's real use is the `.docx`).
3. **Where the generated letter goes**, and its name (proposal: a `Generated` subfolder of the documents folder,
   name `<template>-<identifier>.docx`).
4. **Placeholder syntaxes** accepted (proposal: the three above).
5. **Several clients matched**: refuse and ask (proposal), or one letter per client.
6. **Whether the unresolved placeholder proposal** ("Prix_U -> Prix_Unitaire?") is welcome, or the preview should only
   list it as unresolved.

---

## Progress (5 October 2026)

**Owner decisions, 5 October:** all the proposals above are validated (order E1, E2, E3 then E4; Word is the real
use, with text first; a `Generated` subfolder and `<template>-<identifier>.docx`; the three placeholder syntaxes; several
clients matched: refuse and ask; the abbreviation proposal welcome). **New request: batch generation.** Once the
mapping is confirmed, offer to generate **as many letters as there are orders in the spreadsheet**, one `.docx` each
(the very principle of a mail merge). Recorded as step **E5** below.

| Step | State |
| --- | --- |
| E1 | **Folded into E2.** The "this question had nothing for your tables" line was wrong because the question named an order and nothing could look it up; with the lookup the question is recognised. The line is kept for a question that truly names nothing in the tables (a pure content question) |
| E2 | **Coded, tested, awaiting replay.** An identifier written whole in the question (a token with a digit **and** a letter, such as `CMD-2026-002`) that equals a whole cell of a text column returns the rows holding it, shown as they are with the filter named, no model. Also fixes `detect_filters`: an identifier is one value, not three words ("CMD", "2026", "002" became three filters that matched nothing, so `somme de Total_Ligne pour la commande CMD-2026-001` answered "empty sheet") |
| E3 | **Coded, tested, awaiting replay.** "Is there a supplier above 5 000" is answered from the **totals per supplier**: the groups beyond the threshold with their totals, or "no, the highest total is 1 450 (MedSupply)"; the answer says it compared totals. New engine operation `GroupsBeyond` and value; above and below, French and English. The refusal of lot B2 (`group_threshold_not_supported`) is no longer produced |
| E4a, E4b | Next: template filling, text then Word, with preview and approval |
| E5 | Next, after E4: batch. One letter per distinct value of a **key column** the user confirms (here `No_Commande`: an order with two lines is one letter with two lines); with no key, one per row. A summary of what will be written, then Approve; each file named `<template>-<key>.docx` in the `Generated` subfolder, never overwritten (`-2`, `-3`); a report of what was written |

### Replay list for E2 and E3

Preparation: in the **Data folder** tick only `donnees_publipostage.xlsx` (ticking the `.csv` as well would
correctly ask which workbook) and run the analysis of the data folder if it has not been. For the first four,
untick every document.

1. `Quelle est la commande CMD-2026-002 ?` Expected: one row (Sophie Martin, Chaise Ergonomique, 1, 189, 189) listed as "1 ligne retenue, d'après No_Commande", with "Compris comme : No_Commande = CMD-2026-002", no model wait.
2. `Quelle est la commande CMD-2026-001 ?` Expected: two rows (Ordinateur Portable 899 and Souris Sans Fil 25,5).
3. `Quelle est la commande CMD-2026-009 ?` Expected: a refusal (no such value), never a row.
4. `Quelle est la somme de Total_Ligne pour la commande CMD-2026-001 ?` Expected: a sum, **924,5**.
5. Tick `modele_lettre.docx` in the documents folder and ask the owner's question: `Génère le courrier de la commande CMD-2026-002 en reprenant les données du client et en les insérant dans la lettre correspondante.` Expected: **no** "Cette question ne portait sur aucune de vos tables"; the block "Calculé dans vos données" shows the one row of CMD-2026-002. (The letter is not filled yet: that is E4.)
6. Data folder: tick only `factures-fournisseurs-2026.xlsx` (documents unticked). `Est-ce qu'on a dépensé plus de 5000 euros avec un seul fournisseur ce trimestre ?` Expected: "Aucun fournisseur n'a un total de montant supérieur à 5 000. Le plus haut total est 1 450 (MedSupply)." with the note that it is the total per supplier.
7. Same selection: `Y a-t-il un fournisseur avec plus de 500 euros ?` Expected: MedSupply 1 450 and Fournitures Dupont 550.
8. Same selection: `Y a-t-il un fournisseur avec moins de 300 euros ?` Expected: Papeterie Lefevre 215.
9. No regression: `Combien de factures de plus de 400 euros ?` still gives 2 (a row threshold, no supplier named), and `Combien de factures de plus de 400 euros pour le fournisseur MedSupply ?` gives 2.

### Replay results of E2 and E3 (owner, 5 October 2026, `gemma2:2b`)

| # | Check | Verdict | What was seen |
| --- | --- | --- | --- |
| 1 | `Quelle est la commande CMD-2026-002 ?` | **PASS** | One row (Sophie Martin, Chaise Ergonomique, 1, 189, 189), "Compris comme : No_Commande = CMD-2026-002", "Réponse calculée depuis votre dossier des données, sans l'IA" |
| 2 | `... CMD-2026-001 ?` | **PASS** | Two rows (Ordinateur Portable 899, Souris Sans Fil 25,5) |
| 3 | `... CMD-2026-009 ?` | **PASS-WITH-ISSUES** | Refused, never a row, but only **after the model was asked for 51 s** and with the generic text (BUG-28, fixed below) |
| 4 | `Quelle est la somme de Total_Ligne pour la commande CMD-2026-001 ?` | **PASS** | 924,5 over 2 rows, "Compris comme : No_Commande = CMD-2026-001" |
| 5 | The owner's own publipostage question (template and data ticked) | **PASS for E2, as expected before E4** | The line "Cette question ne portait sur aucune de vos tables" is gone and the block "Calculé dans vos données" shows the row of CMD-2026-002. The model still echoes the template with its guillemets, with the order number put where the name goes ("«Civilite» «Prenom» «Nom» : «CMD-2026-002»"): it cannot fill a letter from data it must not read, which is why E4 does it in code |
| 6 | Plus de 5000 euros avec un seul fournisseur | **PASS** | "Aucun fournisseur n'a un total de montant supérieur à 5 000. Le plus haut total est 1 450 (MedSupply)." with the note that it is the total per supplier |
| 7 | `Y a-t-il un fournisseur avec plus de 500 euros ?` | **PASS** | MedSupply 1 450 and Fournitures Dupont 550 |
| 8 | `... moins de 300 euros ?` | **PASS** | Papeterie Lefevre 215, with the note "pas de la plus petite ligne isolée" |
| 9 | `Combien de factures de plus de 400 euros ?` | **PASS** | 2, "Compris comme : montant > 400". The second half of the check (the same question with "pour le fournisseur MedSupply") was not pasted |

The owner appended a path to the questions ("... dans Data/Test/publipostage/donnees_publipostage.csv") to choose between two
copies of the file found in the data folder; the typed path resolved correctly.

### Findings

| ID | Severity | Title | Change |
| --- | --- | --- | --- |
| BUG-27 | S4 | "1 ligne retenues": the sentence did not agree with one row | `filteredOne` and `sortedOne` in both catalogues; chosen by the number of rows |
| BUG-28 | S3 | An identifier-shaped value that matches no cell cost a 51 s model wait before the refusal | `unmatched_identifier`: a token with a digit and a letter, at least five characters, that equals no cell is refused at once with the identifiers closest to it (the existing "« X » ne correspond à aucune valeur réelle ..." with close values), no model call |
| UX-8 | S4 | The nudge example picked `Civilite` (two values) as the group to rank by: "Quel(le) Civilite a le plus de Prix_Unitaire ?" | Not changed; a column with more distinct values would make a better example. Backlog |

### Steps E4 and E5, engine core (coded 5 October, not yet visible in the interface)

`src-tauri/src/template_fill.rs` is the whole mail merge as pure functions, with no file, no model and no
interface: placeholders (`«Name»`, `[Name]`, `{{Name}}`), the mapping (exact / proposed / unresolved), grouping by a key
column, the line columns, the text letter and the Word letter. **Tested on the owner's own files** (`tests/template_fill.rs`,
`fixtures/publipostage/`):

- The Word template's eight placeholders match the columns **exactly**; the note at the end of the file, which quotes a Word
  rule between guillemets with spaces ("« Suivant si »"), is **not** read as a field (a name between guillemets or square
  brackets must not start or end with a space).
- An order with two lines gives **one** letter with **two** table rows (the header row stays, the template row is repeated);
  an order with one line gives one row and nothing of another order or client.
- Every other part of the Word file is copied byte for byte, in the same order.
- The text template's `[Prix_U]` and `[Total]` are only **proposed** (`Prix_Unitaire`, `Total_Ligne`): unconfirmed, they stay
  as written in the letter and are reported as unresolved; confirmed, the letter is filled.

Still to do for the feature to exist for the user: finding the template and the rows from the question, the plan card (template,
rows, mapping with a choice per placeholder, preview, one letter or one per order, Approve), the command that writes the file(s)
into a `Generated` subfolder of the documents folder without overwriting, and the audit line. That is the next step.

### Steps E4 and E5 delivered (6 October 2026, coded and tested, awaiting live replay)

What now happens when a conversation has documents **and** tables selected and the question asks for something to be
written (`fill_tier` in `commands.rs`, before the data-only router and the mixed tier):

1. The workbook the question points at is found (an identifier it names, such as `CMD-2026-002`, or "each"/"every"/"chaque/tous"
   for every row). Exactly one selected workbook must qualify; with several, nothing is picked.
2. The selected documents that are templates are found (`.docx`, `.txt`, `.md` holding at least one field that names, or
   begins, a column), the best match first.
3. The answer is **a plan, not text**: a card under the answer with the template, the data, how many letters, the field-to-
   column mapping (a choice per field), the preview of the letter, and a button. **Nothing is written** and **no model** is called.
4. A field that is only a **proposal** (`Prix_U` for `Prix_Unitaire`) is never used until she presses Confirm (or picks a column);
   the button stays disabled while one waits. A field with no column stays as written in the letter and is reported.
5. The button writes the letters into a `Generated` subfolder of the documents folder, named `<template>-<key>.<extension>` in the
   clean-name alphabet, **never overwriting** (`-2`, `-3`), the template and the data untouched. One approval writes at most 500
   letters. Every file written is logged in `generated-files.jsonl` (app data folder): when, template, data file, key, output; no cell.
6. "One letter per": the order named, or every value of a key column she picks on the card (here `No_Commande`: an order with two lines
   is **one** letter with **two** table rows); with no key column, one letter per row.

Where the code is: `template_fill.rs` (the merge), `fill_plan.rs` (plan, preview, generation, log), `tabular_answer::locate_fill_source`
and `load_fill_table`, `commands::{fill_tier, fill_preview, fill_generate}`, `FillPlanCard.tsx`, `lib/fill.ts`. New machine codes:
`fill_template_unreadable`, `fill_source_unavailable`, `fill_nothing_to_fill`, `fill_too_many`, `fill_write_failed` (both catalogues).

Tested on the owner's own files (`tests/template_fill.rs`, `tests/fill_plan.rs`, `fill_plan` unit tests): the owner's question gives a plan
with the Word template first, Sophie Martin and the chair in the preview, nothing written; approving writes one new `.docx`; for each order gives
three letters, the first with two lines; a proposal stays unfilled until confirmed; a log line holds no cell; more than 500 letters is refused
before anything is written; a name already taken is never overwritten.

Known limits, to decide after the replay: values are written **as the cell carries them** (`189.0`, no currency, no locale formatting); the
folder name `Generated` is a constant (`fill_plan::OUTPUT_FOLDER`); a template whose placeholder Word splits across runs is handled, one split
inside a field code or a text box is not tried; several selected workbooks that all hold the identifier make no plan (the mixed tier answers).

### Replay list for E4 and E5 (exact steps)

Restart `pnpm tauri dev` first.

**Preparation.** Documents folder: tick `modele_lettre.docx` only (the `.txt` copy comes in test 4). Data folder: tick **one** workbook
holding the orders, for example `donnees_publipostage.xlsx` (ticking two copies of it, as in `Data/Test/publipostage`, makes the question
ambiguous and no plan is offered). The data folder must have been analysed. Open the documents folder in the file manager and note that there
is no `Generated` folder yet.

1. **The owner's question.** Ask: `Génère le courrier de la commande CMD-2026-002 en reprenant les données du client et en les insérant dans la lettre correspondante.`
   Expected: a card "Courrier à générer". "Données : ..., No_Commande = CMD-2026-002". Modèle `modele_lettre.docx`. "Seulement CMD-2026-002" selected.
   The eight fields (No_Commande, Civilite, Prenom, Nom, Article, Quantite, Prix_Unitaire, Total_Ligne) each point at the column of the same name, no "Confirmer"
   button. The preview reads "Confirmation de votre commande n° CMD-2026-002", "Bonjour Mme Sophie Martin", and the table row Chaise Ergonomique, 1, 189.0, 189.0.
   Under it the line "Aucune IA n'a lu vos données ...". **No `Generated` folder exists yet.**
2. **Generate.** Press "Générer 1 courrier(s)". Expected: "1 courrier(s) créé(s)", the path `Generated/modele_lettre-CMD-2026-002.docx` and a "Voir" button
   that shows the file. Open it in Word: the letter is filled, one table row. `modele_lettre.docx` itself is unchanged (modification time, content).
3. **No overwrite.** Ask the same question again and generate: a second file `modele_lettre-CMD-2026-002-2.docx`; the first one is unchanged.
4. **Two lines.** Ask `Génère le courrier de la commande CMD-2026-001`. The preview shows two lines (Ordinateur Portable, Souris Sans Fil); the generated Word letter has two table rows.
5. **A proposal waits.** Also tick `modele_lettre.txt` in the documents folder and ask the question again. The card now has a template choice; pick `modele_lettre.txt`.
   Its fields `Prix_U` and `Total` show "Confirmer" and the line "À confirmer avant de générer : Prix_U, Total."; the generate button is **disabled**; the preview still shows
   `[Prix_U]` and `[Total]` as written. Press Confirmer on both: the preview fills them (`Prix_Unitaire`, `Total_Ligne`) and the button is enabled. Generate: a `.txt` file in `Generated`.
6. **Every order.** Ask `Génère un courrier pour chaque commande`. Expected: "Données : toutes les lignes de ...", "Un courrier par : chaque ligne" with a count of **5** letters.
   Choose `No_Commande` in the list: **3** letters. Generate: `modele_lettre-CMD-2026-001.docx`, `-002`, `-003` in `Generated`; the 001 letter has two table rows, the 003 letter has Rousseau and two lines.
7. **No template, no card.** Untick every template, ask the owner's question: no card, the answer comes from the mixed tier as before, with the computed row. Then ask
   `Rédige un résumé de la somme des montant.` (with a data folder where that column exists): it still goes to the model.
8. **The log.** Open the app data folder (`%LOCALAPPDATA%` on Windows, the application's folder) and read `generated-files.jsonl`: one line per letter written, with the template, the data file,
   the key and the output name, and no name of a person or amount.

### Replay results of E4 and E5 (owner, 6-7 October 2026)

The owner replayed the mail merge on the real stack and is **very satisfied: "le publipostage fonctionne bien"**. What was verified:

| Check | Result |
| --- | --- |
| The owner's question | A plan card, with the template found in a subfolder (`Test/modele_lettre.docx`), the line "Aucune IA n'a lu vos données ..." and the label "Réponse calculée depuis votre dossier des données, sans l'IA". Nothing written by the question |
| Generate one letter | Done: the log shows `Generated/modele_lettre-CMD-2026-002.docx`, then `-CMD-2026-001.docx` |
| A text template | Done: `Generated/modele_lettre-CMD-2026-002.txt` |
| Every row | Done: with no key column, six letters keyed 1 to 6 (the owner's workbook has six lines), `modele_lettre-1.docx` to `-6.docx` |
| "Rédige un résumé de la somme des montant." with the template unticked | Tier 2 as designed: "Somme de Total_Ligne : 1 683,3, calculé sur 6 lignes", read by the model from "montant" (the column is `Total_Ligne`) and marked as such |
| The log | **Exists and is correct**; the owner could not find it because it is in the hidden application data folder: `C:\Users\<user>\AppData\Local\com.assistantcabinetai.desktop\generated-files.jsonl` (next to `renamed-files.jsonl`, `index.sqlite3`, the workbook caches). One line per letter: time, template, data file, key, output; **no cell value** (checked on the real file) |

The details of replay steps 3 to 6 (no overwrite, two lines in a letter, the proposal waiting, the per-order batch) were not pasted; the log proves the letters were written and the owner reports the feature works.

### Owner decisions, 7 October 2026

- **The folder name is a setting**, not a constant: "Dossier des courriers générés" in the settings panel (`generatedFolderName` in `settings.json`, default `Generated`). Whatever is typed becomes **one clean name** (letters without accents, digits, `_`, `.`, `-`; at most 60 characters; never a path, never `.` or `..`; the default when nothing usable is left), on the way in and again on the way out, so a hand-edited `settings.json` cannot make the program write outside the documents folder. Unit-tested.
- **No formatting of values.** They are written as the cell carries them. A symbol or a unit belongs in the template, written after the field: `«Total_Ligne» €`. Closed.
