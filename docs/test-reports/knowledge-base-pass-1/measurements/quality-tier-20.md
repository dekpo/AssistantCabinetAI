### Entity quality, tier 20 (20 documents read)

| Side | Type | Names in the library | Found by the base | Active | File links expected | File links found | File links the base made | Link recall | Link precision |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| documents | person | 18 | 18 (100.0 %) | 17 | 36 | 34 | 34 | 94.4 % | 100.0 % |
| documents | organization | 13 | 10 (76.9 %) | 9 | 20 | 15 | 15 | 75.0 % | 100.0 % |
| tables | person | 165 | 164 (99.4 %) | 164 | 211 | 200 | 202 | 94.8 % | 99.0 % |
| tables | organization | 10 | 10 (100.0 %) | 10 | 12 | 12 | 12 | 100.0 % | 100.0 % |

False merges (one base entity, two different people): **2**
- Roy Lucas (active) holds p-lucas-roy + p-roy-lucas - 0 document(s), 2 workbook(s)
- Thomas Pierre (active) holds p-pierre-thomas + p-thomas-pierre - 0 document(s), 2 workbook(s)

Entities the base holds that match no invented name: 3 active, 1 candidate (and 11 in the surname-then-initial form of a table, which are possible matches by design). All of them:
- person / candidate / l'Hôtel de Ville / 1 doc(s), 0 workbook(s)
- organization / active / Entre Bureautique Léman SA / 1 doc(s), 0 workbook(s)
- person / active / Dre Vuagniaux / 0 doc(s), 1 workbook(s)
- person / active / Dre Anne-Sophie Vuagniaux / 0 doc(s), 1 workbook(s)

Names in documents never found (3):
- organization: Cerballiance Alpes
- organization: Hôpitaux universitaires de Genève
- organization: Unilabs Genève

Names in tables never found (1):
- person: Anne-Sophie Vuagniaux

Links the base made that the generator does not know (first 25):
- Lucas Roy -> patients-ch.xlsx
- Roy Lucas -> patients-fr.csv

Recall by the shape the name is written in (files where the name appears in that one shape):

| Type | Shape | Files | Linked | Recall |
| --- | --- | --- | --- | --- |
| organization | full | 20 | 15 | 75.0 % |
| person | full | 9 | 9 | 100.0 % |
| person | initial_surname | 1 | 0 | 0.0 % |
| person | title_full | 17 | 17 | 100.0 % |
| person | title_surname | 1 | 0 | 0.0 % |

Recall by the format of the file (names written in a form that can be linked):

| Format (type) | Names in files | Linked | Recall |
| --- | --- | --- | --- |
| docx (organization) | 4 | 3 | 75.0 % |
| docx (person) | 8 | 8 | 100.0 % |
| jpg (organization) | 1 | 1 | 100.0 % |
| jpg (person) | 1 | 1 | 100.0 % |
| md (person) | 1 | 1 | 100.0 % |
| pdf (organization) | 12 | 8 | 66.7 % |
| pdf (person) | 15 | 15 | 100.0 % |
| scan-pdf (organization) | 1 | 1 | 100.0 % |
| scan-pdf (person) | 3 | 3 | 100.0 % |
| txt (organization) | 2 | 2 | 100.0 % |
| txt (person) | 7 | 7 | 100.0 % |

Recall when the name is whole on its line, and when a line break splits it (a name counts once per file, split if its first mention there is split):

| Type | Name | Files | Linked | Recall |
| --- | --- | --- | --- | --- |
| organization | on one line | 20 | 15 | 75.0 % |
| person | on one line | 35 | 35 | 100.0 % |
### Questions, tier 20, mode `Auto` (0 questions)

| Category | Questions | Answered by the folder | Documents reduced | Would reduce | Recall when reduced | Files searched, median | Files dropped, median | Irrelevant files kept | Wrong entity | Leaks | Workbooks reduced | Workbook recall |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |

Questions whose documents were reduced: 0 of 0. Recall of the reduced set on those: **-** (0 of 0 relevant files). Median reduction: -. Wrong entity picked: 0. Files outside the selection or left out on purpose that were searched: 0.
Why a question was not reduced: {}
Knowledge lookup: median - ms, p95 - ms, max - ms.

Relevant files lost by a reduced set, by reason: {}

Questions where the reduced set lost a relevant file (0):

Questions where the reduction was built on an entity that is not the one the question names (0):
### Questions, tier 20, mode `Suggest` (0 questions)

| Category | Questions | Answered by the folder | Documents reduced | Would reduce | Recall when reduced | Files searched, median | Files dropped, median | Irrelevant files kept | Wrong entity | Leaks | Workbooks reduced | Workbook recall |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |

Questions whose documents were reduced: 0 of 0. Recall of the reduced set on those: **-** (0 of 0 relevant files). Median reduction: -. Wrong entity picked: 0. Files outside the selection or left out on purpose that were searched: 0.
Why a question was not reduced: {}
Knowledge lookup: median - ms, p95 - ms, max - ms.

Relevant files lost by a reduced set, by reason: {}

Questions where the reduced set lost a relevant file (0):

Questions where the reduction was built on an entity that is not the one the question names (0):
