## Tier 30

### Analyse

- to 30 documents: 24756 ms. {"pass":"documents","filesScanned":28,"filesProcessed":28,"hashMs":0,"extractMs":142,"ocrMs":3870,"chunkMs":0,"embedMs":19974,"embedBatches":28,"embeddedChars":16667,"writeMs":413,"buildInventoryMs":0,"chunksTotal":28,"chunksRepeatingEarlierText":0,"totalMs":24707,"slowestFiles":[{"position":12,"totalMs":3869,"extractMs":141,"ocrMs":2947,"chunkMs":0,"embedMs":762,"writeMs":18,"chunks":1},{"position":24,"totalMs":1110,"extractMs":0,"ocrMs":462,"chunkMs":0,"embedMs":630,"writeMs":18,"chunks":1},{"position":14,"totalMs":1073,"extractMs":0,"ocrMs":0,"chunkMs":0,"embedMs":1051,"writeMs":21,"chunks":1},{"position":10,"totalMs":1046,"extractMs":0,"ocrMs":0,"chunkMs":0,"embedMs":1027,"writeMs":18,"chunks":1},{"position":27,"totalMs":1017,"extractMs":0,"ocrMs":461,"chunkMs":0,"embedMs":538,"writeMs":17,"chunks":1}]}

Documents read by the product at this tier: 28.

### What OCR read of the scanned documents

| Document | Format | Words kept | Names and numbers kept (exact) | Kept without accents | OCR text empty |
| --- | --- | --- | --- | --- | --- |
| courriers-entrants/2026-02/2026-02-25_courrier-cardiologie-monnier-veronique.pdf | scan-pdf | 53.2 % | 7 of 9 | 7 | false |
| patients/courriers/2026-03-08_courrier-patient-meunier-veronique.jpg | jpg | 93.9 % | 1 of 3 | 1 | false |
| scans/2026-10/2026-10-07_ordonnance.png | png | 100.0 % | 3 of 3 | 3 | false |

Words kept: **75.0 %**. Names and numbers kept verbatim: **73.3 %** (11 of 15); without accents: 73.3 %.
### Real run, tier 30

**Recall of the candidate set, on the files that hold the answer** (cited with the base off AND known to hold the answer): **93.3 %** (84 of 90), over 64 questions the base would reduce; 6 of them lost at least one such file. Lost, by reason: {"not_linked_to_the_name": 6}

Raw shadow measure (every cited file, filler included): 24.7 % (95 of 384). Filler the reduced set dropped: 283; filler it kept: 11.

| Category | Relevant cited files | Kept by the candidate set | Recall | Filler dropped | Filler kept |
| --- | --- | --- | --- | --- | --- |
| alias | 8 | 6 | 75.0 % | 4 | 0 |
| homonym | 12 | 9 | 75.0 % | 6 | 0 |
| identifier | 12 | 12 | 100.0 % | 46 | 2 |
| mixed | 6 | 5 | 83.3 % | 12 | 0 |
| multi_entity | 6 | 6 | 100.0 % | 22 | 8 |
| ocr | 4 | 4 | 100.0 % | 20 | 0 |
| org_vs_person | 1 | 1 | 100.0 % | 5 | 0 |
| organization | 12 | 12 | 100.0 % | 48 | 0 |
| person | 21 | 21 | 100.0 % | 92 | 1 |
| practitioner | 8 | 8 | 100.0 % | 28 | 0 |

Questions that search the documents: 116. Reduced in `auto`: 64. Searched again on the whole selection after a reduction (widened): 19.

- The `auto` answer still cites the sources the base-off answer cites (on the 64 reduced questions): **49.5 %** (190 of 384). Same first source: 52 of 64.
- Questions with a known answer in the library: 105. Share of cited files that really hold the answer: base off **28.8 %**, `auto` **40.8 %**. At least one right source: off 102, auto 103.

| Mode | Embedding of the question, median / p95 ms | Knowledge lookup, median / p95 ms | Search, median / p95 ms | Chunks scored, median | Files searched, median | Passages in the prompt, median | Characters of passages, median / p95 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| off | 331.4 / 402.3 | 0.0 / 0.0 | 0.6 / 1.2 | 28.0 | 28.0 | 6.0 | 3689.0 / 5141.0 |
| suggest | 331.4 / 402.3 | 2.0 / 9.0 | 0.5 / 1.2 | 28.0 | 28.0 | 6.0 | 3689.0 / 5141.0 |
| auto | 331.4 / 402.3 | 2.0 / 11.0 | 0.6 / 1.7 | 28.0 | 3.0 | 6.0 | 2918.0 / 4823.0 |

Lost sources (first 60):
- Q065 [homonym] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (not_linked_to_the_name)
- Q066 [homonym] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (not_linked_to_the_name)
- Q067 [homonym] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (not_linked_to_the_name)
- Q074 [alias] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (not_linked_to_the_name)
- Q075 [alias] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (not_linked_to_the_name)
- Q192 [mixed] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (not_linked_to_the_name)
