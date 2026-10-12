# Lot 10 - Activity profiles (withdrawn)

12 October 2026 · branch `feat/kb-activity-profiles` (never committed) · based on `kb-after-lot-09` (`7e2fee8`) · tab "KB 10 - Activity profiles"

Written for the developer or agent of a later lot, and for the owner. English.

## Summary

The lot was built, tested and tried by the owner, and **withdrawn entirely on her decision the same day**. Nothing of it is in the product or in `kb/integration`: this report and the decision in `docs/DECISIONS.md` ("KB lot 10, activity profiles: built, tried by the owner, withdrawn") are all that remain in git. The work itself is kept locally, outside git, in `data/withdrawn-lot-10/`. The notion of "module" (what a vocabulary pack is for, how it is chosen and switched off) comes back later, **with precise use cases**.

## What was built (so that nobody rebuilds it blind)

While an Analyse read a document or a workbook, a matcher counted, for **every shipped pack, on or off**, how many of the pack's `detection_terms` the source held (three integers per source and pack: hits, different terms, units read). A card « Profil détecté » in « Réglages » turned those counts into « Votre bibliothèque ressemble à : Santé : France 36 %, Droit : France 32 %, Comptabilité : France 27 % » with the files each figure rested on, and offered **Activer ce vocabulaire** / **Ignorer**. Accepting added the pack to `knowledgePacks` and started nothing (the next Analyse the user starts read the documents again from the stored text; the owner's rule of 11 October 2026 forbade the automatic refresh the lot file asked for). First thresholds: 20 files read and 25 % resemblance, both settings.

Verification when it was complete (all green, now moot): 1294 cargo (772 lib), 641 vitest, clippy lib 21 = baseline, build OK. Measured: counting the terms of 4 packs in 1 000 chunks of 315 characters took 17 ms (release); the walk of the human test (42 documents, 2 workbooks) took 30 to 50 ms per Analyse.

## What the owner found when she ran the human test

- **The system worked as designed**: the percentages, the buttons, the note after an acceptance, the re-reading in the next Analyse.
- **A vocabulary accepted from the card could not be switched off.** Only « Santé : France » and « Santé : Suisse » have a tick in « Réglages » (named « Module santé »); legal and accounting had none, and the card did not say that the health ticks and the health lines of the card were the same thing.
- **Doubt about the usefulness**, answered with the code: a pack changes the identifiers it recognises (NIR; case number; SIRET and VAT number), the meaning of a handful of column headers, a few never-name words and the role labels of the knowledge dialog; never an answer, a file or a scope; titles and organisation markers are read whatever is active; the pilot already has the health pack by default; and no other feature depends on a profile.

## Decision

Retire the lot entirely (owner, 12 October 2026). Recorded in `docs/DECISIONS.md`, with what stays in the code (`kb_source_signals` and `kb_profile` empty and unused, the `detection_terms` field of the pack format accepted and ignored, `SignalDraft` and `KnowledgeDelta.signals` always empty, `CURRENT_KB_VERSION` still 1) and what to remember if the notion returns.

## Defects

None added, none changed, none blocking. The one modelling limit found (the country packs of one domain cannot share their general words) is written in the decision and not registered, because the code that had it is gone.

## Human test

`human-tests/lot-10-activity-profiles.md` and its two scripts were written and **run by the owner**; they are deleted from the repository with the lot (kept in `data/withdrawn-lot-10/`). Nothing is left to run.

## Next

Lot 11 (`SESSION-KB-LOT-11-Prove-it.md`) validates lots 0 to 9; there is no lot 10 to check and no tag `kb-after-lot-10`.
