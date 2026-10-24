# TABULAR step 14, phase A: column glossary (design note)

Written 24 October 2026 on branch `feat/column-glossary`. **Analysis only: no feature code was written, no test was added.** Stops here for the owner's written go (section 11) before any code.

## 1. How the claims are marked

**(M)** measured on a copy of her real index (`index.sqlite3` copied to the scratchpad and read there with a throwaway Python script, deleted with the copy; her index and her folders were not touched). **(C)** read in the code, not run. **(NV)** not verified, and what would verify it. Her settings point the Documents folder at `AssistantCabinetAI-test\Docs` and the Data folder at `C:\Users\elise\AssistantCabinetAI\Data`, **not** at the test folder.

## 2. What her files say (M)

Her analysed index holds 15 workbooks, 88 columns, **28 distinct header names**, all plain words (`date`, `montant`, `fournisseur`, `Prenom`, `Total_Ligne`, `duree_min`, `calendar_year`). One header is an abbreviation (`tva`). No header is synthetic (`column_N`), no sheet repeats a header. There is **no 0/1 column**; six text columns have 3 to 4 distinct values (`fournisseur`, `salle`, `rev_type`), none is a code. Consequences:

* Her current files do not need the glossary: the "Mt TTC" and "NE" cases of the prompt are real in a practice but absent from her fictional folders. The human test therefore needs a workbook **built for the occasion** in a scratch folder outside the repository and outside her test folders, as step 13 did (section 10).
* Proposals (section 5) must stay quiet on files like hers: on this index they would offer one abbreviation (`tva`) and the six few-values columns, nothing else.

## 3. What can be reused and what cannot

| Existing | Verdict |
|---|---|
| `file_profile::user_types` (`file_profile_user_types`): manual rows in `index.sqlite3`, written by no Analyse and no Reset, validated with caps (`validate`), `save`/`delete`/`list`, JSON payload | **Pattern reused, table not.** A user type is a rule that classifies a *file*; a glossary entry is about a *column or a value*. Same shape for the code: a new table `tabular_glossary`, caps, a `validate`, `save`/`delete`/`list`, rows that no Analyse or clear touches (C). |
| `file_profile::vocabulary` | **Not reused** (document-type phrases per place). Only its tokenising helpers (`words_of`, `words_key`) can be called (C). |
| Knowledge base (`kb_aliases`, `kb_column_semantics`, `ownNames`, `knowledge::manage`) | **Not reused.** Aliases name an *entity* (a person, an organisation); `kb_column_semantics` is the base's own automatic reading (type, role) of a column, cascade-deleted with its source. A glossary alias names a *column of a file*. The base's manual-row discipline is the lesson, not its tables (C). The `key` entry could later feed `kb_column_semantics.role`; not now. |
| `tabular::question` (`find_column_name`, `find_measure_column`, `columns_for_matching`) and `tabular_answer` (`names_a_column`, `column_value_words`, `detect_filters`) | **Extended, not duplicated** (section 6). They match a folded column name as a **substring** of the folded question (C): fine for a header, wrong for a two-letter alias (`CA` inside `caisse`); aliases are matched as whole tokens. |
| `TabularInventory` | **Not the storage.** It is rebuilt by every Analyse (C, `retain_tabular_inventories`, `clear_tabular_inventories`). |
| `kb`-style management dialog (`KnowledgeDialog`, `components/knowledge/*`) | Look and feel reused; user types have **no interface today** (C: `TypesTab` is read-only), so there is nothing to extend. A new dialog (section 8). |

## 4. The four kinds of entry, storage and identity

One table `tabular_glossary` in `index.sqlite3`, next to `tabular_inventories` and `file_profile_user_types`, plus a small `tabular_glossary_files (scope_key, last_workbook_id, last_seen)` for re-attachment. Rows are **manual**: no Analyse, no `reset_data_index`, no clear of the folder ever deletes one; only the user, from the glossary dialog, does. (Today `reset_data_index` clears inventories, cached workbooks and profiles only: C.)

| Kind | Payload (user text, user's language) | Caps |
|---|---|---|
| `column` | `meaning` (what the column holds), `aliases` (other names a question may use) | meaning 120 characters, 8 aliases of 60 |
| `value` | `value` (as it appears in the cells), `meaning`, `words` (the phrases a question may use: "impayé", "non encaissé"); several values may share one meaning | 8 values, 8 words |
| `convention` | one free sentence about the file or a sheet ("amounts in thousands of euros") | 200 characters, 10 per file |
| `key` | 1 to 3 column names that identify a row | 3 columns, one entry per table |

**Scope.** `scope_kind` is `file` or `folder`. A `file` entry is keyed by the file's **index key** (`file_key`: a plain path, or `/documents/<path>` for a foreign file), never by the content hash. A `folder` entry is keyed by a folder of the Data or the Documents folder (the Data folder itself is the empty path) and applies to the files under it, subfolders included; it holds `column` aliases and `value` meanings by column **name** only (no sheet, no key, no convention, since those are never common). The nearest folder wins, and a file entry beats any folder entry; a shadowed folder entry is said in the dialog.

**Where inside a file.** Step 13 made a table of a sheet that holds several its own sheet, named `<sheet> <range>` or `<sheet> (<Excel Table>)`, and that name moves with the rows (C, `SheetLayout`). The key of a table is therefore built from what does not move, never from the range:

1. one table on the sheet: `(raw sheet, column)`; `raw_sheet` is `SheetLayout.raw_sheet`, the sheet name of the file, which is also the inventory name for a plain sheet;
2. an Excel Table: `(Excel Table name, column)`; a table name is unique in the workbook and survives a sheet rename and added rows;
3. several tables on a sheet without Excel Tables: `(raw sheet, header signature, column)`, the signature being the folded header labels in order. Adding rows moves the range and changes nothing; changing a header makes the entry an orphan (below). Never an ordinal and never the range.

A CSV is one table with no sheet. A column is named by its **header text** (folded for comparison), never by its letter; the letter shown in the dialog and in the card is `SheetLayout.origin_col` + `ColumnInventory.index`, the same as the card.

**When the file changes (stated rules).** *Added rows, edited values*: nothing happens, the key does not move. *A header renamed*: the entry becomes an **orphan column**: kept, shown in the dialog under "not found in the file any more", with a one-click "move to column..." listing the columns of that table; never applied to a question. *A file renamed, moved, or its table changed so much that the key no longer resolves*: its entries are **orphan file entries**. At the end of a hand-started Analyse, after the files present are known, an orphan file whose `last_workbook_id` equals the content hash of exactly one new file with no glossary of its own is **re-attached to it** (same bytes, so a pure rename or move), and the Analyse report says so in one line; every other case stays orphan and is offered in the dialog with "attach to a file..." (a picker over the analysed workbooks) or "delete". This reads no file: the hashes are already computed by the pass (C: `workbook_id` is the content SHA-256 and the pass compares it to decide what to rebuild). *A file that disappears for good*: its entries stay until the user deletes them, because they are her work and a file often comes back (the monthly export); the count of orphan entries is shown once in the dialog, not in the explorer. *A clear of the folder or of the analysis*: entries kept, by construction (stored apart).

**Limits.** 200 entries per file, 2 000 in all; a store that exceeds them refuses the new entry with a machine code (new `AppError` variant, a sentence in both catalogues, enforced by `guards/sources.test.ts`).

## 5. Proposals from structure alone

Computed when the user **opens the glossary of one file** (a user-started action on one cached workbook, so no background pass and nothing at launch); never stored except the "ignore" the user chooses (`dismissed` list in `tabular_glossary_files`); never applied.

| # | Trigger (structure only) | Proposal shown |
|---|---|---|
| P1 | a header all of whose words, or some of whose words, are in the per-locale `abbreviations` list (`mt`, `qte`, `nb`, `ca`, `ttc`, `ht`, `tva`...) | meaning = the expansion ("Mt TTC" -> "montant toutes taxes comprises") and the expansion as an alias; the user edits, accepts or ignores |
| P2 | a column with exactly two distinct values (`0`/`1`, `oui`/`non`, `Y`/`N`: the pairs are per-locale data) | "This column has two values, 0 and 1. What do they mean?" with two empty meaning fields. The app **never fills in which is "yes"** |
| P3 | a text or whole-number column with 2 to 12 distinct values over at least 6 rows | the list of values, "does one of them need a meaning?"; the user picks the value and types the meaning |
| P4 | a header of 3 characters or fewer, or containing a full stop or a digit group, that P1 does not know | "Say what this column holds" with an empty meaning |
| P5 | a header with a word of the per-locale `identifier_words` list (`id`, `ref`, `no`, `code`, `commande`...) whose values are all distinct and non-empty | "Is this the identifier of a row?" |

The values shown in P2, P3 and P5 are read **by the dialog on screen, on the workstation** and are never sent anywhere. **A model may phrase a proposal from the card only (names and types, no value). I recommend not building it in this step** (Q4): P1 to P5 cover the real cases without a model call, a model call adds a failure mode and a model-facing string to guard, and nothing is lost if it is added later behind a "suggest a wording" button. The contract if wanted: one call, started by that button, on the card text only, answer shown in an editable field, never saved without the user pressing "Add".

## 6. How a meaning is used

**The seam.** The glossary is read once per question into a `ResolvedGlossary` (file and folder entries merged, orphans left out, conflicts listed) and **attached to the inventory the question layer reads** by one pure function `glossary::annotate(&mut TabularInventory, &ResolvedGlossary)`, which fills a new `#[serde(skip)] gloss` field on `ColumnInventory`. Every reader that already receives an inventory then sees it, none gets a new parameter, and nothing glossary-related is ever written into the cached inventory. `usable_inventory` has 19 call sites in `tabular_answer.rs` (C), so the annotation is done where the inventory is produced: in `DataFolder` once, and again after the two `view_of` calls of `tabular_answer.rs` that rebuild an inventory from the view (`fresh` in `detect_filters`). `ColumnInventory` is built at four places (C), each gets `..Default::default()`-style completion. **(NV)** whether the `DataFolder` that holds `usable` is rebuilt per question or cached between commands: if cached, `set_glossary` re-annotates it after each edit (in memory, cheap); Phase B checks it first.

**Column meaning and aliases, in question reading.** The meaning text and each alias become match phrases of that column, **resolved before** `columns_for_matching` (step 13, item 4):

* matching is by whole tokens, folded (accents and case), consecutive tokens for a phrase; minimum two characters; the longest phrase wins; a real header found in the question beats an alias, as today's longest-name rule does;
* an alias matching two columns of the sheet: the existing refusal to choose applies (`find_column_name` returns `None`, the nudge lists the columns); the dialog also refuses it at entry (section 9);
* the full `meaning` is a match phrase too, so typing "montant toutes taxes comprises" works without an alias; **its words are not**: splitting it would make "montant" match every amount column;
* `find_measure_column` keeps its numeric, formula-free rule: an alias on a text column does not make it summable;
* an alias can name a table (Q3): see below.

**Value meaning, in filters.** In `detect_filters`, a residual word run that equals a value-meaning phrase of a column of the sheet adds `Comparison::In(values)` (or `Equals` for one) on that column, through the same `consumed` mechanism that stops the word from being read twice. The longest phrase wins ("non payé" beats "payé"; negation is not read, the user defines the phrase she needs). The filter is anchored on the **cleaned cells** of the view (step 13, item 5): the value is compared with the engine's own `fold_text` equality, exactly as an explicit filter is. `Comparison::In` already exists and ANDs with other filters (C). If no cell holds the value, the filter still runs and gives zero rows, and the existing "No row matches" line points at "Understood as". **(NV)** that `Equals("1")` matches a numeric cell `1.0`: `text_value` of a number is not read here; one Phase B test settles it before the 0/1 case is promised. `column_value_words` (used by the mixed tier to anchor an excerpt word on real data) also receives the value-meaning words, so the same phrase works in a mixed question; the decision on `residual_for_rows` (the rows chosen for the writer, step 10) is yes: a value meaning selects rows the same way an explicit filter does, and the rows sent are the rows the figure used.

**Convention.** Free text. It joins the card, the calculation record and the explanation; it changes **no figure** and selects nothing. One exception is deliberately *not* made: "dates are day/month/year" would settle `ColumnInventory.ambiguous_date`, which today refuses a month filter on such a column. That changes how the engine types a column and which rows match, so it breaks the rule of section 9; it is a different, typed mechanism with its own consequences on Analyse. **Recommendation: leave it out of step 14 and record it as the first follow-up** (Q5).

**Key.** Names the columns that identify a row. Used in step 14 for: a line in the card ("a row is identified by: No_Commande"), the dialog's check (duplicate and empty values are counted from the cached workbook when the dialog opens and **said**, never hidden), and `identifier_hits` preferring that column when an identifier typed in the question matches more than one. Mail merge (step 09's `fill_draft::fields_of`) and joins (step 18) read it later; step 14 stores it and states it, they are not changed here (the placeholder alphabet of `template_fill::valid_name` still applies to anything a merge would take from a column).

**The card.** `card::build_card` stays pure: `CardOptions` gains `glossary: Option<&ResolvedGlossary>`. Appended to the sheet lines only: after a column's description ` -- meaning: <text>`, one `    Values: <column>: "<value>" = "<meaning>"` line per value meaning, one `  Notes from the user: ...` line for conventions and one `  Row identifier: ...` line for the key. **Aliases are never sent** (they are for the program; the meaning is what the model needs). Only the columns of a workbook whose card is sent carry text, so a named or implied workbook only; never the sample (`tabular::sample`) or the rows; a value appears only as the value the user is defining. Budget: when the card does not fit, shrink in this order, after the existing steps: drop value lines, then column meanings, then the notes line; the key line last; `card_truncated` is recorded as today. The shape (`WorkbookShape`, used by `formula_check`) is unchanged: it holds real names only, so a formula check cannot accept an alias. The new builder strings get a `..._stays_neutral_about_who_the_user_is` guard.

**Calculation record, explanation, trace.** A meaning that was applied is written where the reading is already written: the `Understood as` line (`format_calculation_record`) gets `"montant total" read as column "Mt TTC"` and `Statut = "NE" ("impayé")`, with a machine field `glossary: Vec<GlossaryUse { kind, phrase, column, value }>` on the record and on `WorkbookUse` (the way `layout` is carried), rendered by `lib/workbookUse.ts` and the answer with catalogue keys in both languages. The user therefore sees that a meaning was applied and which; "Expliquer le calcul" repeats it (step 06's record).

**Formula help (step 07).** A formula is checked against the real shape (above); a written formula that names an alias is reported by the existing "column does not exist" check, and the card tells the model the meaning so it writes the real name. No change to `formula_check` is needed (C).

**Follow-ups (step 11).** `TableCarryOver` keeps real column names and is validated against the real inventory (`validate`); the alias is resolved in the new turn's reading, before the state is built, so no alias ever enters the state. A follow-up "et les impayées ?" is read through the same value meaning.

**Several tables and the hidden interpreter.** Step 13 declines to run the hidden interpreter when a sheet holds several tables and the question names none (`several_tables_none_named`). An alias or value phrase **that exists in exactly one of those tables** names it (the user said so by defining it) and the answer states which table in "Understood as"; one that exists in several tables, or in none, leaves the question at "which table?" (Q3, recommended). A hidden sheet gets no proposal (step 13, item 3); a hidden column may be described.

**What changes in the existing tests (C).** With an empty glossary every new field is empty and `annotate` does nothing, so the wave 1 and step 13 suites are expected to pass unchanged; the only existing assertions that move are card golden texts, which gain lines **only when a glossary exists**, so none changes. `tests/tabular_pack_parity.rs` gains the new lists.

## 7. Locale and per-locale data

User text (meanings, aliases, value words, conventions) is stored as typed, in her language, and matched folded. The lists for proposals and reserved words are data, `resources/tabular-glossary/<locale>.json`: `abbreviations` (word to expansions), `binary_pairs`, `identifier_words`, each `#[serde(default)]`, checked by `tests/tabular_pack_parity.rs`, read from **every shipped pack at once** (step 13, item 6), so adding German and Italian is two files. The reserved-word check of section 9 reads the existing question packs (operation, connector and filler words), no second list.

## 8. The interface

A **dialog**, "Vocabulaire du fichier" (`glossary.*` keys in both catalogues), not a workspace panel: the workspace layout is a different, recently settled programme and a dialog is what the knowledge base already uses. Entry points: the explorer row menu of an analysed workbook (the step 9 actions menu), and a link on the "Understood as" line when a meaning was applied. Sections: **Columns** (one row per column: letter, name, type, meaning, other names, with the proposals inline as greyed rows with "Add" / "Ignore"), **Values**, **Notes about this file**, **Row identifier**, **Not found any more** (orphans). A scope selector on column and value rows: "this file" (default) or "every file of this folder". A visible sentence at the top: *what you write here is sent with the description of the file to the AI model*, the privacy statement of section 9, shown once per opening. **Import / export**: export writes one JSON file (`glossary-<file stem>.json`, versioned, no path, no content hash) into the Generated folder or to the clipboard, as any export of the product; import reads a file the user picks, shows "N new, M replace, K refused" before applying, and validates every entry exactly as a typed one. Nothing is written into the user's workbook.

## 9. Privacy, conflicts and safety

* **Privacy.** Entries are the user's own words about her data. They join the card sent to the model, so the dialog says so, and a **new DPIA line** is written beside DECISIONS W-6: *"column meanings, the value-meaning pairs and the file notes the user types reach the gateway with the card; they may contain a cell value (the one being defined) and no other."* No value is ever copied into an entry automatically: P2, P3 and P5 show values on screen and the user types the meaning; a value enters an entry only when she presses "Add" on it.
* **Conflicts, said at entry.** An alias equal to the real header of another column of the sheet: refused. An alias already on another column of the sheet: refused. An alias made only of operation, connector or filler words of the packs ("total", "par", "le"): refused, since it would collide with the operation reader. Two value phrases of one column with the same words: refused. A value that no cell holds (checked on the cached workbook when the dialog opens): kept, **shown with "no cell has this value"**. A key whose column has repeated or empty values: kept, the count shown. A meaning that contradicts the header cannot be detected from structure: the dialog shows the type and the letter beside it, and the used meaning is always shown in "Understood as".
* **A meaning never changes a figure.** It changes *which* column or filter a question maps to, and that mapping is shown. It never touches the engine's arithmetic, the row policy (`rowPolicy`, step 13), or a conversion of unit. It is not an instruction to the model.
* **Text injection.** Every entry is untrusted text of the user's own, but it reaches the model: it passes `card::clean` (length, control characters, quotes) and sits inside the card, not in the instruction; no entry can name a tool or an action. Covered by the text audit planned after the series.

## 10. Test plan and human test outline

**Automatic tests (Phase B).** Inline workbooks only, built with `tests/common/tabular_fixtures.rs` (`write_xlsx`, `LaidOutSheet`); no file under `fixtures/`. Hand-stated golden results: workbook `Date | Fournisseur | Mt TTC | Statut` with Statut `OK`/`NE`, five rows, `Mt TTC` 100, 200, 300, 400, 500 and `NE` on rows 2 and 4.
(1) store: validation caps, replace, delete, survives `reset_data_index` and a second Analyse, orphan on a renamed header, re-attachment by content hash on a rename (one match), none on two matches;
(2) reading: *"somme du montant total"* with alias `montant total` -> `Sum` of `Mt TTC` = 1 500; the same question **without** the entry still gives the nudge (the baseline); *"montant total des factures impayées"* with value words `impayées` -> `Statut In [NE]` -> 600; "ca" does not match inside "caisse"; two columns claiming one alias -> still asks; a real header beats an alias; a numeric 0/1 column with `Equals`;
(3) several tables: an alias in one table names it and the answer says which; in two tables, still "which table?";
(4) card: meaning, value line and notes appear for a named workbook and **not** for an unnamed one; shrink order; no alias in the text; neutrality guard; the shape has only real names;
(5) record and trace: `Understood as` carries the applied meaning; `WorkbookUse.glossary` in both languages; follow-up "et les impayées ?" carries a real filter and no alias;
(6) proposals: P1 to P5 on inline workbooks, none on a clean file like her `factures-test.csv`; ignore persists;
(7) import/export round trip and refusal of a bad file; pack parity for `tabular-glossary`; the guards of `sources.test.ts` for new codes and keys.
**Human test outline** (full French script at the end of Phase B). One workbook built by a test helper into a scratch folder outside the repository and her test folders, chosen as the Data folder through the existing picker; backup of `settings.json` at the start and one restore at the end. Parts: (1) ask the question with no entry: the nudge; (2) open "Vocabulaire du fichier", accept the P1 proposal for `Mt TTC`, ask again: the figure and the "Understood as" line; (3) add `NE` = "non encaissé" and ask about unpaid ones; (4) re-Analyse and ask again: entries survive; (5) a close of the application and a new Analyse: the entries are still there; the orphan and re-attachment rules are proved by automatic tests only, since she is never asked to rename or edit a file by hand; (6) the previous wave's regression script. Counts come from a copy of her real analysed index, run through the real path before the script is written.

## 11. Estimate, proposed subset and questions for the owner

**Sizes (Phase B).** Store, validation, orphans and re-attachment 1 day; annotation and question reading 1 day; card, record, trace 0.5 day; proposals 0.5 day; dialog, catalogues, import/export 1.5 days; tests and human test 0.5 day. About **5 working days**. **Proposed subset for the first merge:** all four kinds on `file` scope, proposals P1 to P3 and P5, orphans and re-attachment, the dialog, export and import. **Second, same step if time allows, otherwise a small follow-up:** the `folder` scope and P4. **Out:** the model-phrased proposal, the date-order convention, reading data-validation lists.

* **Q1. Scope of the first merge.** The subset above (recommended), or all of it including the folder scope.
* **Q2. Orphans.** Keep them until the user deletes them and re-attach by identical content hash at Analyse (recommended), or delete the entries of a file that is no longer found at the next Analyse.
* **Q3. Does an alias name a table?** Yes when it exists in exactly one table of the workbook, always said in "Understood as" (recommended), or never (the question stays "which table?" whenever several tables share a sheet).
* **Q4. A model-phrased proposal.** Not now (recommended), or a hand-started "suggest a wording" button that sends the card only.
* **Q5. "Dates are day/month/year".** Leave out of this step and treat it as the first follow-up, since it changes how a column is typed (recommended), or build it now as a typed entry that settles `ambiguous_date` and re-types the column on the next Analyse.
* **Q6. Data-validation lists** of an `.xlsx` or `.ods` as a source of value proposals: not now (recommended; P3 already finds the values that occur), or a small read in `tabular/layout_read.rs`, about 0.5 day.
* **Q7. The entry point.** Explorer row menu plus a link on "Understood as" (recommended), or the menu only.

## 12. Prompts to correct after the go

Read for this note: `SESSION-TABULAR-15-formula-columns-and-totals.md` is the next prompt and does not depend on this step; `SESSION-TABULAR-18-richer-operations.md` (joins, key detection) and `SESSION-TABULAR-26-saved-analyses.md` (it names "glossary of step 14 for renamed columns") are the two that will read the glossary. They are corrected **after the owner's answers**, since the key's storage (section 4) and the orphan rule (Q2) are what they need to know, not before. Plan section 4.1 (card) gains the glossary lines after Phase B.
