## Tier 100

### Analyse

- to 30 documents: 35843 ms. {"pass":"documents","filesScanned":28,"filesProcessed":28,"hashMs":0,"extractMs":112,"ocrMs":4804,"chunkMs":0,"embedMs":30025,"embedBatches":28,"embeddedChars":16667,"writeMs":501,"buildInventoryMs":0,"chunksTotal":28,"chunksRepeatingEarlierText":0,"totalMs":35808,"slowestFiles":[{"position":0,"totalMs":7343,"extractMs":0,"ocrMs":0,"chunkMs":0,"embedMs":7334,"writeMs":8,"chunks":1},{"position":12,"totalMs":4798,"extractMs":104,"ocrMs":3775,"chunkMs":0,"embedMs":892,"writeMs":25,"chunks":1},{"position":13,"totalMs":1332,"extractMs":0,"ocrMs":0,"chunkMs":0,"embedMs":1308,"writeMs":23,"chunks":1},{"position":27,"totalMs":1235,"extractMs":0,"ocrMs":620,"chunkMs":0,"embedMs":592,"writeMs":22,"chunks":1},{"position":15,"totalMs":1196,"extractMs":1,"ocrMs":0,"chunkMs":0,"embedMs":1175,"writeMs":18,"chunks":1}]}
- to 100 documents: 609123 ms. {"pass":"documents","filesScanned":89,"filesProcessed":61,"hashMs":0,"extractMs":809,"ocrMs":7676,"chunkMs":2,"embedMs":598170,"embedBatches":109,"embeddedChars":826219,"writeMs":1461,"buildInventoryMs":0,"chunksTotal":850,"chunksRepeatingEarlierText":0,"totalMs":609085,"slowestFiles":[{"position":53,"totalMs":338891,"extractMs":289,"ocrMs":0,"chunkMs":1,"embedMs":338382,"writeMs":214,"chunks":460},{"position":54,"totalMs":211627,"extractMs":2,"ocrMs":0,"chunkMs":1,"embedMs":211473,"writeMs":147,"chunks":331},{"position":23,"totalMs":3818,"extractMs":125,"ocrMs":2522,"chunkMs":0,"embedMs":1139,"writeMs":30,"chunks":1},{"position":86,"totalMs":2593,"extractMs":78,"ocrMs":1541,"chunkMs":0,"embedMs":947,"writeMs":25,"chunks":1},{"position":17,"totalMs":2351,"extractMs":0,"ocrMs":0,"chunkMs":0,"embedMs":2334,"writeMs":16,"chunks":1}]}

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

**Recall of the candidate set, on the files that hold the answer** (cited with the base off AND known to hold the answer): **95.6 %** (173 of 181), over 103 questions the base would reduce; 4 of them lost at least one such file. Lost, by reason: {"capped": 5, "not_linked_to_the_name": 3}

Raw shadow measure (every cited file, filler included): 35.0 % (207 of 591). Filler the reduced set dropped: 376; filler it kept: 34.

| Category | Relevant cited files | Kept by the candidate set | Recall | Filler dropped | Filler kept |
| --- | --- | --- | --- | --- | --- |
| alias | 14 | 14 | 100.0 % | 34 | 0 |
| homonym | 12 | 12 | 100.0 % | 6 | 0 |
| identifier | 11 | 11 | 100.0 % | 39 | 1 |
| leak | 16 | 16 | 100.0 % | 12 | 0 |
| mixed | 8 | 7 | 87.5 % | 10 | 0 |
| multi_entity | 3 | 3 | 100.0 % | 5 | 8 |
| ocr | 7 | 6 | 85.7 % | 26 | 3 |
| organization | 13 | 13 | 100.0 % | 13 | 0 |
| person | 75 | 74 | 98.7 % | 205 | 22 |
| practitioner | 22 | 17 | 77.3 % | 26 | 0 |

Questions that search the documents: 184. Reduced in `auto`: 103. Searched again on the whole selection after a reduction (widened): 34.

- The `auto` answer still cites the sources the base-off answer cites (on the 103 reduced questions): **58.7 %** (347 of 591). Same first source: 88 of 103.
- Questions with a known answer in the library: 173. Share of cited files that really hold the answer: base off **37.0 %**, `auto` **51.6 %**. At least one right source: off 161, auto 162.

| Mode | Embedding of the question, median / p95 ms | Knowledge lookup, median / p95 ms | Search, median / p95 ms | Chunks scored, median | Files searched, median | Passages in the prompt, median | Characters of passages, median / p95 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| off | 295.4 / 354.3 | 0.0 / 0.0 | 18.6 / 39.0 | 878.0 | 89.0 | 6.0 | 3932.0 / 5526.0 |
| suggest | 295.4 / 354.3 | 6.0 / 13.0 | 18.2 / 33.1 | 878.0 | 89.0 | 6.0 | 3932.0 / 5526.0 |
| auto | 295.4 / 354.3 | 6.0 / 12.0 | 15.4 / 30.7 | 878.0 | 5.0 | 6.0 | 3201.0 / 5511.0 |

Lost sources (first 60):
- Q025 [person] a file that holds the answer, cited with the base off, not in the candidate set: patients/courriers/2026-03-08_courrier-patient-meunier-veronique.jpg (not_linked_to_the_name)
- Q102 [practitioner] a file that holds the answer, cited with the base off, not in the candidate set: patients/courriers/2026-02-06_courrier-patient-zufferey-stephane.docx (capped)
- Q102 [practitioner] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/pharmacies/2026-08-26_courrier-pharmacie.pdf (capped)
- Q102 [practitioner] a file that holds the answer, cited with the base off, not in the candidate set: scans/2026-10/2026-10-07_ordonnance.png (capped)
- Q102 [practitioner] a file that holds the answer, cited with the base off, not in the candidate set: messagerie/2026-08-27_message.txt (capped)
- Q102 [practitioner] a file that holds the answer, cited with the base off, not in the candidate set: reunions/2026-09-01_pv-reunion-cabinet.docx (capped)
- Q177 [ocr] a file that holds the answer, cited with the base off, not in the candidate set: patients/courriers/2026-03-08_courrier-patient-meunier-veronique.jpg (not_linked_to_the_name)
- Q192 [mixed] a file that holds the answer, cited with the base off, not in the candidate set: courriers-entrants/2026-06/2026-06-30_courrier-cardiologie.pdf (not_linked_to_the_name)
