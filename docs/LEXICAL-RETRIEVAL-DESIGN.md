# Indexing and file search without a model: design note (revision 4)

14 October 2026 (night) · branch `feat/lexical-retrieval` · **no product code written**. Revision 4 records the answers Q1 to Q5, adds the file name as a first-class indexing criterion (5.3, gap G9) and the isolation rule that protects the spreadsheet engine (4.8, 8); revision 3 recorded the owner's answers D1 to D6 (section 10) and adds the sections on types and formats (5.1), long documents (5.2) and the search fields in the two folder cards (6.1); revision 2 replaced the first version after the owner's reframing of the same day. Inputs: `docs/SESSION-LEXICAL-FIRST.md`, `docs/INDEXING-RETRIEVAL-INVENTORY.md`, `docs/RETRIEVAL-LAB.md`, `docs/KNOWLEDGE-BASE.md`, `docs/DECISIONS.md`, and the code read for it (`knowledge/store.rs`, `knowledge/mod.rs`, `analysis_scope.rs`, `tabular/inventory.rs`, `index_store.rs`, `folder_questions.rs`, `retrieval.rs`, `commands.rs`). English; every interface string it mentions is French and lives in the catalogues only.

## 1. The owner's directives (14 October 2026, binding, recorded verbatim in substance)

1. **Indexing is the most important step.** Search comes after, and only from a *perfect* indexing of the documents.
2. **The Knowledge Base work is enormous and must be used.** Entities must be a first-class part of indexing: where and how they are handled at indexing must be examined and deepened, to extract the maximum from the **metadata of the documents** and from the **spreadsheets**.
3. **The interface will be redefined in the near future** as a real workspace: **the files of the work folder on the left, a viewer of the "opened" files in the centre, the chat on the right.**
4. **File search is the user's first step**, in the left list: a system to **filter the "selected" files**, those that may be processed by the user's questions. So the priority of *this* work is: **perfect indexing methods, then refined search methods, with no LLM, to filter efficiently the files "selected" in the scope.**
5. Corrections must be made **with that UX/UI vision in mind**. Requests to the owner must be **clear and precise**.
6. Standing rules still apply: engine searches, model only writes; heavy work only in an Analyse started by hand; the tabular data is never touched by text re-indexing; short documents stay byte-identical; modules never change entity types; Windows and macOS both; no French in Rust; no real patient data.

## 2. What this changes in the plan

| | First version of this note | This version |
| --- | --- | --- |
| Centre of gravity | a chat sentence (« Quels documents parlent de X ? ») answered from the existing index | **the file profile built at indexing**, then **one file finder** that serves the left panel, the chat and the entity chips |
| Order | question frame, identifiers, set-question road | **indexing first** (profile, metadata, entities, spreadsheets, clean identifiers), **then** the finder, **then** the chat sentence as one more way into the finder |
| Schema | none (query side only) | **yes**: new tables for the profile and its full-text columns. The index may be rewritten (her test files are saved); an old index is detected at launch and never rebuilt by itself |
| Output of search | a Markdown list in the chat | a **structured result for any interface**: files, why each matched, facet counts, coverage; the chat sentence is a rendering of it |
| Entities | not mentioned | central, section 4 |

What is kept from the first version (sections 6 and 7): the question frame, identifier forms, the contracts `Locator` / `Coverage`, the six-sentence acceptance. They become front doors of the finder instead of a separate road.

## 3. Where entities are handled at indexing today (read in the code, not assumed)

**The flow.** The Analyse extracts text, chunks it, embeds it, and writes `documents`, `chunks`, `chunks_fts`. **At the end of the same Analyse**, and only when the knowledge mode is not `off`, the Knowledge Base reads the text the index already holds (documents, `knowledge::extract::text`) and the headers of the workbooks (`knowledge::extract::table`), resolves every name against the entities it already knows (`DeterministicResolver`), and writes `kb_sources`, `kb_entities`, `kb_aliases`, `kb_mentions`, `kb_attributes`, `kb_relations`, `kb_possible_matches`, `kb_column_semantics`. Personal identifiers are stored as a keyed hash. Manual edits survive re-analysis. A second Analyse over unchanged files writes nothing (I14).

**What a mention can point at.** `kb_mentions.locator_kind` is one of `chunk`, `cells`, `filename`, `metadata`, with `chunk_id`, `sheet`, `column_name`, rows, a `role`, a `method` and a `confidence`. So **per-passage** and **per-cell-column** provenance already exists.

**What is used today.** The only consumer of this knowledge at question time is *targeting*: names in a question → files of the selection that mention them → a smaller search. The picker, suggestions, sheet of a name and the management dialog read the same tables.

**The gaps that matter for the new goal** (each verified):

| # | Gap | Evidence |
| --- | --- | --- |
| G1 | **No document metadata is read at all.** Nothing reads the PDF Info/XMP dictionary, the DOCX `docProps/core.xml` and `app.xml`, or the properties of a workbook. Only file size and modification time (`discovery.rs`). | no match in `ingest.rs`, `extract/*.rs`, `extraction.rs` for any of it |
| G2 | **The `metadata` locator exists in the schema but nothing writes it.** The hook for « an entity found in the metadata » is built and empty. | `store.rs` line 124; no writer |
| G3 | **Roles are only column roles** (`client`, `provider`, ...). A document's *author*, *sender*, *recipient* are not recorded, although the header block of a letter states them. | `knowledge/extract/text.rs` reads titles, organisations, identifiers, a name alone in an address block; no header-block roles |
| G4 | **No file → entities view.** The base answers « name → files » for targeting. Nothing builds « file → its people, organisations, identifiers » to show on a file row or to filter by. | consumers listed above |
| G5 | **No document type, no document date, no title** as indexed facts. The date is in a file-name prefix by habit, not read. | no field in `documents`, `FileRecord` |
| G6 | **Spreadsheets are only inventoried for the table engine** (sheets, columns, types) and read for entity columns; the **names of the sheets, the vocabulary of the headers, the date range of the date columns** are not findable as a file. | `tabular/inventory.rs`, `kb_column_semantics` |
| G7 | **The KB runs after the text index and is skipped when the mode is `off`.** The file profile must not depend on it for the fields that need no entity (title, type, date, format). | `knowledge/mod.rs` `reads_files` |
| G9 | **The file name is not searchable as words.** `chunks_fts` holds `chunk_id` and the passage text only. The only use of a name is the **reference resolver** (`file_reference.rs`), which turns a name typed *in full* into one file (exact path, name, name without extension, end of a name once a date prefix is dropped, a folder-ending path) and never searches: « dermatologie » does not find `2026-10-01_courrier-dermatologie.pdf` unless the passages say it; two words of a name in another order find nothing; folder names are not facts. Names of people and organisations in a file name are linked to the base only when **the whole run of name-like words equals one known entity** (`knowledge/extract/text.rs` `file_name`, `hit.len != run.len()`): `decompte-helsana-martin-marie` mixes a type word, an organisation and a person in one run, so from my reading of the code none of them is linked (to be proved by a test in step 1 before anything is changed). The method `filename_token` has confidence 0.50 and only links to existing entities. | resolver and extractor read; no full-text column for names |
| G8 | **A cell's text is not in the full-text index.** A word typed in a search box cannot find a workbook by what its cells say, except through an entity the base extracted. The cells *are* already at rest in the same index file (`tabular_workbooks` holds cell values; `index_store.rs`, `put_tabular_workbook`), so searching them adds no new exposure. | decision 2, section 10 |

## 4. Entities in indexing: the deepening

Principle (AGENTS.md): **a module never excludes or transforms an entity type.** A person stays a person, an organisation stays an organisation, whatever packs are active; packs add vocabulary, patterns and roles. Everything below adds *evidence and roles*, never new types.

**4.1 Entities from the metadata (closes G1, G2).** Read the document's own properties at Analyse and record them as evidence with a `method` of their own (`metadata_author`, `metadata_title`...) and a locator `metadata`, confidence below a title-and-name hit, **in the profile's own entity table first (4.8), not yet in `kb_mentions`**, because metadata is often the *software's* or the *scanner's* name. A list of generic values (« Microsoft Word », a scanner model, `admin`, the user's own name) is **data in the pack**, never code, and such a value produces nothing. Sources by format: PDF Info and XMP (through `pdfium-render`, already a dependency, Windows and macOS), DOCX core and app properties (the `zip` reader already used for extraction), TXT and MD front matter, workbook properties (`calamine`).

**4.2 Roles at document level (closes G3).** A document gets, per entity, a role from a small neutral vocabulary: `author`, `sender`, `recipient`, `subject` (the person the document is about: patient, client), `issuer` (an organisation), `mentioned`. The header block of a letter or form (« De », « À », « Objet », « Date », signature block) is read with **per-country pattern packs as data** (`health-fr`, `health-ch`...), cumulative, never a flag inside one pack. The same entity row serves all roles; the role is on the *mention*, as today.

**4.3 The file → entities view (closes G4).** One query over `kb_mentions` grouped by `source_id` gives each file its entities with role, confidence, method and count; it is joined into the profile (section 5) and refreshed with the KB pass. Manual edits (rename, merge, delete) show in the finder at once because it reads the live tables, not a copy.

**4.4 Per passage.** `kb_mentions.chunk_id` already ties a mention to a passage. The finder uses it to show *which passage* matched an entity and to highlight it; using it to reduce a search inside a long document stays in shadow mode (inventory 3.2).

**4.5 Spreadsheets (closes G6; owner decision D2 = B).** From the inventory and the base, per workbook and per sheet: sheet names, header vocabulary (folded), the semantic type of each column (`kb_column_semantics`), the entity columns and how many distinct entities each holds, the date range of the date columns, the currencies. All findable and filterable as a file. In addition, the **text values of the cells are searchable by word** (decision D2 = B), with these rules, which answer the owner's question on names, organisations and identifiers:

- **Names of people, companies, organisations, places: included.** They are exactly what a search must find. Columns the base already reads as entity columns (a header such as `patient`, `fournisseur`) keep producing entities as today; their values are also searchable as words.
- **Identifiers (health insurance numbers, IBAN, e-mail, phone): also included**, as exact tokens (compact and grouped forms, section 7), because documents already hold them in clear in the full-text index and the workbook cells are already at rest in the same file; leaving them out of workbooks only would be inconsistent and would hide a workbook from the question « which file holds this number ». *This corrects a restriction proposed in revision 2.* The KB keeps its keyed hash for its own identity matching; that is unchanged.
- **Numbers, amounts, dates: excluded** from the word index (they are the table engine's data and are already queried there); a date column still contributes its **date range** to the profile.
- **Size rule.** Free-text columns: distinct values up to a cap (starting value 5 000 per column, to be measured on the fixtures); when a column exceeds it the profile records `truncated` and every answer that rests on it says so. Entity and identifier columns are not capped (they are what the search is for).
- **Cross-checking with entities (new).** Today a column is read for names only if its header is recognised, or if all its values have an identifier's shape (`knowledge/extract/table.rs`). A free-text column with no recognised header (`Remarques`, `Motif`) is not read at all. Added: a **known-names pass** over those columns, which links a cell to an entity **already known** from elsewhere (the gazetteer: names found in documents and in entity columns), with method `gazetteer` and its usual confidence, and **never creates a new entity from free text** (a surname alone stays ambiguous and is not linked). The effect: one entity, one person, whether she appears in a letter, in a patient column, or in a remark; « which files concern Marie Dupont » returns letters and workbooks alike.

**4.6 Order and independence (closes G7).** The profile is built in two layers inside the Analyse: layer 1, **no KB needed** (format, size, pages, language, title, type, dates, metadata fields, sheet names, headers, identifiers); layer 2, **entities** joined after the KB pass. With the knowledge mode `off`, layer 1 still exists and the finder still works on it.

**4.7 Candidates are not facts.** The base's rule is kept: a *candidate* entity ranks and is shown as « possible », and never silently narrows. In the finder an entity filter returns the confirmed files and lists the candidate-based ones in a separate « à vérifier » group.

**4.8 Isolation: new evidence goes to the profile tables, not into `kb_mentions` (owner: break nothing).** `kb_mentions` feeds targeting (`knowledge::reduce`, `targeting`), the « Quel classeur ? » annotations and the validated entity measures (lot 11). A new kind of mention (a metadata author, a header-block recipient, a name found in a remark cell) would change which files targeting keeps and which workbooks the deterministic engine is offered. So the new work writes its entity links in **its own table** (`file_profile_entities`: file, entity id or surface text, role, method, confidence, locator), read by the finder only. Promoting a kind of link into `kb_mentions` is a separate, later step, taken only if the knowledge validation and the `data` exam show no regression. With the knowledge mode `off` the table holds surface text without an entity id; nothing else changes.

## 5. The file profile (« fiche de fichier »): what indexing must produce

One row per file in a new table, built at Analyse, rebuilt when the file's content, the extractor version or the vocabulary changes (the existing `kb_version` / epoch pattern), idempotent (a second Analyse writes nothing).

| Field | Sources, in order | Notes |
| --- | --- | --- |
| identity | path, SHA-256, format, size, pages, language, OCR share and lowest page confidence | language by a stop-word vote from pack data |
| **file name and folder** | the name as stored (already cleaned at Analyse), the original name before cleaning (from the rename log, kept as an alias), the folder path | section 5.3: searchable words, prefixes, exact keys, and a source of facts (date, person, organisation, type, identifier) |
| title | metadata title → first heading (DOCX styles, Markdown `#`) → first meaningful line → file name | each with its source; a generic title is ignored |
| document type | file-name tokens → header words (« Objet », « Facture n° », « Résultats d'analyses ») → body markers | a **code** (`letter`, `lab_result`, `invoice`, `insurer_statement`, `prescription`, `certificate`, `quote`, ...) from pack data; the French label is in the catalogue |
| document date | header date → file-name prefix `YYYY-MM-DD_` → metadata created → modified | **all dates kept with their source**; the chosen one is flagged; the file modification time is never presented as the document's date |
| people and organisations | the KB mentions of section 4, with role and confidence | author, sender, recipient, subject, issuer |
| identifiers | the schemes of the packs (invoice, claim reference, IBAN, health insurance number...) | personal ones stay hashed in the KB; the searchable text is already in `chunks_fts` |
| workbook | section 4.5 | per sheet |
| state | analysed, partly read, unreadable, not yet analysed, source of the text (native or OCR) | feeds the `Coverage` of every answer |

**Rule: a field is filled with its source and confidence, or it is empty. Never a guess.** A wrong value is worse than an empty one, because the user filters on it.

Full-text: the profile's text fields (title, author, subject, entity names, header vocabulary, identifiers) are mirrored into one FTS5 table with **separate columns**, so `bm25()` column weights (title above body) become possible later (inventory 1.6) without another migration.

### 5.1 Types and formats (owner decision D5)

Three separate levels, never mixed:

1. **Domain**: `documents` or `data` (today's two folders, kept: the same relative path can exist in both).
2. **Format**: documents `txt` (and `md`), `docx`, `pdf`, `jpg`/`jpeg`, `png` (images go through local text recognition, so their profile is built from the recognised text, with the recognition confidence); data `xls`, `xlsx` (and `xlsm`), `csv`. The code already separates them (`file_record.rs`).
3. **Business type**: `letter`, `lab_result`, `prescription`, `invoice`, `insurer_statement`, `certificate`, `quote`, `meeting_minutes`, `appointment_letter` (convocation), `other`. Validated by the owner. A type is **assigned only when enough evidence exists; otherwise `other`** (never forced). For data files the same mechanism applies with their own starting list, to be validated (question Q4, section 10).

**Extensible by the user.** A type is a *rule as data*: a code, a label per locale, and marker words looked for in the file name, the title, the header block and the first lines (for a workbook, in sheet names and headers). The shipped list lives in the packs (cumulative, per country). The user's own types are stored in the local index file as manual rows (never overwritten by an Analyse, like manual KB rows) and edited from the interface later. Re-evaluating the types after a change reads the **stored profile only, not the files**, so it is a cheap operation, not a re-Analyse. Which extra types are worth defining is to be seen in use (owner).

### 5.2 Long documents are a design constraint from the first line (owner, 14 October)

Documents of a thousand pages were left out of the lab, but they **must be treated**, and the indexing is designed for them now so that nothing has to be redone: the profile has a place for a **table of contents** (the PDF outline, DOCX heading styles, Markdown headings, else slices of N pages) and for **per-section entities and dates**; extraction and profile building work **page by page and are written as they go** (never the whole document in memory); the Analyse of one file is **interruptible and resumable**; the finder returns a long document **once**, with the pages or sections that matched. The structure tables (inventory 1.3), the two-speed resumable ingestion (1.8) and chunking v2 (1.4) are the next branch, `feat/long-documents`, started as soon as the profile is in; a synthetic document of 1 000 pages (built locally from the extracts the owner already has, never committed) is its test.

### 5.3 The file name is a first-class indexing criterion (owner, 14 October)

If the user cannot find a file by citing its name, nothing else matters. Today (G9) a name only works when typed whole. The profile therefore treats the name and the folder as **indexed text and as evidence**, built by **one function** used both at Analyse (stored in the profile) and on the fly for files not analysed yet (so the left panel works from the first launch, before any Analyse, over the inventory):

1. **Searchable words.** The stem and the folder path are cut at `-`, `_`, `.`, `/` and between letters and digits, folded (case and accents, the same folding as the sanitizer), and written to their own full-text columns `name` and `folder`. The query is folded the same way, so `Ordonnance pour Esaie`, `ordonnance-pour-esaie` and `ordonnance esaie` all meet. A date prefix `2026-10-01_` is not words: it becomes the filename date (below).
2. **Prefix search.** `derma` finds `courrier-dermatologie` (prefix match on the `name` column, from two or three letters), because the field is used while typing.
3. **Any order, any subset.** Two words of a name in the other order, or only one word, find the file. Several files can match: all are returned (the resolver's rule « never choose between two » is kept: the finder lists, it does not pick).
4. **Exact keys.** The full name, the stem, the stem without the date prefix and the relative path are kept as exact keys, so the **reference resolver keeps working unchanged** and a named file is still never reduced (I2).
5. **The original name.** The sanitizer renames « Ordonnance pour Esaie.pdf » to a clean name and logs the original. The original is kept as a searchable alias, because that is the name she remembers and types.
6. **Weight.** A word found in the name outranks the same word found in a passage (the name is chosen by a person to describe the file). Column weights are fixed in the profile's full-text table from the start.
7. **The name as evidence for the other fields**, each with source `filename` and its confidence: the **date** (`YYYY-MM-DD_` prefix), the **business type** (marker words such as `courrier`, `facture`, `decompte`), **people and organisations** (all sub-runs of the name against the known entities, not only the whole run, which repairs G9; links to existing entities only, never a new one from a name alone), **identifiers** (the schemes of the packs applied to the stem, `FAC-2026-0114`), and the **folder** words (`patients/2026-03/` gives a month and a context).
8. **Not analysed, unreadable, image.** All of this works for any file in the inventory, including a scan whose text recognition failed or has not run: the name is the only thing known about it, and it is enough to find it.

**Acceptance (100 %, it is deterministic).** For every file of the fixtures, each of these queries returns the file (when the name is shared by several files, all of them): the full name; the stem; the name without date prefix; the name without extension; two of its words in another order; the beginning (3 letters) of one of its words; the same with capitals, without accents, with spaces instead of hyphens; the original name before cleaning. A person, an organisation or a type that is only in the file name finds the file through the finder and, when the entity exists, through the entity filter. One test per rule, written first.

## 6. The finder: search without a model, for the left panel first

**What it is.** One deterministic engine, one entry point, three ways in:

```text
left-panel search box and filters ─┐
chat sentence ("Quels documents…") ─┼──►  Finder::find(FinderQuery, FinderScope)  ──►  FinderResult
entity chips / "@" picker           ─┘
```

```text
FinderQuery  { text: Option<String>,                 // a word, a phrase, an identifier, or a whole sentence
               entities: Vec<EntityRef>,             // ids picked, or surfaces to resolve
               kinds: Vec<DocType>, date: Option<Range>, date_field,
               folders: Vec<String>, formats: Vec<String>, states: Vec<State>,
               limit, offset }
FinderScope  { WholeFolder | Selection(paths) }
FinderResult { total, files: Vec<FileHit>, facets: Facets, coverage: Coverage }
               // + typed_formats: the file extensions the command read out of the typed text (`.docx`, `xlsx`) and applied as a format filter; the panel lights the matching choice (explorer step 3)
FileHit      { relative_path, card: FileCard,        // title, type, date, format, top entities with roles, state
               reasons: Vec<Reason>,                 // word | phrase | identifier | entity | field(name)
               excerpt, locator }                    // the passage that matched, page or section
Facets       { by_type, by_year, by_entity (top n), by_folder, by_format }   // counts, for the filter controls
Coverage     { searched, unreadable, unanalysed, partial }
```

**Design consequences of the workspace target**

- **Interface-agnostic.** Machine codes and data only; no sentence, no chat Markdown. The chat renders a `FinderResult` as its set-question answer; the left panel renders the same result as rows with chips. This is a Tauri command; the web view never touches the server.
- **Cheap enough to run as the user types.** No embedding, no model, no file read: SQL over the profile and `chunks_fts` and the KB tables. Target (to be measured, not promised): under 50 ms at 5 000 files, under 200 ms at 20 000. It is a « cheap check » in the sense of AGENTS.md; it starts nothing heavy.
- **Works before and during an Analyse.** Files not yet analysed are findable by what the file system knows (name, folder, extension, size, modification time), and the result says how many files it could not search (`Coverage`).
- **Whole folder or selection.** The left panel searches the **whole work folder** (its purpose is to extend the selection); the chat sentence searches the **selection** only (nothing outside it may be searched, as today). Same engine, a different `FinderScope`.
- **The finder never changes the selection.** The selection is built only from the user's own actions (`docs/SELECTION-AND-MEMORY.md`). A result offers « add these N files to the selection » (and « replace »); the action goes through the existing `AnalysisScope`. Sheet restrictions of a workbook stay the table engine's business.
- **Honest explanations.** Every hit says why (`reasons`); a word search says it is a word search (a synonym or another form of the word is not matched); a count carries its denominator.
- **Facets** (counts per type, year, entity, folder, format) are computed in the same pass so the filter controls can show « courriers (41) · factures (22) » without a second query.
- **The pilot's vocabulary is data.** Document types, header words, generic metadata values: per pack, per locale, never in Rust.

### 6.1 The first interface test: a search field in each folder card (owner proposal D4)

> **Superseded in part (18 and 19 October 2026).** The bullet « the result replaces the selection » and the layout line « N fichiers sélectionnés par la recherche » below no longer hold: a search only filters the list, and the files found are added to the selection by a separate click, « Cocher les N résultats » (`docs/DECISIONS.md`, « the left panel becomes one file explorer » and « the explorer's search step »). Clearing the field never changes the selection, which is still true. An extension typed is now a format filter and a date typed alone also finds the files dated so (same section).

Feasible, and small, because the pieces already exist: the two cards (`WorkFolderCard.tsx`, `DataFolderCard.tsx`) already hold a disclosure titled « Documents utilisés : … » / « Données utilisées : … » with a select-all box and a ticked list; `analysisScope.ts` already builds an explicit selection (`scopeOfEntries`, `addScopeFiles`) with each file pinned to its content; Rust already refuses to look past that selection. What is added:

- **One small field per card**, searching only its own domain (`documents` or `data`), calling the finder through a Tauri command. The webview never touches the server.
- **The result replaces the selection** (as asked: the files found are the only ones the chat considers). Typing a search is the user's own action, so « the selection is built only from her own actions » still holds. The ticks stay editable by hand afterwards.
- Only **analysed** files can be selected; the field says how many found files are not analysed yet (« N trouvés, non analysés »).
- The count and the two existing warnings (« aucun », « tous ») must stay visible: proposed layout, the field takes the place of the title line and the line under it reads « 12 fichiers sélectionnés par la recherche « LAMal » » with a clear button (question Q1).
- **Clearing the field (owner Q1 = a):** the selection stays as it is; the found files stay ticked. **The unselected files stay visible**: while a filter is active the list shows the results first and a control « tous les fichiers » that shows the whole folder (view only, the selection does not change), and a **« Réinitialiser »** control that clears the filter and shows the whole list again (selection untouched). Filtering is a view; selecting is the effect of a search the user ran.
- Cheap by construction (no model, no embedding, no file read), so it can run after a short pause in typing.
- **The long-term single field costs nothing extra**: the finder result carries `domain` on every hit. Today two fields call it with one domain each; the unified side panel calls it with both.

Scheduled **after the finder** (step 9), so that the field is tested on the solid index, not on a poor one (question Q2).

## 7. The question frame, identifiers, and the set-question sentence (kept from revision 1, now front doors)

**Frame** (`question_frame.rs`, pure, no I/O). A set question is the prefix `opener + determiners + collection noun + [qui/que] + set verb`, then X. Any other word between the noun and the verb (a filter) means *not a set question* (it keeps its road). X is 1 to 4 tokens, or a quoted phrase, or an identifier; longer is a question about meaning and keeps its road. Quoted phrases and identifiers are taken out before tokenising. Entities are resolved through a small port backed by the KB resolver; an unresolved name stays a phrase. Vocabulary: a `set_question` object in `work-folder-questions/{fr-FR,en-US}.json` (openers, collection nouns, linkers, verbs as token sequences, determiners, `max_terms`), compared on folded forms. With the finder, the **kind noun becomes a real filter** (« courriers » → `kinds`) as soon as document types are indexed (section 5); until then it is read and reported, not applied.

**Identifiers.** The scheme regexes of every shipped pack (cumulative) already accept the compact spelling. For one identifier the finder searches **compact + grouped + as-written** forms as phrases, the grouping coming from a new optional `layouts` field on the scheme (AVS `[3,4,4,2]`, NIR `[1,2,2,2,3,3,2]`, Swiss IBAN, French IBAN). Index side, the profile also stores the identifier in its compact form so the compact and grouped spellings meet (inventory 1.6). On the ordinary chat road an exact identifier whose forms hit is **pinned first** among the passages (the flat `+0.2` bonus cannot tell it from `appartient` or `numéro`), behind a switch, kept only if the `identifier` family rises with no other regression.

**The chat answer.** `QuestionRoute::SetSearch` (pure parse in the router, placed after the named-file check and before `folder_answer`); `commands.rs` runs the finder with `FinderScope::Selection`, emits the existing `FolderAnswer` event with a new `DocumentSearch` kind, and returns before any embedding call. The first 50 hits are shown with excerpts, the others as paths, the total always stated, `knowledgeMaxCandidates` never truncates. No model call: the timing-log row has no generation field.

**Contracts** (inventory 2.1): `Locator { page, section }` (section empty until structure is indexed), `Coverage { searched, unreadable, unanalysed, partial }`, `DocumentListing`. `EvidenceCoverage` is left alone and merged later.

## 8. Schema, safety, and the Analyse

- **New tables only**, no change to `tabular_inventories`, `tabular_workbooks`, `kb_column_semantics` or any `kb_*` table: `file_profiles`, `file_profile_fields` (value, source, confidence, one row per field and file) and `file_profile_fts`. `documents`, `chunks`, `chunks_fts` change only when a later step (text cleaning, identifier forms, chunking v2) needs it, each with its own go.
- **The spreadsheet engine is not touched (owner Q5: break nothing, the deterministic engine scored 100 %).** The inventory code (`tabular/inventory.rs`), the adapters, the query plan, the engine, and the tables `tabular_inventories`, `tabular_workbooks`, `kb_column_semantics` are **read-only** for this work: no edit to those sources, no write to those tables. Everything new about workbooks (cell words, the known-names pass, sheet and header words, date range) is **additional**: it reads what the Analyse already stored, runs after the existing pass, and writes only to the profile tables (4.8). Proof, in this order: (1) before any change, a **snapshot** of the three tables and a replay of the `data` questions of the 205-question exam, recorded as the reference; (2) after the step, the three tables **byte-identical** and the same replay with the **same 100 %** result; (3) the cost of the added pass is measured and shown in the Analyse summary. If any difference appears, the step is reverted, not adjusted.
- **Old-index detection** at launch by one cheap query (a version row), shown as « L'index doit être refait : appuyez sur Analyser », never rebuilt by itself; an automatic test proves an old index is detected and left untouched (inventory 1.11).
- **Tabular protection test** (inventory 1.12): an index holding workbook data is migrated, and `tabular_*` and `kb_column_semantics` are byte-identical afterwards; the `data` questions of the 205-question exam are replayed before and after every step.
- **Analyse**: the profile is written in the existing Analyse, in two layers (section 4.6), per file, cancellable and resumable as the Analyse already is; metadata and header reading cost milliseconds a file, the embedding stays the dominant cost. Nothing runs at launch or in the background.
- **Privacy**: profile text and excerpts stay on the workstation in the index file; nothing is logged as text; the diagnostics rows carry counts and codes only.

## 9. Measurement: what « perfect indexing » means, and how it is proved

No claim without a ground truth, and no hand-checking.

- **Truth.** The deterministic generators (`scripts/gen-retrieval-lab.py`, the knowledge sandbox) already know each file's person, date and kind. They are extended to write, per file, the expected profile (title, type, date, roles, identifiers; for workbooks, sheets and headers) and, where realistic, documents whose metadata and header block are filled the way a word processor, a scanner and a hospital system fill them (including generic and misleading values).
- **Indexing metrics**, per field and per format: **precision of filled fields** (target to hold: a filled field is right; proposed 0.98 or more), **fill rate / recall** (reported, target set after the first measure), source distribution, and **zero silent loss** (an unreadable value is recorded as unknown with the reason).
- **Finder metrics**: file-level precision and recall on a query set covering every kind of door (word, phrase, identifier, person, role such as « courriers du Dr X », type, date range, folder, spreadsheet header or sheet name); **the six set sentences at 105 of 111 or more with zero wrong file**; the AVS-without-dots family from 1 of 17 to 17 of 17; latency at 500, 5 000 and 20 000 files.
- **File name** (section 5.3): 100 % of the listed name queries on every fixture file, one test per rule, including a file known only by its name.
- **Spreadsheets unchanged** (section 8): snapshot of the three tabular tables byte-identical and the `data` replay at the same 100 %.
- **Regression**: the 205-question exam and the big set replayed once before and once after the last step; no family worse; short documents' chunks byte-identical (golden); entity quality of the KB unchanged (`knowledge_validation`, I1 to I14).
- Each method sits behind a switch and is kept only if it helps and nothing regresses (inventory 0.6).

## 10. Decisions

### 10.1 Taken by the owner on 14 October 2026

| # | Decision | Taken |
| --- | --- | --- |
| D1 | Schema change: new tables for the file profile; the index is rebuilt by the next Analyse; an old index is detected, never rebuilt silently | **Yes** |
| D2 | Spreadsheets: option B, text values of cells searchable by word | **Yes**, with the rules of section 4.5 (names, organisations, places and identifiers included; numbers and dates excluded; cross-check with known entities) |
| D3 | Document-level roles `author`, `sender`, `recipient`, `subject`, `issuer` on existing entities, as precise and strategic as possible | **Yes**. Rule added: a role is recorded only with its evidence (header block, metadata, filename) and a confidence, and a doubtful one is a candidate, never a filter |
| D4 | A small search field inside each folder card, replacing the « utilisés » title line, whose result selects the files (the only ones the chat then considers); single unified field later | **Yes, to test** (section 6.1). Priority stays **indexing first**; the field is built on top of it |
| D5 | Types: domain (documents / data), then format, then business type; list validated; extensible | **Yes** (section 5.1) |
| D6 | Branch names `feat/file-profiles`, then `feat/file-finder`; the substance is an irreproachable indexing method fitted to the existing code, using entities, metadata, formats and types, and handling big documents (1 000 pages), with search then built on it | **Yes** (section 5.2 for big documents, then `feat/long-documents`) |

### 10.2 Questions Q1 to Q5, answered by the owner on 14 October 2026

| # | Question | Answer |
| --- | --- | --- |
| Q1 | Clearing the search field | **(a)** the selection stays; and the unselected files must stay visible, with a way to **reset** the filter (section 6.1) |
| Q2 | When to build the two fields | **(a)** after the finder, on the solid index; **concentrate on indexing now** |
| Q3 | Card layout (field in place of the title line, count and warnings kept under it) | **Yes**; design is not the heart of this work |
| Q4 | Starting business types for data files | **Yes**: patient or client list, invoicing, accounting, schedule, stock, other; to be adjusted in use |
| Q5 | Known-names pass on free-text columns | **Yes, with one absolute condition: break nothing in the spreadsheet indexing, inventory and deterministic engine** (sections 4.8 and 8) |

Added by the owner the same night: the **file name** must be a solid indexing criterion, including the people, organisations, companies and types found in it (section 5.3). **No question is open; the next move is the owner's go to start step 1.**

## 11. Order of work and estimates

### 11.1 How the first estimate (about 40 h) was made, and why it was wrong

I listed nine steps and gave each the duration a human developer would need, by judgement, then added them: 3 + 4 + 5 + 6 + 5 + 6 + 5 + 3 + 2 = 39 h. **Nothing was measured.** In particular: (1) I had not measured a build or a test run on this machine, so the figures hide no real timing; (2) each step silently included its own verification and a margin for debugging, which contradicts the rule of **one final pass**; (3) it contained work that was not requested (full text cleaning, a heavy truth generator); (4) a human's pace, not mine. **There was never an intention of hours of tests running**; the only genuinely long computing is one full test run, and the lab replay at the very end (a few minutes on the tower since its speed-up).

### 11.2 Rules that keep the compute short

- During a step, only the single test or module that proves the change (`cargo test --lib <name>`, `vitest <file>`); never the whole suite, never a measure repeated after a small edit.
- The full suite, `clippy`, `rustfmt --check` (my files only), `pnpm run build`, `pnpm run test` and the lab replay: **once per branch, at the end**.
- Before step 1, I measure **once** the real time of a build and of one targeted test on this machine and report it, so the following estimates rest on a figure. After each step I report the real elapsed time against the estimate.

### 11.3 Revised estimates (my working time, wall-clock, plus or minus 50 %, to be replaced by measured figures after step 1)

| # | Step | Branch | Est. |
| --- | --- | --- | --- |
| 1 | Measure one build and one test; **tests that prove the gaps G1 to G9 on today's code** (including the file-name cases); truth for the profile fields in the existing generators + a small scoring script; the snapshot and the `data` replay used as the spreadsheet reference | file-profiles | 1 h 30 |
| 2 | Metadata readers (PDF, DOCX, TXT/MD, workbook properties) + generic-value list as data | file-profiles | 1 h 30 |
| 3 | File name and folder (5.3), header block, business type (with user-defined types), document date, roles | file-profiles | 3 h 00 |
| 4 | Profile tables + FTS, join with the KB (metadata locator, roles), Analyse integration, the old-index test and the tabular-protection test | file-profiles | 3 h 00 |
| 5 | Index-side identifier forms; workbook cell values and known-names pass (4.5) | file-profiles | 2 h 00 |
| — | **Checkpoint: a written report on the profile quality on the fixtures. No interface. You decide whether to go on.** | | |
| 6 | Finder engine + contracts + facets + Tauri command + TypeScript type | file-finder | 2 h 30 |
| 7 | Question frame, identifier query forms, `SetSearch` route, chat sentence, catalogues | file-finder | 2 h 00 |
| 8 | P-4, P-7 (temperature 0, page figures in the checked numbers), P-2 (rank question) | file-finder | 1 h 30 |
| 9 | The two search fields in the folder cards (section 6.1) | file-finder | 2 h 00 |
| 10 | One final pass, one report, one human-test script (backup first, one restore at the end) | file-finder | 1 h 30 |
| | **Total** | | **about 21 h**, of which `feat/file-profiles` about 11 h |

`feat/long-documents` (structure, resumable two-speed ingestion, chunking v2, the 1 000-page test) is estimated at its start from the figures measured by then.

Not in this work: the redesign of the interface (left list, viewer, chat), which is the owner's to schedule; this work delivers the engine, its contracts and the two test fields.

Still deferred, as before: French stemming, `bm25()` weights, `NEAR`, the thesaurus, trigram, fusion, rerank, the embedding model choice, summaries; dates and amounts as facts; the new readers (ODT, RTF, EML, VTT, ODP, PPTX, ODS); full text cleaning (inventory 1.1) beyond what the identifier forms need.

### 11.4 Step 1: result (14 October 2026)

- **Measured on this machine:** a build of the library plus one targeted unit test, **20 s** (cached dependencies); a new integration test binary, **6 s**; the whole tabular reference set (`tests/tabular_reference.rs`, 3 tests, 106 cases: 104 + 2 passing, 0 known failures), **12 s** including its build. Estimates after this point rest on these figures.
- **Gaps proved on today's code** (`tests/file_profile_gaps.rs`, four tests): G9a the words of a file name are not searchable; G9b a name mixing an organisation and a person (`hug-martin-marie`) links nothing, while a name that is exactly the person (`martin-marie`) is linked (the control passes); G1 and G2 a document's author and title never reach the index and no mention has the `metadata` locator. Each `today_...` test is rewritten when its gap is closed.
- **Spreadsheet reference:** `tabular_snapshot` (tests/common) dumps `tabular_inventories`, `tabular_workbooks`, `kb_column_semantics`; two passes over unchanged files leave them identical. *Fact found:* `tabular_workbooks` (the cells at rest) is **cleared by every Analyse pass by design**, so the byte-identity proof compares the inventory and the column readings across a step, and the cell table is compared after the same pass rebuilds it. The reference for the deterministic engine is the 106 cases above.
- **Truth and scoring:** `scripts/file_profile_score.py` derives the profile truth from the sandbox's own `documents.json` and `workbooks.json` (kind, date, subjects, file-name mentions, sheet names), so no generator was extended, and produces **767 name queries over the 208 files** (full name 208, stem 206, without date prefix 132, two words reversed 64, word prefix 25, capitals 132) with the files each must return; it scores a profile dump on precision and fill rate per field. Nine script tests pass.
- **Not done in step 1, by design:** no product code. Format check (`rustfmt --check` on the two test files only) and the full suite wait for the end of the branch.

### 11.5 Steps 2 to 5 and the checkpoint: result (14 October 2026)

**Real elapsed time** (wall clock, 10:21 to 11:34, against the estimates of 11.3): step 2 metadata readers 9 min (est. 1 h 30); step 3 name, header, type, date, roles 20 min (est. 3 h 00); step 4 tables, full-text row, Analyse pass 12 min (est. 3 h 00); step 5 identifier forms, workbook words 11 min (est. 2 h 00); checkpoint measure and scorer 3 min (est. 0 h 30); the one final pass 18 min, of which 9 min 40 s for the whole `cargo test` (6 min 19 s of it compiling). The estimates were a human's pace; the figures above are measured.

**What exists** (all in `apps/desktop/src-tauri/src/file_profile/`, data in `resources/profile/`):

| Module | Does |
| --- | --- |
| `vocabulary.rs` | the profile's words as data per pack and language: generic property values, months, header labels and roles, business types, identifier layouts |
| `metadata.rs` | PDF Info and XMP (`lopdf`), DOCX and XLSX core and app properties (`zip`), Markdown and text front matter, JPEG capture date; generic values, file-name titles and impossible dates are dropped and counted, never filled |
| `name.rs` | the file name and folder as one function: words, exact keys, original name, date prefix, folder period, identifiers, links of every run of words to known entities |
| `header.rs`, `dates.rs`, `types.rs` | title, labelled lines, references, roles from labels, dates classified by where they stand, the chosen date only when the evidence agrees, the business type only with enough evidence |
| `identifiers.rs`, `cells.rs` | compact and grouped forms of every identifier; the words of a workbook's cells with the cap and `truncated`; the date range of date columns |
| `build.rs`, `store.rs` | layer 1 as a pure function; the tables, the full-text row and the pass that runs at the end of each Analyse |

**Measured on the sandbox** (`dump_the_stored_profiles_of_a_real_pass_for_the_scorer`, through the real Analyse, knowledge base on, no text recognition engine, scored by `scripts/file_profile_score.py` against the sandbox's own truth; the vocabulary was **calibrated on this same truth**, so these figures are an upper bound until she tries real documents):

- **Name queries**: 767 of 767 through `NameProfile::matches`; 171 files beyond the expected ones across all queries (the finder lists, it does not pick).
- **Business type**, 157 analysed documents: 156 filled, 156 right (precision 1.00, fill 0.994); first measure before calibration 0.73. By format: pdf 107 of 108, docx 24 of 24, jpg 10 of 10, png 5 of 5, txt 7 of 7, md 3 of 3. Formats the product does not read yet (odt, rtf, eml, vtt, pptx, odp, ppt) are not analysed and have no profile.
- **Document date**, 176 analysed documents: 172 filled, 172 right (precision 1.00, fill 0.977): pdf 109 of 110, docx 27 of 28, txt 15 of 17, jpg 11 of 11, png 5 of 5, md 5 of 5.
- **Subject role** from `Patient :`-type labels: 24 documents filled of 124 that are about someone (fill 0.19), 24 right. The other letters name the person without a label and are left empty on purpose.
- **Identifier family** (AVS in four spellings): the passages' index finds 1 of 4 documents for the dotless form; the profile finds 4 of 4 by the compact form and 4 of 4 by the grouped form.
- **Cost of the profile pass** (debug build): 1 897 ms for 176 documents (about 11 ms a file, 19 % of a 10 s pass whose embeddings are faked); 151 ms for 7 workbooks. A second Analyse over unchanged files builds and writes nothing and reads no workbook. The count `elapsedMs` is in the Analyse summary (`IndexSummary.profile`).
- **Spreadsheets**: `tabular_reference` 104 + 2 passing, 0 failing; `tabular_inventories`, `tabular_workbooks` and `kb_column_semantics` byte-identical across the profile pass, a cleared and rebuilt profile and a second Analyse; the `kb_*` tables identical with and without the profile pass.
- **Final pass**: `cargo test` 1 414 passed, 0 failed, 46 ignored; `clippy` with no warning in the new code; `rustfmt --check` clean on the new files and no new difference in the files already not clean on `main`; `pnpm run build` and `pnpm run test` (645 passed); Python script tests (41 passed).

**Where the implementation differs from the note above, and why** (to confirm):

1. **The profile's vocabulary is its own set of files**, not new fields of the knowledge packs: a new field would change the packs' fingerprint and make the next Analyse read every document again for names. Same pack ids, so a country is still its own pack. The `layouts` of the identifier schemes (4.5, 7) live there for the same reason.
2. **The language is not chosen by the interface locale**: a French practice with an English interface still files `facture`, so every shipped language is read; what a pack selects is its business types and header labels (only when the pack is active).
3. **PDF properties are read with `lopdf`** (already in the tree through `pdf-extract`), not `pdfium-render`: `pdfium` needs its dynamic library at run time and a laboratory has none. **Workbook properties** are read from the XLSX package (`zip`), not `calamine`, which does not expose them; the old binary `.xls` and `.csv` have none to read.
4. **A workbook's cells are read again from the file** when its profile is built or its links are recomputed, because `tabular_workbooks` is emptied by every Analyse of the Data Folder by design and is not rebuilt. The reader is the Analyse's own; nothing is written to the spreadsheet tables.
5. A `cells` column was added to the full-text row (the words of workbook cells), and a `file_profile_sections` table exists, empty, for the table of contents of long documents.
6. The old-index detection (`profile_index_status`, a registered command, and `index_status`) is tested on the backend; **the French message and its wiring in the interface are not written** (not part of this branch's scope).

### 11.6 Steps 6 to 10, the file finder: result (14 October 2026)

**Real elapsed time** (wall clock, about 11:50 to 13:15; approximate, the log was not kept to the minute): step 6 the engine about 35 min (of which three release builds for the latency), step 7 the set sentence and identifiers about 25 min, step 8 about 10 min, step 9 the two search fields about 20 min, step 10 the one final pass and the human test about 30 min, of which the whole `cargo test` took 10 min 41 s. The estimates of the table in 11.3 were a human's pace.

| Module | Does |
| --- | --- |
| `finder/contracts.rs`, `finder/engine.rs` | `find(connection, domain, inventory, query, scope)`: name and folder through the one name function (files never analysed included), the profile's full-text row one column at a time (so each hit says which column answered), the passages as a phrase, identifiers in their stored compact form; entity filter (knowledge base and profile) and role filter (profile only, a link without an entity never filters); type, date (workbooks by the span of their date columns), folder, format and state filters; facets and coverage; excerpts and cards for the page only |
| `finder/question_frame.rs`, `finder/set_search.rs` | the set sentence `[opener] [determiners] noun [linker] verb X`, vocabulary as data (`set_question` in `work-folder-questions/*.json`), `QuestionRoute::SetSearch`, `FolderAnswer::DocumentSearch`, `RetrievalPath::DocumentSearch`; the identifier forms of what was typed |
| `finder/rank.rs` | the rank sentence (`rank_question` in the same files), `FolderAnswer::RankedFile` |
| client | `findFiles` and the finder types in `ipc.ts`, `folderAnswer.ts` (both new answers), `FinderControls` and `useFinderSearch` (the field in both folder cards), `scopeFromSearch`, `useProfileStatus` and the line « L'index doit être refait : appuyez sur Analyser. », `businessType.*` labels in both catalogues |

**Measured.**

- **Name queries**: 767 of 767 through the finder's own entry (`find`), over the sandbox inventory with an empty index (`tests/file_finder.rs`).
- **The six set sentences of the final test**, 200-document sandbox, through the router and the finder: **111 of 111 files, 0 wrong file, 0 gateway call**. The first run lost 17 files because the noun « courriers » was a filter on the type `letter`; the word is no longer a type (`docs/DECISIONS.md`).
- **Identifiers**: the 37 distinct AVS numbers of the sandbox, typed without separators, found in all 48 files that write them, 0 wrong file; the lab's question about `7561234567897` finds `patients/2026-03-09_dossier-administratif-praz.docx` and the absent number finds nothing. (The 1 of 17 of the lab was an application-path figure; the pinned-first rule on the ordinary chat road is not built, see the decisions.)
- **Rank**: the latest courrier of the sandbox is `2026-10-07_courrier-harmonie.pdf` (13 and 77 readable courriers in the 30- and 200-document libraries), checked against the truth's own dates; `2026-10-01_courrier-dermatologie.pdf`, named in the handoff, is one of the later ones, not the last.
- **Latency**, synthetic index with three passages and two entity links per document, release build, median of 15, warm connection (a fresh connection per call, as the command opens it, costs 141 ms at 20 000 files for the same listing): 500 files 1 to 6 ms; **5 000 files 8 to 35 ms (target 50)**; **20 000 files 37 to 148 ms (target 200)**, the worst being a word present in 69 % of the passages. Before the 64 MiB page cache the same 20 000 files cost 250 to 420 ms.
- **Spreadsheets**: `tabular_snapshot` byte-identical around the finder; `src/tabular/*` and the three tables untouched (the only edit in the neighbourhood is the temperature of the query-plan call in `tabular_answer.rs`); `tabular_reference` passes.
- **Final pass**: `cargo test --no-fail-fast` 1 436 passed, 1 failed, 48 ignored; the failure was a test written for the previous routing (`Quels documents concernent HUG ?` is now a word search), changed on purpose and re-run green with the three files that exercise the router; one other test (`a_refusal_of_the_request_itself_is_never_retried`) failed once under load in the first run and passed three times alone and in the second full run. `clippy` with no warning in the new code (test files carry one `allow` for building a query field by field); `rustfmt --check` clean on the new files and no new difference in the files already unclean; `pnpm run build` and `pnpm run test` (673 passed); Python script tests (41 passed).

**Where the implementation differs from the handoff, and why** (to confirm):

1. **The pinned-first exact identifier on the ordinary chat road (`LexicalOptions`) is not built.** It is judged on the `identifier` family of the real stack, which needs the AI machine's embeddings; the finder already answers « Quels documents contiennent <numéro> ? » and the search field finds a number in any spelling.
2. **The rank sentence reads the inventory, not the profile**: the date of the clean name, else the modification date, with the basis said in the answer. The profile's chosen document date is not used yet.
3. **Entities as X are not resolved**: an unresolved name stays a phrase (the design's small port over the KB resolver is left for a later lot); a person's name is found by the passages and the profile's entity column.
4. **The set sentence works on the documents of the conversation only**; with documents and tables both selected it falls through to the mixed tier as before.
5. **The folder cards' field replaces nothing**: it sits above the « Documents utilisés » line, which keeps its count and warnings, as the least risky layout; the owner decides whether the line should go.

### 11.7 Stabilising the finder (tab 1 of the long-documents chain): result (14 October 2026)

**Elapsed time**: not logged to the minute; the whole tab, from the three decisions to the final pass, was one working session. The human test of `docs/test-reports/file-finder/human-test.md` was **set aside by the owner** and has not been played: **tab 2 must not start before it is** (handoff rule), or the owner lifts that rule herself.

| Item | Built | Where |
| --- | --- | --- |
| 1. Decisions | Confirmed: the field above « Documents utilisés », `courrier` a path word, the rank sentence read from the profile | `docs/DECISIONS.md`, « Settled while stabilising the file finder » |
| 3. Entities as X | `finder::names::NameResolver` (port) and `KnowledgeNames`; `set_search::answer_with_names`; `DocumentSearch.entity`; the answer names the entity | `tests/finder_names.rs` (3) |
| 4. Rank from the profile | `ProfileFacts` through `CorpusWords::profiles()`; date with its source, business-type filter, path-word fallback, `RankedFile.by_type` | `tests/finder_rank_profile.rs` (4) |
| 5. Set sentence, documents and tables | `set_search::answer_in`, `answer_both`, `FolderAnswer::DocumentSearchBoth`, `DocumentSearch.domain`; the mixed tier answers before the model; `tabular_answer::selected_workbook_paths` (read only, added) | `tests/set_search_mixed.rs` (1) |
| 6. User-defined types | `file_profile/user_types.rs`, table `file_profile_user_types`, `ProfileVocabulary::with_user_types`, `types::classify_among` and `combine`, `reevaluate` (stored profile only) | `tests/file_profile_user_types.rs` (5), `types.rs` unit test |
| 7. Own names | `Settings.own_names`, `KnowledgeContext::with_own_names`, the names enter the vocabulary hash | `tests/file_profile_own_names.rs` (1), settings unit test |
| 8. Unstable test | Immediate replies no longer run under the 250 ms deadline | `tests/embedding_failures.rs` |

**Honest notes.**

- The tests of items 3, 4 and 6 were written beside the code and run when both existed; the one that would have failed on the old behaviour is the one that asserts the old behaviour first (`another_name_of_the_entity_finds_the_files_that_write_the_first_one` asserts the plain word search finds one file, then three with the name). The rank test that first failed was wrong about the product (an unanalysed file is `not_assessed`, not ranked): the product was right and the test changed.
- The profile's FTS `entities` column already holds the **canonical name** of every linked entity, so a typed canonical name already finds its files by words. The port adds the case the words cannot reach: **another name of the entity** (a manual alias) and the entity's files whatever they write.
- Re-evaluating the user types cannot see the body of the text (the profile does not keep it): the following Analyse, which builds every profile again because the vocabulary hash changed, completes the reading. A test pins that the quick pass settles no type the full pass then changes.
- The rank sentence over **workbooks** is not part of this tab; the set sentence over workbooks is.

**Measured (final pass, after the last edit).**

- `cargo test --no-fail-fast`: **1 454 passed, 0 failed, 48 ignored**, including `tabular_snapshot` byte-identical around the finder, `tabular_reference`, and the unstable test.
- The six set sentences, through the application's path (names port on): **111 of 111, 0 wrong file, 0 gateway call**; the 37 AVS numbers in all 48 files, 0 wrong file; 767 of 767 name queries (`tests/file_finder.rs`).
- Finder latency (`tests/file_finder_bench.rs`, release, once): 500 files 1 to 6 ms, **5 000 files up to 31 ms (target 50)**, **20 000 files up to 128 ms (target 200)**; the run that passed is the second, because the first was cut by a pipe (it showed the same order of figures). The names port adds one resolver call and at most seven finder runs to a set sentence whose X is one entity; no change for an ordinary X.
- `clippy --all-targets`: no warning in the new or changed code (the existing warnings of other files are untouched); `rustfmt --edition 2021 --check`: the new files are clean and no file already unclean has a new difference; `pnpm run build`; `pnpm run test` **678 passed**; the Python script tests **41 passed**.

**Own names, to keep.** `Settings.own_names` is kept for the multi-workstation use with an authentication on the server (owner, 14 October 2026): the authenticated user's name will feed it, and the next step is to also keep that name out of the entities and out of the search (`docs/DECISIONS.md`, « Settled while stabilising the file finder »).

**Left for later tabs.** The editing screen and the Tauri commands for the user types, a settings field in the dialog for the own names, the labels of `user_*` codes in the facets (tab 3); the finder on long documents (tab 2); the pinned-first identifier on the ordinary chat road (tab 3).
