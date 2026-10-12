### Entity quality, tier 100 (89 documents read)

| Side | Type | Names in the library | Found by the base | Active | File links expected | File links found | File links the base made | Link recall | Link precision |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| documents | person | 78 | 78 (100.0 %) | 74 | 184 | 182 | 182 | 98.9 % | 100.0 % |
| documents | organization | 34 | 33 (97.1 %) | 29 | 90 | 86 | 86 | 95.6 % | 100.0 % |
| tables | person | 165 | 165 (100.0 %) | 165 | 211 | 199 | 199 | 94.3 % | 100.0 % |
| tables | organization | 10 | 10 (100.0 %) | 10 | 12 | 12 | 12 | 100.0 % | 100.0 % |

False merges (one base entity, two different people): **0**

Entities the base holds that match no invented name: 3 active, 3 candidate (and 11 in the surname-then-initial form of a table, which are possible matches by design). All of them:
- person / active / Le Gall / 1 doc(s), 0 workbook(s)
- person / active / El Khoury / 1 doc(s), 0 workbook(s)
- organization / candidate / cabinet du Parc / 1 doc(s), 0 workbook(s)
- organization / candidate / cabinet des Lilas / 1 doc(s), 0 workbook(s)
- person / active / Camille D'Aubigné / 1 doc(s), 1 workbook(s)
- person / candidate / d'Aubigné / 1 doc(s), 0 workbook(s)

Names in documents never found (1):
- organization: Unilabs Genève

Names in tables never found (0):

Links the base made that the generator does not know (first 25):

Recall by the shape the name is written in (files where the name appears in that one shape):

| Type | Shape | Files | Linked | Recall |
| --- | --- | --- | --- | --- |
| organization | acronym | 2 | 2 | 100.0 % |
| organization | full | 82 | 78 | 95.1 % |
| person | full | 31 | 31 | 100.0 % |
| person | hyphen_error | 1 | 1 | 100.0 % |
| person | initial_surname | 4 | 4 | 100.0 % |
| person | no_accent | 3 | 3 | 100.0 % |
| person | reordered | 5 | 5 | 100.0 % |
| person | title_full | 101 | 101 | 100.0 % |
| person | title_surname | 1 | 0 | 0.0 % |

Recall by the format of the file (names written in a form that can be linked):

| Format (type) | Names in files | Linked | Recall |
| --- | --- | --- | --- |
| docx (organization) | 19 | 19 | 100.0 % |
| docx (person) | 43 | 43 | 100.0 % |
| jpg (organization) | 2 | 1 | 50.0 % |
| jpg (person) | 3 | 3 | 100.0 % |
| md (organization) | 1 | 1 | 100.0 % |
| md (person) | 4 | 4 | 100.0 % |
| pdf (organization) | 55 | 53 | 96.4 % |
| pdf (person) | 95 | 95 | 100.0 % |
| png (organization) | 2 | 2 | 100.0 % |
| png (person) | 3 | 3 | 100.0 % |
| scan-pdf (organization) | 6 | 5 | 83.3 % |
| scan-pdf (person) | 16 | 15 | 93.8 % |
| txt (organization) | 5 | 5 | 100.0 % |
| txt (person) | 15 | 15 | 100.0 % |

Recall when the name is whole on its line, and when a line break splits it (a name counts once per file, split if its first mention there is split):

| Type | Name | Files | Linked | Recall |
| --- | --- | --- | --- | --- |
| organization | on one line | 86 | 82 | 95.3 % |
| person | on one line | 178 | 177 | 99.4 % |
| organization | split by a line break | 1 | 1 | 100.0 % |
| person | split by a line break | 1 | 1 | 100.0 % |
### Questions, tier 100, mode `Auto` (195 questions)

| Category | Questions | Answered by the folder | Documents reduced | Would reduce | Recall when reduced | Files searched, median | Files dropped, median | Irrelevant files kept | Wrong entity | Leaks | Workbooks reduced | Workbook recall |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| alias | 15 | 0 | 7 | 7 | 94.7 % | 89 | 88 | 0 | 0 | 0 | 0 | - |
| data | 10 | 2 | 4 | 4 | - | 89 | 88 | 5 | 0 | 0 | 0 | - |
| entity_free | 7 | 1 | 0 | 0 | - | 89 | - | 0 | 0 | 0 | 0 | - |
| follow_up | 2 | 0 | 2 | 2 | 40.0 % | 2 | 87 | 2 | 0 | 0 | 0 | - |
| homonym | 3 | 0 | 3 | 3 | 90.0 % | 9 | 80 | 0 | 0 | 0 | 0 | - |
| identifier | 17 | 0 | 10 | 10 | 100.0 % | 3 | 87 | 2 | 0 | 0 | 0 | - |
| leak | 5 | 0 | 5 | 5 | 100.0 % | 3 | 12 | 0 | 0 | 0 | 0 | - |
| long_document | 8 | 0 | 0 | 0 | - | 89 | - | 0 | 0 | 0 | 0 | - |
| mixed | 3 | 0 | 3 | 3 | 92.9 % | 2 | 87 | 0 | 0 | 0 | 0 | - |
| multi_entity | 6 | 0 | 3 | 3 | 100.0 % | 89 | 86 | 6 | 0 | 0 | 0 | - |
| named_file | 9 | 0 | 0 | 0 | - | 89 | - | 0 | 0 | 0 | 0 | - |
| ocr | 6 | 0 | 6 | 6 | 100.0 % | 1 | 88 | 0 | 0 | 0 | 0 | - |
| org_vs_person | 2 | 0 | 1 | 1 | 100.0 % | 89 | 88 | 0 | 0 | 0 | 0 | - |
| organization | 17 | 0 | 14 | 14 | 100.0 % | 2 | 87 | 0 | 0 | 0 | 0 | - |
| person | 54 | 0 | 51 | 51 | 100.0 % | 1 | 88 | 9 | 0 | 0 | 0 | - |
| practitioner | 14 | 0 | 8 | 8 | 46.5 % | 15 | 86 | 0 | 0 | 0 | 0 | - |
| small_scope | 2 | 0 | 0 | 0 | - | 4 | - | 0 | 0 | 0 | 0 | - |
| term | 8 | 0 | 0 | 0 | - | 89 | - | 0 | 0 | 0 | 0 | - |
| unknown | 7 | 0 | 0 | 0 | - | 89 | - | 0 | 0 | 0 | 0 | - |

Questions whose documents were reduced: 117 of 195. Recall of the reduced set on those: **84.5 %** (251 of 297 relevant files). Median reduction: 98 %. Wrong entity picked: 0. Files outside the selection or left out on purpose that were searched: 0.
Why a question was not reduced: {"-": 117, "ambiguous": 10, "named_file": 15, "no_candidates": 7, "no_discriminating_entity": 7, "no_entity": 34, "router": 3, "small_scope": 2}
Knowledge lookup: median 3 ms, p95 6 ms, max 11 ms.

Relevant files lost by a reduced set, by reason: {"capped": 38, "carry_over": 3, "weak_form_only": 5}

Questions where the reduced set lost a relevant file (7):
- Q065 [homonym] `Quels documents concernent Jean Dupont ?` 9/10 found; lost: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (weak_form_only)
- Q066 [homonym] `Que dit le courrier du Dr Jean Dupont ?` 9/10 found; lost: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (weak_form_only)
- Q067 [homonym] `Quel est le prochain rendez-vous de Jean Dupont ?` 9/10 found; lost: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (weak_form_only)
- Q075 [alias] `Que dit Jean-Dupont ?` 9/10 found; lost: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (weak_form_only)
- Q102 [practitioner] `Quels documents concernent le Dr Hélène Marchetti ?` 15/53 found; lost: courriers-entrants/2026-04/2026-04-07_courrier-rhumatologie-baumann-christian.pdf (capped), courriers-entrants/2026-04/2026-04-18_courrier-cardiologie.pdf (capped), courriers-entrants/2026-04/2026-04-20_courrier-rhumatologie-lucas-roy.pdf (capped), courriers-entrants/2026-04/2026-04-21_courrier-endocrinologie.docx (capped), courriers-entrants/2026-04/2026-04-22_courrier-rhumatologie-dupontet-jean.pdf (capped), courriers-entrants/2026-04/2026-04-23_courrier-dermatologie.docx (capped), courriers-entrants/2026-05/2026-05-05_courrier-neurologie-martin-marie.pdf (capped), courriers-entrants/2026-06/2026-06-25_courrier-orl.docx (capped), courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (capped), courriers-entrants/2026-07/2026-07-08_courrier-dermatologie-nguyen-thi-lan.pdf (capped), courriers-entrants/2026-07/2026-07-17_courrier-compte-rendu.docx (capped), courriers-entrants/2026-07/2026-07-20_courrier-dermatologie.docx (capped), courriers-entrants/2026-07/2026-07-25_courrier-cardiologie-girard-frederic.pdf (capped), courriers-entrants/2026-08/2026-08-01_courrier-rhumatologie.pdf (capped), courriers-entrants/2026-08/2026-08-04_courrier-orl-demirci-ibrahim.docx (capped), courriers-entrants/2026-08/2026-08-17_courrier-orl-roy-lucas.md (capped), courriers-entrants/2026-10/2026-10-01_courrier-dermatologie.pdf (capped), courriers-entrants/pharmacies/2026-08-26_courrier-pharmacie.pdf (capped), documentation/guide-pratique-cabinet.pdf (capped), dupont-jean.pdf (capped), juridique/2026-02-14_courrier-etude-chappuis.pdf (capped), messagerie/2026-08-27_message.txt (capped), patients/courriers/2026-02-06_courrier-patient-zufferey-stephane.docx (capped), patients/courriers/2026-02-28_courrier-patient.txt (capped), patients/courriers/2026-03-04_courrier-patient.docx (capped), patients/courriers/2026-03-08_courrier-patient-meunier-veronique.jpg (capped), patients/courriers/2026-03-30_courrier-patient.txt (capped), patients/courriers/2026-04-22_courrier-patient-deleze-christelle.pdf (capped), patients/courriers/2026-07-18_courrier-patient-demirci-ibrahim.pdf (capped), patients/courriers/2026-07-28_courrier-patient-crettenand-gregoire.txt (capped), patients/courriers/2026-08-01_courrier-patient-tschopp-raphael.pdf (capped), patients/courriers/2026-08-12_courrier-patient-martin-claire.pdf (capped), patients/courriers/2026-09-01_courrier-patient.docx (capped), patients/courriers/2026-09-17_courrier-patient-chappuis-nathalie.txt (capped), reunions/2026-09-01_pv-reunion-cabinet.docx (capped), scans/2026-04/2026-04-14_courrier-endocrinologie.pdf (capped), scans/2026-08/2026-08-30_courrier-orl-chappuis-nathalie.pdf (capped), scans/2026-10/2026-10-07_ordonnance.png (capped)
- Q192 [mixed] `Compare les rendez-vous de septembre de Jean Dupont avec la correspondance qui le concerne.` 9/10 found; lost: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (weak_form_only)
- Q197 [follow_up] `Que contiennent les notes internes ?` 0/3 found; lost: administratif/notes/2026-02-20_note-interne-planning.docx (carry_over), administratif/notes/2026-04-30_note-interne-photocopieuse.txt (carry_over), administratif/notes/2026-08-01_note-interne-stock.md (carry_over)

Questions where the reduction was built on an entity that is not the one the question names (0):
### Questions, tier 100, mode `Suggest` (195 questions)

| Category | Questions | Answered by the folder | Documents reduced | Would reduce | Recall when reduced | Files searched, median | Files dropped, median | Irrelevant files kept | Wrong entity | Leaks | Workbooks reduced | Workbook recall |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| alias | 15 | 0 | 0 | 7 | - | 89 | - | 0 | 0 | 0 | 0 | - |
| data | 10 | 2 | 0 | 4 | - | 89 | - | 0 | 0 | 0 | 0 | - |
| entity_free | 7 | 1 | 0 | 0 | - | 89 | - | 0 | 0 | 0 | 0 | - |
| follow_up | 2 | 0 | 0 | 2 | - | 89 | - | 0 | 0 | 0 | 0 | - |
| homonym | 3 | 0 | 0 | 3 | - | 89 | - | 0 | 0 | 0 | 0 | - |
| identifier | 17 | 0 | 0 | 10 | - | 89 | - | 0 | 0 | 0 | 0 | - |
| leak | 5 | 0 | 0 | 5 | - | 15 | - | 0 | 0 | 0 | 0 | - |
| long_document | 8 | 0 | 0 | 0 | - | 89 | - | 0 | 0 | 0 | 0 | - |
| mixed | 3 | 0 | 0 | 3 | - | 89 | - | 0 | 0 | 0 | 0 | - |
| multi_entity | 6 | 0 | 0 | 3 | - | 89 | - | 0 | 0 | 0 | 0 | - |
| named_file | 9 | 0 | 0 | 0 | - | 89 | - | 0 | 0 | 0 | 0 | - |
| ocr | 6 | 0 | 0 | 6 | - | 89 | - | 0 | 0 | 0 | 0 | - |
| org_vs_person | 2 | 0 | 0 | 1 | - | 89 | - | 0 | 0 | 0 | 0 | - |
| organization | 17 | 0 | 0 | 14 | - | 89 | - | 0 | 0 | 0 | 0 | - |
| person | 54 | 0 | 0 | 51 | - | 89 | - | 0 | 0 | 0 | 0 | - |
| practitioner | 14 | 0 | 0 | 8 | - | 89 | - | 0 | 0 | 0 | 0 | - |
| small_scope | 2 | 0 | 0 | 0 | - | 4 | - | 0 | 0 | 0 | 0 | - |
| term | 8 | 0 | 0 | 0 | - | 89 | - | 0 | 0 | 0 | 0 | - |
| unknown | 7 | 0 | 0 | 0 | - | 89 | - | 0 | 0 | 0 | 0 | - |

Questions whose documents were reduced: 0 of 195. Recall of the reduced set on those: **-** (0 of 0 relevant files). Median reduction: -. Wrong entity picked: 0. Files outside the selection or left out on purpose that were searched: 0.
Why a question was not reduced: {"-": 117, "ambiguous": 10, "named_file": 15, "no_candidates": 7, "no_discriminating_entity": 7, "no_entity": 34, "router": 3, "small_scope": 2}
Knowledge lookup: median 3 ms, p95 6 ms, max 10 ms.

Relevant files lost by a reduced set, by reason: {}

Questions where the reduced set lost a relevant file (0):

Questions where the reduction was built on an entity that is not the one the question names (0):
