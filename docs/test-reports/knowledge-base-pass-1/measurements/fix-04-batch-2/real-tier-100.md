## Tier 100

### Analyse

- to 30 documents: 24756 ms. {"pass":"documents","filesScanned":28,"filesProcessed":28,"hashMs":0,"extractMs":142,"ocrMs":3870,"chunkMs":0,"embedMs":19974,"embedBatches":28,"embeddedChars":16667,"writeMs":413,"buildInventoryMs":0,"chunksTotal":28,"chunksRepeatingEarlierText":0,"totalMs":24707,"slowestFiles":[{"position":12,"totalMs":3869,"extractMs":141,"ocrMs":2947,"chunkMs":0,"embedMs":762,"writeMs":18,"chunks":1},{"position":24,"totalMs":1110,"extractMs":0,"ocrMs":462,"chunkMs":0,"embedMs":630,"writeMs":18,"chunks":1},{"position":14,"totalMs":1073,"extractMs":0,"ocrMs":0,"chunkMs":0,"embedMs":1051,"writeMs":21,"chunks":1},{"position":10,"totalMs":1046,"extractMs":0,"ocrMs":0,"chunkMs":0,"embedMs":1027,"writeMs":18,"chunks":1},{"position":27,"totalMs":1017,"extractMs":0,"ocrMs":461,"chunkMs":0,"embedMs":538,"writeMs":17,"chunks":1}]}
- to 100 documents: 622714 ms. {"pass":"documents","filesScanned":89,"filesProcessed":61,"hashMs":2,"extractMs":642,"ocrMs":7386,"chunkMs":2,"embedMs":611256,"embedBatches":109,"embeddedChars":826219,"writeMs":1616,"buildInventoryMs":0,"chunksTotal":850,"chunksRepeatingEarlierText":0,"totalMs":622688,"slowestFiles":[{"position":53,"totalMs":332016,"extractMs":151,"ocrMs":0,"chunkMs":1,"embedMs":331501,"writeMs":358,"chunks":460},{"position":54,"totalMs":238790,"extractMs":6,"ocrMs":0,"chunkMs":1,"embedMs":238573,"writeMs":206,"chunks":331},{"position":23,"totalMs":2990,"extractMs":54,"ocrMs":1976,"chunkMs":0,"embedMs":940,"writeMs":19,"chunks":1},{"position":86,"totalMs":2452,"extractMs":98,"ocrMs":1496,"chunkMs":0,"embedMs":824,"writeMs":32,"chunks":1},{"position":87,"totalMs":2057,"extractMs":108,"ocrMs":1069,"chunkMs":0,"embedMs":839,"writeMs":39,"chunks":1}]}

Documents read by the product at this tier: 89.

### What OCR read of the scanned documents

| Document | Format | Words kept | Names and numbers kept (exact) | Kept without accents | OCR text empty |
| --- | --- | --- | --- | --- | --- |
| courriers-entrants/2026-02/2026-02-25_courrier-cardiologie-monnier-veronique.pdf | scan-pdf | 53.2 % | 7 of 9 | 7 | false |
| patients/courriers/2026-03-08_courrier-patient-meunier-veronique.jpg | jpg | 93.9 % | 1 of 3 | 1 | false |
| scans/2026-10/2026-10-07_ordonnance.png | png | 100.0 % | 3 of 3 | 3 | false |
| laboratoire/2026-06/2026-06-07_resultats-analyses-favre-laurent.jpg | jpg | 77.4 % | 3 of 5 | 4 | false |
| laboratoire/2026-10/2026-10-04_resultats-analyses-maillard-pascal.pdf | scan-pdf | 60.6 % | 2 of 5 | 3 | false |
| laboratoire/2026-03/2026-03-23_resultats-analyses-n-diaye-aicha.pdf | scan-pdf | 90.6 % | 4 of 5 | 4 | false |
| courriers-entrants/2026-01/2026-01-11_courrier-endocrinologie-hasanovic-anna.pdf | scan-pdf | 82.5 % | 8 of 9 | 8 | false |
| scans/2026-04/2026-04-14_courrier-endocrinologie.pdf | scan-pdf | 91.4 % | 9 of 9 | 9 | false |
| scans/2026-08/2026-08-30_courrier-orl-chappuis-nathalie.pdf | scan-pdf | 96.2 % | 8 of 9 | 8 | false |
| caisses-assurances/france/2026-04-24_courrier-cpam69.png | png | 98.4 % | 3 of 3 | 3 | false |

Words kept: **84.3 %**. Names and numbers kept verbatim: **80.0 %** (48 of 60); without accents: 83.3 %.
### Real run, tier 100

**Recall of the candidate set, on the files that hold the answer** (cited with the base off AND known to hold the answer): **97.0 %** (191 of 197), over 113 questions the base would reduce; 2 of them lost at least one such file. Lost, by reason: {"capped": 5, "not_linked_to_the_name": 1}

Raw shadow measure (every cited file, filler included): 31.5 % (202 of 642). Filler the reduced set dropped: 434; filler it kept: 11.

| Category | Relevant cited files | Kept by the candidate set | Recall | Filler dropped | Filler kept |
| --- | --- | --- | --- | --- | --- |
| alias | 14 | 14 | 100.0 % | 34 | 0 |
| homonym | 12 | 12 | 100.0 % | 6 | 0 |
| identifier | 11 | 11 | 100.0 % | 38 | 2 |
| leak | 16 | 16 | 100.0 % | 12 | 0 |
| mixed | 8 | 7 | 87.5 % | 10 | 0 |
| multi_entity | 3 | 3 | 100.0 % | 7 | 6 |
| ocr | 7 | 7 | 100.0 % | 29 | 0 |
| org_vs_person | 1 | 1 | 100.0 % | 5 | 0 |
| organization | 28 | 28 | 100.0 % | 43 | 0 |
| person | 75 | 75 | 100.0 % | 224 | 3 |
| practitioner | 22 | 17 | 77.3 % | 26 | 0 |

Questions that search the documents: 184. Reduced in `auto`: 113. Searched again on the whole selection after a reduction (widened): 41.

- The `auto` answer still cites the sources the base-off answer cites (on the 113 reduced questions): **57.9 %** (372 of 642). Same first source: 95 of 113.
- Questions with a known answer in the library: 173. Share of cited files that really hold the answer: base off **37.0 %**, `auto` **54.7 %**. At least one right source: off 161, auto 163.

| Mode | Embedding of the question, median / p95 ms | Knowledge lookup, median / p95 ms | Search, median / p95 ms | Chunks scored, median | Files searched, median | Passages in the prompt, median | Characters of passages, median / p95 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| off | 322.6 / 412.3 | 0.0 / 0.0 | 28.9 / 54.1 | 878.0 | 89.0 | 6.0 | 3932.0 / 5526.0 |
| suggest | 322.6 / 412.3 | 8.0 / 16.0 | 22.5 / 36.8 | 878.0 | 89.0 | 6.0 | 3932.0 / 5526.0 |
| auto | 322.6 / 412.3 | 9.0 / 19.0 | 19.6 / 49.0 | 878.0 | 3.0 | 6.0 | 3182.0 / 5538.0 |

Lost sources (first 60):
- Q102 [practitioner] a file that holds the answer, cited with the base off, not in the candidate set: patients/courriers/2026-02-06_courrier-patient-zufferey-stephane.docx (capped)
- Q102 [practitioner] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/pharmacies/2026-08-26_courrier-pharmacie.pdf (capped)
- Q102 [practitioner] a file that holds the answer, cited with the base off, not in the candidate set: scans/2026-10/2026-10-07_ordonnance.png (capped)
- Q102 [practitioner] a file that holds the answer, cited with the base off, not in the candidate set: messagerie/2026-08-27_message.txt (capped)
- Q102 [practitioner] a file that holds the answer, cited with the base off, not in the candidate set: reunions/2026-09-01_pv-reunion-cabinet.docx (capped)
- Q192 [mixed] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (not_linked_to_the_name)
