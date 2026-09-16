# Contributing

Written in English, like the whole repository.

`docs/` is the versioned English specification. Two exceptions stay on the local machine and are **not** committed: `docs/private/` (personal context about the pilot, purchase logistics, commercial notes) and `docs/SESSION-*.md` (Cursor tab handoffs). Never mix two languages in one document.

## Language

The rule: **instruct the model in English, show the user their own language.** Full design, including where the `locale` variable lives and how a language is added: `docs/LANGUAGE-AND-LOCALE.md`.

| Surface | Language |
| --- | --- |
| Instructions **to** the model (`prompts/`) and prompt keys | English, **versioned** |
| Everything a user reads in the product: interface, model output, proposed file names, errors, disclaimers | The user's `locale` — French for the pilot |
| Open WebUI (owner workbench) | English |
| All documentation under `docs/` | English |
| Branch names, commits, pull requests | English |
| Source code, comments, APIs, tests, image names, machine error codes | English |
| `README.md`, this file, `AGENTS.md` | English, **versioned** |
| `docs/user/` | English when the reader is the project owner (for example the Compose install guide); the user's language when the reader is the GP |

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

If the only edits are in `docs/private/` or a `docs/SESSION-*.md` handoff, **do not** propose a commit. Say that those notes stay local.

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

Ship a `compose.yaml` as the portable environment (Ollama + Open WebUI, later the gateway). Do not treat a native Windows-only install as the official path. Open WebUI **flags** live in Compose (`ENABLE_PERSISTENT_CONFIG=false`). Account, chats, and models live in host folder `data/` (gitignored); back up with `scripts/backup-local-data.ps1`. Details: `docs/OPERATIONS.md`.

## What is versioned

| In git | Not in git (local only) |
| --- | --- |
| Source code, `compose.yaml`, `prompts/`, tests | `docs/private/` (pilot personal context, purchase, commercial) |
| `docs/` — the English specification | `docs/SESSION-*.md` (tab handoffs) |
| `README.md`, `CONTRIBUTING.md`, `AGENTS.md`, `docs/user/` | `.env`, keys, model weight files, `data/`, `backups/` |
| `.cursor/rules/`, `.gitignore`, `.env.example` | Patient-like fixtures marked `real/` |

The remote is `github.com/dekpo/AssistantCabinetAI`, **private**, and stays private through the pilot. Publication is reconsidered only after real-world validation, so do not treat versioned files as already public. Open WebUI copies created in the UI live in `data/` (not git): do not invent a commit for that paste. Versioned prompt text is `prompts/`.

When the owner asks for another Cursor tab: write `docs/SESSION-<topic>.md` locally and point to it. Do not dump a long handoff only in chat.

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
