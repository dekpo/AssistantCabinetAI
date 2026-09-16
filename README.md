# Assistant Cabinet AI

Local AI for professional practices (GP, secretary, notary, lawyer, accountant, in-house counsel). Data stays on the internal network. No consumer LLM in production.

## Goal

Help staff process large volumes of documents (DOC, DOCX, PDF, text and similar): classify, reorganize, create, edit, rename, and automate repetitive admin — **without replacing their usual software**.

## Reference stack

| Layer | Technology | Role |
| --- | --- | --- |
| Environment | Docker Compose | Same stack on a Windows PC and later on a Mac mini |
| Inference | [Ollama](https://ollama.com/) | Local models |
| Gateway | Python, FastAPI, Pydantic (`apps/server`) | OpenAI-compatible `/v1` plus product endpoints. Every client goes through it, never straight to Ollama |
| Admin UI | [Open WebUI](https://openwebui.com/) | Owner workbench only — **not** the GP’s screen, **not** a store for case files |
| Practice app | Assistant Cabinet AI — Tauri 2, React, TypeScript (`apps/desktop`) | Native window on Windows and Mac: work folder + plan + chat |
| Retrieval | Index **on each workstation** | Parsing, chunking and the index stay local. Sensitive files are not stored on the AI server |
| Models | [Hugging Face](https://huggingface.co/) via Ollama | Open weights; check the licence before practice use |
| Target host | Apple Silicon Mac mini, 64 GB | Internal **inference** server, unified memory |
| Workstations | Windows and macOS PCs | Existing habits stay (practice software unchanged) |

## Local prototype

Docker Compose on localhost (Ollama + Open WebUI, English workbench). Install steps: [docs/user/local-compose.md](docs/user/local-compose.md). Admin prompt bodies: `prompts/`. Fictional GP files: `fixtures/gp-sandbox/`. Licence register: [models/LICENSES.md](models/LICENSES.md).

## Current milestone

**v0 prototype.** By 30 September 2026 the whole chain holds on fictional fixtures: native window, gateway, local index, answer with sources, and a test proving the server keeps no document. By 14 October 2026 the pilot GP uses it on her own documents, with a Windows installer and the server on a Mac mini at her practice. Answers cite filename, page and passage; when the documents do not carry the answer, the product says so rather than generating one.

## Project status

**Pilot:** French liberal GP (admin only). Compose files are in this repo. Model instructions: `prompts/` (English in, French out). Open WebUI is an English owner workbench. The practice UI is a native **Assistant Cabinet AI** window, not Open WebUI in a browser. Do not use Open WebUI Computer on a practice machine.

Public docs and contribution rules: [CONTRIBUTING.md](CONTRIBUTING.md). Owner-only notes live under `docs/` on the local machine and are **not** in git. Published user guides will go in `docs/user/`.

## Short verdict

A Mac mini M4 Pro 64 GB can serve about **1–8 seats** depending on mix (especially if retrieval and OCR stay on the PCs). Beyond that: a second node or a GPU. Chat matters, but the product is **Assistant Cabinet AI** (native window) + the **gateway + approved folder plans**. Case files do not live on the AI server. Open WebUI is the owner’s workbench only.
