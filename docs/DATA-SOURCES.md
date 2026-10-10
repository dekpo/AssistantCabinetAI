# Data sources and their licences

Written for the owner and for any agent that adds reference data. English.

Reference data (lists of names, vocabularies) used to test or to feed the product. Models have their own register, `models/LICENSES.md`. The rule is the same: **free licences only**, and the licence is checked and written here **before** a file derived from the source is added to the repository.

## What the licences ask of us

| Source | Licence | What we must do | Status |
| --- | --- | --- | --- |
| Insee, "Fichier des noms" (surnames by decade of birth, 1891-2000), page 3536630, published 22 May 2018 | Licence Ouverte 2.0 (Etalab), the default licence of the Insee site (legal notice, `insee.fr/fr/information/2008466`). The dataset page itself states no licence: re-read the legal notice when a file is actually added. | Cite the source ("Source : Insee"), give the date of the update, do not suggest Insee endorses the product, do not alter the meaning of the data. Commercial reuse is allowed. | In use since KB lot 2 bis (9 October 2026): `names-sample-fr-surnames.tsv`, the 300 most frequent names with their counts summed over the decades. Citation in `README.md`, "Sources". |
| Insee, "Fichier des prénoms" (first names, 1900-2024), page 8894961, published 9 July 2025 | Same | Same. Counts are rounded to the nearest 5 by Insee. | In use since KB lot 2 bis: `names-sample-fr-first-names.tsv`, the 300 most frequent names, counts summed over the periods and split by sex. Citation in `README.md`. |
| US Census Bureau, "Frequently Occurring Surnames from the 2010 Census" | Work of the US federal government (17 U.S.C. section 105). The page itself states no licence. | Cite the Census Bureau as the source. | In use since KB lot 2 bis: `names-sample-en-surnames.tsv`. Citation in `README.md`. |
| US Social Security Administration, "Baby names from Social Security card applications, national data" | CC0 (data.gov catalogue) | None; we cite it anyway. | In use since KB lot 2 bis: `names-sample-en-first-names.tsv`, counts summed over every year and split by sex. Citation in `README.md`. |

Where the citation goes: a "Sources" section of the README (done) and an "About" page of the application. The About page does not exist yet; its text is a user-facing string, so it is French and goes through the locale catalogues like every other one. Nothing in the product shows these names to a user, so the page is not blocking.

## Rules for using these sources

- The full files are never versioned (about 8 MB and 13 MB). Only a small derived sample is, with the citation above next to it.
- Aggregated surnames and first names are not a patient record. Nothing in these files identifies a person. This does not change the rule that no real patient file enters the repository.
- Checked on 9 October 2026 (KB lot 2 bis): the first-name file is UTF-8 without a byte order mark, upper case with accents (`À Â Ä Æ Ç È É Ê Ë Î Ï Ô Ö Ù Û Ü Ÿ`); the surname file is ASCII with CRLF line ends and a last row "AUTRES NOMS" that gathers every rarer name. Tools that assume another code page show the first-name file as damaged, which is what the first look saw.
- Still not verified: the wording of the licence on each Insee dataset page (the pages need a browser to render; an HTTP fetch returns no licence text). The default licence of the site is the one used; re-read the legal notice before any release.
- How the files are fetched: `scripts/fetch_name_lists.py` (Python standard library, same on Windows and macOS) downloads the four archives into `data/names/<source>/`, extracts each into its own folder (plain file names only, size caps), and prints the SHA-256. Archives seen on 9 October 2026: `noms2008nat_txt.zip` `c8693ff6...`, `prenoms-2024-nat_csv.zip` `5a61af8b...`, `names.zip` of the Census `117c41cb...`, `names.zip` of the SSA `cd78e975...` (it includes `yob2025.txt`). The SSA site returns HTTP 403 to clients that do not send browser headers; the script sends them and says how to download by hand if that stops working.
- Samples: `apps/desktop/src-tauri/tests/fixtures/knowledge/names-sample-{fr-surnames,fr-first-names,en-surnames,en-first-names}.tsv`, rewritten by `ACAI_NAMES_WRITE_SAMPLES=1 cargo test --release --test knowledge_names_calibration write_samples -- --ignored`.
