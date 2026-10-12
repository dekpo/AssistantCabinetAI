### Entity quality, tier 30 (28 documents read)

| Side | Type | Names in the library | Found by the base | Active | File links expected | File links found | File links the base made | Link recall | Link precision |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| documents | person | 26 | 26 (100.0 %) | 25 | 54 | 52 | 52 | 96.3 % | 100.0 % |
| documents | organization | 19 | 12 (63.2 %) | 9 | 29 | 20 | 20 | 69.0 % | 100.0 % |
| tables | person | 165 | 165 (100.0 %) | 165 | 211 | 201 | 203 | 95.3 % | 99.0 % |
| tables | organization | 10 | 10 (100.0 %) | 10 | 12 | 12 | 12 | 100.0 % | 100.0 % |

False merges (one base entity, two different people): **2**
- Lucas Roy (active) holds p-lucas-roy + p-roy-lucas - 1 document(s), 2 workbook(s)
- Thomas Pierre (active) holds p-pierre-thomas + p-thomas-pierre - 0 document(s), 2 workbook(s)

Entities the base holds that match no invented name: 2 active, 2 candidate (and 11 in the surname-then-initial form of a table, which are possible matches by design). All of them:
- person / active / l'Hôtel de Ville / 2 doc(s), 0 workbook(s)
- organization / active / Entre Bureautique Léman SA / 1 doc(s), 0 workbook(s)
- organization / candidate / Étude Chappuis / 1 doc(s), 0 workbook(s)
- person / candidate / Dupont Immobilier / 1 doc(s), 0 workbook(s)

Names in documents never found (7):
- organization: Cerballiance Alpes
- organization: Centre hospitalier universitaire vaudois
- organization: Dupont Immobilier SA
- organization: Étude Chappuis & Associés
- organization: Hospices Civils de Lyon
- organization: Hôpitaux universitaires de Genève
- organization: Unilabs Genève

Names in tables never found (0):

Links the base made that the generator does not know (first 25):
- Lucas Roy -> patients-ch.xlsx
- Roy Lucas -> patients-fr.csv

Recall by the shape the name is written in (files where the name appears in that one shape):

| Type | Shape | Files | Linked | Recall |
| --- | --- | --- | --- | --- |
| organization | full | 27 | 20 | 74.1 % |
| person | full | 11 | 11 | 100.0 % |
| person | initial_surname | 1 | 0 | 0.0 % |
| person | title_full | 30 | 30 | 100.0 % |
| person | title_surname | 1 | 0 | 0.0 % |

Recall by the format of the file (names written in a form that can be linked):

| Format (type) | Names in files | Linked | Recall |
| --- | --- | --- | --- |
| docx (organization) | 5 | 3 | 60.0 % |
| docx (person) | 11 | 11 | 100.0 % |
| jpg (organization) | 1 | 1 | 100.0 % |
| jpg (person) | 1 | 1 | 100.0 % |
| md (organization) | 1 | 1 | 100.0 % |
| md (person) | 4 | 4 | 100.0 % |
| pdf (organization) | 18 | 11 | 61.1 % |
| pdf (person) | 25 | 25 | 100.0 % |
| png (organization) | 1 | 1 | 100.0 % |
| png (person) | 2 | 2 | 100.0 % |
| scan-pdf (organization) | 1 | 1 | 100.0 % |
| scan-pdf (person) | 3 | 3 | 100.0 % |
| txt (organization) | 2 | 2 | 100.0 % |
| txt (person) | 7 | 7 | 100.0 % |

Recall when the name is whole on its line, and when a line break splits it (a name counts once per file, split if its first mention there is split):

| Type | Name | Files | Linked | Recall |
| --- | --- | --- | --- | --- |
| organization | on one line | 28 | 20 | 71.4 % |
| person | on one line | 53 | 53 | 100.0 % |
| organization | split by a line break | 1 | 0 | 0.0 % |
### Questions, tier 30, mode `Auto` (127 questions)

| Category | Questions | Answered by the folder | Documents reduced | Would reduce | Recall when reduced | Files searched, median | Files dropped, median | Irrelevant files kept | Wrong entity | Leaks | Workbooks reduced | Workbook recall |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| alias | 8 | 0 | 2 | 2 | 75.0 % | 28 | 25 | 0 | 0 | 0 | 0 | - |
| data | 10 | 2 | 2 | 2 | - | 28 | 26 | 5 | 0 | 0 | 0 | - |
| entity_free | 5 | 1 | 0 | 0 | - | 28 | - | 0 | 0 | 0 | 0 | - |
| follow_up | 2 | 0 | 2 | 2 | 33.3 % | 1 | 27 | 1 | 0 | 0 | 0 | - |
| homonym | 3 | 0 | 3 | 3 | 75.0 % | 3 | 25 | 0 | 0 | 0 | 0 | - |
| identifier | 16 | 0 | 10 | 10 | 100.0 % | 2 | 27 | 1 | 0 | 0 | 0 | - |
| leak | 3 | 0 | 0 | 0 | - | 6 | - | 0 | 0 | 0 | 0 | - |
| mixed | 3 | 0 | 3 | 3 | 85.7 % | 2 | 26 | 0 | 0 | 0 | 0 | - |
| multi_entity | 6 | 0 | 6 | 6 | 100.0 % | 3 | 26 | 14 | 0 | 0 | 0 | - |
| named_file | 3 | 0 | 0 | 0 | - | 28 | - | 0 | 0 | 0 | 0 | - |
| ocr | 4 | 0 | 4 | 4 | 75.0 % | 1 | 27 | 1 | 0 | 0 | 0 | - |
| org_vs_person | 2 | 0 | 0 | 0 | - | 28 | - | 0 | 0 | 0 | 0 | - |
| organization | 15 | 0 | 3 | 3 | 100.0 % | 28 | 27 | 0 | 0 | 0 | 0 | - |
| person | 21 | 0 | 19 | 19 | 95.2 % | 1 | 27 | 17 | 0 | 0 | 0 | - |
| practitioner | 12 | 0 | 7 | 7 | 100.0 % | 3 | 27 | 0 | 0 | 0 | 0 | - |
| small_scope | 1 | 0 | 0 | 0 | - | 3 | - | 0 | 0 | 0 | 0 | - |
| term | 6 | 0 | 0 | 0 | - | 28 | - | 0 | 0 | 0 | 0 | - |
| unknown | 7 | 0 | 0 | 0 | - | 28 | - | 0 | 0 | 0 | 0 | - |

Questions whose documents were reduced: 61 of 127. Recall of the reduced set on those: **88.5 %** (77 of 87 relevant files). Median reduction: 96 %. Wrong entity picked: 0. Files outside the selection or left out on purpose that were searched: 0.
Why a question was not reduced: {"-": 61, "ambiguous": 9, "named_file": 3, "no_candidates": 8, "no_discriminating_entity": 11, "no_entity": 28, "router": 3, "small_scope": 4}
Knowledge lookup: median 3 ms, p95 7 ms, max 10 ms.

Relevant files lost by a reduced set, by reason: {"carry_over": 2, "weak_form_only": 8}

Questions where the reduced set lost a relevant file (9):
- Q025 [person] `Y a-t-il des résultats d'analyses pour Véronique Meunier ?` 0/1 found; lost: patients/courriers/2026-03-08_courrier-patient-meunier-veronique.jpg (weak_form_only)
- Q065 [homonym] `Quels documents concernent Jean Dupont ?` 3/4 found; lost: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (weak_form_only)
- Q066 [homonym] `Que dit le courrier du Dr Jean Dupont ?` 3/4 found; lost: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (weak_form_only)
- Q067 [homonym] `Quel est le prochain rendez-vous de Jean Dupont ?` 3/4 found; lost: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (weak_form_only)
- Q074 [alias] `Que dit DUPONT Jean ?` 3/4 found; lost: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (weak_form_only)
- Q075 [alias] `Que dit Jean-Dupont ?` 3/4 found; lost: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (weak_form_only)
- Q177 [ocr] `Que dit le courrier de Véronique Meunier ?` 0/1 found; lost: patients/courriers/2026-03-08_courrier-patient-meunier-veronique.jpg (weak_form_only)
- Q192 [mixed] `Compare les rendez-vous de septembre de Jean Dupont avec la correspondance qui le concerne.` 3/4 found; lost: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (weak_form_only)
- Q197 [follow_up] `Que contiennent les notes internes ?` 0/2 found; lost: administratif/notes/2026-04-30_note-interne-photocopieuse.txt (carry_over), administratif/notes/2026-08-01_note-interne-stock.md (carry_over)

Questions where the reduction was built on an entity that is not the one the question names (0):
### Questions, tier 30, mode `Suggest` (127 questions)

| Category | Questions | Answered by the folder | Documents reduced | Would reduce | Recall when reduced | Files searched, median | Files dropped, median | Irrelevant files kept | Wrong entity | Leaks | Workbooks reduced | Workbook recall |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| alias | 8 | 0 | 0 | 2 | - | 28 | - | 0 | 0 | 0 | 0 | - |
| data | 10 | 2 | 0 | 2 | - | 28 | - | 0 | 0 | 0 | 0 | - |
| entity_free | 5 | 1 | 0 | 0 | - | 28 | - | 0 | 0 | 0 | 0 | - |
| follow_up | 2 | 0 | 0 | 2 | - | 28 | - | 0 | 0 | 0 | 0 | - |
| homonym | 3 | 0 | 0 | 3 | - | 28 | - | 0 | 0 | 0 | 0 | - |
| identifier | 16 | 0 | 0 | 10 | - | 28 | - | 0 | 0 | 0 | 0 | - |
| leak | 3 | 0 | 0 | 0 | - | 6 | - | 0 | 0 | 0 | 0 | - |
| mixed | 3 | 0 | 0 | 3 | - | 28 | - | 0 | 0 | 0 | 0 | - |
| multi_entity | 6 | 0 | 0 | 6 | - | 28 | - | 0 | 0 | 0 | 0 | - |
| named_file | 3 | 0 | 0 | 0 | - | 28 | - | 0 | 0 | 0 | 0 | - |
| ocr | 4 | 0 | 0 | 4 | - | 28 | - | 0 | 0 | 0 | 0 | - |
| org_vs_person | 2 | 0 | 0 | 0 | - | 28 | - | 0 | 0 | 0 | 0 | - |
| organization | 15 | 0 | 0 | 3 | - | 28 | - | 0 | 0 | 0 | 0 | - |
| person | 21 | 0 | 0 | 19 | - | 28 | - | 0 | 0 | 0 | 0 | - |
| practitioner | 12 | 0 | 0 | 7 | - | 28 | - | 0 | 0 | 0 | 0 | - |
| small_scope | 1 | 0 | 0 | 0 | - | 3 | - | 0 | 0 | 0 | 0 | - |
| term | 6 | 0 | 0 | 0 | - | 28 | - | 0 | 0 | 0 | 0 | - |
| unknown | 7 | 0 | 0 | 0 | - | 28 | - | 0 | 0 | 0 | 0 | - |

Questions whose documents were reduced: 0 of 127. Recall of the reduced set on those: **-** (0 of 0 relevant files). Median reduction: -. Wrong entity picked: 0. Files outside the selection or left out on purpose that were searched: 0.
Why a question was not reduced: {"-": 61, "ambiguous": 9, "named_file": 3, "no_candidates": 8, "no_discriminating_entity": 11, "no_entity": 28, "router": 3, "small_scope": 4}
Knowledge lookup: median 3 ms, p95 8 ms, max 13 ms.

Relevant files lost by a reduced set, by reason: {}

Questions where the reduced set lost a relevant file (0):

Questions where the reduction was built on an entity that is not the one the question names (0):
