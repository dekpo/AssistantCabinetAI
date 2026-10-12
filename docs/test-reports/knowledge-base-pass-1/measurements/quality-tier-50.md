### Entity quality, tier 50 (41 documents read)

| Side | Type | Names in the library | Found by the base | Active | File links expected | File links found | File links the base made | Link recall | Link precision |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| documents | person | 33 | 31 (93.9 %) | 29 | 83 | 78 | 80 | 94.0 % | 97.5 % |
| documents | organization | 23 | 14 (60.9 %) | 12 | 41 | 27 | 27 | 65.9 % | 100.0 % |
| tables | person | 165 | 164 (99.4 %) | 164 | 211 | 200 | 202 | 94.8 % | 99.0 % |
| tables | organization | 10 | 10 (100.0 %) | 10 | 12 | 12 | 12 | 100.0 % | 100.0 % |

False merges (one base entity, two different people): **2**
- Lucas Roy (active) holds p-lucas-roy + p-roy-lucas - 2 document(s), 2 workbook(s)
- Thomas Pierre (active) holds p-pierre-thomas + p-thomas-pierre - 0 document(s), 2 workbook(s)

Entities the base holds that match no invented name: 4 active, 4 candidate (and 11 in the surname-then-initial form of a table, which are possible matches by design). All of them:
- person / active / l'Hôtel de Ville / 2 doc(s), 0 workbook(s)
- organization / active / Entre Bureautique Léman SA / 1 doc(s), 0 workbook(s)
- organization / candidate / Étude Chappuis / 1 doc(s), 0 workbook(s)
- person / candidate / Dupont Immobilier / 1 doc(s), 0 workbook(s)
- organization / candidate / cabinet du Parc / 1 doc(s), 0 workbook(s)
- organization / candidate / cabinet des Lilas / 1 doc(s), 0 workbook(s)
- person / active / Dre Vuagniaux / 0 doc(s), 1 workbook(s)
- person / active / Dre Anne-Sophie Vuagniaux / 2 doc(s), 1 workbook(s)

Names in documents never found (11):
- person: Jean-Luc Rapin
- person: Anne-Sophie Vuagniaux
- organization: Cerballiance Alpes
- organization: Centre hospitalier universitaire vaudois
- organization: Dupont Immobilier SA
- organization: Étude Chappuis & Associés
- organization: Hospices Civils de Lyon
- organization: Hôpital fribourgeois
- organization: Hôpitaux universitaires de Genève
- organization: Ordre des médecins de la Haute-Savoie
- organization: Unilabs Genève

Names in tables never found (1):
- person: Anne-Sophie Vuagniaux

Links the base made that the generator does not know (first 25):
- Lucas Roy -> courriers-entrants/2026-10/2026-10-01_courrier-dermatologie.pdf
- Roy Lucas -> courriers-entrants/2026-08/2026-08-17_courrier-orl-roy-lucas.md
- Lucas Roy -> patients-ch.xlsx
- Roy Lucas -> patients-fr.csv

Recall by the shape the name is written in (files where the name appears in that one shape):

| Type | Shape | Files | Linked | Recall |
| --- | --- | --- | --- | --- |
| organization | acronym | 1 | 0 | 0.0 % |
| organization | full | 38 | 27 | 71.1 % |
| person | full | 14 | 14 | 100.0 % |
| person | initial_surname | 2 | 1 | 50.0 % |
| person | no_accent | 1 | 1 | 100.0 % |
| person | reordered | 1 | 1 | 100.0 % |
| person | title_full | 48 | 45 | 93.8 % |
| person | title_surname | 1 | 0 | 0.0 % |

Recall by the format of the file (names written in a form that can be linked):

| Format (type) | Names in files | Linked | Recall |
| --- | --- | --- | --- |
| docx (organization) | 8 | 4 | 50.0 % |
| docx (person) | 21 | 20 | 95.2 % |
| jpg (organization) | 1 | 1 | 100.0 % |
| jpg (person) | 1 | 1 | 100.0 % |
| md (organization) | 1 | 1 | 100.0 % |
| md (person) | 4 | 4 | 100.0 % |
| pdf (organization) | 26 | 16 | 61.5 % |
| pdf (person) | 39 | 37 | 94.9 % |
| png (organization) | 1 | 1 | 100.0 % |
| png (person) | 2 | 2 | 100.0 % |
| scan-pdf (organization) | 1 | 1 | 100.0 % |
| scan-pdf (person) | 3 | 3 | 100.0 % |
| txt (organization) | 3 | 3 | 100.0 % |
| txt (person) | 11 | 11 | 100.0 % |

Recall when the name is whole on its line, and when a line break splits it (a name counts once per file, split if its first mention there is split):

| Type | Name | Files | Linked | Recall |
| --- | --- | --- | --- | --- |
| organization | on one line | 39 | 27 | 69.2 % |
| person | on one line | 80 | 78 | 97.5 % |
| organization | split by a line break | 1 | 0 | 0.0 % |
| person | split by a line break | 1 | 0 | 0.0 % |
### Questions, tier 50, mode `Auto` (127 questions)

| Category | Questions | Answered by the folder | Documents reduced | Would reduce | Recall when reduced | Files searched, median | Files dropped, median | Irrelevant files kept | Wrong entity | Leaks | Workbooks reduced | Workbook recall |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| alias | 8 | 0 | 1 | 1 | 85.7 % | 41 | 35 | 0 | 0 | 0 | 0 | - |
| data | 10 | 2 | 2 | 2 | - | 41 | 39 | 7 | 0 | 0 | 0 | - |
| entity_free | 5 | 1 | 0 | 0 | - | 41 | - | 0 | 0 | 0 | 0 | - |
| follow_up | 2 | 0 | 2 | 2 | 25.0 % | 1 | 40 | 1 | 0 | 0 | 0 | - |
| homonym | 3 | 0 | 3 | 3 | 85.7 % | 6 | 35 | 0 | 0 | 0 | 0 | - |
| identifier | 16 | 6 | 8 | 8 | 100.0 % | 2 | 39 | 1 | 0 | 0 | 0 | - |
| leak | 3 | 0 | 2 | 2 | 100.0 % | 2 | 6 | 0 | 0 | 0 | 0 | - |
| mixed | 3 | 0 | 3 | 3 | 90.0 % | 2 | 39 | 0 | 0 | 0 | 0 | - |
| multi_entity | 6 | 0 | 3 | 3 | 100.0 % | 41 | 39 | 3 | 0 | 0 | 0 | - |
| named_file | 3 | 0 | 0 | 0 | - | 41 | - | 0 | 0 | 0 | 0 | - |
| ocr | 4 | 0 | 4 | 4 | 75.0 % | 1 | 40 | 1 | 0 | 0 | 0 | - |
| org_vs_person | 2 | 0 | 0 | 0 | - | 41 | - | 0 | 0 | 0 | 0 | - |
| organization | 15 | 1 | 4 | 4 | 87.5 % | 41 | 40 | 4 | 0 | 0 | 0 | - |
| person | 21 | 0 | 19 | 19 | 96.3 % | 2 | 40 | 8 | 0 | 0 | 0 | - |
| practitioner | 12 | 0 | 5 | 5 | 100.0 % | 41 | 39 | 1 | 1 | 0 | 0 | - |
| small_scope | 1 | 0 | 0 | 0 | - | 3 | - | 0 | 0 | 0 | 0 | - |
| term | 6 | 0 | 4 | 4 | 100.0 % | 10 | 36 | 0 | 0 | 0 | 0 | - |
| unknown | 7 | 0 | 0 | 0 | - | 41 | - | 0 | 0 | 0 | 0 | - |

Questions whose documents were reduced: 60 of 127. Recall of the reduced set on those: **91.9 %** (124 of 135 relevant files). Median reduction: 95 %. Wrong entity picked: 1. Files outside the selection or left out on purpose that were searched: 0.
Why a question was not reduced: {"-": 60, "ambiguous": 10, "named_file": 9, "no_candidates": 9, "no_discriminating_entity": 5, "no_entity": 23, "router": 10, "small_scope": 1}
Knowledge lookup: median 2 ms, p95 6 ms, max 10 ms.

Relevant files lost by a reduced set, by reason: {"carry_over": 3, "no_mention_in_file": 1, "weak_form_only": 7}

Questions where the reduced set lost a relevant file (9):
- Q025 [person] `Y a-t-il des résultats d'analyses pour Véronique Meunier ?` 0/1 found; lost: patients/courriers/2026-03-08_courrier-patient-meunier-veronique.jpg (weak_form_only)
- Q065 [homonym] `Quels documents concernent Jean Dupont ?` 6/7 found; lost: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (weak_form_only)
- Q066 [homonym] `Que dit le courrier du Dr Jean Dupont ?` 6/7 found; lost: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (weak_form_only)
- Q067 [homonym] `Quel est le prochain rendez-vous de Jean Dupont ?` 6/7 found; lost: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (weak_form_only)
- Q075 [alias] `Que dit Jean-Dupont ?` 6/7 found; lost: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (weak_form_only)
- Q103 [organization] `Quels documents concernent les Hôpitaux universitaires de Genève ?` 1/2 found; lost: documentation/guide-pratique-cabinet.pdf (no_mention_in_file)
- Q177 [ocr] `Que dit le courrier de Véronique Meunier ?` 0/1 found; lost: patients/courriers/2026-03-08_courrier-patient-meunier-veronique.jpg (weak_form_only)
- Q192 [mixed] `Compare les rendez-vous de septembre de Jean Dupont avec la correspondance qui le concerne.` 6/7 found; lost: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (weak_form_only)
- Q197 [follow_up] `Que contiennent les notes internes ?` 0/3 found; lost: administratif/notes/2026-02-20_note-interne-planning.docx (carry_over), administratif/notes/2026-04-30_note-interne-photocopieuse.txt (carry_over), administratif/notes/2026-08-01_note-interne-stock.md (carry_over)

Questions where the reduction was built on an entity that is not the one the question names (1):
- Q099 [practitioner] `Que dit le Dr Gaillard ?` 3/3 found
### Questions, tier 50, mode `Suggest` (127 questions)

| Category | Questions | Answered by the folder | Documents reduced | Would reduce | Recall when reduced | Files searched, median | Files dropped, median | Irrelevant files kept | Wrong entity | Leaks | Workbooks reduced | Workbook recall |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| alias | 8 | 0 | 0 | 1 | - | 41 | - | 0 | 0 | 0 | 0 | - |
| data | 10 | 2 | 0 | 2 | - | 41 | - | 0 | 0 | 0 | 0 | - |
| entity_free | 5 | 1 | 0 | 0 | - | 41 | - | 0 | 0 | 0 | 0 | - |
| follow_up | 2 | 0 | 0 | 2 | - | 41 | - | 0 | 0 | 0 | 0 | - |
| homonym | 3 | 0 | 0 | 3 | - | 41 | - | 0 | 0 | 0 | 0 | - |
| identifier | 16 | 6 | 0 | 8 | - | 41 | - | 0 | 0 | 0 | 0 | - |
| leak | 3 | 0 | 0 | 2 | - | 8 | - | 0 | 0 | 0 | 0 | - |
| mixed | 3 | 0 | 0 | 3 | - | 41 | - | 0 | 0 | 0 | 0 | - |
| multi_entity | 6 | 0 | 0 | 3 | - | 41 | - | 0 | 0 | 0 | 0 | - |
| named_file | 3 | 0 | 0 | 0 | - | 41 | - | 0 | 0 | 0 | 0 | - |
| ocr | 4 | 0 | 0 | 4 | - | 41 | - | 0 | 0 | 0 | 0 | - |
| org_vs_person | 2 | 0 | 0 | 0 | - | 41 | - | 0 | 0 | 0 | 0 | - |
| organization | 15 | 1 | 0 | 4 | - | 41 | - | 0 | 0 | 0 | 0 | - |
| person | 21 | 0 | 0 | 19 | - | 41 | - | 0 | 0 | 0 | 0 | - |
| practitioner | 12 | 0 | 0 | 5 | - | 41 | - | 0 | 0 | 0 | 0 | - |
| small_scope | 1 | 0 | 0 | 0 | - | 3 | - | 0 | 0 | 0 | 0 | - |
| term | 6 | 0 | 0 | 4 | - | 41 | - | 0 | 0 | 0 | 0 | - |
| unknown | 7 | 0 | 0 | 0 | - | 41 | - | 0 | 0 | 0 | 0 | - |

Questions whose documents were reduced: 0 of 127. Recall of the reduced set on those: **-** (0 of 0 relevant files). Median reduction: -. Wrong entity picked: 0. Files outside the selection or left out on purpose that were searched: 0.
Why a question was not reduced: {"-": 60, "ambiguous": 10, "named_file": 9, "no_candidates": 9, "no_discriminating_entity": 5, "no_entity": 23, "router": 10, "small_scope": 1}
Knowledge lookup: median 2 ms, p95 6 ms, max 11 ms.

Relevant files lost by a reduced set, by reason: {}

Questions where the reduced set lost a relevant file (0):

Questions where the reduction was built on an entity that is not the one the question names (0):
