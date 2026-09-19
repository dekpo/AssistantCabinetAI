# GP sandbox (fictional only)

All names, dates, findings, and file names here are invented. They are stand-ins for PDFs so the admin prompts can be tried without a real inbox.

Do not copy files from a practice system into this tree. A `real/` folder is gitignored and must stay empty.

## How to try the prompts

| Prompt key | Input in this folder | First prototype |
| --- | --- | --- |
| `gp-letter-summary` | `inbox/2026-03-10_courrier-specialiste.txt` | **Yes** |
| `gp-inbox-classify` | the files in `inbox/` (treat them as unlabeled PDFs; `2026-09-02_mssante-sans-titre.txt` is a deliberate duplicate of the 10 March letter) | **Yes** |
| `gp-referral-letter` | `notes/referral-notes-camille-exemple.txt` | Later (pilot already writes referrals in the practice software) |
| `gp-certificate-draft` | `forms/certificate-form-hugo-bacasable.txt` | Later (templates already in the practice software; sick leave is Ameli, not this prompt) |

Success for this prototype: a specialist-letter summary in **French** with citations, and a French Markdown rename plan that **flags the duplicate**. Prompt **bodies** are English (`prompts/`). File-name stems in the plan are French (`lettre-specialiste`). Nothing is sent. A human must review every output.

## Binary fixtures (Sprint 2a, document extraction and retrieval)

`inbox/` also holds fictional PDF and DOCX files, generated once by `scripts/gen-sandbox-fixtures.py`
(stdlib only, no extra install) so extraction can be exercised on real binary formats rather than on
`.txt` stand-ins:

| File | Stands in for |
| --- | --- |
| `2026-03-12_compte-rendu-biologie.pdf` | A native-text, single-page lab report |
| `2026-03-18_courrier-neurologie.pdf` | A native-text, two-page specialist letter |
| `2026-03-14_courrier-endocrinologie.docx` | A native-text Word letter |
| `2026-03-20_radiographie-scan.pdf` | A scanned page with no text layer, reserved for the OCR contract: extraction must report it empty, never guess at it |

Regenerate with `python scripts/gen-sandbox-fixtures.py` from the repository root if these files are
ever lost or need to change; do not hand-edit the binaries.

## Hand-supplied scans (Sprint 2.5, OCR)

A real scan is a better OCR fixture than a generated one: a scanner adds skew, speckle, JPEG artefacts
and a header band that a clean synthetic bitmap never will, and those are the conditions that decide
whether recognition is usable. Adding one is encouraged, under two rules.

**Fictional content only.** This tree is committed to Git, so anything placed here is permanent and
copied to every clone. No patient name, no real correspondent, no genuine report, and not a redacted one
either — a redacted real report is still a real report. The safe recipe: type an invented letter in Word,
print it, scan the paper. That gives a genuine scan of a fake document.

**Check before committing.** Strip metadata, and confirm whether the scanner added its own OCR text
layer — that changes which case the file exercises, so either remove it or name the file for what it
actually tests. List every new fixture in the table above.

A genuine practice scan needed to diagnose a recognition problem goes in `real/`, which is gitignored and
must stay empty in the repository.
