# Brief — Sprint 2.5, local OCR (recorded direction)

**Recorded:** 19 September 2026. **Source:** project owner, verbatim.
**Status:** canonical direction for the OCR milestone. Where an older note says scan OCR is out of v0,
this brief wins. The decision it reverses is logged in `docs/DECISIONS.md`.

Assessment answering its section 21: `docs/SPRINT-2.5-ASSESSMENT.md`.
Dated plan: `docs/ROADMAP.md`. Contracts: `docs/ARCHITECTURE.md`. Pipeline: `docs/RETRIEVAL.md`.

Do not edit the verbatim section below. Scheduling, acceptance criteria and risks live in
`docs/ROADMAP.md`; the engineering answer lives in the assessment.

## What this brief changes

1. **Scan OCR moves into v0.** It was frozen by `.cursor/rules/v0-sprint.mdc` and recorded as "out of
   v0" in `docs/DECISIONS.md`. It becomes Sprint 2.5, between the document retrieval that carries
   milestone A and the GP workflows that consume it.
2. **OCR is ingestion, not a GP workflow.** A workflow must not know whether its input arrived as a
   born-digital PDF or as a photograph of one.
3. **The OCR engine is a replaceable port**, like `AIProvider`, `IndexStore` and `Embedder` before it.

## What it does not change

The two milestone dates, the privacy boundaries, the work-folder allow-list, the no-store rule, the
language contract, or the position of tabular data after the pilot. DOCX-to-PDF conversion is
explicitly excluded. Nothing here authorises a cloud service of any kind.

---

## Verbatim brief

# AssistantCabinetAI — Roadmap Reframe and Sprint 2.5 Local OCR

## Mission

We are continuing development of the existing `AssistantCabinetAI` repository.

This is **not a greenfield project**.

The project already has:

* a Tauri desktop client
* a FastAPI private gateway
* an `AIProvider` abstraction
* Ollama integration
* local document retrieval
* provenance/citations
* privacy/isolation tests
* PDF/DOCX/TXT/MD document processing
* CSV/XLSX architecture planned for the next phase

The current product direction is already established:

> **AssistantCabinetAI is a Private AI Platform for professional practices, not simply a desktop application connected to a local LLM.**

The GP pilot remains the first real-world validation.

---

# 1. FIRST TASK — INSPECT BEFORE MODIFYING

Before changing anything:

1. Read the current `docs/ROADMAP.md`.
2. Inspect the current repository structure.
3. Inspect the completed Sprint 2 / Sprint 2a implementation.
4. Inspect existing document extraction and retrieval code.
5. Inspect existing tests.
6. Identify exactly what is already implemented versus what the roadmap says is planned.
7. Preserve existing working architecture and tests.

Do not rewrite existing components unnecessarily.

---

# 2. ROADMAP REFRAME

The current roadmap must be adjusted because the real GP pilot is expected to receive:

* scanned PDFs
* image-only PDFs
* JPEG documents
* potentially PNG/image documents

These documents cannot reliably enter the existing text extraction/RAG pipeline without OCR.

Therefore introduce a new intermediate milestone:

# Sprint 2.5 — Local OCR

Position it **after Sprint 2a / current document retrieval work and before tabular data and GP workflows.**

The revised sequence should be:

```text
Sprint 2a
Document Retrieval
PDF / DOCX / TXT / MD
        ↓
Sprint 2.5
Local OCR
PDF images / JPEG / PNG
        ↓
Sprint 2b
CSV / XLSX / Tabular Data
        ↓
Sprint 3
GP Workflows
        ↓
Sprint 4
Deployment / Pilot
```

Do not simply append OCR to Sprint 3.

OCR is now considered part of the **document ingestion foundation**.

---

# 3. WHY OCR COMES BEFORE GP WORKFLOWS

The GP workflows must operate on documents that the doctor actually receives.

A workflow should not care whether its input document was:

* born-digital PDF
* DOCX
* TXT
* Markdown
* scanned PDF
* JPEG
* PNG

The document pipeline should normalize these sources before they reach:

* retrieval
* summarization
* classification
* naming
* structured extraction
* future workflows

Therefore:

> OCR is an ingestion capability, not a GP workflow feature.

Sprint 3 should consume the normalized document representation produced by the ingestion pipeline.

---

# 4. OCR ARCHITECTURAL PRINCIPLE

Do NOT hard-code one OCR engine into the application.

Introduce an abstraction:

```text
Document
    ↓
DocumentExtractor
    ↓
Text available?
    ├── YES → normalized text
    │
    └── NO → OCRProvider
                 ↓
             OCR engine
                 ↓
          normalized text
```

The application must depend on:

```text
OCRProvider
```

not directly on:

```text
Tesseract
PaddleOCR
OCRmyPDF
```

The first implementation may use one local OCR engine.

The architecture must allow another engine to be substituted later without changing:

* retrieval
* document processing
* workflows
* Tauri
* business logic

---

# 5. OCR PROVIDER CONTRACT

Design a clean provider abstraction appropriate to the existing codebase.

Conceptually:

```python
class OCRProvider:
    def supports(self, document) -> bool:
        ...

    def extract(self, document) -> OCRResult:
        ...
```

The exact interface and naming should follow existing project conventions.

`OCRResult` should preserve provenance and useful metadata, including where possible:

* document ID
* page number
* extracted text
* confidence information if available
* OCR engine/provider
* processing status
* optional bounding boxes/coordinates if the selected engine provides them

Do not over-engineer the result model.

Only retain information that can support future:

* citations
* document search
* structured extraction
* visual/document workflows

---

# 6. SUPPORTED OCR INPUTS

Sprint 2.5 should initially support:

* image-only PDF
* JPEG/JPG
* PNG

It should also detect when a PDF already contains an adequate text layer.

For such PDFs:

> Do NOT run OCR unnecessarily.

The preferred behavior is:

```text
PDF
 ↓
detect usable text layer
 ├── yes → native extraction
 └── no  → OCRProvider
```

This avoids unnecessary CPU usage and improves performance.

---

# 7. OCR MUST BE LOCAL

This is a strict privacy requirement.

Sensitive professional documents must never be uploaded to a cloud OCR service.

The OCR pipeline must execute on the authorized workstation/local environment.

The architecture must therefore support:

```text
User workstation
 ├── documents
 ├── OCR
 ├── extraction
 ├── local index
 └── retrieval
        ↓
 selected excerpts only
        ↓
 private AI gateway
```

Do not introduce:

* cloud OCR APIs
* external document-processing services
* mandatory Internet access

---

# 8. OCR ENGINE SELECTION

Do not prematurely lock the architecture to one OCR technology.

Evaluate the practical local options available to the project, such as:

* Tesseract
* PaddleOCR
* OCRmyPDF
* another appropriate local OCR engine

Select the first implementation based on:

* local/offline operation
* installation/deployment feasibility
* Windows compatibility
* French language quality
* PDF/image support
* performance
* licensing
* ease of replacement

The provider abstraction must make future replacement straightforward.

Do not build multiple OCR engines in Sprint 2.5.

---

# 9. OCR AND PROVENANCE

OCR output must integrate with the existing provenance model.

For example:

```text
document
  → page
      → OCR text
          → passage/chunk
```

A retrieved answer must still be able to cite:

* source file
* page
* passage/chunk

The system must never fabricate an OCR citation.

If OCR cannot reliably extract sufficient information:

> return insufficient evidence rather than hallucinating.

---

# 10. OCR QUALITY AND CONFIDENCE

Where the selected OCR engine provides confidence information, preserve it internally.

Do not automatically expose raw confidence scores to the user unless the UX later requires it.

The important principle is:

> OCR uncertainty must not silently become LLM certainty.

If OCR output is poor or insufficient, the downstream system should be able to detect that and avoid presenting unsupported information as fact.

---

# 11. OCR → EXISTING RETRIEVAL PIPELINE

Do not create a second RAG pipeline.

The desired architecture is:

```text
PDF / DOCX / TXT / MD / JPEG / PNG
              ↓
       Document ingestion
              ↓
       DocumentExtractor
              ↓
       OCRProvider when required
              ↓
       Normalized Document
              ↓
          Chunking
              ↓
       Local Index / Retrieval
              ↓
       Provenance / Citations
              ↓
          AI Orchestrator
```

OCR is therefore simply another extraction path.

---

# 12. TABULAR DATA REMAINS SPRINT 2b

After OCR, continue with the planned CSV/XLSX milestone.

CSV/XLSX must remain first-class sources.

The architecture should eventually support:

```text
Documents
 ├── PDF
 ├── DOCX
 ├── TXT
 ├── MD
 ├── scanned PDF
 ├── JPEG
 └── PNG

Structured Data
 ├── CSV
 └── XLSX
```

For CSV/XLSX, preserve the LocalGridMind principle:
https://github.com/dekpo/LocalGridMind/tree/develop
also available locally here C:\Users\elise\Documents\CURSOR\LocalGridMind

> deterministic, verifiable tabular facts should be answered locally without invoking the LLM whenever possible.

Do not move the full LocalGridMind application into AssistantCabinetAI.

Reuse/adapt its proven concepts behind the AssistantCabinetAI platform boundaries.

---

# 13. SPRINT 3 — GP WORKFLOWS

Do not start Sprint 3 implementation until Sprint 2.5 has established a reliable OCR ingestion path.

Sprint 3 should then operate on the normalized document/data layer.

GP workflows may include:

* document classification
* naming
* summarization
* structured extraction
* duplicate detection
* controlled file operations

These workflows must not contain their own OCR implementation.

They should consume normalized sources.

---

# 14. PRODUCT ARCHITECTURE

Preserve the established long-term architecture:

```text
Desktop Client
Mobile Client (future)
Web Client (future)
        ↓
    Private API
        ↓
Policy / Authorization
        ↓
Retrieval / Documents / Data
        ↓
AI Orchestrator
        ↓
AIProvider
        ↓
Inference Runtime
        ↓
Model
```

The platform capabilities remain:

* Documents
* Structured Data
* Retrieval
* Deterministic Analysis
* OCR
* AI
* Models
* Workflows
* Devices
* Policies
* Provenance
* Audit

OCR is therefore a **replaceable document-ingestion capability**, not part of the AI model layer.

---

# 15. FUTURE OCR EXTENSIBILITY

The architecture should allow future support for:

* better OCR engines
* GPU acceleration
* layout-aware OCR
* tables detected from images
* handwriting recognition if eventually required
* mobile camera capture
* batch OCR
* OCR quality evaluation

Do not implement these features now.

The important requirement is that today's implementation does not prevent them later.

---

# 16. TESTING REQUIREMENTS

Add tests for:

### Native PDF

```text
PDF with text layer
→ native extraction
→ OCR NOT invoked
```

### Scanned PDF

```text
image-only PDF
→ OCRProvider
→ extracted text
→ page provenance
→ retrieval
```

### JPEG/PNG

```text
JPEG/PNG
→ OCRProvider
→ normalized document
→ retrieval
```

### Failure

```text
unreadable image
→ OCR failure / insufficient evidence
→ no fabricated text
```

### Privacy

Verify that:

* source files remain on the workstation
* OCR text is not written to server logs
* sensitive OCR content is not stored in audit records
* only selected excerpts are sent to the AI gateway

### Regression

All existing Sprint 1 and Sprint 2 tests must remain passing.

---

# 17. PERFORMANCE

Do not OCR every document blindly.

Use the cheapest reliable path:

```text
native text extraction
        ↓
if usable → stop

otherwise
        ↓
OCR
```

Consider local caching based on file hash so unchanged documents are not repeatedly OCR'd.

Only implement caching if it fits the existing architecture cleanly.

---

# 18. DEPLOYMENT CONSTRAINT

Sprint 2.5 must consider the eventual Windows deployment.

The OCR provider must be compatible with the planned Windows installer/deployment strategy.

Do not choose a technically excellent OCR engine that makes the final pilot impossible to install or maintain.

Deployment complexity is therefore part of the provider selection criteria.

---

# 19. DOCUMENTATION

Update:

`docs/ROADMAP.md`

to reflect:

```text
Sprint 2a — Document Retrieval
        ↓
Sprint 2.5 — Local OCR
        ↓
Sprint 2b — CSV/XLSX and Tabular Analysis
        ↓
Sprint 3 — GP Workflows
        ↓
Sprint 4 — Deployment / Pilot
```

Also document the OCR architecture in the appropriate architecture documentation.

Keep:

`ROADMAP.md`

focused on what is being built and when.

Keep long-term architecture in:

`docs/ARCHITECTURE.md`

and/or:

`docs/PLATFORM-VISION.md`

---

# 20. DOCX → PDF

Do NOT add DOCX → PDF conversion to Sprint 2.5.

Document conversion is a separate future capability.

The current priorities are:

1. reliable document ingestion
2. local OCR
3. structured data
4. GP workflows
5. deployment

A future document-transformation service may eventually provide:

```text
DOCX → PDF
PDF → other formats
document generation
```

but this must not expand Sprint 2.5.

---

# 21. REQUIRED FIRST ACTION

Before coding, provide a concise implementation assessment containing:

### A. Current document ingestion architecture

### B. Where OCRProvider should live

### C. Existing components that can be reused unchanged

### D. Components requiring minimal refactoring

### E. Recommended first local OCR engine

### F. Alternative OCR engines and replacement strategy

### G. Exact OCRProvider contract

### H. PDF text-layer detection strategy

### I. JPEG/PNG integration strategy

### J. Provenance integration

### K. Caching strategy, if justified

### L. Windows deployment implications

### M. Tests to add

### N. Exact Sprint 2.5 implementation sequence

Do not perform a large refactor before presenting this assessment.

---

# 22. FINAL PRINCIPLE

The product remains:

> **Private AI Platform**

not:

> Desktop application + local LLM.

The AI model is replaceable.

The OCR engine is replaceable.

The embedding engine is replaceable.

The retrieval/index implementation is replaceable.

The client is replaceable.

The professional workflows must consume stable platform capabilities.

For the current GP pilot:

> **Make every document usable locally, regardless of whether it arrived as native text, a scanned PDF, JPEG or PNG.**

Then build the GP workflows on top of that reliable foundation.

Build for today's pilot.

Architect for tomorrow's private professional AI platform.

Do not sacrifice reliability for speculative features.
