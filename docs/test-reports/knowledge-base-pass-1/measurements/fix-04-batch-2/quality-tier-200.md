### Entity quality, tier 200 (176 documents read)

| Side | Type | Names in the library | Found by the base | Active | File links expected | File links found | File links the base made | Link recall | Link precision |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| documents | person | 104 | 103 (99.0 %) | 87 | 326 | 314 | 314 | 96.3 % | 100.0 % |
| documents | organization | 50 | 49 (98.0 %) | 46 | 193 | 184 | 184 | 95.3 % | 100.0 % |
| tables | person | 165 | 165 (100.0 %) | 165 | 211 | 199 | 199 | 94.3 % | 100.0 % |
| tables | organization | 10 | 10 (100.0 %) | 10 | 12 | 12 | 12 | 100.0 % | 100.0 % |

False merges (one base entity, two different people): **0**

Entities the base holds that match no invented name: 2 active, 3 candidate (and 11 in the surname-then-initial form of a table, which are possible matches by design). All of them:
- person / active / Le Gall / 1 doc(s), 0 workbook(s)
- organization / candidate / cabinet du Parc / 1 doc(s), 0 workbook(s)
- organization / candidate / cabinet des Lilas / 1 doc(s), 0 workbook(s)
- person / active / Camille D'Aubigné / 1 doc(s), 1 workbook(s)
- person / candidate / d'Aubigné / 1 doc(s), 0 workbook(s)

Names in documents never found (2):
- person: Dominique Denis
- organization: Unilabs Genève

Names in tables never found (0):

Links the base made that the generator does not know (first 25):

Recall by the shape the name is written in (files where the name appears in that one shape):

| Type | Shape | Files | Linked | Recall |
| --- | --- | --- | --- | --- |
| organization | acronym | 2 | 2 | 100.0 % |
| organization | full | 184 | 175 | 95.1 % |
| person | full | 67 | 66 | 98.5 % |
| person | hyphen_error | 1 | 1 | 100.0 % |
| person | initial_surname | 5 | 5 | 100.0 % |
| person | no_accent | 3 | 3 | 100.0 % |
| person | reordered | 6 | 6 | 100.0 % |
| person | surname_only | 1 | 0 | 0.0 % |
| person | title_full | 178 | 178 | 100.0 % |
| person | title_surname | 9 | 0 | 0.0 % |

Recall by the format of the file (names written in a form that can be linked):

| Format (type) | Names in files | Linked | Recall |
| --- | --- | --- | --- |
| docx (organization) | 31 | 31 | 100.0 % |
| docx (person) | 60 | 60 | 100.0 % |
| jpg (organization) | 13 | 12 | 92.3 % |
| jpg (person) | 15 | 14 | 93.3 % |
| md (organization) | 2 | 2 | 100.0 % |
| md (person) | 9 | 9 | 100.0 % |
| pdf (organization) | 121 | 114 | 94.2 % |
| pdf (person) | 173 | 173 | 100.0 % |
| png (organization) | 7 | 7 | 100.0 % |
| png (person) | 5 | 5 | 100.0 % |
| scan-pdf (organization) | 13 | 12 | 92.3 % |
| scan-pdf (person) | 24 | 23 | 95.8 % |
| txt (organization) | 6 | 6 | 100.0 % |
| txt (person) | 25 | 25 | 100.0 % |

Recall when the name is whole on its line, and when a line break splits it (a name counts once per file, split if its first mention there is split):

| Type | Name | Files | Linked | Recall |
| --- | --- | --- | --- | --- |
| organization | on one line | 177 | 171 | 96.6 % |
| person | on one line | 310 | 308 | 99.4 % |
| organization | split by a line break | 5 | 2 | 40.0 % |
| person | split by a line break | 1 | 1 | 100.0 % |
### Questions, tier 200, mode `Auto` (205 questions)

| Category | Questions | Answered by the folder | Documents reduced | Would reduce | Recall when reduced | Files searched, median | Files dropped, median | Irrelevant files kept | Wrong entity | Leaks | Workbooks reduced | Workbook recall |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| alias | 15 | 0 | 7 | 7 | 96.4 % | 176 | 175 | 0 | 0 | 0 | 0 | - |
| data | 10 | 2 | 4 | 4 | - | 176 | 175 | 5 | 0 | 0 | 0 | - |
| entity_free | 7 | 1 | 0 | 0 | - | 176 | - | 0 | 0 | 0 | 0 | - |
| follow_up | 2 | 0 | 2 | 2 | 22.2 % | 2 | 174 | 2 | 0 | 0 | 0 | - |
| homonym | 3 | 0 | 3 | 3 | 93.8 % | 15 | 161 | 0 | 0 | 0 | 0 | - |
| identifier | 22 | 0 | 15 | 15 | 100.0 % | 2 | 175 | 2 | 0 | 0 | 0 | - |
| leak | 5 | 0 | 4 | 4 | 100.0 % | 5 | 14 | 0 | 0 | 0 | 0 | - |
| long_document | 8 | 0 | 0 | 0 | - | 176 | - | 0 | 0 | 0 | 0 | - |
| mixed | 3 | 0 | 3 | 3 | 95.8 % | 7 | 169 | 1 | 0 | 0 | 0 | - |
| multi_entity | 6 | 0 | 3 | 3 | 100.0 % | 176 | 172 | 9 | 0 | 0 | 0 | - |
| named_file | 9 | 0 | 0 | 0 | - | 176 | - | 0 | 0 | 0 | 0 | - |
| ocr | 6 | 0 | 6 | 6 | 100.0 % | 1 | 175 | 0 | 0 | 0 | 0 | - |
| org_vs_person | 3 | 0 | 2 | 2 | 100.0 % | 3 | 173 | 0 | 0 | 0 | 0 | - |
| organization | 19 | 0 | 16 | 16 | 100.0 % | 4 | 173 | 0 | 0 | 0 | 0 | - |
| person | 55 | 0 | 52 | 52 | 100.0 % | 2 | 174 | 17 | 0 | 0 | 0 | - |
| practitioner | 15 | 0 | 7 | 7 | 100.0 % | 176 | 173 | 0 | 0 | 0 | 0 | - |
| small_scope | 2 | 0 | 0 | 0 | - | 4 | - | 0 | 0 | 0 | 0 | - |
| term | 8 | 0 | 0 | 0 | - | 176 | - | 0 | 0 | 0 | 0 | - |
| unknown | 7 | 0 | 0 | 0 | - | 176 | - | 0 | 0 | 0 | 0 | - |

Questions whose documents were reduced: 124 of 205. Recall of the reduced set on those: **96.7 %** (348 of 360 relevant files). Median reduction: 99 %. Wrong entity picked: 0. Files outside the selection or left out on purpose that were searched: 0.
Why a question was not reduced: {"-": 124, "ambiguous": 12, "named_file": 15, "no_candidates": 7, "no_discriminating_entity": 7, "no_entity": 35, "router": 3, "small_scope": 2}
Knowledge lookup: median 3 ms, p95 7 ms, max 12 ms.

Relevant files lost by a reduced set, by reason: {"carry_over": 7, "weak_form_only": 5}

Questions where the reduced set lost a relevant file (6):
- Q065 [homonym] `Quels documents concernent Jean Dupont ?` 15/16 found; lost: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (weak_form_only)
- Q066 [homonym] `Que dit le courrier du Dr Jean Dupont ?` 15/16 found; lost: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (weak_form_only)
- Q067 [homonym] `Quel est le prochain rendez-vous de Jean Dupont ?` 15/16 found; lost: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (weak_form_only)
- Q075 [alias] `Que dit Jean-Dupont ?` 15/16 found; lost: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (weak_form_only)
- Q192 [mixed] `Compare les rendez-vous de septembre de Jean Dupont avec la correspondance qui le concerne.` 15/16 found; lost: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (weak_form_only)
- Q197 [follow_up] `Que contiennent les notes internes ?` 0/7 found; lost: administratif/notes/2026-02-20_note-interne-planning.docx (carry_over), administratif/notes/2026-04-30_note-interne-photocopieuse.txt (carry_over), administratif/notes/2026-06-23_note-interne-fermeture.txt (carry_over), administratif/notes/2026-08-01_note-interne-stock.md (carry_over), administratif/notes/2026-08-08_note-interne-photocopieuse.txt (carry_over), administratif/notes/2026-09-12_note-interne-informatique.txt (carry_over), administratif/notes/2026-09-27_note-interne-fermeture.txt (carry_over)

Questions where the reduction was built on an entity that is not the one the question names (0):
### Questions, tier 200, mode `Suggest` (205 questions)

| Category | Questions | Answered by the folder | Documents reduced | Would reduce | Recall when reduced | Files searched, median | Files dropped, median | Irrelevant files kept | Wrong entity | Leaks | Workbooks reduced | Workbook recall |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| alias | 15 | 0 | 0 | 7 | - | 176 | - | 0 | 0 | 0 | 0 | - |
| data | 10 | 2 | 0 | 4 | - | 176 | - | 0 | 0 | 0 | 0 | - |
| entity_free | 7 | 1 | 0 | 0 | - | 176 | - | 0 | 0 | 0 | 0 | - |
| follow_up | 2 | 0 | 0 | 2 | - | 176 | - | 0 | 0 | 0 | 0 | - |
| homonym | 3 | 0 | 0 | 3 | - | 176 | - | 0 | 0 | 0 | 0 | - |
| identifier | 22 | 0 | 0 | 15 | - | 176 | - | 0 | 0 | 0 | 0 | - |
| leak | 5 | 0 | 0 | 4 | - | 19 | - | 0 | 0 | 0 | 0 | - |
| long_document | 8 | 0 | 0 | 0 | - | 176 | - | 0 | 0 | 0 | 0 | - |
| mixed | 3 | 0 | 0 | 3 | - | 176 | - | 0 | 0 | 0 | 0 | - |
| multi_entity | 6 | 0 | 0 | 3 | - | 176 | - | 0 | 0 | 0 | 0 | - |
| named_file | 9 | 0 | 0 | 0 | - | 176 | - | 0 | 0 | 0 | 0 | - |
| ocr | 6 | 0 | 0 | 6 | - | 176 | - | 0 | 0 | 0 | 0 | - |
| org_vs_person | 3 | 0 | 0 | 2 | - | 176 | - | 0 | 0 | 0 | 0 | - |
| organization | 19 | 0 | 0 | 16 | - | 176 | - | 0 | 0 | 0 | 0 | - |
| person | 55 | 0 | 0 | 52 | - | 176 | - | 0 | 0 | 0 | 0 | - |
| practitioner | 15 | 0 | 0 | 7 | - | 176 | - | 0 | 0 | 0 | 0 | - |
| small_scope | 2 | 0 | 0 | 0 | - | 4 | - | 0 | 0 | 0 | 0 | - |
| term | 8 | 0 | 0 | 0 | - | 176 | - | 0 | 0 | 0 | 0 | - |
| unknown | 7 | 0 | 0 | 0 | - | 176 | - | 0 | 0 | 0 | 0 | - |

Questions whose documents were reduced: 0 of 205. Recall of the reduced set on those: **-** (0 of 0 relevant files). Median reduction: -. Wrong entity picked: 0. Files outside the selection or left out on purpose that were searched: 0.
Why a question was not reduced: {"-": 124, "ambiguous": 12, "named_file": 15, "no_candidates": 7, "no_discriminating_entity": 7, "no_entity": 35, "router": 3, "small_scope": 2}
Knowledge lookup: median 3 ms, p95 7 ms, max 11 ms.

Relevant files lost by a reduced set, by reason: {}

Questions where the reduced set lost a relevant file (0):

Questions where the reduction was built on an entity that is not the one the question names (0):
