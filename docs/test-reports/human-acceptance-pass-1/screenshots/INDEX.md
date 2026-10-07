# Screenshot index (HAP-1)

51 PNG files, `S01.png` .. `S51.png`, **in chronological order** (S01 is the first screenshot of the pass,
14:58; S51 the last, about 19:05). They were copied from the clipboard-tool folder after the owner had to
fork the chat session, which lost the numbering written in the original notes. The correspondence below was
rebuilt by opening every image and reading the timestamp of the question, the question text, the model
selector and the sidebar selection that each one shows.

Every screenshot shows the full application window (documents folder card, data folder card, conversation,
model selector). Times are the workstation clock, format dd-mm-yyyy hh:mm:ss.

| File | Q | Time of the question | Model | What it shows |
| --- | --- | --- | --- | --- |
| S01 | Q1 | 14:58:24 | gemma2:2b | "Votre dossier de documents contient 6 fichiers." All 6 documents ticked |
| S02 | Q2 | 14:59:19 | gemma2:2b | Q1 and Q2 answers: 6 files, 5 PDF |
| S03 | Q3 | 15:00:11 | gemma2:2b | After sending, the view shows Q2's answer and the Q3 question; the Q3 answer is below the fold (BUG-08) |
| S04 | Q3 | 15:00:11 | gemma2:2b | The six-file list, visible after a manual scroll |
| S05 | Q4 | 15:07:28 | gemma2:2b | Q3 list at the top, Q4 question at the bottom, "go to latest" arrow shown: answer hidden (BUG-08) |
| S06 | Q4 | 15:07:28 | gemma2:2b | "Plusieurs fichiers s'appellent neurologie.pdf. Lequel voulez-vous ?" with the two paths |
| S07 | Q5 | 15:25:07 | gemma2:2b | CPAM answer, quotation form, "Généré par gemma2:2b en 1m52s", Sources disclosure |
| S08 | Q6 | 15:28:41 | gemma2:2b | Convention summary, 1m34s; the "durée du bail" claim visible |
| S09 | Q7 | 15:31:32 | gemma2:2b | Lease duration answer, 1m24s |
| S10 | Q8-a | 15:35:05 | gemma2:2b | "facturée 1 000,00 euros HT", 1m22s, all 6 documents ticked |
| S11 | Q8-b | 15:40:36 | ministral-3:3b | Red banner "Cela n'a pas fonctionné. Le modèle n'a pas répondu correctement. Réessayez." (BUG-11) |
| S12 | Q8-c | 15:47:51 | ministral-3:3b | Third attempt streaming; the answer is cut at the bottom edge, "go to latest" arrow shown (BUG-08) |
| S13 | Q8-c | 15:47:51 | ministral-3:3b | Complete answer: 1 200,00 € HT, printer alone not priced, "Généré par ministral-3:3b en 3m5s" |
| S14 | Q8-d | 16:06:38 | ministral-3:3b | Quote unticked ("Documents utilisés : 5 fichiers"); question sent, the answer is below the fold (BUG-08) |
| S15 | Q8-d | 16:06:38 | ministral-3:3b | "Aucun document dans le WORK_FOLDER_CONTEXT ne mentionne..." 2m28s (BUG-10) |
| S16 | Q8-e | 16:21:33 | gemma2:2b | Quote unticked, memory kept: "facturée 1 000,00 euros HT ... [extrait 2]" 1m45s (BUG-09) |
| S17 | Q8-f | 16:29:00 | gemma2:2b | Conversation cleared first: "Aucun document ne mentionne le prix ... extrait." 1m13s |
| S18 | Q9 | 16:37:34 | gemma2:2b | Both workbooks ticked: "Plusieurs tableurs sont sélectionnés. Lequel voulez-vous ?" |
| S19 | Q10 | 16:56:07 | gemma2:2b | Invoices only: maximum 620 over 8 rows (the earlier question above is the Q9 refusal) |
| S20 | Q11 | 16:57:05 | gemma2:2b | "Plus grand total de montant par fournisseur : MedSupply, avec 1 450." |
| S21 | Q9 | 16:58:50 | gemma2:2b | Invoices only: "Somme de montant : 2 215." over 8 rows |
| S22 | Q10 | 17:02:13 | gemma2:2b | Maximum 620 again (second run, after S21) |
| S23 | Q11 | 17:03:42 | gemma2:2b | MedSupply 1 450 again |
| S24 | Q12 | 17:05:03 | gemma2:2b | Totals per supplier |
| S25 | Q13 | 17:06:33 | gemma2:2b | Appointments only: 3 on a Monday, "Compris comme : date un lundi." |
| S26 | Q14 | 17:08:16 | gemma2:2b | Date range: 0 rows, "Compris comme : date entre 2026-03-09 et 2026-03-15 et date en 2026 et duree_min = 15 et date en 2026." (BUG-02) |
| S27 | Q15 | 17:10:46 | gemma2:2b | Mean duree_min 24,5 over 10 rows (Q14's answer above) |
| S28 | Q16 | 17:11:48 | gemma2:2b | "Salle 2, avec 30 ... Compris comme : patient = Martin A." (BUG-02) |
| S29 | Q17 | 17:12:58 | gemma2:2b | Rows per room 5 / 3 / 2 (Q16's tail above) |
| S30 | Q18 | 17:15:16 | gemma2:2b | Invoices only: 3 rows, "fournisseur = MedSupply" (after rooms question above) |
| S31 | Q18 | 17:18:25 | gemma2:2b | Appointments ticked again: "Recherche" spinner with a Stop button that does nothing (BUG-01) |
| S32 | Q18 | 17:38:27 | gemma2:2b | After restarting the app, invoices only: 3 rows |
| S33 | Q19 | 17:42:28 | gemma2:2b | "cinq mille": « liste des fournisseurs » ne correspond à aucune valeur réelle ..., model attempt 40s |
| S34 | Q19 | 17:45:58 | ministral-3:3b | "Je ne peux pas répondre directement ...", model attempt 1m25s |
| S35 | Q19 | 17:57:01 | ministral-3:3b | "5000": "Nombre de lignes : 0 ... Compris comme : date en 5000." (BUG-02) |
| S36 | Q20 | 18:01:58 | gemma2:2b | "Cette feuille n'existe pas dans factures-fournisseurs-2026.xlsx.", model attempt 40s |
| S37 | Q21 | 18:05:23 | gemma2:2b | View still on Q20's answer; the Q21 question at the bottom, answer hidden (BUG-08) |
| S38 | Q21 | 18:05:23 | gemma2:2b | "« Alfa » ne correspond à aucune valeur réelle ...", "Votre question précédente n'a pas pu être calculée non plus", model attempt 17s |
| S39 | Q21 | later | ministral-3:3b | Same answer, model attempt 1m12s |
| S40 | Q23 | 18:14:04 | gemma2:2b | Quote + invoices ticked: "Somme de montant : 1 450 ... fournisseur = MedSupply", with "les documents sélectionnés n'étaient pas nécessaires" (BUG-03) |
| S41 | Q24 | 18:21:34 | gemma2:2b | Zoomed crop of the left panel and the beginning of the Q24 refusal |
| S42 | Q24 | 18:21:34 | gemma2:2b | "Cette colonne de factures-fournisseurs-2026.xlsx ne contient pas que des nombres ..." (BUG-05) |
| S43 | Q26 | 18:34:21 | gemma2:2b | Gateway stopped (`docker compose down`): same refusal as Q24, plus the red banner "Aucune réponse de l'IA à l'adresse http://127.0.0.1:8080 ..." overflowing the sidebar (UX-2) |
| S44 | Q27 | 18:45:15 | gemma2:2b | "Pour créer un e-mail de relance, il faudrait que vous choisissiez un document ..." 35s, "Réponse sans vos documents." |
| S45 | Q27 | 18:45:15 | gemma2:2b | Same screen captured a second time (window inactive); duplicate of S44 |
| S46 | Q27 | 18:49:41 | ministral-3:3b | The drafted e-mail, cut at the bottom edge with the "go to latest" arrow (BUG-08) |
| S47 | Q27 | 18:49:41 | ministral-3:3b | Complete e-mail with placeholders, 1m22s |
| S48 | Q28 | 18:59:37 | gemma2:2b | Question visible with the arrow; the answer is below the fold (BUG-08) |
| S49 | Q28 | 18:59:37 | gemma2:2b | "Aucun document sélectionné pour cette conversation." (BUG-07) |
| S50 | Q28 | 19:05:06 | ministral-3:3b | Second question with the other model, answer below the fold |
| S51 | Q28 | 19:05:06 | ministral-3:3b | Both answers are "Aucun document sélectionné pour cette conversation." |

S39's question timestamp is not visible in the capture (the answer is shown; it follows S38 and precedes
S40 at 18:14).

## Concordance with the numbers written in the owner's original notes

The owner's notes cited screenshot numbers 1 to 46. They match this index exactly up to S18 and then drift
(the real captures include repeated runs the notes did not number). Use this table to translate an old
reference.

| Owner's note says | Meaning | File(s) here |
| --- | --- | --- |
| Screenshot 1, 2 | Q1, Q2 | S01, S02 |
| Screenshot 3, 4 | Q3 (scroll, then answer) | S03, S04 |
| Screenshot 5, 6 | Q4 (scroll, then answer) | S05, S06 |
| Screenshot 7, 8, 9 | Q5, Q6, Q7 | S07, S08, S09 |
| Screenshot 10, 11, 12, 13 | Q8-a, Q8-b, Q8-c streaming, Q8-c final | S10, S11, S12, S13 |
| Screenshot 14, 15 | Q8-d hidden, then visible | S14, S15 |
| Screenshot 16, 17 | Q8-e, Q8-f | S16, S17 |
| Screenshot 18 | Q9, both workbooks | S18 |
| Screenshot 19 | Q9 with one workbook | **S21** (S19 is Q10) |
| Screenshot 20, 21, 22 | Q10, Q11, Q12 | S19 or S22, S20 or S23, S24 |
| Screenshot 23 | Q13 and Q14 | S25 (Q13), S26 (Q14) |
| Screenshot 24, 25, 26 | Q15, Q16, Q17 | S27, S28, S29 |
| Screenshot 27 | Q18 answered | S30 |
| Screenshot 28 | Q18 hang | S31 |
| Screenshot 29 | Q18 after restart | S32 |
| Screenshot 30, 31, 32 | Q19 gemma, ministral, "5000" | S33, S34, S35 |
| Screenshot 33 | Q20 | S36 |
| Screenshot 34, 35, 36 | Q21 scroll, answer, ministral | S37, S38, S39 |
| Screenshot 37 | Q23 | S40 |
| Screenshot 38 | Q24 | S41 (zoom), S42 |
| Screenshot 39 | Q26 | S43 |
| Screenshot 40, 41, 42 | Q27 gemma, ministral cut, ministral complete | S44 (S45 duplicate), S46, S47 |
| Screenshot 43, 44 | Q28 gemma scroll, answer | S48, S49 |
| Screenshot 45, 46 | Q28 ministral scroll, answer | S50, S51 |

Q10 and Q11 each appear twice (16:56 and 16:57, then again at 17:02 and 17:03). The notes do not say why the
sequence was repeated; both runs gave the same answers.
