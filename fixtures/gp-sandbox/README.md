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
