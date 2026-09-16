# GP admin prompts (English in, French out)

Source of truth for **model instructions**. Paste the **Body** into Open WebUI (Workspace → Prompts). Keys stay English. Open WebUI UI stays English (owner workbench).

All **visible model output** for the French pilot must be French, including proposed file names.

Index for Elise (French): `docs/PROMPTS-METIER.md`.

| Key | Open WebUI title (EN) | Slash command | First prototype |
| --- | --- | --- | --- |
| `gp-letter-summary` | Letter summary | `/letter-summary` | **Yes** |
| `gp-inbox-classify` | Filing plan | `/filing-plan` | **Yes** |
| `gp-referral-letter` | Referral letter | `/referral-letter` | Later |
| `gp-certificate-draft` | Certificate draft | `/certificate-draft` | Later |

After editing a prompt here, recreate it in Open WebUI (stored in `data/`, not git).
