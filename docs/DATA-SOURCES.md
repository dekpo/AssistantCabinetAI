# Data sources and their licences

Written for the owner and for any agent that adds reference data. English.

Reference data (lists of names, vocabularies) used to test or to feed the product. Models have their own register, `models/LICENSES.md`. The rule is the same: **free licences only**, and the licence is checked and written here **before** a file derived from the source is added to the repository.

## What the licences ask of us

| Source | Licence | What we must do | Status |
| --- | --- | --- | --- |
| Insee, "Fichier des noms" (surnames by decade of birth, 1891-2000), page 3536630, published 22 May 2018 | Licence Ouverte 2.0 (Etalab), the default licence of the Insee site (legal notice, `insee.fr/fr/information/2008466`). The dataset page itself states no licence: re-read the legal notice when a file is actually added. | Cite the source ("Source : Insee"), give the date of the update, do not suggest Insee endorses the product, do not alter the meaning of the data. Commercial reuse is allowed. | Candidate. Nothing derived is in the repository yet. |
| Insee, "Fichier des prénoms" (first names, 1900-2024), page 8894961, published 9 July 2025 | Same | Same. Counts are rounded to the nearest 5 by Insee. | Candidate. Nothing derived is in the repository yet. |
| US Census Bureau, "Frequently Occurring Surnames from the 2010 Census" | Work of the US federal government (17 U.S.C. section 105). The page itself states no licence. | Cite the Census Bureau as the source. | Later, English only. |
| US Social Security Administration, "Baby names from Social Security card applications, national data" | CC0 (data.gov catalogue) | None; we cite it anyway. | Later, English only. |

Where the citation goes, once any derived file is versioned: a "Sources" section of the README, and an "About" page of the application. The About page does not exist yet; its text is a user-facing string, so it is French and goes through the locale catalogues like every other one.

## Rules for using these sources

- The full files are never versioned (about 8 MB and 13 MB). Only a small derived sample is, with the citation above next to it.
- Aggregated surnames and first names are not a patient record. Nothing in these files identifies a person. This does not change the rule that no real patient file enters the repository.
- Not verified when this was written: the exact wording of the licence on each Insee dataset page, and the encoding of the first-name file (it did not read as UTF-8 in a first look).
