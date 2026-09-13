# Assistant Cabinet AI

Local AI for professional practices (GP, secretary, notary, lawyer, accountant, in-house counsel). Data stays on the internal network. No consumer LLM in production.

## Goal

Help staff process large volumes of documents (DOC, DOCX, PDF, text and similar): classify, reorganize, create, edit, rename, and automate repetitive admin — **without replacing their usual software**.

## Reference stack

| Layer | Technology | Role |
| --- | --- | --- |
| Environment | Docker Compose | Same stack on a Windows PC and later on a Mac mini |
| Inference | [Ollama](https://ollama.com/) | Local models |
| Gateway | OpenAI-compatible `/v1` | All clients (pane, Open WebUI, VS Code / Continue) |
| Admin UI | [Open WebUI](https://openwebui.com/) | Advanced chat and admin — **not** a store for case files |
| Retrieval | Index **on each workstation** | Sensitive files are not stored on the AI server |
| Models | [Hugging Face](https://huggingface.co/) via Ollama | Open weights; check the licence before practice use |
| Target host | Apple Silicon Mac mini, 64 GB | Internal server, unified memory |
| Clients | Windows PCs (macOS later) | Existing habits stay |

## Local prototype

Docker Compose on localhost (Ollama + Open WebUI). Install steps: [docs/user/local-compose.md](docs/user/local-compose.md). Fictional GP files: `fixtures/gp-sandbox/`. Licence register: [models/LICENSES.md](models/LICENSES.md).

## Project status

**Pilot:** French liberal GP (admin only). Compose files are in this repo. Do not use Open WebUI Computer on a practice machine.

Public docs and contribution rules: [CONTRIBUTING.md](CONTRIBUTING.md). Owner-only notes live under `docs/` on the local machine and are **not** in git. Published user guides will go in `docs/user/`.

## Short verdict

A Mac mini M4 Pro 64 GB can serve about **1–8 seats** depending on mix (especially if retrieval and OCR stay on the PCs). Beyond that: a second node or a GPU. Chat matters, but the product is the **gateway + folder pane + approved plans**. Case files do not live on the AI server.
