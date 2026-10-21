# The Knowledge Base

The long-lived description of the local, source-grounded Knowledge Base (KB) of the desktop client: what it is, what
may feed it, how a question uses it, what it costs, what it cannot do. Written at the end of the KB programme (lots 0
to 11, 8 to 12 October 2026) for a developer or an agent who was not in the sessions. The step-by-step history is
`docs/test-reports/knowledge-base-pass-1/`; the decisions are in `docs/DECISIONS.md` ("Settled by KB lot N"); the
numbers measured by lot 11 are in `docs/test-reports/knowledge-base-pass-1/04-final-report.md`. This file does not
repeat them: it says how the thing is built and where each rule lives.

## 1. What it is, and what it is not

A **directory of the entities found in the files the user has analysed** (people, organisations, places,
identifiers, items, terms), with their other spellings, the places they were found, the sound of their names and the
role they play, kept in the same SQLite file as the index (`index.sqlite3`, tables `kb_*`), on the workstation, and
nowhere else.

```text
AnalysisScope  = user authority (what may be looked at)
Knowledge      = candidate discovery and shared vocabulary INSIDE that authority
Retrieval      = which passages are relevant            (retrieval.rs, unchanged engine)
DATA engine    = deterministic structured truth          (tabular::engine, unchanged authority)
LLM            = interpretation of supplied evidence only
```

It is **not** a replacement for retrieval, not an authority over a number, not a memory of conversations, and not a
knowledge of the world: it knows only what is written in the user's own analysed files and what the user typed in
the management dialog.

Two jobs:

1. **Candidate discovery inside the selection**: from the names a question holds, the files of the selection that
   mention them, so that a search can look at fewer files (`knowledge::{query, reduce, targeting}`).
2. **The shared vocabulary of the product**: entity autocomplete (`@`), question suggestions, the sheet of a name,
   transcript repair and spoken forms for a future speech feature, an outbound-text guard for a future connector
   (`knowledge::{picker, suggest, sheet, surface, repair, guard}`). These are read-only services over the same
   tables.

## 2. Three modes

| Mode | Names are read at Analyse | A question | The user sees |
| --- | --- | --- | --- |
| `off` | no | no lookup, the search is the one that has always run | nothing (the product as it was before the KB) |
| `suggest` (default) | yes | the base works out what it *would* have searched and records the overlap with the sources actually cited (counts only); **the search is never narrowed by the base itself** | entity picker, suggestions, ambiguity chips, the management dialog; a name the user picks as a chip narrows the search at her request |
| `auto` | yes | the search covers only the files the base ties to the names of the question, when the rules allow | a line under the answer (« Recherche ciblée : N sur M documents — nom »), the files used, and **Élargir à ma sélection**, which asks again on the whole selection |

The setting is « Ciblage intelligent » in « Réglages » (group « Connaissances »). Nothing heavy runs when it is
changed: names are read **only inside an Analyse the user started** (`AGENTS.md`, "No costly silent background
work").

## 3. What may feed it (the persistence rules)

Rows come **only** from (a) analysed documents, (b) analysed workbooks, (c) explicit manual edits. **Never** from
a question, an answer, the conversation history, a transcript, a retrieval result or any model output
(invariant I3; `tests/knowledge_isolation.rs` compares a snapshot of every `kb_` table before and after a question).

- **Documents** (`knowledge::extract::text`, `knowledge::ingest`): titled names (`Dr`, `Dre`, `Docteur`, `Pr`,
  `Madame`...), full names matching the gazetteer of names already known, organisation markers and legal suffixes,
  identifier schemes from the packs (invoice, order, e-mail, phone, IBAN, social security and AVS numbers), names
  in file names. An untitled capitalised run is a **candidate**, never active, until a second source or a column
  supports it. Extraction reads the text the index already holds, at the end of an Analyse.
- **Workbooks** (`knowledge::extract::table`): the headers of the columns are read as a whole against the
  vocabulary of the packs (`patient`, `fournisseur`, `n° AVS`, `Nom` + `Prénom`...); a column that is not read
  is recorded with the reason. A table's entities are active at once (the column is the evidence). Mentions
  point at a sheet, a column and up to three sample rows, never at a cell's text.
- **Personal identifiers** (social security, AVS, IBAN, e-mail) are stored as a **keyed hash**; the key is a file
  beside the index and never inside it (`knowledge::secret`, `docs/PRIVACY-AND-SECURITY.md`).
- **Manual edits** (`knowledge::manage`): create, rename, add a name, merge (a redirect, reversible), split, delete
  (a tombstone, so the next Analyse does not recreate what the user removed), restore, reset. They are the only
  rows that survive a re-analysis of their source.

Idempotence (I14): a second Analyse over unchanged files writes nothing. A document is read again when its content
changed, when the extractor is newer (`CURRENT_KB_VERSION`) or when the vocabulary changed.

## 4. Entities, names and provenance

```text
kb_sources      one row per analysed file (domain 'documents' | 'data', relative path, content id = SHA-256)
kb_entities     type, canonical name, status candidate|active|merged|deleted, origin automatic|manual
kb_aliases      every spelling (canonical variant, reordered, initial, partial surname, title form, filename form,
                manual) with its phonetic key
kb_mentions     where it was found: chunk, cells (sheet, column, rows), filename or metadata; role; method; confidence
kb_relations    only from an explicit schema (two role columns of one table), never from co-occurrence
kb_attributes   spoken form, hashed identifiers, e-mail, phone (open vocabulary)
kb_possible_matches   pairs that sound alike or share an initial: a question to the user, never a link
kb_diagnostics  one row per question, **counts and codes only**, a ring of 2 000
```

The same relative path can exist in both folders: a source is keyed by `(domain, relative_path)` and mapped to the
selection by `(domain, relative_path, content_id == pinned id)`, so a same-named file of another folder never
matches a stale mention (I6).

**Types are neutral and never changed by a module**: a person stays a person and an organisation stays an
organisation whatever packs are active. Packs (`health-fr`, `health-ch`, `legal-fr`, `accounting-fr`, named
`<domain>-<country>`) are cumulative and add only vocabulary, identifier patterns and column names
(`docs/DECISIONS.md`, "Knowledge packs are named domain-country").

**Confidence by method** (stored per mention): identifier 1.00, column value 0.95, gazetteer hit 0.90, title and
full name 0.85, organisation suffix 0.80, an organisation a pack lists by name 0.80, a name alone under a
salutation 0.60, title and surname 0.70, name in a file name 0.50, untitled capitalised run 0.40 (a candidate). A
reduction needs 0.80 or more, an entity that is not a candidate, and a share of the selection under the ceiling.
The method kept for a mention is the strongest evidence of that place: a name the base already knew and that a
title also found is recorded as found by the title (`Dr Jean Dupont`), not as a hit on the list.

**What the extractor reads** (`knowledge::extract::text`, version 3 of the knowledge base): a title and a name; an
organisation by a legal form after it (`Garage Martin & Fils SARL`, an ampersand joins two halves), by a word that
opens it (`Association`, `Hopitaux`, `Etude`, with lower case words allowed before the capitalised name) or by its
**name in a pack** (`org_names`: `HUG`, `CHUV`, `SUVA`, `URSSAF`... written as the pack writes the acronym, the
entity carries the full name); an untitled run of two or three capitalised words in a sentence; a name **alone on
a line** in the block that opens a letter (a salutation above it, or an address below); a name split by the
margin of a wrapped paragraph (a PDF: only in a text whose lines stop at the margin); a particle or two inside a
name (`de la`, `van den`); an apostrophe inside a surname (`D'Amico`, `N'Diaye`, `O'Brien`: a single capital
letter, `D`, `N` or `O`, an apostrophe and a capital). It never reads a street or a place as a person (`rue
Jean Jaures`, `place de l'Hotel de Ville`) and never a heading or an acronym nobody listed.

## 5. Resolution: from a name to an entity

`DeterministicResolver` (`knowledge::resolve`), strongest signal first: exact identifier, exact canonical name,
exact alias, strong pattern (title and full name; organisation suffix and name), column and exact value, then
**safe fuzzy** (same sound and one edit on words of four letters or more) which is **never a merge**: it is a
`PossibleMatch`, a question for the user. Outcomes: `NewEntity`, `Existing`, `PossibleMatch`, `Ambiguous`,
`Unresolved`. Ambiguity is judged **inside the selection** first (a name that is ambiguous only globally is not
shown as ambiguous, which respects I7). Initial and surname (`J. Dupont`) links only when the surname is unique in
the source. Surname then initial (`Dupont J`, the form of a table) is a possible match of the full name, in both
orders. Whatever the method, a name with no support is a candidate and never narrows anything.

**Two orders of the same words.** `DUPONT Jean` (a surname in capitals first) is read given name first and is
`Jean Dupont`. `Roy Lucas` in plain case, when `Lucas Roy` is known from a source of the **same folder**, is **not** the same
person on that evidence: at ingestion it is a new entity with a possible match of reason `swapped_order`, in a
question it is a "did you mean". Across folders (a table and a letter) the two orders are one person. A possible match of that reason is never kept in a reduction (two people are not searched as one).

**An entity promoted by a second source goes back to a candidate** when fewer than two sources name it (a file
deleted, changed, or read again without the name). An entity a pattern made active at once (a title and a full
name, a legal form, a listed organisation, a column) is never demoted, and neither is anything the user typed.

## 6. How a question uses it

The step sits between the folder router and the question's single embedding call
(`docs/RETRIEVAL.md`, "Targeting inside the selection"). In order:

1. **Router first**: a question that names a file, asks for a count of the folder, or asks about "each document"
   is handled before the base is consulted; a **named file is never reduced** (I2).
2. **Read the question** (`QueryEntityDetector`): runs of one to five words are looked up in one query, then
   resolved in query mode inside the selection; chips the user picked apply at any size; the previous answer's
   entities are carried over for a short follow-up (8 words or fewer: the product's existing follow-up rule).
3. **Decide** (`reduce`): a side is reduced only if the selection holds at least `knowledge_min_scope_files` files
   (8), an entity is resolved at 0.80 or more, is not a candidate, is present in no more than
   `knowledge_max_selectivity` of the selection (0.6: the user's own name is ignored), nothing is ambiguous, and at
   least `knowledge_min_reduction` of the selection (0.3) is dropped. Candidates are capped at
   `knowledge_max_candidates` (15) for documents; workbooks are not capped.
4. **Safety net**: a file the base has never read is kept; the files of an entity's open possible matches are kept;
   the files whose text holds another form of the name are added (the lexical net, held to the selection), except
   a surname alone that other entities of the selection carry.
5. **Search**: the reduced set only, then once on the whole selection if nothing came back or the closest passage
   by meaning is under `knowledge_widen_below_score` (0.65, measured on `nomic-embed-text`, and unable to tell a
   reduced set that names the right person but lacks the answer from one that holds it: KBD-17).
6. **Any failure is the old path** (I4): zero candidates, ambiguity, a low confidence, a lookup error, a locked
   database: the full selection, as if the base were off.

Documents and workbooks are decided independently. The base never writes a sheet restriction and never touches the
DATA engine's result (I5): it may narrow the list of workbooks the engine is given, built from the user's own
selection, and annotate the buttons of « Quel classeur ? ».

## 7. In the window

A targeting line under an answer, the **Élargir à ma sélection** button, the `@` list and the chips above the box,
the small **i** that opens the sheet of a name, the block « Suggestions », the **Connaissances** management dialog
(entities, to verify, types and roles, reset). Design and rules: `docs/CLIENT.md`, `docs/DECISIONS.md` "Settled by
KB lot 7, 8, 9". Nothing in the conversation opens the dialog; a name is never shown for a file outside the
selection except in the one labelled « add these files to my selection » offer (I7).

### The filters of the explorer (explorer step 7)

The explorer filter panel narrows the file lists by what the base knows: **person**, **organisation** (with a role as a second
step: author, subject, recipient, sender, issuer, mentioned), **business type** and **year**. The data is one read per folder
(`file_knowledge_facts`, `finder/facts.rs`): for each analysed file its profile business type, the years of its document date, of
the date its name starts with and (workbooks) of its date columns, and its people and organisations from **both** the profile
(roles with evidence) and the base mentions (`mention`, or the role the base gave: `client`, `provider`, `supplier`). A merged entity
answers as the one it was merged into; a deleted one links nothing; candidates count as names. Only the types `person` and
`organization` are listed: **a person stays a person and an organisation an organisation** whatever packs are active, and no pack
excludes or transforms a type. Nothing here changes how entities are extracted. Counts are worked out client-side (cost measured
20 October 2026 on a copy of the owner analysed index: about 55 ms in a debug build for 1,246 documents, 325 KB of JSON). On that
index the most frequent person is the practice own doctor (700 of 1,246 files, all as « mention »): the list is by frequency, as
specified, so such a name tops it; a name in almost every file narrows nothing (the same reason `knowledge_max_selectivity` ignores it
for the targeting of a question).

## 8. Privacy and boundaries

- Local only, per workstation, in the index file; nothing is synchronised and nothing is sent to the AI, the server,
  Open WebUI or a log as text. Diagnostics and the timing log hold ids, counts, codes and milliseconds
  (`docs/PRIVACY-AND-SECURITY.md`, "The knowledge base: a directory of names in the local index").
- The KB adds **no model-facing string**; types, roles and names are never interpolated into a prompt (I8, I9).
- Rust returns machine codes; every sentence is an interface catalogue key (I10). No `cfg` in the KB (I13).
- Resetting: the Documents Reset removes the documents' sources and their automatic rows, the Data Reset the
  workbooks', neither removes manual rows; « Réinitialiser » in the dialog, confirmed, removes everything
  (`docs/PRIVACY-AND-SECURITY.md`).
- No destructive action on a file, ever (I11): deleting or merging an entity touches KB rows only.

## 9. Performance, and where the time goes

Measured by lot 0 and lot 11 (`docs/test-reports/knowledge-base-pass-1/00-baseline.md` and `04-final-report.md`):
at question time the only embedding call is the question's, the vectors of the chunks are computed at Analyse, and
the prompt is capped (6 passages, 6 000 characters), so **the time the user waits is the model's, and the base does
not change it**. What the base changes is the number of files scored (a few milliseconds on a few hundred files) and,
above all, **what the search looks at**. The lookup itself is milliseconds. Reading names costs a share of an
Analyse. The numbers are in the final report; the rule for any figure here is that it states the machine, the
library and the method.

## 10. Testing strategy

| Layer | Where | What |
| --- | --- | --- |
| Unit | `src/knowledge/**` | normalisation, phonetics, resolver outcomes, extractors, packs, reducer rules |
| Store | `tests/knowledge_store.rs` | migrations from the pre-KB schema, FK enforcement, cascade, GC, idempotence, rollbacks, two connections |
| Invariants I1 to I14 | `tests/knowledge_targeting.rs`, `knowledge_isolation.rs`, ... | each by a named test (the table is in the lot reports) |
| Integration | `tests/knowledge_*.rs` | analyse then ask in `suggest` and `auto` over invented folders |
| Validation (lot 11) | `tests/knowledge_validation*.rs`, `fixtures/knowledge-sandbox` | a generated library with ground truth: entity quality, recall, false merges, leaks, timings at 30 to 1 000 files, OCR and the real stack |
| Interface | `apps/desktop/src/**/*.test.ts` | targeting line, chips, picker, summaries, catalogue parity |
| Manual | `docs/test-reports/knowledge-base-pass-1/human-tests/` | scripts the owner follows in the real application |

`cargo test` runs every target; **CI runs only `cargo test --lib`** (`.github/workflows/ci.yml`), so the integration
tests of the KB do not run there (decision pending: the lot 11 report asks the owner to extend the step).

The validation sandbox is generated, deterministic, fictional and versioned
(`fixtures/knowledge-sandbox/README.md`): names of France and French-speaking Switzerland, social security and AVS
numbers with valid keys, CPAM and LAMal letters, scans, 100-page documents, and formats the product does not read.

## 11. Future contracts the KB offers and does not build

`surface::surface_forms` and `repair::propose` for speech (an engine-independent text-in, proposals-out contract;
a dictation never writes to the KB), `kb_attributes.spoken_form` for text to speech, `guard::scan` for outbound
connector text, `query::resolve_text` for a calendar, `store::move_source` for a file organiser. None needs a
migration. The calendar, connectors and speech remain separate owner decisions.

## 12. Known limitations

The complete, numbered list is the defects register
(`docs/test-reports/knowledge-base-pass-1/defects-register.md`) and the fix battery of the final report. The
shape of them:

- **Names a human reads from context and no rule can link**: a letter that says only `M. Dupont` in a library with
  several Dupont, a surname alone, a title and a surname when two doctors share it. The base asks or leaves the
  search whole; it cannot read them.
- **A surname-first convention indistinguishable from a swapped pair**: fixed in fix 4, batch 2 (KBD-25). Only a
  surname in capitals says "surname first"; a swapped pair in plain case is two people with a possible match when the
  two orders come from the same folder (two tables, two letters); a table and a letter that write the two orders
  are one person.
- **Organisations no pack lists and that carry no marker word**: `Cerballiance` is found because the pack lists it;
  an unlisted clinic written `Les Cerisiers` is not. An acronym nobody listed is never read (`IRM`, `TSH`, `PDF`
  would be thousands of false organisations). Listed organisations live in the packs (`org_names`), per country.
- **A name with a lower case `d'`** (`Camille d'Aubigne` in a sentence) keeps its article elided, while the same
  name written `D'Aubigne` in a table is one word: the two meet only when a table or a letter writes both the same
  way.
- **Titles of Switzerland**: fixed in fix 4, batch 1 (KBD-26): `Dre`, `Dresse`, `Docteure`, `Professeure` are titles
  of the base, `Pre` is a weak one (it is also the word `pre`, so it needs a capital and a capitalised word after it).
- **A short question that names nobody** is read as the follow-up of the previous answer (KBD-19).
- **Formats the product does not read**: ODT, ODP, PPT, PPTX, RTF, EML, VTT (documents) and ODS (tables). The
  files are listed and never searched; since fix 4, batch 1, the Analyse summary of the documents says how many
  files were not read and in which formats (KBD-30; the extractors themselves are a later batch).
- **The widening score** cannot catch a reduced set that names the right person but lacks the answer (KBD-17).
- The base is per workstation: two machines have two bases.
