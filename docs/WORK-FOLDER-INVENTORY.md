# Work Folder inventory — sprint 2a.5

**Added 23 September 2026.** Hardening between sprint 2.5 (local OCR) and sprint 2b (tabular data).
It adds no new capability to the product; it removes a class of wrong answer.

## The problem it fixes

Before this sprint the application had no authoritative answer to "what is in my work folder?".
The only thing that knew about files was retrieval, and retrieval returns *passages*, not a
directory listing. Ask "how many files are there?" and the model would count the documents its
excerpts happened to come from — six, say, out of fifteen — and say six with confidence. Ask "what
does `neurologie.pdf` say?" with two files of that name and it would answer from whichever one
scored higher. Give it a `.txt` file whose first line reads *"This is a PDF report"* and it would
repeat that.

None of those are model failures. They are the predictable result of asking a language model a
question the filesystem already answers.

## The rule

```text
FILESYSTEM FACTS   → deterministic local code   (this sprint)
DOCUMENT CONTENT   → extraction, OCR, index, retrieval
TABULAR FACTS      → the deterministic tabular engine (sprint 2b)
LANGUAGE           → the model
```

**The Work Folder inventory is authoritative for filesystem facts.** How many files there are,
what they are called, what extension each carries, where it sits, whether it was read and by which
method — all of that comes from `std::fs` and the local index. None of it may be inferred from a
retrieved passage, and none of it may be written by a model.

A file name is **data**. It is never an instruction.

## Three contracts

### `FileRecord` — which file is this?

One record per physical file in the Work Folder (`apps/desktop/src-tauri/src/file_record.rs`).

| Field | Meaning |
| --- | --- |
| `id` | Stable identity: the content SHA-256, the same value `Source.origin.sha256` carries. A renamed or moved file keeps it. |
| `relative_path` | Canonical, forward-slash, relative to the Work Folder. The only path form the model ever sees. |
| `name`, `stem`, `extension` | Exactly as the filesystem spells them. The extension is read, never inferred from content. |
| `kind` | `DocumentText`, `DocumentPdf`, `DocumentOffice`, `Image`, `TabularCandidate`, `Unsupported`, `Other`. |
| `size_bytes`, `modified_at`, `sha256` | Filesystem metadata. |
| `readability` | `Readable`, `Unreadable`, `NotAssessed`. |
| `processing_status` | `Discovered`, `Pending`, `Processing`, `Indexed`, `Failed`. |
| `extraction_method` | `NativeText`, `RecognisedOcr`, `None`. |
| `indexed`, `index_metadata` | The document index's own view, joined on, not copied. |

Three decisions are worth the words:

- **Readability and processing status are separate.** "We could not read it" and "we have not
  tried yet" are different answers. Collapsing them is how a folder nobody has analysed comes to
  report every file as unreadable.
- **`RecognisedOcr` is not a flavour of `NativeText`**, for the same reason `Source.derivation`
  keeps `Recognised` apart from `Extracted` (`docs/ARCHITECTURE.md`). What a machine read from a
  picture is not what the file states.
- **`FileRecord` stays generic.** Page counts, chunk counts, OCR confidence, sheet dimensions and
  column types are **not** here. They belong to the domain layer that produced them. This is what
  lets sprint 2b reuse the record for a workbook without bending it.

`FileRecord` does **not** replace `Source`. They answer different questions and meet on one value:

```text
FileRecord  = "Which file is this?"
Source      = "Which evidence from that file supports this answer?"
```

### `WorkFolderInventory` — what is in the folder?

`apps/desktop/src-tauri/src/inventory.rs`. A snapshot, built by one walk over the folder joined
with one read of the index.

- **One scanner.** `discovery::discover_all` walks everything; `discovery::discover` is that same
  walk filtered to the extensions the extractor understands. The inventory and the indexing pass
  cannot disagree about what is on disk because they read the same walk.
- **One index.** `IndexStore::all_documents` is a read-only view: path, hash, empty flag, OCR
  engine, chunk counts. The index stays the owner; nothing is duplicated into a second database.
- **Every file counts.** A `.zip`, a `.csv`, a file with no extension: all discovered, all counted,
  classified by `kind`. Fifteen files are fifteen files even when ten are indexed.
- **Deterministic ordering**, by canonical relative path. The same folder state always produces the
  same list, on Windows and on macOS.
- **Metadata only.** No page text ever enters a `FileRecord`, so an inventory cannot leak a
  document.

Symbolic links are refused, as they already were, and `absolute_path` only resolves a record the
inventory actually holds — so a forged record cannot reach the disk.

### `FileReferenceResolver` — which file did she mean?

`apps/desktop/src-tauri/src/file_reference.rs`. Reads the inventory and nothing else, which is why
`../../secret.pdf` has nowhere to go: there is no path to escape from, only a list of records to
fail to match.

Deterministic, in a fixed order: canonical relative path, then file name, then name without
extension. Case-sensitive first, then case-insensitive, and an extension in the reference is never
swapped for another — `report.pdf` does not resolve to `report.txt`.

**Ambiguity is a result, not a tie to break.**

```text
Exact            one file, and its whole record
MultipleMatches  every candidate, and no choice made
NoMatch          nothing carries that name, or the reference left the folder
```

With `2026/mars/neurologie.pdf` and `2026/janvier/neurologie.pdf` both present, "open
neurologie.pdf" returns both and asks. It never picks the one that sorted first.

A shortened name resolves too, because that is how people refer to a dated file: they drop the
date, not the subject. `12_compte-rendu-biologie.pdf` and `compte-rendu-biologie.pdf` both reach
`inbox/2026-03-12_compte-rendu-biologie.pdf`. Three rules keep that safe. A file **actually called**
the reference wins over one that merely ends with it. The fragment is matched against names only,
never paths, so `vier/neurologie.pdf` cannot reach into `janvier/`. And a fragment that is only an
extension is not a reference at all — `.pdf` would otherwise match every PDF in the folder. Several
tails matching is an ambiguity, reported like any other.

`resolve_in_question` is deliberately conservative: a question only names a file when it contains
something shaped like a path, something shaped like a file name with an alphabetic extension, or a
word that is exactly one file's stem. Anything looser would let an ordinary sentence silently
narrow retrieval to one document. Semantic resolution — "the March biology report", with no name in
it — is **not** implemented; it would slot in behind this same interface and return the same
`FileReferenceResolution`.

## Deterministic before generative

`apps/desktop/src-tauri/src/folder_questions.rs` routes a question before anything is embedded:

```text
question
  ├─ names a file?
  │    ├─ several match      → ask which. No retrieval, no gateway call.
  │    ├─ none match         → say so. No retrieval, no gateway call.
  │    ├─ nothing read from it → say so. No retrieval, no gateway call.
  │    └─ one readable file  → retrieve inside that file only, then the model.
  ├─ a filesystem question?  → answer from the inventory. No gateway call at all.
  ├─ about EVERY document?   → one excerpt from each indexed file, then the model.
  └─ otherwise               → the existing whole-folder retrieval, unchanged.
```

Questions answered with the model switched off: list files, count files, count or list by
extension, list images, list unreadable files, list indexed files, list folders, show the folder
structure, and the state of one named file.

**A file is not a document.** A *file* is anything on disk; a *document* is a file something could
be read from. Her distinction, and the one a person means: "tous mes documents" is the fifteen that
were analysed, not the seventeen that happen to be there. The pack keeps the two words apart and
the interface uses them the same way — *"17 fichiers — 15 documents analysés, 2 illisibles"*.

**An answer says where it came from.** A deterministic answer carries *"Réponse établie depuis votre
dossier de travail, sans l'IA"* in the slot a model answer uses for *"Généré par …"*: an answer with
no model behind it is a different kind of answer, and an empty slot is too quiet a way to say so.
Each listed file carries what happened to it, in the same words the Work Folder panel uses, because
whether anything could be read from a file is what decides whether it can be asked about.

**And the model stays reachable.** On a deterministic answer, "Régénérer" would recompute the same
answer byte for byte — it is computed, not written. There the same control reads *"Demander à
l'IA"* and asks the question again with the deterministic path skipped. One button, two honest
meanings, rather than a second button in a window that has no room for one.

**The vocabulary is data.** `apps/desktop/src-tauri/resources/work-folder-questions/{en-US,fr-FR}.json`,
one pack per catalogue, compiled in with `include_str!`. No sentence and no regex lives in Rust —
the language guard would fail it anyway, and a phrasing is not code (`docs/DECISIONS.md`).

Three things must hold before a question is answered from the folder:

1. An **intent** and a **subject** — something to do, and something to do it to.
2. No **content word**. "Give me a summary of each document" shares every other word with a listing
   request; answering it with a list of names would answer a question nobody asked. The pack
   carries a short denylist (`summary`, `mention`, `résume`, `contient`, …) that disqualifies the
   deterministic path outright. A denylist rather than an allowlist, because the ways of asking for
   content are few and the ways of asking politely are not.
3. No unknown word that the **documents themselves contain**. This is what separates "how many
   documents mention metformin" from "can you list all the available documents": `metformin` is a
   word in the corpus, `available` is not. The FTS index already knows this, so no dictionary has
   to be maintained, and it reaches the router through a `CorpusWords` port so the routing stays
   testable without a database.

The earlier rule — *every* word must be in the pack — was too brittle. On 23 September a politely
phrased listing request failed on four ordinary French words and went to a model, which then had to
transcribe sixteen near-identical paths by hand and got three of them wrong.

Rust returns a machine code plus facts and never a sentence. `apps/desktop/src/lib/folderAnswer.ts`
writes the wording from the catalogues, in the practice's language
(`docs/LANGUAGE-AND-LOCALE.md`). An answer produced this way carries no model label, because no
model wrote it.

## "Each document" is a question about the corpus

**Added 23 September 2026, from the pilot workstation.** A folder of 15 files, 10 of them indexed,
was asked "give me a summary of each document" and answered about six of them.

The model was not hiding four documents: it never received them. Ordinary retrieval ranks chunks
by **similarity** and stops at `MAX_EVIDENCE_CHUNKS` (6), so four indexed files were never sent to
the gateway at all. No prompt can fix that, because the text was never there to read — and the
model in use, a one-billion-parameter one, would not reliably honour a prompt anyway. A correctness
property belongs in code.

So a question that says **each**, **every**, **chaque** or **tous** together with a word for the
documents takes its own route. The **inventory supplies the list** — `indexed_files()`, in
canonical order — and retrieval runs once inside each file with `RetrievalScope::File`. Ranking
still decides which passage inside a file answers best; it no longer decides which files get a
voice.

Three rules keep it honest:

- **Whole stored chunks only.** An excerpt is never truncated to make it fit, because a citation
  has to point at a passage that really exists. Files are taken in inventory order until
  `MAX_PER_DOCUMENT_CHARS` (12,000) runs out, so the result is deterministic.
- **A file that ranks badly is still represented.** A summary question shares almost no words with
  a lab report, so ranking inside it can come back empty; the file then contributes its opening
  passage, scored zero. Coverage must not depend on the ranking.
- **The filesystem questions are matched first.** "all" is a distributive word, so "list all files"
  would otherwise become a request to read all of them. It stays a listing.

A distributive word on its own changes nothing — "at each consultation" is ordinary French and
English. Both halves are required.

### Coverage is stated, never implied

Whatever the route, Rust knows how many distinct files the evidence covered and how many the
inventory holds. For a question about every document it emits `EvidenceCoverage`, and the interface
writes it under the answer:

> Based on 10 of 10 analysed documents. 5 files could not be read.

It sits **beside** the answer, not inside it, so the model cannot leave it out. It is shown only
for this route: a question about one fact is properly answered from one passage, and "1 of 10"
there would be noise.

What this does **not** do: one pass per document. Every document reaches the model, but in one
call, so each summary is shallower than a dedicated pass would give. A per-document model pass is
Sprint 3's report-summary workflow, and on the pilot's hardware it would cost minutes rather than
seconds.

## What the model is told

When a question does need the model, the system turn carries three things, kept apart on purpose:
the retrieval instruction, the **Work Folder knowledge contract**, and a compact
`WORK_FOLDER_CONTEXT` block (`apps/desktop/src-tauri/src/work_folder_context.rs`).

```json
WORK_FOLDER_CONTEXT
{"root":"AssistantCabinetAI","view":"summary",
 "summary":{"total_files":15,"indexed_files":10,"unreadable_files":5,"by_extension":{"pdf":5,"txt":6}}}
```

The contract forbids inventing a file, altering a name or an extension, inferring counts from
excerpts, claiming an unreadable file was read, presenting recognised text as native text, and
choosing between several matches. It is English, like every prompt body, and it says nothing about
the output language — that directive is the gateway's and duplicating it would give the model two
masters.

The block never carries an absolute path, a content hash, a file size or a line of any document.
The **view** matters: a question about one file gets that file, a general question gets counts
only, and the full listing is never sent — a question that needs the full listing was answered
deterministically and never reached the gateway.

## Fixtures

`fixtures/inventory-sandbox/`, generated by `scripts/gen-sandbox-fixtures.py`, fictional content
only.

- `flat/` — exactly 15 files: 6 TXT, 5 PDF, 1 DOCX, 2 PNG, 1 JPG. Ten are readable and indexed;
  five genuinely cannot be read (two PDFs with no text layer, three images with nothing in them).
  Unreadability comes from the real failure path, never from a flag set in a test. Three files are
  adversarial: `misleading.txt` claims the folder holds two files and that it is a PDF,
  `report.txt` calls itself a PDF report, and `procedure.txt` refers to a `fake-document.pdf` that
  does not exist and whose text really is in the index.
- `nested/` — `2026/mars/neurologie.pdf`, `2026/mars/biologie.pdf`,
  `2026/janvier/neurologie.pdf`, `administratif/assurance.txt`. The repeated stem is the point.

Acceptance: `apps/desktop/src-tauri/tests/work_folder_inventory.rs`. The gateway double counts
every request it receives; after indexing the counter is reset, and every filesystem question must
leave it at zero.

## Future reuse — two Work Folders

The product will eventually have two user-configurable folders, and they stay separate:

```text
        WorkFolderInventory  (FileRecord, FileReferenceResolver)
                 │
      ┌──────────┴──────────┐
      ▼                     ▼
DocumentWorkFolderContext   TabularWorkFolderContext   ← sprint 2b, not built
PDF / DOCX / TXT / MD /     CSV / XLS / XLSX
images, OCR                 WorkbookInventory, SheetInventory,
      │                     deterministic analysis
      ▼                     ▼
Document index              Tabular inventory
      └──────────┬──────────┘
                 ▼
          CrossDomainContext  ← later still
```

What this sprint did to keep that possible, and nothing more: `FileKind::TabularCandidate` exists
so a spreadsheet is *recognised* without entering the document pipeline; `FileRecord` carries no
document-specific field, so a workbook needs no second identity model; and
`FileReferenceResolver` resolves by filesystem metadata alone, so it is already domain-neutral.

**Not built, deliberately:** CSV or XLSX parsing, workbook inventory, sheets, tables, formula
evaluation, cross-domain reasoning, semantic file resolution, background folder watching, and any
file-management interface beyond showing the counts and a list.

## Known cost, and the improvement that would remove it

The inventory hashes every file in the folder on every build, because identity is the content
SHA-256 and because `Pending` - "indexed once, changed since" - cannot be detected without it. That
is one pass over the folder's bytes per question about the folder, and per refresh of the panel.

For the pilot it is the right trade: `docs/RETRIEVAL.md` already assumes a work folder of tens of
files, and retrieval already loads every stored chunk and vector on every question, which is work of
the same order. It stops being the right trade for a folder of hundreds of large scans.

The improvement, **not built here**: record size and modification time alongside the hash in
`documents`, and reuse the stored hash when both still match. That is an index schema change and a
change to the skip rule in `indexing.rs`, which is more than this sprint should touch.

## Non-goals of sprint 2a.5

No new ingestion format. No change to OCR behaviour. No change to the gateway. No agent framework,
MCP tool, shell access or arbitrary filesystem tool. No index lifecycle rewrite: a file deleted
from the folder disappears from the inventory, and cleaning its rows out of the index stays a
later, separate decision.
