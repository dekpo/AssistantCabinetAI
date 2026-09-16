# Pilot — French liberal GP

Findings from the interview rounds of 13, 14 and 15 September 2026. Raw notes and the personal detail stay in
`docs/private/PILOT-INTERVIEW.md`. No patient name and no record extract appears here or there.

## The practice

Practice software **Medilink** (formerly Chorus), on a Windows PC installed in 2019. No separate secretariat is
mentioned: the doctor performs the whole intake circuit herself — receive, scan, name, file, annotate. Most
practices she saw when replacing were also on Windows.

Software already in place, which the assistant grafts onto rather than replaces: Medilink and its embedded
secure messaging (mssantechorus), MSSanté outside Chorus, Apicrypt, Word, an Orange email account, and a
scanner.

## Where documents arrive

| Channel | Use |
| --- | --- |
| **MSSanté** outside Chorus | Specialist and imaging reports arrive here, then forward automatically into mssantechorus. Requires a login at least every 45 days or the account is deleted |
| **mssantechorus** | The main queue. A backlog of **150 to 160** reports has accumulated since May |
| **Apicrypt** in the practice software | Lab results from nearby laboratories. Fastest path: **4 clicks** |
| Distant laboratories | MSSanté or paper |
| **Orange** email | Two colleagues still send reports this way |
| Paper | Always the same two or three colleagues, plus organised screening second-reading reports, scanned |
| Regional screening coordination centre | Follow-up questionnaires when colon screening is positive, returned via MSSanté |

Anything downloaded or scanned lands in **My Documents**, at the root, not in a dedicated subfolder. Ameli
paperwork lands there too.

## Volumes and formats

- About **20** specialist and imaging reports **per day** through MSSanté.
- **10 to 20** lab results **per day**.
- About **10** paper letters **per week**, and still **10 to 20** scans per week.
- Seasonal: July and August are low; the peak runs mid-September to April or May.
- Mostly **PDF**. Imaging is usually PDF in mssantechorus. Apicrypt looks like a Word-ish format, and some
  results are now a **web link**. Occasionally a format that forces copy-paste into Word and export to PDF, or
  a paid Word service — in that case she asks the correspondent for a PDF instead.
- Incoming MSSanté subject lines are often uninformative.
- Her existing convention: name, sometimes with month and year, then file into the patient record.

## Where the time goes

- **Specialist and imaging reports: 1.5 to 2 hours per day** to stay current, and/or catch-up on Saturday
  afternoon from 14:00 to about 18:00. Not caught up since May. Many skipped lunches go into this queue.
- Lab results: between consultations, at lunch, and about an hour in the evening. Up to date.
- Paper and Apicrypt: up to date.
- **The worst part is not filing the PDF.** It is opening the record and **annotating** it: the date of the
  specialist follow-up visit, treatment changes, recommended lab targets. That is her method, and locums who
  neither renamed nor dated reports left records that are hard to read.

## Duplicates — a strong, late finding

Duplicates are frequent in both MSSanté and paper mail. Sometimes two sends in a row, which she understands.
Often weeks or months apart — one week, one month, six months — because the correspondent or their secretary
no longer knows what was already sent.

The cost she describes: open the message, open the report, sense that she has read it, open the patient record,
search "received letters", confirm the duplicate.

Honest limit: the assistant does not open Medilink, so it cannot search "received letters" for her. What it can
do is flag two files that are too similar **within a batch**, and later match against a **local** fingerprint
register — hash plus an extracted signature such as type, document date and title — of documents that already
passed through the work folder. She decides. No report database on the Mac mini.

## Value order for the prototype

| Rank | Workflow | Why | Forbidden |
| --- | --- | --- | --- |
| 1 | **Specialist report summary** (`gp-letter-summary`) | The real cost: 1.5–2 h/day, Saturdays, the backlog. Extract with citations what she annotates: visit date, treatment changes **according to the letter**, lab targets **according to the letter** | Diagnosing, or saying a treatment must change |
| 2 | **Naming plan with duplicates** (`gp-inbox-classify`) | ~20 PDFs a day with unhelpful titles; name plus month/year convention | Writing into Medilink; moving silently; pointing at all of Documents |
| 3 | **Structured extraction** | The fields she retypes by hand | Inventing a field that is not in the document |
| — | Apicrypt lab results | Already 4 clicks and up to date. Low priority | Following a laboratory web link; writing in the follow-up banner |
| — | Referral letters | Very frequent but **already** in Medilink, with automatic history | Automatic MSSanté sending |
| — | Certificates | Medilink templates she retouches | Out of the first prototype |

Sick leave, occupational disease and work accident declarations go through the **Ameli** professional account,
because the Medilink-to-Ameli link did not work. Her circuit: fill in online, download to My Documents, print
the patient's copy, file the set under administrative documents. The only file action we might help with later
is **naming** that downloaded set, like any other PDF.

Accounting was a real pain — six half-days of catch-up — but mandatory e-invoicing and the accountant's software
have taken it over since September 2026. Not a module. The remainder, supplier invoices and a month-end check,
could later use the same work folder.

## Not touched in the first version

Prescriptions, care sheets, Vitale and CPS cards, insurance returns, DMP filing, secure-messaging sending,
clinical decisions or triage, accounting and URSSAF, the Ameli account, and Open WebUI Computer on her machine.

## Real documents

She offered to work on real records in a "closed circuit". **Declined** until there is a spoken conversation on
the point, a DPIA draft, a named data controller, and disk encryption. Not knowing the patients is not
anonymisation, and a local circuit is still health-data processing. Until then the sandbox is
`fixtures/gp-sandbox/`. See `docs/PRIVACY-AND-SECURITY.md`.

## Still to see

- Show and record: the Apicrypt format, the "web link" case, typical MSSanté subject lines, the 4-click path.
- Secretariat: yes, no, or partial.
- Screening follow-up questionnaires: how frequent a burden?
- The exact format of the Medilink certificate templates — she said she would check.
- One anonymised duplicate example; two fictional PDFs are enough for the sandbox.
- The 2019 PC specification — the project owner installed it and can check it herself rather than asking the doctor.
- A more precise volume count, expected from the week of 15 September 2026.
