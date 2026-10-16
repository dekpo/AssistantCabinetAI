# Result: ministral-3:3b

- model: ministral-3:3b
- embed_alias: assistant-embed
- gateway: http://192.168.1.83:8080
- temperature: 0.10000000149011612
- commit: 61c30a5
- date: 2026-10-16T10:48:09
- machine: eliseberthier
- seconds: 133.2607593

**273 of 300 passed.**

| Block | Passed | Total |
| --- | --- | --- |
| documents | 126 | 150 |
| mixed | 47 | 50 |
| spreadsheets | 100 | 100 |

| Feature | Name | Passed | Total |
| --- | --- | --- | --- |
| D01 | fact_from_document | 8 | 8 |
| D02 | lookalike_names | 6 | 10 |
| D03 | identifiers_in_text | 11 | 11 |
| D04 | organisations | 6 | 6 |
| D05 | set_search | 12 | 12 |
| D06 | set_search_identifier | 2 | 4 |
| D07 | rank_by_date | 2 | 2 |
| D08 | folder_questions | 6 | 6 |
| D09 | named_file | 6 | 6 |
| D10 | follow_up | 2 | 6 |
| D11 | absent_information | 8 | 8 |
| D12 | isolation_selection | 6 | 6 |
| D13 | small_selection | 3 | 3 |
| D14 | long_document | 9 | 10 |
| D15 | scanned_document | 5 | 6 |
| D16 | reader_vtt | 6 | 6 |
| D17 | reader_srt | 5 | 6 |
| D18 | reader_rtf | 6 | 6 |
| D19 | reader_eml | 6 | 6 |
| D20 | reader_odt | 6 | 6 |
| D21 | reader_odp | 1 | 6 |
| D22 | reader_pptx | 2 | 6 |
| D23 | amounts | 2 | 4 |
| M01 | entity_link | 18 | 18 |
| M02 | document_against_table | 10 | 13 |
| M03 | routing | 8 | 8 |
| M04 | mixed_folder | 2 | 2 |
| M05 | mixed_set | 4 | 4 |
| M06 | fill_plan | 5 | 5 |
| T01 | structure | 8 | 8 |
| T02 | aggregates | 27 | 27 |
| T03 | filter_value | 6 | 6 |
| T04 | filter_date | 10 | 10 |
| T05 | filter_number | 5 | 5 |
| T06 | groups | 12 | 12 |
| T07 | distinct_values | 6 | 6 |
| T08 | trap_title_row | 3 | 3 |
| T09 | trap_merged_title | 2 | 2 |
| T10 | trap_formula | 4 | 4 |
| T11 | trap_empty_column | 2 | 2 |
| T12 | trap_notes_sheet | 3 | 3 |
| T13 | trap_text_dates | 2 | 2 |
| T14 | trap_locale_numbers | 3 | 3 |
| T18 | which_workbook | 2 | 2 |
| T19 | unknown_or_unselected_file | 2 | 2 |
| T20 | honest_refusals | 2 | 2 |
| T21 | model_plan | 1 | 1 |

| Format | Passed | Total |
| --- | --- | --- |
| - | 48 | 50 |
| csv | 44 | 47 |
| docx | 11 | 15 |
| eml | 7 | 8 |
| jpg | 1 | 1 |
| md | 2 | 3 |
| odp | 1 | 6 |
| ods | 36 | 36 |
| odt | 11 | 12 |
| pdf | 30 | 34 |
| pptx | 2 | 6 |
| rtf | 7 | 7 |
| scan-pdf | 4 | 5 |
| srt | 5 | 6 |
| txt | 4 | 4 |
| vtt | 6 | 6 |
| xlsx | 54 | 54 |

- Refusals correct: 8 of 8
- Leaks (a forbidden fact reached an answer): 0
- Wrong files in a listing: 0
- Answers by the engines alone (no model), passed: 273 of 300
- Questions that need a person's reading: 6
- Answers with a figure that is in no source: 0
- Time per question: median 0.5 s, 95th percentile 0.7 s; the model's part: median 0.0 s, 95th 0.0 s

## Failures

- D-022 (D06) Quels documents contiennent 1 04 11 38 296 167 42 ? - route: global_retrieval (expected set_search)
- D-024 (D06) Quels documents contiennent 1 37 09 25 350 128 73 ? - route: global_retrieval (expected set_search)
- D-033 (D17) Dans la transcription de la réunion, quel est le montant du bon de commande BC-2026-0403 ? - sources_any: cited ['courriers-entrants/2026-08/2026-08-31_courrier-dermatologie-martin-luc.md', 'reunions/2026-02-04_pv-reunion-cabinet.pdf', 'reunions/2026-02-14_pv-reunion-cabinet.docx']
- D-037 (D21) Quel est le coût par participant du module MOD-0101 ? - page: pages [1, 2]
- D-038 (D21) À quelle date a lieu la session du module MOD-0103 ? - page: pages [1, 2]
- D-040 (D21) Combien de places y a-t-il dans le module MOD-0108 ? - page: pages [1, 2]
- D-041 (D21) Quel est le coût par participant du module MOD-0110 ? - page: pages [1, 2]
- D-042 (D21) À quelle date a lieu la session du module MOD-0112 ? - page: pages [1, 2]
- D-043 (D22) Quel est le coût par participant du module MOD-0100 ? - page: pages [1, 2]
- D-046 (D22) Combien de places y a-t-il dans le module MOD-0106 ? - page: pages [1, 2]
- D-047 (D22) Quel est le coût par participant du module MOD-0107 ? - page: pages [1, 2]
- D-048 (D22) À quelle date a lieu la session du module MOD-0109 ? - page: pages [1, 2]
- D-055 (D14) À qui signaler un écart de la chaîne du froid ? - sources_any: cited ['formation/2026-01-05_formation-chaine.pptx', 'formation/2026-01-07_formation-chaine.odp', 'formation/2026-01-21_formation-chaine.odp']
- D-078 (D23) Quel est le total hors taxes de la facture FAC-2026-118 ? - sources_any: cited ['comptabilite/factures/2026-01-05_facture-orange-fac-2026-175.docx', 'comptabilite/factures/2026-01-29_facture-imprimerie-chablais-fac-2026-146.pdf', 'comptabilite/factures/2026-02-19_facture-edf-fac-2026-204.rtf']
- D-079 (D23) Quel est le total hors taxes de la facture FAC-2026-119 ? - sources_any: cited ['comptabilite/factures/2026-01-05_facture-orange-fac-2026-175.docx', 'comptabilite/factures/2026-01-29_facture-imprimerie-chablais-fac-2026-146.pdf', 'comptabilite/factures/2026-02-19_facture-edf-fac-2026-204.rtf']
- D-087 (D10) Et son taux d'hémoglobine glyquée dans ses résultats d'analyses ? - sources_any: cited ['laboratoire/2026-01/2026-01-10_resultats-analyses-benali-fatima.pdf', 'laboratoire/2026-01/2026-01-21_resultats-analyses-crettenand-gregoire.odt', 'laboratoire/2026-02/2026-02-20_resultats-analyses-petrovic-elena.odt']
- D-088 (D02) Quel montant a été pris en charge pour Justine Haddad d'après le décompte de sa caisse ? - sources_any: cited ['caisses-assurances/suisse/2026-02-13_decompte-suva-python-monique.odt', 'caisses-assurances/suisse/2026-02-27_decompte-css.pdf', 'caisses-assurances/suisse/2026-03-16_decompte-suva.docx']
- D-089 (D10) Et son prochain rendez-vous d'après le courrier du confrère ? - sources_any: cited ['courriers-entrants/2026-01/2026-01-10_courrier-endocrinologie.pdf', 'courriers-entrants/2026-01/2026-01-18_courrier-neurologie-menoud-raphael.docx', 'courriers-entrants/2026-03/2026-03-11_courrier-neurologie-dupont-jean-pierre.docx']
- D-094 (D02) Quel montant a été pris en charge pour Nathalie Girard d'après le décompte de sa caisse ? - sources_any: cited ['caisses-assurances/suisse/2026-02-13_decompte-suva-python-monique.odt', 'caisses-assurances/suisse/2026-02-27_decompte-css.pdf', 'caisses-assurances/suisse/2026-03-16_decompte-suva.docx']
- D-095 (D10) Et son prochain rendez-vous d'après le courrier du confrère ? - sources_any: cited ['courriers-entrants/2026-01/2026-01-10_courrier-endocrinologie.pdf', 'courriers-entrants/2026-01/2026-01-18_courrier-neurologie-menoud-raphael.docx', 'courriers-entrants/2026-03/2026-03-11_courrier-neurologie-dupont-jean-pierre.docx']
- D-097 (D10) Et son prochain rendez-vous d'après le courrier du confrère ? - sources_any: cited ['courriers-entrants/2026-01/2026-01-10_courrier-endocrinologie.pdf', 'courriers-entrants/2026-01/2026-01-18_courrier-neurologie-menoud-raphael.docx', 'courriers-entrants/2026-03/2026-03-11_courrier-neurologie-dupont-jean-pierre.docx']
- D-116 (D02) Quel montant a été pris en charge pour Rui Bernasconi d'après le décompte de sa caisse ? - sources_any: cited ['caisses-assurances/suisse/2026-02-13_decompte-suva-python-monique.odt', 'caisses-assurances/suisse/2026-02-27_decompte-css.pdf', 'caisses-assurances/suisse/2026-03-16_decompte-suva.docx']
- D-119 (D02) Quel montant a été pris en charge pour Thierry Thomas d'après le décompte de sa caisse ? - sources_any: cited ['caisses-assurances/suisse/2026-01-08_decompte-css.docx', 'caisses-assurances/suisse/2026-02-13_decompte-suva-python-monique.odt', 'caisses-assurances/suisse/2026-02-27_decompte-css.pdf']
- D-128 (D15) Quel est le prochain rendez-vous de Véronique Monnier d'après le courrier du confrère ? - sources_any: cited ['caisses-assurances/suisse/2026-03-23_decompte-suva.pdf', 'caisses-assurances/suisse/2026-06-10_decompte-assura.jpg', 'caisses-assurances/suisse/2026-09-18_decompte-assura.pdf']
- M-025 (M02) Le total TTC de la facture FAC-2026-119 de EDF dans le document est-il le même que dans le fichier factures-fournisseurs-2026-t2-edf.csv ? - sources_any: cited ['comptabilite/factures/2026-01-05_facture-orange-fac-2026-175.docx', 'comptabilite/factures/2026-01-29_facture-imprimerie-chablais-fac-2026-146.pdf', 'comptabilite/factures/2026-02-19_facture-edf-fac-2026-204.rtf']
- M-030 (M02) Le total TTC de la facture FAC-2026-131 de Imprimerie du Chablais SAS dans le document est-il le même que dans le fichier factures-fournisseurs-2026-t3-imprimerie-du-chablais-sas.csv ? - sources_any: cited ['comptabilite/devis/2026-02-28_devis-imprimerie-chablais.jpg', 'comptabilite/devis/2026-03-10_devis-imprimerie-chablais.pdf', 'comptabilite/devis/2026-03-31_devis-imprimerie-chablais.md']
- M-032 (M02) Le total TTC de la facture FAC-2026-133 de EDF dans le document est-il le même que dans le fichier factures-fournisseurs-2026-t2-edf.csv ? - sources_any: cited ['comptabilite/factures/2026-01-05_facture-orange-fac-2026-175.docx', 'comptabilite/factures/2026-01-29_facture-imprimerie-chablais-fac-2026-146.pdf', 'comptabilite/factures/2026-02-19_facture-edf-fac-2026-204.rtf']
