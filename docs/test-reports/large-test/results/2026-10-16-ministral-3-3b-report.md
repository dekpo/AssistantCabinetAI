# Result: ministral-3:3b

- model: ministral-3:3b
- embed_alias: assistant-embed
- gateway: http://192.168.1.83:8080
- temperature: 0.10000000149011612
- commit: 61c30a5
- date: 2026-10-16T10:50:46
- machine: eliseberthier
- seconds: 12.2111231

**248 of 300 passed.**

| Block | Passed | Total |
| --- | --- | --- |
| documents | 120 | 150 |
| mixed | 31 | 50 |
| spreadsheets | 97 | 100 |

| Feature | Name | Passed | Total |
| --- | --- | --- | --- |
| D01 | fact_from_document | 7 | 8 |
| D02 | lookalike_names | 5 | 10 |
| D03 | identifiers_in_text | 11 | 11 |
| D04 | organisations | 5 | 6 |
| D05 | set_search | 12 | 12 |
| D06 | set_search_identifier | 2 | 4 |
| D07 | rank_by_date | 2 | 2 |
| D08 | folder_questions | 6 | 6 |
| D09 | named_file | 6 | 6 |
| D10 | follow_up | 2 | 6 |
| D11 | absent_information | 6 | 8 |
| D12 | isolation_selection | 6 | 6 |
| D13 | small_selection | 3 | 3 |
| D14 | long_document | 8 | 10 |
| D15 | scanned_document | 5 | 6 |
| D16 | reader_vtt | 6 | 6 |
| D17 | reader_srt | 5 | 6 |
| D18 | reader_rtf | 6 | 6 |
| D19 | reader_eml | 6 | 6 |
| D20 | reader_odt | 6 | 6 |
| D21 | reader_odp | 1 | 6 |
| D22 | reader_pptx | 2 | 6 |
| D23 | amounts | 2 | 4 |
| M01 | entity_link | 11 | 18 |
| M02 | document_against_table | 1 | 13 |
| M03 | routing | 8 | 8 |
| M04 | mixed_folder | 2 | 2 |
| M05 | mixed_set | 4 | 4 |
| M06 | fill_plan | 5 | 5 |
| T01 | structure | 8 | 8 |
| T02 | aggregates | 27 | 27 |
| T03 | filter_value | 6 | 6 |
| T04 | filter_date | 10 | 10 |
| T05 | filter_number | 3 | 5 |
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
| T21 | model_plan | 0 | 1 |

| Format | Passed | Total |
| --- | --- | --- |
| - | 46 | 50 |
| csv | 34 | 47 |
| docx | 10 | 15 |
| eml | 7 | 8 |
| jpg | 1 | 1 |
| md | 2 | 3 |
| odp | 1 | 6 |
| ods | 32 | 36 |
| odt | 11 | 12 |
| pdf | 27 | 34 |
| pptx | 2 | 6 |
| rtf | 7 | 7 |
| scan-pdf | 4 | 5 |
| srt | 5 | 6 |
| txt | 4 | 4 |
| vtt | 6 | 6 |
| xlsx | 49 | 54 |

- Refusals correct: 6 of 8
- Leaks (a forbidden fact reached an answer): 3
- Wrong files in a listing: 0
- Answers by the engines alone (no model), passed: 134 of 149
- Questions that need a person's reading: 6
- Answers with a figure that is in no source: 5
- Time per question: median 1.2 s, 95th percentile 2.7 s; the model's part: median 1.1 s, 95th 2.2 s

## Failures

- D-022 (D06) Quels documents contiennent 1 04 11 38 296 167 42 ? - route: global_retrieval (expected set_search); total: total None, expected 2; listing: 0 of 2 expected files listed
- D-024 (D06) Quels documents contiennent 1 37 09 25 350 128 73 ? - route: global_retrieval (expected set_search); total: total None, expected 1; listing: 0 of 1 expected files listed
- D-033 (D17) Dans la transcription de la réunion, quel est le montant du bon de commande BC-2026-0403 ? - value: expected 1507.9, numbers [-2026.0, -403.0]; sources_any: cited ['courriers-entrants/2026-08/2026-08-31_courrier-dermatologie-martin-luc.md', 'reunions/2026-02-04_pv-reunion-cabinet.pdf', 'reunions/2026-02-14_pv-reunion-cabinet.docx']
- D-037 (D21) Quel est le coût par participant du module MOD-0101 ? - value: expected 168.5, numbers [-101.0]; page: pages [1, 2]
- D-038 (D21) À quelle date a lieu la session du module MOD-0103 ? - date: expected 2026-04-18, dates ['2026-02-23']; page: pages [1, 2]
- D-040 (D21) Combien de places y a-t-il dans le module MOD-0108 ? - page: pages [1, 2]
- D-041 (D21) Quel est le coût par participant du module MOD-0110 ? - value: expected 162.5, numbers [2026.0, -2.0, -15.0, 1.0]; page: pages [1, 2]
- D-042 (D21) À quelle date a lieu la session du module MOD-0112 ? - date: expected 2026-08-27, dates ['2026-07-13']; page: pages [1, 2]
- D-043 (D22) Quel est le coût par participant du module MOD-0100 ? - value: expected 46.5, numbers [2026.0, -8.0, -18.0, 1.0, -100.0]; page: pages [1, 2]
- D-046 (D22) Combien de places y a-t-il dans le module MOD-0106 ? - patterns: missing ['\\b21\\b']; page: pages [1, 2]
- D-047 (D22) Quel est le coût par participant du module MOD-0107 ? - value: expected 386.5, numbers [-107.0]; page: pages [1, 2]
- D-048 (D22) À quelle date a lieu la session du module MOD-0109 ? - date: expected 2026-05-11, dates ['2026-04-16']; page: pages [1, 2]
- D-051 (D14) Qui renouvelle le code de l'armoire à pharmacie ? - facts: missing [['Sandrine Colas']]
- D-055 (D14) À qui signaler un écart de la chaîne du froid ? - facts: missing [['Nadia Bensaïd']]; sources_any: cited ['formation/2026-01-05_formation-chaine.pptx', 'formation/2026-01-07_formation-chaine.odp', 'formation/2026-01-21_formation-chaine.odp']
- D-078 (D23) Quel est le total hors taxes de la facture FAC-2026-118 ? - value: expected 523.8, numbers [-2026.0, -118.0]; sources_any: cited ['comptabilite/factures/2026-01-05_facture-orange-fac-2026-175.docx', 'comptabilite/factures/2026-01-29_facture-imprimerie-chablais-fac-2026-146.pdf', 'comptabilite/factures/2026-02-19_facture-edf-fac-2026-204.rtf']
- D-079 (D23) Quel est le total hors taxes de la facture FAC-2026-119 ? - value: expected 88.5, numbers [-2026.0, -119.0]; sources_any: cited ['comptabilite/factures/2026-01-05_facture-orange-fac-2026-175.docx', 'comptabilite/factures/2026-01-29_facture-imprimerie-chablais-fac-2026-146.pdf', 'comptabilite/factures/2026-02-19_facture-edf-fac-2026-204.rtf']
- D-084 (D04) Quel est le prochain rendez-vous de Åsa Haddad d'après la lettre du CHUV ? - date: expected 2026-11-28, dates []
- D-087 (D10) Et son taux d'hémoglobine glyquée dans ses résultats d'analyses ? - value: expected 6.8, numbers [28621.0, 2026.0, -2.0, 2026.0, -2.0, -20.0]; sources_any: cited ['laboratoire/2026-01/2026-01-10_resultats-analyses-benali-fatima.pdf', 'laboratoire/2026-01/2026-01-21_resultats-analyses-crettenand-gregoire.odt', 'laboratoire/2026-02/2026-02-20_resultats-analyses-petrovic-elena.odt']
- D-088 (D02) Quel montant a été pris en charge pour Justine Haddad d'après le décompte de sa caisse ? - value: expected 77.55, numbers []; sources_any: cited ['caisses-assurances/suisse/2026-02-13_decompte-suva-python-monique.odt', 'caisses-assurances/suisse/2026-02-27_decompte-css.pdf', 'caisses-assurances/suisse/2026-03-16_decompte-suva.docx']
- D-089 (D10) Et son prochain rendez-vous d'après le courrier du confrère ? - date: expected 2026-08-05, dates ['2026-10-05']; sources_any: cited ['courriers-entrants/2026-01/2026-01-10_courrier-endocrinologie.pdf', 'courriers-entrants/2026-01/2026-01-18_courrier-neurologie-menoud-raphael.docx', 'courriers-entrants/2026-03/2026-03-11_courrier-neurologie-dupont-jean-pierre.docx']
- D-094 (D02) Quel montant a été pris en charge pour Nathalie Girard d'après le décompte de sa caisse ? - value: expected 65.62, numbers [2026.0, -7.0, -12.0, 1.0, 264.83, 2026.0]; sources_any: cited ['caisses-assurances/suisse/2026-02-13_decompte-suva-python-monique.odt', 'caisses-assurances/suisse/2026-02-27_decompte-css.pdf', 'caisses-assurances/suisse/2026-03-16_decompte-suva.docx']
- D-095 (D10) Et son prochain rendez-vous d'après le courrier du confrère ? - date: expected 2026-11-11, dates ['2026-09-08']; sources_any: cited ['courriers-entrants/2026-01/2026-01-10_courrier-endocrinologie.pdf', 'courriers-entrants/2026-01/2026-01-18_courrier-neurologie-menoud-raphael.docx', 'courriers-entrants/2026-03/2026-03-11_courrier-neurologie-dupont-jean-pierre.docx']
- D-097 (D10) Et son prochain rendez-vous d'après le courrier du confrère ? - date: expected 2026-09-23, dates []; sources_any: cited ['courriers-entrants/2026-01/2026-01-10_courrier-endocrinologie.pdf', 'courriers-entrants/2026-01/2026-01-18_courrier-neurologie-menoud-raphael.docx', 'courriers-entrants/2026-03/2026-03-11_courrier-neurologie-dupont-jean-pierre.docx']
- D-116 (D02) Quel montant a été pris en charge pour Rui Bernasconi d'après le décompte de sa caisse ? - value: expected 255.38, numbers []; sources_any: cited ['caisses-assurances/suisse/2026-02-13_decompte-suva-python-monique.odt', 'caisses-assurances/suisse/2026-02-27_decompte-css.pdf', 'caisses-assurances/suisse/2026-03-16_decompte-suva.docx']
- D-117 (D02) Quel est le taux d'hémoglobine glyquée de Damien Hasanović dans ses résultats d'analyses ? - must_not: leaked ['5,2 %']
- D-119 (D02) Quel montant a été pris en charge pour Thierry Thomas d'après le décompte de sa caisse ? - value: expected 167.78, numbers []; sources_any: cited ['caisses-assurances/suisse/2026-01-08_decompte-css.docx', 'caisses-assurances/suisse/2026-02-13_decompte-suva-python-monique.odt', 'caisses-assurances/suisse/2026-02-27_decompte-css.pdf']
- D-122 (D01) Quel est le taux d'hémoglobine glyquée de Thomas Pierre dans ses résultats d'analyses ? - value: expected 6.0, numbers [2026.0, -4.0, 2026.0, -4.0, -4.0, 1.0]
- D-128 (D15) Quel est le prochain rendez-vous de Véronique Monnier d'après le courrier du confrère ? - date: expected 2026-09-09, dates ['2026-06-20']; sources_any: cited ['caisses-assurances/suisse/2026-03-23_decompte-suva.pdf', 'caisses-assurances/suisse/2026-06-10_decompte-assura.jpg', 'caisses-assurances/suisse/2026-09-18_decompte-assura.pdf']
- D-149 (D11) Quel est le prochain rendez-vous de Philippe Python ? - refusal: answered where it should refuse; must_not: leaked ['14 juin 2026', '6 mai 2026', '27 octobre 2026']
- D-150 (D11) Quel est le prochain rendez-vous de Pauline Rappaz ? - refusal: answered where it should refuse; must_not: leaked ['7 octobre 2026']
- T-034 (T21) Combien de lignes avec Prix unitaire supérieur à 19.67 dans le fichier stock-2026-02-bureau.csv ? - route: tabular:nudge (expected tabular:value or tabular:model_plan); value: expected 7, numbers [-2026.0, -2.0, -2026.0, -2.0, 19.67, 1282.0]
- T-037 (T05) Combien de lignes avec Débit supérieur à 1264 dans le fichier compta-depenses-2026-01-site-1.xlsx ? - route: tabular:nudge (expected tabular:value or tabular:model_plan); value: expected 16, numbers [-2026.0, -1.0, -1.0, 1264.0, 555.0, -3.0]
- T-040 (T05) Combien de lignes avec Montant TTC supérieur à 835 dans le fichier factures-fournisseurs-2026-t2-pharma-depot-romandie-sa.ods ? - route: tabular:nudge (expected tabular:value or tabular:model_plan); value: expected 9, numbers [-2026.0, 2.0, 835.0, 264.0, -3.0, 3.0]
- M-020 (M02) Le total TTC de la facture FAC-2026-014 de MedSupply SA dans le document est-il le même que dans le fichier factures-fournisseurs-2026-t1-medsupply-sa.ods ? - values: missing [1109.32, 1164.79], numbers [-2026.0, 1.0]; sources_any: cited []
- M-021 (M02) Le total TTC de la facture FAC-2026-014 de Papeterie du Lac SARL dans le document est-il le même que dans le fichier factures-fournisseurs-2026-t2-papeterie-du-lac-sarl.xlsx ? - values: missing [871.8, 915.39], numbers [-2026.0, 2.0]; sources_any: cited []
- M-023 (M02) Le total TTC de la facture FAC-2026-117 de Imprimerie du Chablais SAS dans le document est-il le même que dans le fichier factures-fournisseurs-2026-t3-imprimerie-du-chablais-sas.csv ? - values: missing [883.92], numbers [-2026.0, 3.0, -2026.0, 3.0]; sources_any: cited []
- M-024 (M02) Le total TTC de la facture FAC-2026-118 de MedSupply SA dans le document est-il le même que dans le fichier factures-fournisseurs-2026-t3-medsupply-sa.xlsx ? - values: missing [566.23], numbers [3.0, -2026.0, 3.0]; sources_any: cited []
- M-025 (M02) Le total TTC de la facture FAC-2026-119 de EDF dans le document est-il le même que dans le fichier factures-fournisseurs-2026-t2-edf.csv ? - values: missing [106.2, 111.51], numbers [3.0, -2026.0, 2.0, -2026.0, 2.0]; sources_any: cited []
- M-026 (M02) Le total TTC de la facture FAC-2026-121 de Imprimerie du Chablais SAS dans le document est-il le même que dans le fichier factures-fournisseurs-2026-t3-imprimerie-du-chablais-sas.csv ? - values: missing [920.4], numbers [-2026.0, 3.0, -2026.0, 3.0]; sources_any: cited []
- M-027 (M02) Le total TTC de la facture FAC-2026-122 de MedSupply SA dans le document est-il le même que dans le fichier factures-fournisseurs-2026-t1-medsupply-sa.ods ? - values: missing [144.96, 152.21], numbers [-2026.0, 1.0]; sources_any: cited []
- M-028 (M02) Le total TTC de la facture FAC-2026-126 de Orange SA dans le document est-il le même que dans le fichier factures-fournisseurs-2026-t1-orange-sa.ods ? - values: missing [281.52], numbers [-2026.0, 1.0]; sources_any: cited []
- M-029 (M02) Le total TTC de la facture FAC-2026-130 de Bureautique Léman SA dans le document est-il le même que dans le fichier factures-fournisseurs-2026-t3-bureautique-leman-sa.xlsx ? - values: missing [191.66], numbers [-2026.0, 3.0]; sources_any: cited []
- M-030 (M02) Le total TTC de la facture FAC-2026-131 de Imprimerie du Chablais SAS dans le document est-il le même que dans le fichier factures-fournisseurs-2026-t3-imprimerie-du-chablais-sas.csv ? - values: missing [836.16, 877.97], numbers [-2026.0, 3.0, -2026.0, 3.0]; sources_any: cited []
- M-031 (M02) Le total TTC de la facture FAC-2026-132 de Imprimerie du Chablais SAS dans le document est-il le même que dans le fichier factures-fournisseurs-2026-t3-imprimerie-du-chablais-sas.csv ? - values: missing [776.88], numbers [-2026.0, 3.0, -2026.0, 3.0]; sources_any: cited []
- M-032 (M02) Le total TTC de la facture FAC-2026-133 de EDF dans le document est-il le même que dans le fichier factures-fournisseurs-2026-t2-edf.csv ? - values: missing [150.84], numbers [3.0, -2026.0, 2.0, -2026.0, 2.0]; sources_any: cited []
- M-041 (M01) D'après le courrier du confrère, quel est le prochain rendez-vous de François Ducommun, et combien de lignes pour François Ducommun dans le fichier rendez-vous-2026-07-cabinet-des-lilas.csv ? - date: expected 2026-07-01, dates []; sources_any: cited []
- M-042 (M01) D'après le courrier du confrère, quel est le prochain rendez-vous de Roland Huguenin, et combien de lignes pour Roland Huguenin dans le fichier rendez-vous-2026-07-cabinet-de-thonon.csv ? - value: expected 1, numbers [16.0, 2026.0, 4.0, -2026.0, -7.0, 0.0]
- M-043 (M01) D'après le courrier du confrère, quel est le prochain rendez-vous de Gaëlle Ducrettet, et combien de lignes pour Gaëlle Ducrettet dans le fichier rendez-vous-2026-08-cabinet-de-thonon.xlsx ? - value: expected 1, numbers [-2026.0, -8.0, 0.0]; date: expected 2026-12-07, dates []
- M-046 (M01) D'après le courrier du confrère, quel est le prochain rendez-vous de Åsa Haddad, et combien de lignes pour Åsa Haddad dans le fichier rendez-vous-2026-07-cabinet-des-lilas.csv ? - date: expected 2026-11-28, dates []
- M-047 (M01) D'après le courrier du confrère, quel est le prochain rendez-vous de Fatima Benali, et combien de lignes pour Fatima Benali dans le fichier rendez-vous-2026-05-cabinet-des-lilas.csv ? - value: expected 1, numbers [8.0, 2026.0, 2.0, -2026.0, -5.0, 0.0]
- M-048 (M01) D'après le courrier du confrère, quel est le prochain rendez-vous de Grégoire Crettenand, et combien de lignes pour Grégoire Crettenand dans le fichier rendez-vous-2026-12-cabinet-des-lilas.csv ? - value: expected 1, numbers [29.0, 2026.0, 2.0, -2026.0, -12.0, 0.0]
- M-050 (M01) D'après le courrier du confrère, quel est le prochain rendez-vous de Yvonne Bugnon, et combien de lignes pour Yvonne Bugnon dans le fichier rendez-vous-2026-02-cabinet-des-lilas.csv ? - value: expected 1, numbers [3.0, 2026.0, 2.0, -2026.0, -2.0, 0.0]
