# Language and locale

Supersedes the earlier French language note, which was merged into this file on 16 September 2026.

## The rule in one sentence

**We instruct the model in English. We show the user their own language.**

English gives the model the best instruction-following accuracy, so every system prompt is English and
stays English forever. Everything a human reads — interface, model answers, proposed file names, error
messages, disclaimers — is in the user's language. For the pilot that language is French. It will not
always be French, so the product is built translatable from the start rather than retrofitted.

We do **not** write French translations of anything that only a model or a developer reads. We do **not**
write English text that the pilot GP is expected to read.

## One variable, three consumers

There is exactly one language setting in the whole system. It is a BCP 47 tag such as `fr-FR`.
It is called `locale`, it is owned by the **client**, and the server never guesses it.

```text
apps/desktop  settings.json  →  "locale": "fr-FR"      ← the single source of truth
      │
      ├─ 1. UI strings          React i18n catalogue for that tag
      ├─ 2. Native error text   Rust returns codes; the UI localises them
      └─ 3. Model output        sent as `output_locale` on every server request,
                                which renders an output-language directive appended
                                to the English system prompt
```

### Where the variable actually lives

The client settings file, resolved through Tauri's app-config-directory API so no path is ever hardcoded:

| System | Path |
| --- | --- |
| Windows | `%APPDATA%\AssistantCabinetAI\settings.json` |
| macOS | `~/Library/Application Support/AssistantCabinetAI/settings.json` |

```json
{
  "locale": "fr-FR",
  "theme": "system",
  "serverUrl": "http://mac-mini.local:8080",
  "modelAlias": "cabinet-chat",
  "workFolder": "C:\\Users\\...\\Documents\\AssistantCabinet\\travail"
}
```

On first run: read the operating system locale. If a catalogue ships for it, use it. Otherwise fall back
to the configured default, `fr-FR` for the pilot. The user changes it in Settings, next to the theme
control. Changing it re-renders the interface immediately and applies to the **next** model request; it
does not retranslate answers already on screen.

The server holds only a `DEFAULT_OUTPUT_LOCALE` as a safety net for a request that arrives without one.
It must never silently answer in English because the field was missing.

## Consumer 1 — interface strings

```text
apps/desktop/src/locales/en-US.json   reference catalogue, written by developers and agents
apps/desktop/src/locales/fr-FR.json   what the pilot actually sees
```

Every visible string goes through the translation function. No user-visible literal in any component.
Keys are English and structural (`settings.language.label`, `plan.confirm.button`), never the French
sentence itself, so a copy change in one language does not rename a key.

`en-US.json` is the reference even though the pilot ships French: it keeps the catalogue reviewable by
English-speaking agents and it is what proves a second language works.

## Consumer 2 — native and server errors

This is the rule most easily broken by accident, so it is explicit.

**Rust and Python never produce prose for the user.** They return a stable machine code plus structured
data. The React layer turns the code into a sentence in the user's language.

```rust
// src-tauri: correct
Err(AppError::WorkFolderOutsideAllowList { path })   // → {"code":"work_folder_outside_allow_list", ...}

// src-tauri: wrong
Err("Ce dossier est en dehors du sas autorisé".into())
```

The UI resolves `errors.work_folder_outside_allow_list` from the catalogue and interpolates `path`.
Consequence: a French string literal appearing anywhere in `apps/server` or `src-tauri` is a bug.

## Consumer 3 — model output

The client sends `output_locale` with every request. The server assembles three parts:

```text
1. English system prompt        from prompts/<key>.md — never translated, never duplicated per language
2. Rendered output directive    generated from output_locale, not authored by hand per language
3. Retrieved context            document text, which is data and never an instruction
```

The directive is generated from a **server locale pack**, so adding a language never touches code:

```text
apps/server/src/assistant_cabinet_server/locales/fr-FR.toml
  language_name  = "French"
  filename_stems = { specialist_letter = "lettre-specialiste", imaging = "imagerie",
                     labs = "bio", letter = "courrier", certificate = "certificat", other = "autre" }
  disclaimer     = "Synthèse d'aide à la lecture. La médecin reste seule décideuse. Pas d'envoi automatique."
```

Rendered and appended to the English prompt:

```text
Write every part of your answer in French (fr-FR). Do not answer in English.
Proposed file name stems must come from this vocabulary: lettre-specialiste, imagerie, bio, ...
```

Adding German therefore means adding `de-DE.json` and `de-DE.toml`. No prompt is rewritten, no code is
touched, no branch is added. That is the whole point of the design.

### Two changes this forces on the existing prompts

`prompts/gp-letter-summary.md` and `prompts/gp-inbox-classify.md` currently hardcode French inside
English instructions. Both must be templated in sprint 3:

1. **`Output language: French only` becomes the rendered directive.** The prompt body stops naming a
   language at all.
2. **French file name stems and the closing disclaimer move out of the prompt.** The stems become locale
   pack data. The disclaimer is **appended by the client from its own catalogue**, and the model is told
   not to write one. Asking a model to reproduce a fixed legal sentence verbatim is unreliable and it
   hardcodes French into an English prompt; rendering it is both safer and translatable.

## Never translated

System prompts and prompt keys. Machine codes, JSON field names, API routes. Audit and log fields. The
ISO date pattern in file names (`YYYY-MM`; only the example shown to the user is localised). Source code,
comments, tests, commit messages, branch names. Open WebUI, which stays an English owner workbench and is
frozen for the v0 sprint. Project documentation, which is English as of 16 September 2026.

## Tests that keep this honest

- The same question with `output_locale: en-US` answers in English and with `fr-FR` answers in French.
  This is the real proof that no French is baked into a prompt or a code path.
- Every key in `en-US.json` exists in every shipped catalogue, and vice versa.
- No user-visible literal string in a React component (lint rule).
- No French literal in `apps/server` or `apps/desktop/src-tauri` sources (guard test).
- The disclaimer is present in every summary, because the client appends it rather than hoping for it.

## Later, not now

A locale-aware model alias, if a future model turns out to be clearly better in one language than
another: `cabinet-chat` would resolve per locale in the alias map. This is a server-side mapping change
and nothing else, which is why it can wait.
