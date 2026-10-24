# TABULAR step 13, phase A: real-world spreadsheets (design note)

Written 24 October 2026 on branch `fix/real-world-spreadsheets`. **Analysis only: no feature code was written, no test was added.** Two throwaway test files and one Python scanner were used to measure and then deleted (the scanner lives in the session scratchpad, outside the repository). Stops here for the owner's written go (section 10) before any code.

## 1. What this note proves, and how

Everything stated as **measured (M)** was run on the real adapters and the real engine (`build_inventory_with_source_rows`, `engine::execute`) on a workbook built inline for the occasion; everything stated **by code (C)** was read in the code or in the source of `calamine` 0.26.1 and not run; **not verified (NV)** says so. Her files were scanned read-only **on copies** placed in the scratchpad; no file of hers was touched, and her index was not opened (no human test is proposed yet, so no count of this note depends on it).

Her settings (`settings.json`, read first) point the Data Folder at `C:\Users\elise\AssistantCabinetAI\Data`, **not** at the test folder. Both were scanned (section 4).

## 2. What the reader exposes (verified in `calamine` 0.26.1 and in the adapters)

The adapters (`xlsx_adapter.rs`, `csv_adapter.rs`) turn a file into `Workbook { sheets: [SheetData { name, rows }] }`, a dense grid of typed cells, plus `SourceRows` (the file row each grid row came from; used only by the knowledge base). **Nothing about layout reaches the inventory or the engine.**

| Fact about the file | `calamine` 0.26.1 | Adapter keeps today | Cost to obtain |
|---|---|---|---|
| Sheet visible / hidden / very hidden | yes, all formats (`sheets_metadata()`, `Sheet.visible`) | no (every sheet is read and reachable) | trivial, no new code path |
| Defined (named) ranges | yes, all formats (`defined_names()`, name and formula text) | no | trivial |
| Merged regions | `.xlsx` only, on the concrete `Xlsx` type (`load_merged_regions`); `.xls` on the `Xls` type (`worksheet_merge_cells`); **`.ods` none** | no | `open_workbook_auto` returns an enum; the extension must open the concrete type. `.ods` needs `content.xml` |
| Excel Tables (name, sheet, columns, data range) | `.xlsx` only (`load_tables`, `table_names`, `Table::{name, sheet_name, columns, data}`) | no | small; whether the table's totals-row flag is exposed: NV |
| **Hidden rows, hidden columns, outline groups** | **no**, in no format | no | direct XML read |
| **Autofilter and its active criteria** | **no** | no | direct XML read |
| Data validation, comments, protection, pivot tables, slicers, conditional formats | **no** | no | pivot/slicer: presence is visible from the zip entry names (`xl/pivotTables/`, `xl/slicers/`); the rest: XML |
| Date system 1904 | yes (`is_1904` is read) | handled by the library | none |

**A minimal direct read costs no new crate** (`zip` and `quick-xml` are already dependencies). For `.xlsx`, one streaming pass over `xl/worksheets/sheetN.xml` reading only `<row r hidden outlineLevel>`, `<col hidden>`, `<mergeCells>`, `<autoFilter>` with its `<filterColumn>` children, and `<tableParts>`; for `.ods`, one pass over `content.xml` reading `table:table-row` (`table:visibility` = `collapse` or `filter`), `table:table-row-group` (`table:display`), `table:number-columns-spanned` and `table:number-rows-spanned`, and `table:database-range` with `table:filter`. Estimate: 1 day with tests for both. **`.xls` offers neither hidden rows nor filters** (binary format): the honest answer is a caveat, not a guess (section 8). A filter shows in `.xlsx` as `hidden="1"` on each filtered-out row plus an `<autoFilter>` with a `<filterColumn>`: the rows alone cannot say "filtered" from "hidden by hand", the autofilter element can.

`engine::reachable_sheets` returns **every** sheet of the workbook, usable or not (C), and `resolve_sheet_inventory` answers `AmbiguousSheetName` as soon as two are reachable and none is named (M, experiment I). The card, the sample and the knowledge base ignore a sheet that is not `looks_tabular()`; the engine does not. Whether a visible, unusable sheet such as `Notes` triggers the same ambiguity in a real question: NV here (the hidden-sheet case is measured); Phase B measures it first.

## 3. The inventory of situations

In this table a statement with no mark is **by code (C)**; **M** marks a measurement. Columns: what the engine does **today**; the **right behaviour**; **her files** (section 4). **Pr** is the priority of section 9.

| # | Situation | What the user sees | Engine today | Right behaviour | Pr |
|---|---|---|---|---|---|
| 1 | **Hidden rows** (by hand) | rows absent | read and counted (M: row 4 of 1 000 counted: `Count` 5 where the screen shows 4, `Sum` 1 036 where it shows 36) | counted or not by a stated default; always disclosed (section 5) | 1 |
| 2 | **Rows hidden by an active filter** | the filtered list | same as 1 (M) | same as 1, worded "filter active: N of M rows visible" | 1 |
| 3 | Hidden columns | column absent | read, typed, listed on the card (C) | keep usable (a figure about a hidden column is true); the card marks it "hidden" | 3 |
| 4 | **Merged cells in the body** (a category spanning rows) | the value on every covered row | only the top-left cell holds it; covered cells are empty (M: category merged over 2 rows, `Frais` totals 11 where the screen reads 31; the 20 on the covered row falls out of the groups) | fill the covered cells with the top-left value inside a table, say so on the card | 1 |
| 5 | Merged header (one label over several columns) | one wide label | the label sits on the first column, the others are `column_N` (C) | name the covered columns from the label, with a second header row as a suffix when there is one | 2 |
| 6 | Merged title above the table (`A1:F1`) | a title | read as a one-cell row above the header; the header scorer skips it (M: header found on row 3) | nothing to change; counted as "rows above the table" | 4 |
| 7 | Several header rows | a two-line header | the best-scoring single row wins; the other becomes a data row (C) | detect (second all-text row under a header with merges or blanks) and compose names | 2 |
| 8 | **Two or more tables on one sheet** | two blocks | one header, everything below is data: the second header is a data row; a numeric column of the second table makes the first column `categorical` (M: `Count` 5 for 2 + 2 data rows; `Sum` refused because `montant` is typed `categorical`) | detect regions (section 6); one table: use it; several: ask which, never merge | 2 |
| 9 | Notes or titles **above** a table | text | handled by the header scorer (M, measured on 73 of her files) | none | 4 |
| 10 | Notes **below** a table (after a blank line) | text | counted as data rows (C) | footer block after a gap: left out and reported | 2 |
| 11 | **Total or sub-total row** | a line "Total" | **counted as data**: M: `Sum` 100 where the data sum to 35 (a "Sous-total" 30 and a "Total" 35 added to 35); `Distinct` lists `Total`. With a `SUM` formula in the row, the whole column becomes `has_formulas` and every sum on it is **refused** (M on her files); and the label spoils the typing of the first column (see section 4) | shared with step 15 (section 7) | 1 |
| 12 | Excel Tables (`ListObject`) | a named table | not read; the region is guessed from the header score | exact region and name from the file | 2 |
| 13 | Named ranges | a name in the name box | not read | optional label for a region; no figure depends on it | 4 |
| 14 | Pivot tables and slicers | a summary with totals and indented labels | read as ordinary cells; the totals and grand total lines are data (C) | detect by zip entry names; caution on the card; its totals never added to a sum | 3 |
| 15 | Data-validation lists | a drop-down | ignored | nothing for step 13; the allowed values are a candidate input for the column glossary (step 14) | 4 |
| 16 | Cell comments | a red corner | ignored | nothing; not an arithmetic risk | 4 |
| 17 | **Hidden / very hidden sheets** | absent (very hidden cannot even be unhidden from the interface) | read, reachable, listed when the engine asks "which sheet?" (M, experiment I: the very hidden sheet is offered) | not reachable unless named; reported once ("1 hidden sheet not used") | 1 |
| 18 | Protected sheets (no open password) | read-only | read normally; protection does not block reading | nothing | 4 |
| 19 | File protected by an **open password** | the file will not open | `ReadFailed`, indistinguishable from a broken file (C) | a distinct machine code `password_protected` (the container signature is readable) so the interface says so | 2 |
| 20 | Numbers stored as text | green corner, left-aligned | typed `numeric` when 95 % of the column parses; otherwise `categorical` and every sum refused (M: a column of 5 cells with `n/a`, `-` and `(15,00)` is refused) | placeholders (`-`, `n/a`, an em dash) count as empty, not as unreadable; accounting negatives `(15,00)` are numbers | 2 |
| 21 | Dates stored as text | dates | ISO and day-first text are typed `date` when unambiguous; ambiguous ones are flagged (existing, decision D3) | none; `03/2026` and "le 11 mars 2026" stay as measured in step 12 (S12-3) | 4 |
| 22 | Text with leading/trailing spaces, mixed case | one label | **separate groups** (M: `Frais`, ` Frais`, `Frais `, `frais` are four groups); spreadsheet pivots group case-insensitively | group and compare on a trimmed, case-folded key (accents kept); show the commonest spelling | 2 |
| 23 | Formulas returning errors | `#DIV/0!` | the cell becomes the text `#ERROR:Div0` (a Rust debug string); it counts as an unreadable value in a numeric column and as a label elsewhere (C) | keep the cell out of figures, count error cells per column, write the spreadsheet literal, never `Debug` | 2 |
| 24 | Very wide sheets / inflated used range | scroll | no cap; `calamine` returns a **dense** range, so one stray formatted cell far away makes a very large grid (NV: not measured) | measure with a pathological sheet; add a size ceiling with a machine code if it hangs (plan decision 21: weakest workstation) | 3 |
| 25 | **Blank rows inside a table** | a gap | `.csv`: the reader drops blank lines (M); `.xlsx`/`.ods`: a blank row stays in the grid and is counted as a data row (C; not run) | ignore a fully blank row inside a table, report the number | 2 |
| 26 | Header only, no data (CSV or sheet) | a header | `header_row = None`, the header line is **one data row** and the columns are `column_N` (M: `Count` 1) | a header with no data row below is a header; zero rows | 2 |
| 27 | `.xls` | as `.xlsx` | accepted and read by the library's `xls` reader; **no automated test covers it** (no fixture exists and none may be added) | caveat: hidden rows and filters cannot be checked | 3 |
| 28 | `.xlsb` | as `.xlsx` | **not accepted** (`FileKind::Unsupported`, `file_record.rs`); the library could read it | leave as is (section 10, Q5) | 4 |
| 29 | `.csv`: tab or pipe delimiter, UTF-16 | rows | only `,` and `;` are tried: a tab- or pipe-delimited file is one column and red (M, G1 and G2); UTF-16 falls to CP1252 with NUL bytes (C, not run) | score the four delimiters with the existing consistency function; sniff a UTF-16 byte-order mark | 3 |
| 30 | A small table that has one total formula | a table | **the whole file is red**: `looks_tabular` allows formula cells up to 5 % of the data rows, so a sheet of fewer than 20 rows with a single `SUM` fails (M on her files, section 4) | amber, not red (section 8); resolved at the root by situation 11 | 1 |

## 4. Her files

**Configured Data Folder** (`C:\Users\elise\AssistantCabinetAI\Data`): 15 spreadsheets (9 `.csv`, 6 `.xlsx`; the two other files are a `.docx` and a `.txt`), 17 sheets. Layout situations found: **none** (no hidden row, column or sheet, merge, filter, table, pivot, error cell, title row, total row, blank row, second table). Three `.csv` carry a byte-order mark (read correctly); four workbooks hold ISO dates as text (read as dates); `revenue_sub_agency` (35 854 rows) has 870 amounts that are not numbers, already counted and shown beside a sum.

**Test folder** (`C:\Users\elise\AssistantCabinetAI-test\Data`): 510 spreadsheets (204 `.xlsx`, 153 `.ods`, 153 `.csv`), 626 sheets, all read without error. Counts of files showing each situation:

| Situation | Files | Note |
|---|---|---|
| Title or note row above the table | 73 | header found correctly in every case checked |
| Merged cell | 30 | the 15 `.xlsx` ones are all `A1:D1` to `A1:G1` (the title row); the 15 `.ods` ones were counted, their spans not located |
| Last row "Total" with a `SUM` formula | 31 (15 `.xlsx`, 16 `.ods`) | label always `Total` |
| of which the **whole file is red** (`not usable`) | **11** | small tables: a single total formula exceeds 5 % of fewer than 20 rows (M) |
| of which the first column (`Date`) is **not typed `date`** | **25 of 31** | the word "Total" in a date column makes it `categorical`, so a date question ("in June") finds no date column (M) |
| Extra `Notes` sheet beside the table | 51 | 3 text lines, not a table |
| More than one sheet | 87 | |
| Header-only or one-line `.csv` (header read as data) | 5 sheets usable but without header | e.g. `patients-*` lists |
| Numeric-looking text in a cell (postcodes, identifiers) | 40 | `patients-*` lists; typed as text, which is right for an identifier |
| Hidden or very hidden sheet, hidden row or column, filter, Excel Table, pivot, error cell, protected sheet, comment, data validation, defined name, external link | **0** | not exercised by her fictional files |

So her fictional folder exercises **titles, merged titles, total rows and extra sheets**, and **none of the hidden-row, filter, merge-in-body, table or pivot situations**: those are proved only by files built inline (section 11).

## 5. The rule

Proposed principle (plan decision 18, refined): **the engine computes on what the file holds and states, beside the result, every difference between that and what the screen shows; it never changes a figure silently, and it offers the other reading in one click.**

**Two kinds of output, two defaults.** This is the recommendation; the choice is the owner's (Q1).

| Output | Hidden and filtered rows | Why |
|---|---|---|
| A **figure** (count, sum, mean, min, max, median, groups, distinct) | **included**, with a line "2 hidden rows included (sum 1 000,00 of 1 036,00)" | reproducible from the file alone; the same as the spreadsheet's own `SUM`; the line shows the effect and the click offers the visible-only reading |
| **Rows acted on or sent**: a row listing, the sample and rows sent to the writer, a mail merge | **excluded**, with a line "2 hidden rows not used" | the user cannot see these rows; sending them to a model or writing a letter for each is the worse error; this follows the data-minimisation line of decisions 36 and 37 |

Alternatives for Q1: **B** exclude hidden and filtered rows everywhere (matches the screen, but disagrees with `SUM` and with a reader who hid rows to ignore them for display only); **C** include everywhere (simplest, but a letter is written for a row nobody sees).

**One click.** Under a figure that had hidden or filtered rows: a button "Recompute without the hidden rows" (or "with" when they were excluded). It is a new request, not an edit: `AskOptions` gains a field `rowPolicy` (never a tenth positional parameter); the backend recomputes with the other policy and appends a **new** answer whose "Understood as" line says so; the figure on screen is not replaced (decision 11 keeps figures stable). The choice is carried over to follow-up questions by the existing carry-over (`TableCarryOver`), so "and for March?" stays on the same reading.

**Wording of the disclosure** (two catalogues, machine codes from Rust): `rows.hidden_included`, `rows.hidden_excluded`, `rows.filtered_included`, `rows.filtered_excluded`, `sheets.hidden_skipped`, `cells.merged_filled`, `rows.notes_left_out`, `rows.blank_ignored`. Each carries counts only, never a value.

**Where it appears.** (a) The "Understood as" line (the existing filter vocabulary) gains the row policy. (b) The calculation record of step 06 (`CalculationRecord`) gains a field `layout` and a line `Rows: 4 kept out of 5 data rows (1 hidden row excluded, source row 4)`; source rows come from `SourceRows`, which already survives gaps. (c) The workbook card (step 02) states the sheet's layout facts: `Sheet "Data": 5 data rows (1 hidden), 1 merged region filled down`. (d) The usage trace (step 04) is unchanged: it records which workbook was used; the row policy is a property of the figure, not of the file. (e) The line is a **caution** for read-aloud only when the policy changed a figure (rows included or excluded that moved the result), never otherwise.

**Mechanism: a view, not 20 edits.** `inventory::data_rows` is the single choke point for the engine, the sample, `rows`, `query_plan`, `escalation` and the knowledge base; but a grep finds 13 direct reads of `SheetData.rows` outside `tabular/` (`template_fill` 8, `commands` 2, `fill_plan`, `tabular_answer`, `knowledge/suggest`), among them the mail merge. Recommended: a pure module `tabular/layout.rs` with `fn view(workbook, inventory, policy) -> View { workbook, inventory, source_rows }` that **physically removes** the rows a policy excludes (and re-bases `header_row` and `row_count`), so every reader sees a consistent sheet without a signature change; a source guard test (the style of `guards/sources.test.ts`) lists the allowed direct readers of `SheetData.rows` and fails on a new one. The view is rebuilt per question from the cached workbook and the stored layout (one pass over cells, the same order of cost as the inventory build, measured later on the 35 854-row workbook).

**Where the layout facts live.** On the **inventory** (`SheetInventory.layout: Option<SheetLayout>`, `#[serde(default)]`), not in the typed cell cache: structure only (row numbers hidden or filtered, merged regions, regions, sheet state, counts), already persisted and already read by cards. `SheetInventory {` appears 8 times in 3 files; `SheetData {` 25 times in 14 files, which this avoids. `None` means "analysed before this existed", which is not the same as "nothing found".

## 6. Several tables, notes and totals around a table

**Detection (deterministic, no model).** (1) Split the sheet into bands at fully blank rows and, inside a band, into blocks at fully blank columns. (2) A block is a **table** when it has a header-like first row (all text, distinct labels, at least two columns, typed values below: the existing `detect_header_row` scoring applied per block) and at least one data row; otherwise it is a **note** (title, footnote, legend). (3) A single blank row between two blocks whose columns have the same count and the same type pattern is a **gap inside one table**, not two tables. (4) Excel Tables, when present, give the exact region and override the guess.

**When it is uncertain: ask, never guess.** One table region: use it. Several table regions and the question does not pick one: the answer is a question ("which table: A3:F40 'Date, Libellé, ...' or H3:K20 'Fournisseur, ...'?"), the same pattern as `AmbiguousSheetName` and step 08's `sheet_to_choose` (a new reason `AmbiguousRegion`, a trace reason `region_to_choose`). A block the scorer cannot classify (borderline score) counts as a table candidate, so the user is asked rather than a table silently dropped. Notes are listed in the card as "N note rows around the table" and left out of figures.

**Effect on the card.** One entry per region: `Sheet "Journal", region 1 (A3:F49): 47 data rows`, columns as today; `Sheet "Journal", region 2 (H3:K20)`. The card budget rules of 4.1 apply unchanged (shrink, then truncate columns). `SheetInventory` keeps its present fields for the **primary** region when there is exactly one, so nothing that reads it changes; with several, the fields describe the first and `layout.regions` lists all (the engine, cards, sample and carry-over must pick by region: sized in section 9).

**Totals and sub-totals: who does what.** *Step 13 owns the mechanism*: row roles (`data`, `hidden`, `filtered`, `blank`, `note`, `total`), the policy view, the disclosure line and the calculation-record field. *Step 15 owns the detectors and the formula semantics*: unlabelled totals, a row of `SUM`s over the column above, sub-totals in the middle of a table, a totals column on the right, the formula help. **Recommendation (Q2): move the one unambiguous case into step 13**: a data row whose first text cell is a locale total word (`total`, `sous-total`, `subtotal`, ...: a new per-locale list, plan decision 19, with its parity test) and that is the last data row of its region or is followed only by blank rows. Reason: it is the most frequent defect in her own files (31 of 31 total-bearing files; 11 red, 25 with a spoiled date column), the mechanism already exists in step 13, and without it step 13 would ship the hidden-row disclosure while `Sum` still returns 100 for data that sum to 35 (M). Step 15 then keeps everything else and its prompt is corrected in place.

## 7. Effects on cache, status and existing tests

* **Cache.** No version number exists: `inventory_json` is a serde value and the typed cell cache is cleared on every Analyse. New fields are `#[serde(default)]`; an old inventory has `layout = None`. The risk is silence, not breakage: an old inventory would answer as before with no layout line. Mitigation: the cheap launch check already allowed by `AGENTS.md` (one query) counts inventories without a layout and the explorer shows "N spreadsheets were analysed by an older version: run Analyse again" next to the Analyse button. Nothing re-reads a file by itself; the re-Analyse is started by hand.
* **Status.** The explorer has three states (`analysed`, `toAnalyse`, `unreadable`; `explorerFilters.ts`) and **no amber**. Recommendation (Q4): do not add a fourth state (saved views and filters keep their meaning); add a **caveat marker** on analysed files (icon and tooltip listing the layout facts). A file becomes `unreadable` only when no sheet is usable *after* the layout rules: situation 30 (11 of her files) stops being red.
* **Existing tests.** The wave 1 regression set stays green by construction: with no layout facts the view is the identity. Struct-literal sites: `SheetInventory {` 8 mentions in 3 files, none for `SheetData`. The locale-pack parity test (`tests/tabular_pack_parity.rs`) gains the total-words list. `tests/large_test.rs` still sends no cards and is not touched.
* **`.xls`.** Cannot be covered by an inline fixture (binary format, no writer) and no fixture may be added: coverage of `.xls` is a file the owner supplies, if wanted (Q5); otherwise it is reported as a caveat, not as tested.

## 8. Usable, amber, red

* **Red** (`unreadable`): no usable sheet after layout rules; unreadable bytes; open-password file (with its own code, situation 19).
* **Analysed with caveats** (marker, not a state): hidden or filtered rows present, merged cells filled, several regions, notes left out, hidden sheet skipped, pivot present, `.xls` (hidden rows not checkable), error cells present, blank rows ignored. The tooltip lists them with counts.
* `looks_tabular` is relaxed in one place: formula cells that sit in a **total row** do not count against the 5 % budget (they are not data).

## 9. Priorities, sizes and a proposed subset

Sizes: S under half a day, M about a day, L two days or more; tests included.

| Pr | Item | Situations | Size |
|---|---|---|---|
| 1 | `layout.rs`: facts read from `.xlsx` (XML) and `.ods` (XML) and from the library (sheet state), stored on the inventory | 1, 2, 3, 17 | M |
| 1 | The policy view, the guard test, the `rowPolicy` request field and the one-click, the disclosure lines in the two catalogues, the record, the card line | 1, 2, 17 | L |
| 1 | Merged body cells filled down; merged header names | 4, 5 | S + S |
| 1 | Labelled total rows excluded (Q2) and the red-file fix (`looks_tabular`, date typing) | 11, 30 | M |
| 2 | Regions: bands and blocks, notes, gaps, Excel Tables, `AmbiguousRegion`, card per region, carry-over by region | 7, 8, 10, 12, 25 | L |
| 2 | Values: placeholders and accounting negatives, trimmed case-folded group key, error literals and per-column error count, header-only table | 20, 22, 23, 26 | M |
| 2 | Open-password code | 19 | S |
| 3 | CSV delimiters and UTF-16; pivot and slicer detection; used-range ceiling (measure first); `.xls` caveat | 29, 14, 24, 27 | M |
| 4 | Named ranges, validation lists, comments, `.xlsb` | 13, 15, 16, 28 | not proposed |

**Proposed subset for Phase B (owner decides):** all of priority 1, and from priority 2 the values item, the open-password code and the regions item **restricted to "one table plus notes, or ask"** (situations 8, 10, 25 and Excel Tables; no composed multi-region cards beyond listing them). Roughly 7 working days. Priority 3 and the rest stay out until a real file needs them.

## 10. Questions for the owner (written answers needed before Phase B)

* **Q1. Default for hidden and filtered rows.** A (recommended: figures include and disclose, rows acted on or sent exclude and disclose), B (exclude everywhere), C (include everywhere).
* **Q2. Labelled total rows.** Move the unambiguous case into step 13 (recommended), or leave all of it to step 15.
* **Q3. Regions.** Detect and ask (recommended, subset above), or only note the second table in the card for now.
* **Q4. Status.** Caveat marker, no fourth state (recommended), or a real amber state.
* **Q5. `.xls` and `.xlsb`.** Keep `.xls` with a caveat and do not add `.xlsb` (recommended); if you want `.xls` covered, put one fictional `.xls` of your own in a scratch folder and tell me where.
* **Q6. The one-click.** Appended new answer, figure on screen unchanged (recommended), or replace the figure.

## 11. Test plan and human test outline

**Automatic tests (Phase B).** Extend `tests/common/tabular_fixtures.rs::write_xlsx` (never a file under `fixtures/`) with: hidden rows and columns, an `autoFilter` with and without `filterColumn`, `mergeCells` (title, header, body), a hidden and a very hidden sheet, a `tableParts` table, an error cell (`t="e"`), a fully blank row, a second table under a blank row, a total row. Golden results are stated independently by hand, not by running the engine: for example the workbook of experiment H (rows 10, 20, 1 000 hidden, 5, 1) gives `Sum` 36 with the hidden row excluded and 1 036 included, `Count` 4 and 5, `Frais` 31 after the fill-down and `Charges` 5 with the hidden row excluded (1 005 included); the total experiment (35 data, sub-total 30, total 35) gives `Sum` 35 with the total rows excluded and 100 included. `.ods`: the same shapes through a small inline `content.xml` writer in the same helper file. One test per disclosure code in both catalogues; a guard test for direct readers of `SheetData.rows`; the pack parity test for the new list; the wave 1 suites unchanged.

**Human test outline (written in full in French at the end of Phase B).** Files built for the occasion by a test helper into a scratch folder **outside** the repository and outside her test folders (for example `C:\Users\elise\AssistantCabinetAI-test-real-world`), chosen as the Data Folder through the existing picker; one backup at the start and one restore at the end (her `settings.json` only). Parts: (1) a clean list, nothing new appears; (2) a list with two hidden rows and a filter: the same question gives the stated total and the line, the click gives the other total; (3) a merged-category journal and a labelled total row; (4) a sheet with two tables: the question asks which; (5) a workbook with a hidden sheet; (6) the previous wave's regression script, unchanged. Counts come from her real analysed index copy, run through the real path before the script is written.

## 12. Prompts to correct after the go

Read for this note: `SESSION-TABULAR-14-column-glossary.md` and `-15-formula-columns-and-totals.md` are the two that depend on this step (plan section 5, "Depends on"), and 15's boundary with this note changes if Q2 is answered "yes". They are corrected after the owner's answers, with the other prompts that name `looks_tabular`, `SheetInventory` or `engine::data_rows` (steps 15, 17, 18, 19, 22 to 24), not before: the corrections depend on Q1 to Q4.
