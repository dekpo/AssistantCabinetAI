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
