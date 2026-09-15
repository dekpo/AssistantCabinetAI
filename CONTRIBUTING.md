# Contributing

This file is public. It is written in English.

Internal design notes for the project owner live in French under `docs/` (except `docs/user/`). They stay **on the local machine**. They are **not** committed. Do not mix French and English in the same document. Owner language notes: `docs/LANGUE.md` (local only).

## Language

| Surface | Language |
| --- | --- |
| Chat with the owner, local framing under `docs/` (not `docs/user/`) | French only, **not versioned** |
| Branch names, commits, pull requests | English |
| Source code, comments, APIs, tests, image names | English |
| `README.md`, this file, `AGENTS.md`, `docs/user/` | English, **versioned** |
| UI string **keys** | English (`gp-referral-letter`) |
| UI copy shown to the GP | French (in the product, not in this repo’s cadrage mix) |

## Git ownership

The human owner runs every `git` write. Agents **must not** run:

- `git commit`
- `git push`
- `git add` (unless the owner explicitly asked to stage in that message)
- `git commit --amend`, rebase, reset, force-push, or hook skips

Agents **must** propose, in English, **copy-pasteable commands** (not only a branch name):

```text
git status
git checkout -b feat/<short-topic>
git add path/to/file1 path/to/file2
git commit -m "feat: short subject in english"
git status
```

Also name:

- branch: `feat|fix|docs|chore/<short-topic>`
- Conventional Commit subject, 72 characters or fewer
- PR title and test plan when a branch is ready

If the only edits are internal `docs/` (not `docs/user/`), **do not** propose a commit. Say that the notes stay local.

## Branch names

```text
main
feat/<short-topic>
fix/<short-topic>
docs/<short-topic>
chore/<short-topic>
```

Examples: `feat/local-ollama-openwebui`, `docs/user-install-guide`.

## Commit messages

```text
feat: add Open WebUI compose for localhost
docs: describe GP sandbox fixtures
chore: ignore Ollama model caches
```

Present tense, 72 characters or fewer on the subject line. No secrets, no patient names, **no internal `docs/` files**.

## Environment

Ship a `compose.yaml` as the portable environment (Ollama + Open WebUI, later the gateway). Do not treat a native Windows-only install as the official path. Open WebUI **flags** live in Compose (`ENABLE_PERSISTENT_CONFIG=false`). Account, chats, and models live in host folder `data/` (gitignored); back up with `scripts/backup-local-data.ps1`. Owner-only notes: `docs/DOCKER.md` and `docs/REGLAGES-OPEN-WEBUI.md` (not in git).

## What is versioned

| In git (public) | Not in git (local only) |
| --- | --- |
| Source code, `compose.yaml`, tests | `docs/*` except `docs/user/` |
| `README.md`, `CONTRIBUTING.md`, `AGENTS.md` | Cadrage, audits, interview notes, next-session prompts |
| `docs/user/` (end-user guides, English) | Patient-like fixtures marked `real/` |
| `.cursor/rules/`, `.gitignore`, `.env.example` | `.env`, keys, model weight files |

Agents must never suggest `git add docs/` or `git add docs/VISION.md` and similar.

## Team workflow (when GitHub exists)

1. Owner creates the remote and pushes `main`.
2. Protect `main` (no direct push once a second person joins).
3. One branch per change; open a PR; review; merge.
4. Never commit `.env`, keys, real patient files, or chat exports.

Suggested first commands (**owner only**):

```text
git init
git add README.md CONTRIBUTING.md AGENTS.md .cursor .gitignore docs/user
git commit -m "chore: bootstrap public repo files"
```
