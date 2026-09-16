# GP admin prompts (English in, French out)

Source of truth for **model instructions**. Paste the **Body** into Open WebUI (Workspace → Prompts). Keys stay English. Open WebUI UI stays English (owner workbench).

Prompt bodies are English and stay English. The **output** language is not decided here: the client sends `output_locale` and the gateway appends a rendered output-language directive. Two changes are therefore due in sprint 3, when these bodies move server-side — the hardcoded `Output language: French only` becomes that directive, and the French file name stems and closing disclaimer move into the locale pack. Contract: `docs/LANGUAGE-AND-LOCALE.md`.

| Key | Open WebUI title (EN) | Slash command | First prototype |
| --- | --- | --- | --- |
| `gp-letter-summary` | Letter summary | `/letter-summary` | **Yes** |
| `gp-inbox-classify` | Filing plan | `/filing-plan` | **Yes** |
| `gp-referral-letter` | Referral letter | `/referral-letter` | Later |
| `gp-certificate-draft` | Certificate draft | `/certificate-draft` | Later |

After editing a prompt here, recreate it in Open WebUI (stored in `data/`, not git).
