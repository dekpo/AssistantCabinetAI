## Tier 30

### Analyse

- to 30 documents: 24583 ms. {"pass":"documents","filesScanned":28,"filesProcessed":28,"hashMs":0,"extractMs":99,"ocrMs":3908,"chunkMs":0,"embedMs":19801,"embedBatches":28,"embeddedChars":16667,"writeMs":444,"buildInventoryMs":0,"chunksTotal":28,"chunksRepeatingEarlierText":0,"totalMs":24556,"slowestFiles":[{"position":12,"totalMs":4100,"extractMs":93,"ocrMs":3145,"chunkMs":0,"embedMs":832,"writeMs":28,"chunks":1},{"position":13,"totalMs":1127,"extractMs":1,"ocrMs":0,"chunkMs":0,"embedMs":1099,"writeMs":25,"chunks":1},{"position":9,"totalMs":1111,"extractMs":1,"ocrMs":0,"chunkMs":0,"embedMs":1083,"writeMs":25,"chunks":1},{"position":11,"totalMs":1092,"extractMs":0,"ocrMs":0,"chunkMs":0,"embedMs":1069,"writeMs":23,"chunks":1},{"position":15,"totalMs":1084,"extractMs":1,"ocrMs":0,"chunkMs":0,"embedMs":1062,"writeMs":20,"chunks":1}]}

Documents read by the product at this tier: 28.

### What OCR read of the scanned documents

| Document | Format | Words kept | Names and numbers kept (exact) | Kept without accents | OCR text empty |
| --- | --- | --- | --- | --- | --- |
| courriers-entrants/2026-02/2026-02-25_courrier-cardiologie-monnier-veronique.pdf | scan-pdf | 53.2 % | 7 of 9 | 7 | false |
| patients/courriers/2026-03-08_courrier-patient-meunier-veronique.jpg | jpg | 93.9 % | 1 of 3 | 1 | false |
| scans/2026-10/2026-10-07_ordonnance.png | png | 100.0 % | 3 of 3 | 3 | false |

Words kept: **75.0 %**. Names and numbers kept verbatim: **73.3 %** (11 of 15); without accents: 73.3 %.
### Real run, tier 30

**Recall of the candidate set, on the files that hold the answer** (cited with the base off AND known to hold the answer): **91.6 %** (87 of 95), over 58 questions the base would reduce; 8 of them lost at least one such file. Lost, by reason: {"not_linked_to_the_name": 8}

Raw shadow measure (every cited file, filler included): 31.3 % (109 of 348). Filler the reduced set dropped: 231; filler it kept: 22.

| Category | Relevant cited files | Kept by the candidate set | Recall | Filler dropped | Filler kept |
| --- | --- | --- | --- | --- | --- |
| alias | 8 | 6 | 75.0 % | 4 | 0 |
| homonym | 12 | 9 | 75.0 % | 6 | 0 |
| identifier | 9 | 9 | 100.0 % | 33 | 0 |
| mixed | 6 | 5 | 83.3 % | 12 | 0 |
| multi_entity | 6 | 6 | 100.0 % | 20 | 10 |
| ocr | 4 | 3 | 75.0 % | 19 | 1 |
| organization | 6 | 6 | 100.0 % | 17 | 1 |
| person | 21 | 20 | 95.2 % | 84 | 9 |
| practitioner | 9 | 9 | 100.0 % | 26 | 1 |
| term | 14 | 14 | 100.0 % | 10 | 0 |

Questions that search the documents: 108. Reduced in `auto`: 58. Searched again on the whole selection after a reduction (widened): 16.

- The `auto` answer still cites the sources the base-off answer cites (on the 58 reduced questions): **52.6 %** (183 of 348). Same first source: 48 of 58.
- Questions with a known answer in the library: 97. Share of cited files that really hold the answer: base off **30.0 %**, `auto` **40.4 %**. At least one right source: off 95, auto 95.

| Mode | Embedding of the question, median / p95 ms | Knowledge lookup, median / p95 ms | Search, median / p95 ms | Chunks scored, median | Files searched, median |
| --- | --- | --- | --- | --- | --- |
| off | 301.8 / 395.9 | 0.0 / 0.0 | 0.9 / 2.4 | 28.0 | 28.0 |
| suggest | 301.8 / 395.9 | 6.0 / 23.0 | 1.0 / 3.1 | 28.0 | 28.0 |
| auto | 301.8 / 395.9 | 5.0 / 18.0 | 1.3 / 3.9 | 28.0 | 5.0 |

Lost sources (first 60):
- Q025 [person] a file that holds the answer, cited with the base off, not in the candidate set: patients/courriers/2026-03-08_courrier-patient-meunier-veronique.jpg (not_linked_to_the_name)
- Q065 [homonym] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (not_linked_to_the_name)
- Q066 [homonym] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (not_linked_to_the_name)
- Q067 [homonym] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (not_linked_to_the_name)
- Q074 [alias] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (not_linked_to_the_name)
- Q075 [alias] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (not_linked_to_the_name)
- Q177 [ocr] a file that holds the answer, cited with the base off, not in the candidate set: patients/courriers/2026-03-08_courrier-patient-meunier-veronique.jpg (not_linked_to_the_name)
- Q192 [mixed] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (not_linked_to_the_name)
