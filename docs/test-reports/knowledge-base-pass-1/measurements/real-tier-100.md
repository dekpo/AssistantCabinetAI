## Tier 100

### Analyse

- to 30 documents: 24583 ms. {"pass":"documents","filesScanned":28,"filesProcessed":28,"hashMs":0,"extractMs":99,"ocrMs":3908,"chunkMs":0,"embedMs":19801,"embedBatches":28,"embeddedChars":16667,"writeMs":444,"buildInventoryMs":0,"chunksTotal":28,"chunksRepeatingEarlierText":0,"totalMs":24556,"slowestFiles":[{"position":12,"totalMs":4100,"extractMs":93,"ocrMs":3145,"chunkMs":0,"embedMs":832,"writeMs":28,"chunks":1},{"position":13,"totalMs":1127,"extractMs":1,"ocrMs":0,"chunkMs":0,"embedMs":1099,"writeMs":25,"chunks":1},{"position":9,"totalMs":1111,"extractMs":1,"ocrMs":0,"chunkMs":0,"embedMs":1083,"writeMs":25,"chunks":1},{"position":11,"totalMs":1092,"extractMs":0,"ocrMs":0,"chunkMs":0,"embedMs":1069,"writeMs":23,"chunks":1},{"position":15,"totalMs":1084,"extractMs":1,"ocrMs":0,"chunkMs":0,"embedMs":1062,"writeMs":20,"chunks":1}]}
- to 100 documents: 640191 ms. {"pass":"documents","filesScanned":89,"filesProcessed":61,"hashMs":0,"extractMs":754,"ocrMs":7502,"chunkMs":2,"embedMs":629394,"embedBatches":109,"embeddedChars":826219,"writeMs":1607,"buildInventoryMs":0,"chunksTotal":850,"chunksRepeatingEarlierText":0,"totalMs":640157,"slowestFiles":[{"position":53,"totalMs":341402,"extractMs":258,"ocrMs":0,"chunkMs":1,"embedMs":340934,"writeMs":205,"chunks":460},{"position":54,"totalMs":244252,"extractMs":5,"ocrMs":0,"chunkMs":1,"embedMs":244100,"writeMs":144,"chunks":331},{"position":23,"totalMs":3419,"extractMs":124,"ocrMs":2304,"chunkMs":0,"embedMs":956,"writeMs":33,"chunks":1},{"position":86,"totalMs":2433,"extractMs":73,"ocrMs":1522,"chunkMs":0,"embedMs":799,"writeMs":37,"chunks":1},{"position":87,"totalMs":2010,"extractMs":99,"ocrMs":975,"chunkMs":0,"embedMs":883,"writeMs":51,"chunks":1}]}

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

**Recall of the candidate set, on the files that hold the answer** (cited with the base off AND known to hold the answer): **95.9 %** (186 of 194), over 103 questions the base would reduce; 4 of them lost at least one such file. Lost, by reason: {"capped": 5, "not_linked_to_the_name": 3}

Raw shadow measure (every cited file, filler included): 38.0 % (226 of 594). Filler the reduced set dropped: 360; filler it kept: 40.

| Category | Relevant cited files | Kept by the candidate set | Recall | Filler dropped | Filler kept |
| --- | --- | --- | --- | --- | --- |
| alias | 14 | 14 | 100.0 % | 34 | 0 |
| homonym | 12 | 12 | 100.0 % | 6 | 0 |
| identifier | 7 | 7 | 100.0 % | 29 | 0 |
| leak | 11 | 11 | 100.0 % | 11 | 0 |
| mixed | 8 | 7 | 87.5 % | 10 | 0 |
| multi_entity | 3 | 3 | 100.0 % | 5 | 8 |
| ocr | 7 | 6 | 85.7 % | 26 | 3 |
| organization | 14 | 14 | 100.0 % | 13 | 5 |
| person | 75 | 74 | 98.7 % | 205 | 22 |
| practitioner | 20 | 15 | 75.0 % | 20 | 2 |
| term | 23 | 23 | 100.0 % | 1 | 0 |

Questions that search the documents: 175. Reduced in `auto`: 103. Searched again on the whole selection after a reduction (widened): 37.

- The `auto` answer still cites the sources the base-off answer cites (on the 103 reduced questions): **61.3 %** (364 of 594). Same first source: 90 of 103.
- Questions with a known answer in the library: 164. Share of cited files that really hold the answer: base off **37.4 %**, `auto` **51.7 %**. At least one right source: off 154, auto 155.

| Mode | Embedding of the question, median / p95 ms | Knowledge lookup, median / p95 ms | Search, median / p95 ms | Chunks scored, median | Files searched, median |
| --- | --- | --- | --- | --- | --- |
| off | 299.4 / 376.0 | 0.0 / 0.0 | 22.5 / 53.3 | 878.0 | 89.0 |
| suggest | 299.4 / 376.0 | 7.0 / 20.0 | 18.6 / 46.7 | 878.0 | 89.0 |
| auto | 299.4 / 376.0 | 6.0 / 17.0 | 16.0 / 34.4 | 878.0 | 5.0 |

Lost sources (first 60):
- Q025 [person] a file that holds the answer, cited with the base off, not in the candidate set: patients/courriers/2026-03-08_courrier-patient-meunier-veronique.jpg (not_linked_to_the_name)
- Q102 [practitioner] a file that holds the answer, cited with the base off, not in the candidate set: patients/courriers/2026-02-06_courrier-patient-zufferey-stephane.docx (capped)
- Q102 [practitioner] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/pharmacies/2026-08-26_courrier-pharmacie.pdf (capped)
- Q102 [practitioner] a file that holds the answer, cited with the base off, not in the candidate set: scans/2026-10/2026-10-07_ordonnance.png (capped)
- Q102 [practitioner] a file that holds the answer, cited with the base off, not in the candidate set: messagerie/2026-08-27_message.txt (capped)
- Q102 [practitioner] a file that holds the answer, cited with the base off, not in the candidate set: reunions/2026-09-01_pv-reunion-cabinet.docx (capped)
- Q177 [ocr] a file that holds the answer, cited with the base off, not in the candidate set: patients/courriers/2026-03-08_courrier-patient-meunier-veronique.jpg (not_linked_to_the_name)
- Q192 [mixed] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (not_linked_to_the_name)
