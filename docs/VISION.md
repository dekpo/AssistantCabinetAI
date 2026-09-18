# Vision

## Problem

Professional practices — doctors, medical secretaries, notaries, lawyers, accountants, in-house counsel —
handle a high volume of text and documents. They cannot paste those documents into a consumer assistant:
professional secrecy, health data, GDPR, liability.

The need is not a chatbot. It is a **local administrative colleague** that reads DOC, DOCX, PDF and text,
proposes filing, renaming and reorganisation, helps draft and extract, and automates repetitive work **under
human control**.

## Audience

| Profile | Priority use | Specific constraint |
| --- | --- | --- |
| Secretariat | Filing, letters, document queue | Volume, Office habits |
| Doctor | Reports, letters to colleagues, filing | Health data; HDS if a third party hosts |
| Notary | Deeds, exhibits, case naming conventions | Formalism, traceability |
| Lawyer | Cases, exhibits, notes, internal search | Professional secrecy |
| Accountant | Receipts, filings, reminders | Numerical accuracy, fiscal years |
| In-house counsel | Contracts, internal monitoring, templates | Versions, clauses |

One internal server can serve several workstations. Workspaces stay partitioned by practice, profession and
case.

## Non-negotiable principles

1. **Local-first and minimisation.** Documents, retrieval index and chat history stay **on the workstation**.
   The server infers without storing the confidential content: no-store plus a metadata register.
2. **Human in the loop.** The AI proposes; a person approves every file write.
3. **Not a professional substitute.** Administrative assistance only.
4. **Existing habits preserved.** Word, document management, Explorer and the IDE stay at the centre. A
   gateway lets the assistant graft onto them.
5. **Replaceable by design.** Stable contracts for the API, the tools, the plans and the audit. Agent runtimes
   are adapters. Tomorrow's model replaces today's without rewriting the application.

## What the product is not

- A locally installed clone of a consumer chat assistant.
- A replacement for the document-management or practice software.
- An autonomous system that tidies the server overnight without review.
- A medical device or clinical decision support.

## Success criterion for the first pilot

A professional can, on a real test folder, drop a batch of PDFs and DOCX files, get an understandable filing
and renaming plan, adjust it, apply it, and find a record of who approved what — without a single byte leaving
the LAN, and without the documents remaining stored on the AI server.

Current pilot and its measured workflows: `docs/PILOT-GP.md`. Current sprint: `docs/ROADMAP.md`. The audience
table above is an assumption for every profession except the GP; what the others actually say is collected in
`docs/DISCOVERY.md`.
