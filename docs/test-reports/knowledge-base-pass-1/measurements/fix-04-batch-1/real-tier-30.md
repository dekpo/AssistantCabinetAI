## Tier 30

### Analyse

- to 30 documents: 35843 ms. {"pass":"documents","filesScanned":28,"filesProcessed":28,"hashMs":0,"extractMs":112,"ocrMs":4804,"chunkMs":0,"embedMs":30025,"embedBatches":28,"embeddedChars":16667,"writeMs":501,"buildInventoryMs":0,"chunksTotal":28,"chunksRepeatingEarlierText":0,"totalMs":35808,"slowestFiles":[{"position":0,"totalMs":7343,"extractMs":0,"ocrMs":0,"chunkMs":0,"embedMs":7334,"writeMs":8,"chunks":1},{"position":12,"totalMs":4798,"extractMs":104,"ocrMs":3775,"chunkMs":0,"embedMs":892,"writeMs":25,"chunks":1},{"position":13,"totalMs":1332,"extractMs":0,"ocrMs":0,"chunkMs":0,"embedMs":1308,"writeMs":23,"chunks":1},{"position":27,"totalMs":1235,"extractMs":0,"ocrMs":620,"chunkMs":0,"embedMs":592,"writeMs":22,"chunks":1},{"position":15,"totalMs":1196,"extractMs":1,"ocrMs":0,"chunkMs":0,"embedMs":1175,"writeMs":18,"chunks":1}]}

Documents read by the product at this tier: 28.

### What OCR read of the scanned documents

| Document | Format | Words kept | Names and numbers kept (exact) | Kept without accents | OCR text empty |
| --- | --- | --- | --- | --- | --- |
| courriers-entrants/2026-02/2026-02-25_courrier-cardiologie-monnier-veronique.pdf | scan-pdf | 53.2 % | 7 of 9 | 7 | false |
| patients/courriers/2026-03-08_courrier-patient-meunier-veronique.jpg | jpg | 93.9 % | 1 of 3 | 1 | false |
| scans/2026-10/2026-10-07_ordonnance.png | png | 100.0 % | 3 of 3 | 3 | false |

Words kept: **75.0 %**. Names and numbers kept verbatim: **73.3 %** (11 of 15); without accents: 73.3 %.
### Real run, tier 30

**Recall of the candidate set, on the files that hold the answer** (cited with the base off AND known to hold the answer): **90.4 %** (75 of 83), over 57 questions the base would reduce; 8 of them lost at least one such file. Lost, by reason: {"not_linked_to_the_name": 8}

Raw shadow measure (every cited file, filler included): 28.1 % (96 of 342). Filler the reduced set dropped: 238; filler it kept: 21.

| Category | Relevant cited files | Kept by the candidate set | Recall | Filler dropped | Filler kept |
| --- | --- | --- | --- | --- | --- |
| alias | 8 | 6 | 75.0 % | 4 | 0 |
| homonym | 12 | 9 | 75.0 % | 6 | 0 |
| identifier | 12 | 12 | 100.0 % | 47 | 1 |
| mixed | 6 | 5 | 83.3 % | 12 | 0 |
| multi_entity | 6 | 6 | 100.0 % | 20 | 10 |
| ocr | 4 | 3 | 75.0 % | 19 | 1 |
| organization | 5 | 5 | 100.0 % | 13 | 0 |
| person | 21 | 20 | 95.2 % | 84 | 9 |
| practitioner | 9 | 9 | 100.0 % | 33 | 0 |

Questions that search the documents: 116. Reduced in `auto`: 57. Searched again on the whole selection after a reduction (widened): 13.

- The `auto` answer still cites the sources the base-off answer cites (on the 57 reduced questions): **47.1 %** (161 of 342). Same first source: 46 of 57.
- Questions with a known answer in the library: 105. Share of cited files that really hold the answer: base off **28.8 %**, `auto` **39.0 %**. At least one right source: off 102, auto 102.

| Mode | Embedding of the question, median / p95 ms | Knowledge lookup, median / p95 ms | Search, median / p95 ms | Chunks scored, median | Files searched, median | Passages in the prompt, median | Characters of passages, median / p95 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| off | 357.5 / 513.2 | 0.0 / 0.0 | 1.3 / 2.9 | 28.0 | 28.0 | 6.0 | 3689.0 / 5141.0 |
| suggest | 357.5 / 513.2 | 7.0 / 25.0 | 1.2 / 2.6 | 28.0 | 28.0 | 6.0 | 3689.0 / 5141.0 |
| auto | 357.5 / 513.2 | 6.0 / 19.0 | 1.4 / 4.4 | 28.0 | 7.0 | 6.0 | 3008.0 / 4823.0 |

Lost sources (first 60):
- Q025 [person] a file that holds the answer, cited with the base off, not in the candidate set: patients/courriers/2026-03-08_courrier-patient-meunier-veronique.jpg (not_linked_to_the_name)
- Q065 [homonym] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (not_linked_to_the_name)
- Q066 [homonym] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (not_linked_to_the_name)
- Q067 [homonym] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (not_linked_to_the_name)
- Q074 [alias] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (not_linked_to_the_name)
- Q075 [alias] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (not_linked_to_the_name)
- Q177 [ocr] a file that holds the answer, cited with the base off, not in the candidate set: patients/courriers/2026-03-08_courrier-patient-meunier-veronique.jpg (not_linked_to_the_name)
- Q192 [mixed] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (not_linked_to_the_name)
