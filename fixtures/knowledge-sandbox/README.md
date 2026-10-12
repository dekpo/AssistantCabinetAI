# knowledge-sandbox

A **fictional library with a ground truth**, to measure the Knowledge Base (KB lot 11) and, later, to demonstrate
the product. Written for a developer or an agent. English; the documents themselves are French, as a practice of
Haute-Savoie and French-speaking Switzerland writes them.

**Everything is invented.** No person, number, address or letter is real. The social security numbers and the AVS
numbers carry a valid control key (so a pattern or a checksum accepts them) and belong to nobody; the telephone
numbers are the ranges reserved for fiction (`01 99 00 xx xx`, `06 39 98 xx xx`) or the conventional exchange 555;
the e-mail addresses end in `.example`. The organisations are real public bodies and companies **named as
organisations** (HUG, CHUV, CPAM, Helsana, CSS, Unilabs, SUVA...), which is what a real library holds, and invented
suppliers (`MedSupply SA`, `Papeterie du Lac SARL`...). No real person is named. Never put a real document here.

```text
documents/   the Documents folder: what the application is pointed at (clean ASCII names, sub-folders)
data/        the Data folder: workbooks (xlsx, csv in CP1252, and the same tables as ods and xls)
entities.json    every invented person, organisation and identifier, with every shape it is written in
documents.json   every document: format, tier, and EVERYTHING IT NAMES (the exact string, the entity, the shape)
workbooks.json   every workbook: sheets, and where each entity is (sheet, column, rows)
questions.json   the questions, the files that hold each answer, and what the base is expected to do
manifest.json    parameters, counts per format, tiers
```

Regenerate (deterministic: the same seed gives the same bytes, checked by running it twice):

```text
python scripts/gen-knowledge-sandbox.py                                   # this folder: 200 documents
python scripts/gen-knowledge-sandbox.py --documents 1000 --extra-rows 40 --out data/knowledge-sandbox-1000
```

`data/` is not versioned. Standard library only, plus Pillow for the scanned pages (as `scripts/gen-sandbox-fixtures.py`).

## Tiers

The library is one stream of documents. **A tier is a prefix**: the first 30 documents are the library of 30, the first
100 the library of 100, and so on (30, 100, 200, 500, 1 000). The 30 are written by hand to hold every trap; the next
ones cover every designed name; the rest are drawn from a long tail of 490 people. So a result at 30 documents is
comparable with the same question at 200.

## What is in it

- **Names.** Two families that look alike (`Jean Dupont`, `Jean Dupontet`, `Jean-Pierre Dupont`, `Marie Dupont` and a
  cardiologist of Geneva who is also called `Jean Dupont`; `Dr Paul Martin`, `Dr Pierre Martin`, `Marie Martin`,
  `Claire Martin`), sound-alike pairs (`Favre`/`Fabre`, `Monnier`/`Meunier`, `Girard`/`Girardin`), given name and
  surname swapped (`Lucas Roy`/`Roy Lucas`, `Thomas Pierre`/`Pierre Thomas`), names of French-speaking Switzerland
  (Rochat, Cuendet, Pittet, Chappuis, Bovet, Délèze, Crettenand, Vuilleumier, Python, Oberson, Genoud, Tschopp...),
  of Switzerland and abroad (Müller, Bernasconi, Benali, El Khoury, Nguyen, Kowalski, Dos Santos Ferreira, Demirci,
  Petrović, D'Amico, N'Diaye, O'Brien, Lindqvist), particles and apostrophes (de la Fontaine, d'Aubigné, Le Gall), and
  the Swiss title `Dre`. The owner of the practice (`Dr Hélène Marchetti`) is in every letterhead (a name that must
  never narrow a search).
- **Shapes of a name**, each recorded as such: `full`, `title_full`, `title_surname`, `initial_surname` (`J. Dupont`),
  `reordered` (`DUPONT Jean`), `upper`, `no_accent`, `surname_initial` (`Dupont J`), `surname_only`, `hyphen_error`
  (`Jean-Dupont`), and the weak ones a person reads from context and a rule cannot (`M. Dupont` alone).
- **Organisations** with and without a marker word, with a legal suffix, as an acronym (`HUG`), carrying a surname
  of the library (`Pharmacie Dupont`, `Dupont Immobilier SA`, `Garage Martin & Fils SARL`).
- **Identifiers**: social security numbers, AVS numbers (`756.xxxx.xxxx.xx`), IBAN (FR and CH), invoice numbers
  (`FAC-2026-014`, which **two suppliers use on purpose**, as real libraries do), contract and policy numbers, case numbers,
  telephone numbers (the ranges reserved for fiction, or the exchange 555) and e-mail addresses (`.example`).
- **Places**: communes and postal codes of Haute-Savoie, Ain, Rhône and of Vaud, Genève, Fribourg, Valais, Neuchâtel,
  Jura (Lausanne, Nyon, Morges, Vevey, Yverdon-les-Bains, Annemasse, Thonon-les-Bains...).
- **Formats**: PDF with a text layer, scanned PDF, JPEG, PNG (for the OCR route; the pages are given a skew, a blur
  and speckle so that an engine misreads a letter now and then), DOCX, TXT, MD, and the formats the product does not
  read today: ODT, ODS, ODP, RTF, EML, VTT (a meeting transcript), PPTX, **XLS** (written by `legacy.py`, a real
  BIFF8 file that a spreadsheet reader opens) and **PPT** (a stand-in: a compound file with the words, **not** a real
  PowerPoint, the standard library has no writer for it). Long documents of 116, 198 and 290 pages and DOCX of 110 to
  180 logical pages, each with sentences planted on a known page and a question that has them as its answer.
- **Workbooks**: `Patient | Médecin traitant | Date | Montant` over two sheets, `Nom | Prénom | N° AVS` (split
  columns), a CP1252 CSV with `nom_patient | medecin_traitant | NIR`, supplier invoices, contracts, a duty roster
  with formulas, `N/A` and empty cells, a sheet of free notes and a column called `Nom` that holds zone codes (both
  must be rejected). Patients are written in several forms (`Jean Dupont`, `Dupont Jean`, `Dupont J`).
- **Spreadsheets saved among the documents** (`administratif/commandes/`: two XLSX and a CP1252 CSV): a real library has them in the
  wrong folder, and the interface says « tableur, à placer dans le dossier des données ».
- **Mail**: an e-mail saved as `.eml` and the same kind of message saved as a text, whose header lines then belong to
  the text.

## The ground truth and what it is used for

`documents.json` records, for every mention in every document, the **exact string written**, the entity, the shape
and where (`text` or `filename`): the generator writes the text through functions that record what they write, so
nothing is read back or judged by eye. `questions.json` gives, per question, the entities it is really about, the
documents and workbooks that hold the answer, the selection it is asked in, what the base is expected to do
(`resolved`, `homonym`, `ambiguous`, `possible`, `none`, `named_file`, `folder`, `tolerant`), and, for some, a fact
the answer must contain. The harness is `apps/desktop/src-tauri/tests/knowledge_validation*.rs` and
`tests/common/validation.rs`.

## Not covered, on purpose

Layout-aware OCR of tables, handwriting, real PowerPoint binaries, voice. A real patient file never.
