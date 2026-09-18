# Discovery — field information from professionals

Sector research, kept next to the code and the specification because it is what keeps the product honest.
`docs/VISION.md` lists six professions; only one of them has ever been interviewed. This folder is where the
others get asked, one meeting at a time.

Discovery is **not** a scope change. "One profession at a time (the GP for now)" holds, and a second shared
profession stays out of scope until the pilot holds (`docs/DECISIONS.md`). Findings collected here wait in
this folder; they do not open a branch, a module or a promise before 14 October 2026.

## Ground rules

1. **Information only.** No selling, no quote, no request to become a pilot. Several of these professionals
   are the owner's own advisers on personal matters; the personal relationship comes first. If the person is
   in a hurry, ask nothing.
2. **No client data, ever.** Not a name, not a case reference, not a document. Every question is about the
   practice's workflow, never about its files. The forms say so in writing, at the top.
3. **Blank forms are versioned; answers are not.** A blank form contains no personal data, so it belongs in
   the repository. A completed form identifies a real person and stays under `docs/private/discovery/`, which
   git ignores.
4. **Anonymised when reused.** Any finding quoted in `docs/VISION.md`, `docs/DECISIONS.md` or a commercial
   note is attributed by profession and practice size — "a solo lawyer, family law" — never by name.
5. **The form is written in the reader's language, one language per file.** French for French professionals,
   like everything a human reads in the product. This method file and the interview records stay English,
   like the rest of the specification.
6. **Version the form.** Bump the version line when questions change, so answers collected weeks apart stay
   comparable. The Markdown form is the source; the HTML next to it is only its print rendering and carries
   the same version number.

## Layout

| Path | What it holds | Versioned |
| --- | --- | --- |
| `docs/DISCOVERY.md` | This method, plus the register below | Yes |
| `docs/discovery/questionnaire-lawyer-fr.md` | Blank form for a French law firm, the source text | Yes |
| `docs/discovery/print/questionnaire-lawyer-fr.html` | The same form laid out for A4 printing | Yes |
| `docs/private/discovery/<date>-<profession>.md` | One record per meeting: context, spoken questions, answers, conclusions | No |

## Register

| Date | Profession | Form | Status | Record |
| --- | --- | --- | --- | --- |
| 18 September 2026 | Lawyer, French private practice, family and property | `questionnaire-lawyer-fr.md` v1 | Form issued for a same-day meeting | `docs/private/discovery/2026-09-18-lawyer.md` |
| Not scheduled | Notary, French practice | To be derived from the lawyer form | Not started | — |
| 13–15 September 2026 | General practitioner, French liberal | Ten written questions, three rounds | Answered; conclusions promoted | `docs/private/PILOT-INTERVIEW.md`, `docs/PILOT-GP.md` |

## How a meeting runs

Ask four or five questions out loud, no more, and leave the printed form for later. A form handed over is
worth more than a form filled in under time pressure: it lets the professional answer at their own pace, and
it makes the request feel like research rather than a sales call. Take no notes on the person's own matter,
only on the practice's workflow. Write the record the same day, while the wording is still fresh.

## How a finding becomes a decision

A single interview is an anecdote. Two independent professionals in the same trade saying the same thing is a
signal, and only then does it earn a line in `docs/VISION.md` (audience and constraints) or a row in
`docs/DECISIONS.md`. Cite the register row, not the person. A signal that contradicts the current v0 scope is
recorded here and revisited after 14 October 2026, not acted on mid-sprint.

Four things are worth more than the rest, because they decide whether the product is buildable at all for a
trade: where documents accumulate before filing, the practice's real naming convention, what professional
secrecy actually forbids in that person's reading of it, and who decides on tools.
