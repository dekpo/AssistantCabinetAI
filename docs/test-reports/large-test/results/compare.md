# Models compared on the same library and the same book

| | 2026-10-16-ministral-3-3b | 2026-10-16-gemma4-e4b |
| --- | --- | --- |
| passed | 248/300 | 253/300 |
| model | ministral-3:3b | gemma4:e4b |
| embedding | assistant-embed | assistant-embed |
| refusals correct | 6/8 | 8/8 |
| leaks | 3 | 2 |
| wrong files in a listing | 0 | 0 |
| median time per question | 1.2 s | 3.9 s |
| 95th percentile | 2.7 s | 8.8 s |
| model's part, median | 1.1 s | 4.3 s |

## Blocks

| | 2026-10-16-ministral-3-3b | 2026-10-16-gemma4-e4b |
| --- | --- | --- |
| documents | 120/150 | 119/150 |
| mixed | 31/50 | 34/50 |
| spreadsheets | 97/100 | 100/100 |

## Features

| | 2026-10-16-ministral-3-3b | 2026-10-16-gemma4-e4b |
| --- | --- | --- |
| D01 fact_from_document | 7/8 | 8/8 |
| D02 lookalike_names | 5/10 | 4/10 |
| D03 identifiers_in_text | 11/11 | 11/11 |
| D04 organisations | 5/6 | 5/6 |
| D05 set_search | 12/12 | 12/12 |
| D06 set_search_identifier | 2/4 | 2/4 |
| D07 rank_by_date | 2/2 | 2/2 |
| D08 folder_questions | 6/6 | 6/6 |
| D09 named_file | 6/6 | 6/6 |
| D10 follow_up | 2/6 | 2/6 |
| D11 absent_information | 6/8 | 8/8 |
| D12 isolation_selection | 6/6 | 6/6 |
| D13 small_selection | 3/3 | 3/3 |
| D14 long_document | 8/10 | 8/10 |
| D15 scanned_document | 5/6 | 5/6 |
| D16 reader_vtt | 6/6 | 4/6 |
| D17 reader_srt | 5/6 | 4/6 |
| D18 reader_rtf | 6/6 | 6/6 |
| D19 reader_eml | 6/6 | 6/6 |
| D20 reader_odt | 6/6 | 6/6 |
| D21 reader_odp | 1/6 | 1/6 |
| D22 reader_pptx | 2/6 | 2/6 |
| D23 amounts | 2/4 | 2/4 |
| M01 entity_link | 11/18 | 14/18 |
| M02 document_against_table | 1/13 | 1/13 |
| M03 routing | 8/8 | 8/8 |
| M04 mixed_folder | 2/2 | 2/2 |
| M05 mixed_set | 4/4 | 4/4 |
| M06 fill_plan | 5/5 | 5/5 |
| T01 structure | 8/8 | 8/8 |
| T02 aggregates | 27/27 | 27/27 |
| T03 filter_value | 6/6 | 6/6 |
| T04 filter_date | 10/10 | 10/10 |
| T05 filter_number | 3/5 | 5/5 |
| T06 groups | 12/12 | 12/12 |
| T07 distinct_values | 6/6 | 6/6 |
| T08 trap_title_row | 3/3 | 3/3 |
| T09 trap_merged_title | 2/2 | 2/2 |
| T10 trap_formula | 4/4 | 4/4 |
| T11 trap_empty_column | 2/2 | 2/2 |
| T12 trap_notes_sheet | 3/3 | 3/3 |
| T13 trap_text_dates | 2/2 | 2/2 |
| T14 trap_locale_numbers | 3/3 | 3/3 |
| T18 which_workbook | 2/2 | 2/2 |
| T19 unknown_or_unselected_file | 2/2 | 2/2 |
| T20 honest_refusals | 2/2 | 2/2 |
| T21 model_plan | 0/1 | 1/1 |

## Formats

| | 2026-10-16-ministral-3-3b | 2026-10-16-gemma4-e4b |
| --- | --- | --- |
| - | 46/50 | 48/50 |
| csv | 34/47 | 38/47 |
| docx | 10/15 | 10/15 |
| eml | 7/8 | 7/8 |
| jpg | 1/1 | 1/1 |
| md | 2/3 | 2/3 |
| odp | 1/6 | 1/6 |
| ods | 32/36 | 33/36 |
| odt | 11/12 | 11/12 |
| pdf | 27/34 | 27/34 |
| pptx | 2/6 | 2/6 |
| rtf | 7/7 | 7/7 |
| scan-pdf | 4/5 | 4/5 |
| srt | 5/6 | 4/6 |
| txt | 4/4 | 4/4 |
| vtt | 6/6 | 4/6 |
| xlsx | 49/54 | 50/54 |

Answers by the engines alone (no model) do not depend on the model: the same figure is expected in every column.

| | 2026-10-16-ministral-3-3b | 2026-10-16-gemma4-e4b |
| --- | --- | --- |
| engine-only passed | 134/149 | 137/149 |

