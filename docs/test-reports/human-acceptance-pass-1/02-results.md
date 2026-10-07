# 02 - Results, question by question

How to read an entry: **Setup** (selection, model), **Asked**, **Answer** (verbatim, from the owner's
copy of the conversation), **Screens**, **Check** (against the ground truth in
[fixtures/README.md](fixtures/README.md)), **Verdict**, **Notes** (the owner's observations, and links to
the findings in [03-bug-register.md](03-bug-register.md)).

The owner's own wording of "ok" meant "the application answered", not "the answer is right". The
**Check** lines below are the independent verification the owner asked for.

Timings are the ones the product displays under the answer.

## Summary table

| Q | Verdict | One line |
| --- | --- | --- |
| Q1 | PASS | 6 files |
| Q2 | PASS | 5 PDF |
| Q3 | PASS (UI: scroll) | List correct; answer hidden below the fold (BUG-08) |
| Q4 | PASS (UI: scroll, UX proposal) | Asks which file, correct; choice not clickable (UX-1) |
| Q5 | PASS-WITH-ISSUES | Verbatim quote instead of a summary, `gemma2:2b`, 1m52s |
| Q6 | PASS-WITH-ISSUES | Invents "durée du bail"; omits the 20 %, `gemma2:2b` |
| Q7 | PASS | Exact, `gemma2:2b` |
| Q8-a | FAIL | `gemma2:2b` states 1 000,00 EUR HT; the quote says 1 200,00 total |
| Q8-b | ERROR | `ministral-3:3b` first call: "Le modèle n'a pas répondu correctement" (BUG-11) |
| Q8-c | PASS | `ministral-3:3b`, 3m05s: exact and nuanced |
| Q8-d | PASS-WITH-ISSUES | Quote unticked, `ministral-3:3b`: right refusal, leaks `WORK_FOLDER_CONTEXT` (BUG-10) |
| Q8-e | FAIL | Quote unticked, `gemma2:2b`, memory on: repeats the invented 1 000,00 (BUG-09) |
| Q8-f | PASS | Same, memory cleared: correct refusal |
| Q9 | PASS | Asks which workbook; then 2 215 with one workbook |
| Q10 | PASS | 620 |
| Q11 | PASS | MedSupply, 1 450 |
| Q12 | PASS | Three totals correct |
| Q13 | PASS | 3 |
| Q14 | FAIL | 0 instead of 4: spurious filters (BUG-02) |
| Q15 | PASS | 24,5 |
| Q16 | FAIL | "Salle 2, 30" instead of "Salle 3, 50": spurious filter (BUG-02) |
| Q17 | PASS | 5 / 3 / 2 |
| Q18 | PASS, then CRASH | 3 correct; a later run hung the app (BUG-01) |
| Q19 | FAIL x4 | No usable answer from either model; "5000" gives a meaningless 0 (BUG-02, 13, 14) |
| Q20 | PASS-WITH-ISSUES | Correct refusal for a fixture without that sheet; formula case not testable here |
| Q21 | PASS-WITH-ISSUES | Honest refusal after 17 s / 1m12s of useless model time; misleading advice line |
| Q22 | NOT-RUN | |
| Q23 | FAIL | Only the table half answered; "Demander à l'IA" does nothing (BUG-03, 04) |
| Q24 | FAIL | Refusal "column is not numeric": engine summed `fournisseur` (BUG-05, 06) |
| Q25 | N/A | Not a user action |
| Q26 | NOT-REACHED / FAIL | Gateway stopped, but the mixed tier is never reached; banner is the health banner |
| Q27 | gemma FAIL-ish / ministral PASS | Small model refuses to draft; larger one drafts (BUG-08 on scroll) |
| Q28 | FAIL | Both models: "Aucun document sélectionné" instead of an answer (BUG-07) |

---

## I - Folder questions

### Q1 - `Combien de fichiers au total ?`

- Setup: all 6 documents ticked, `gemma2:2b`, nothing ticked in the Data folder.
- Answer: `Votre dossier de documents contient 6 fichiers.` + "Réponse établie depuis votre dossier de documents, sans l'IA."
- Screens: S01.
- Check: the folder holds 6 files (4 at the root + 2 under `2026/`). Correct.
- Verdict: **PASS**.

### Q2 - `Combien de fichiers PDF ?`

- Answer: `Votre dossier de documents contient 5 fichiers avec l'extension .pdf.`
- Screens: S02.
- Check: 5 PDF (bail, cpam, devis, 2 x neurologie) + 1 DOCX. Correct.
- Verdict: **PASS**.

### Q3 - `Peux-tu me donner la liste des fichiers ?`

- Answer: `Votre dossier de documents contient 6 fichiers :` then the six paths, each followed by "— analysé":
  `2026/janvier/neurologie.pdf`, `2026/mars/neurologie.pdf`, `bail-cabinet-2024.pdf`,
  `convention-remplacement-dr-martin.docx`, `courrier-cpam-radiation.pdf`, `devis-imprimante-medsupply.pdf`.
- Screens: S03 (view after sending: the answer is below the visible area, scrollbar present), S04 (after a manual scroll).
- Check: list and states correct.
- Verdict: **PASS** for content. The owner's observation: the page scrolls *before* the answer is
  rendered, so the answer is not visible and must be scrolled to by hand (BUG-08).

### Q4 - `Que dit neurologie.pdf ?`

- Answer: `Plusieurs fichiers s'appellent neurologie.pdf. Lequel voulez-vous ?` with the two paths as plain bullets.
- Screens: S05 (scroll problem again), S06 (after scrolling).
- Check: two files share that name; the product refuses to choose. Correct and exactly the intended behaviour.
- Verdict: **PASS**. UX proposal from the owner: make each candidate clickable (UX-1, analysed in 05-analysis-notes.md section 5).

---

## II - Content questions (retrieval + model)

All Q5-Q8 runs: all 6 documents ticked unless stated; the Data folder is untouched.

### Q5 - `Que dit le courrier de la CPAM concernant la radiation ?`

- Model: `gemma2:2b`, 1m52s.
- Answer (verbatim):
  `extrait[1]: "CAISSE PRIMAIRE D'ASSURANCE MALADIE DU RHONE (document fictif, usage de test uniquement) Objet : radiation d'un assure du regime general Madame, Monsieur, Nous vous informons que l'assure(e) M. Hugo Exemple, numero de securite sociale fictif 1 85 03 69 123 456 78, a ete radie du regime general a compter du 1er fevrier 2026, suite a son affiliation au regime d'un nouvel employeur."`
- Screens: S07 (Sources disclosure present).
- Check: the quote is word for word from the PDF. It omits the last paragraph (paper forms after 1 Feb go to the new fund). It is a quotation, not an explanation of "what the letter says"; it also repeats the (fictional) social security number, which is faithful to the document.
- Verdict: **PASS-WITH-ISSUES**. Accurate, but a 2B model returns a pasted excerpt headed `extrait[1]:` where a user expects a sentence.

### Q6 - `Résume la convention de remplacement signée avec le Dr Martin.`

- Model: `gemma2:2b`, 1m34s.
- Answer (verbatim):
  `Le Dr Antoine Martin remplacera le Dr Camille Exemple du lundi 6 juillet 2026 au vendredi 31 juillet 2026 inclus. Son honoraires seront fixés à 80% des honoraires encaissés pendant la durée du remplacement. L'accord précise la date de début et de fin du remplacement, ainsi que la durée du bail. Le Dr Martin assurera les consultations du Dr Camille Exemple. [extrait 1]`
- Screens: S08.
- Check: names, the two dates (a Monday and a Friday, both correct) and the 80 % are right. **"ainsi que la durée du bail" is invented**: the DOCX never mentions a lease. The 20 % returned to the titular doctor and the signature date (2 June 2026) are missing. Grammar error: "Son honoraires".
- Verdict: **PASS-WITH-ISSUES** (one fabricated claim). A user reading it would believe the convention states a lease duration.

### Q7 - `Quelle est la durée du bail du cabinet d'après le contrat ?`

- Model: `gemma2:2b`, 1m24s.
- Answer: `La durée du bail est de neuf ans entiers et consecutifs, qui commencera a courir le 1er avril 2024 pour se terminer le 31 mars 2033.`
- Screens: S09, S10 (top).
- Check: identical to the clause in the PDF.
- Verdict: **PASS**.

### Q8 - `Quel est le prix de l'imprimante dans le devis MedSupply ?` (six runs)

Ground truth: the quote gives one total, **1 200,00 euros HT**, covering the printer, 4 cartridges, 10
reams and consumables. There is no separate printer price.

| Run | Selection | Model | Duration | Answer | Screens | Verdict |
| --- | --- | --- | --- | --- | --- | --- |
| a | all 6 documents | `gemma2:2b` | 1m22s | `L'imprimante laser multifonction A4 est facturée 1 000,00 euros HT dans le devis MedSupply.` | S10 | **FAIL**: the figure 1 000,00 does not exist in the document |
| b | all 6 documents | `ministral-3:3b` | n/a | Banner "Cela n'a pas fonctionné. Le modèle n'a pas répondu correctement. Réessayez." | S11 | **ERROR** (BUG-11) |
| c | all 6 documents, idle timeout set to 600 s | `ministral-3:3b` | 3m05s | `extrait[1], page 1 : **Imprimante laser multifonction A4** : non précisé explicitement dans le montant total. **Montant total du devis** : 1 200,00 € HT (comprend les consommables, pas uniquement l'imprimante). *Le prix exact de l'imprimante seule n'est pas indiqué dans ce devis.*` | S12 (truncated while streaming), S13 (complete) | **PASS**: exact and nuanced |
| d | 5 documents, **quote unticked** | `ministral-3:3b` | 2m28s | `Aucun document dans le WORK_FOLDER_CONTEXT ne mentionne le prix de l'imprimante MedSupply.` | S14 (answer hidden by the scroll problem), S15 | **PASS-WITH-ISSUES**: right refusal; internal name `WORK_FOLDER_CONTEXT` shown to the user (BUG-10); scroll (BUG-08) |
| e | 5 documents, quote unticked, **memory kept** | `gemma2:2b` | 1m45s | `L'imprimante laser multifonction A4 est facturée 1 000,00 euros HT dans le devis MedSupply. [extrait 2]` | S16 | **FAIL**: asserts a price from a document that is not selected, with a citation marker `[extrait 2]` |
| f | 5 documents, quote unticked, **conversation cleared first** | `gemma2:2b` | 1m13s | `Aucun document ne mentionne le prix de l'imprimante dans le devis MedSupply. extrait.` | S17 | **PASS** (stray word "extrait." at the end) |

Observations carried by this run (details in 05-analysis-notes.md):

- With the document selected and the right excerpt retrieved, `gemma2:2b` still produced a wrong figure (a); `ministral-3:3b` was exact (c). The larger model was better but 2.4x slower, and its first call after a model switch hit a timeout (b).
- (e) versus (f) are the same model, the same selection and the same question; the only difference is whether the earlier (wrong) answer was in the conversation memory. Memory beat the instruction that forbids answering beyond the excerpts (BUG-09).

---

## III - Data folder, deterministic aggregates

Model selector shows `gemma2:2b` but is not used by these questions: each answer carries "Réponse calculée depuis votre dossier des données, sans l'IA."

### Q9 - `Quelle est la somme des montant ?`

- Both workbooks ticked (S18): `Plusieurs tableurs sont sélectionnés. Lequel voulez-vous ? Nommez-le dans la question :` + the two file names. Correct behaviour; the owner proposes making the choice clickable (UX-1) and notes only one workbook even has a `montant` column (BUG-15).
- Invoices only ticked (S21, 16:58:50): `Somme de montant : **2 215**. Calculé sur 8 lignes.`
- Check: 450+180+620+95+210+380+120+160 = 2 215. Correct.
- Verdict: **PASS**.

### Q10 - `Quel est le maximum de montant ?` (S19 at 16:56:07, S22 at 17:02:13)

- Answer: `Plus grande valeur de montant : **620**. Calculé sur 8 lignes.` Check: max = 620 (MedSupply, 5 Feb). **PASS**.

### Q11 - `Quel fournisseur a le plus de montant ?` (S20 at 16:57:05, S23 at 17:03:42)

- Answer: `Plus grand total de montant par fournisseur : **MedSupply**, avec **1 450**.` + the clarification that it is a total per supplier + the ranking MedSupply 1 450, Fournitures Dupont 550, Papeterie Lefevre 215, "sur 3 groupes en tout", 8 rows.
- Check: matches the hand computation. **PASS**.

### Q12 - `Quel est le total de montant par fournisseur ?` (S24)

- Answer: `Totaux de montant, par groupe :` MedSupply 1 450, Fournitures Dupont 550, Papeterie Lefevre 215; 8 rows. Check: correct. **PASS**.

### Q13 - `Combien de rendez-vous le lundi ?` (S25, rdv-mars only)

- Answer: `Nombre de lignes : **3**. Calculé sur 3 lignes. Compris comme : date un lundi.`
- Check: 2, 9 and 16 March 2026 are Mondays. Correct. **PASS**.

### Q14 - `Combien de rendez-vous entre le 09/03/2026 et le 15/03/2026 ?` (S26)

- Answer: `Nombre de lignes : **0**. Calculé sur 0 ligne. Compris comme : date entre 2026-03-09 et 2026-03-15 et date en 2026 et duree_min = 15 et date en 2026.`
- Check: appointments on 9, 10, 11 and 12 March fall in the range: **expected 4**, got 0.
- The "Compris comme" line shows the cause: besides the date range the engine also applied `date en 2026` (twice) and `duree_min = 15`, from the digits of the dates being read again as separate filters. All filters combine with AND, so nothing matches.
- Verdict: **FAIL** (wrong number, presented as a computed result). BUG-02.

### Q15 - `Quelle est la moyenne de duree_min ?` (S27)

- Answer: `Moyenne de duree_min : **24,5**. Calculé sur 10 lignes.` Check: 245 / 10 = 24.5. **PASS**.

### Q16 - `Quelle salle a le moins de duree_min ?` (S28)

- Answer: `Plus petit total duree_min par salle : **Salle 2**, avec **30**. ... Calculé sur 1 ligne. Compris comme : patient = Martin A.`
- Check: totals per room are Salle 1 = 100, Salle 2 = 95, Salle 3 = 50: **expected Salle 3, 50**.
- Cause: the word "a" in "Quelle salle **a** le moins" matched the cell value `Martin A` (whose own words include "a"), so the engine filtered on `patient = Martin A` before grouping (one row, Salle 2, 30 minutes). The "Compris comme" line again makes the error visible, but the answer is presented as computed.
- Verdict: **FAIL**. BUG-02. (The same question phrased for the invoices workbook, Q11, works only because no supplier name contains the word "a".)

### Q17 - `Combien de rendez-vous par salle ?` (S29)

- Answer: `Nombre de lignes par salle : Salle 1 : 5, Salle 2 : 3, Salle 3 : 2. Calculé sur 10 lignes.` Check: correct. **PASS**.

### Q18 - `Combien de factures pour le fournisseur MedSupply ?` (S30, S31, S32)

- Run 1 (rdv-mars ticked by mistake): the model-assisted path was entered (the question does not fit that workbook) and `gemma2:2b` answered that the question contained no field of the selected file (not captured).
- Run 2 (invoices only, 17:15:16, S30): `Nombre de lignes : **3**. Calculé sur 3 lignes. Compris comme : fournisseur = MedSupply.` Correct.
- Run 3 (rdv-mars ticked again, 17:18:25, S31): the interface shows "Recherche" with a **Stop** button indefinitely; Stop had no effect; the owner had to close the application. The terminal printed:
  `thread 'tokio-rt-worker' (28372) panicked at src\tabular\query_plan.rs:489:13: internal error: entered unreachable code: eq/in are resolved against real data, not reached here` then `[ELIFECYCLE] Command failed with exit code -1.` The answer of run 2 is absent from the owner's copy of the conversation for the same reason (the app was restarted).
- Run 4 (after restart, invoices only, 17:38:27, S32): 3, correct.
- Verdict: **PASS** for the engine path, **CRASH** for run 3: BUG-01.

---

## IV - The model interprets the question (session 14)

### Q19 - `Est-ce qu'on a dépensé plus de cinq mille euros avec un seul fournisseur ce trimestre ?`

Invoices only ticked. Ground truth: no (max supplier total 1 450, table total 2 215). Four runs:

| Run | Model | Duration shown | Answer | Screens | Verdict |
| --- | --- | --- | --- | --- | --- |
| 1 | `gemma2:2b` | 40s | `« liste des fournisseurs » ne correspond à aucune valeur réelle de factures-fournisseurs-2026.xlsx.` + columns + generic examples. Footer: "gemma2:2b a été interrogé, pendant 40s, mais n'a pas réussi à transformer cette question en quelque chose que le moteur pouvait calculer." | S33 | FAIL |
| 2 | `ministral-3:3b` | 1m25s | `Je ne peux pas répondre directement à cette question pour factures-fournisseurs-2026.xlsx.` + columns + generic examples; same footer | S34 | FAIL |
| 3 | same question with `5000` typed instead of "cinq mille", `ministral-3:3b` | none (deterministic) | `Nombre de lignes : **0**. Calculé sur 0 ligne. Compris comme : date en 5000.` | S35 | **FAIL**: meaningless number from a deterministic path |
| - | (reformulations suggested for a re-run) | | `Quel fournisseur a le plus de montant ?` (S20/S23 show it works, MedSupply 1 450 < 5 000) | | |

Notes: (1) The model's plan put `liste des fournisseurs` as a filter value; the nudge then displays that sentence in quotation marks as if the user had typed it. (2) Both nudges say "Ses colonnes sont : date, fournisseur, montant" although the question already names `fournisseur`. (3) The question is a per-group threshold (HAVING), which the closed query-plan vocabulary cannot express whatever the model: with "5000" the classifier took the number as a year (BUG-02); with "cinq mille" nothing could be planned (BUG-14). See BUG-02, 13, 14.

---

## V - Honest refusal

### Q20 - `Quelle est la somme de cumul dans Calculs ?` (S36, gemma2:2b)

- Answer: `Cette feuille n'existe pas dans factures-fournisseurs-2026.xlsx.` + columns + examples; footer: "gemma2:2b a été interrogé, pendant 40s, mais n'a pas réussi...".
- Check: this fixture has one sheet (`Factures`), no `Calculs`, no `cumul`: the refusal is right. The scenario the catalogue intended (a workbook with formulas, refused as unverifiable) was **not testable** with these fixtures: a test-design gap, not a product bug. A formula workbook (`mixte.xlsx` of the reference tests) must be added for a re-run.
- 40 seconds of model time were spent on a question that names a sheet that does not exist.
- Verdict: **PASS-WITH-ISSUES**.

### Q21 - `Combien de factures pour Alfa ?` (S37, S38, S39)

- Runs: `gemma2:2b` 17s (S38), `ministral-3:3b` 1m12s (S39); the first display of S37 is cut by the scroll problem (BUG-08).
- Answer: `« Alfa » ne correspond à aucune valeur réelle de factures-fournisseurs-2026.xlsx.` + columns + examples; on the second ask: `Votre question précédente n'a pas pu être calculée non plus : nommez exactement l'une des colonnes ci-dessus.`
- Check: no supplier name is near "Alfa" (suppliers are MedSupply, Fournitures Dupont, Papeterie Lefevre), so no close value to suggest. The refusal is right.
- Issues: the model was consulted (17 s to 1m12s) for a question already settled by the data; the advice "nommez exactement l'une des colonnes" is wrong for a missing *value*; the earlier question referred to was a different, unrelated one (BUG-13).
- Verdict: **PASS-WITH-ISSUES**.

---

## VI - Q22 (data-only question with both kinds selected)

**Not run.**

## VII - Documents and tables selected together

Selection for Q23, Q24, Q26: the quote `devis-imprimante-medsupply.pdf` and the invoices workbook (S40: "Documents utilisés : 1 fichier", "Données utilisées : 1 fichier").

### Q23 - `Le montant total facturé par MedSupply correspond-il à ce qui est indiqué dans le devis signé ?` (S40, gemma2:2b)

- Answer: `Somme de montant : **1 450**. Calculé sur 3 lignes. Compris comme : fournisseur = MedSupply.` with "Réponse calculée depuis votre dossier des données, sans l'IA." and "Réponse établie depuis vos tableaux ; les documents sélectionnés n'étaient pas nécessaires."
- Check: 1 450 is right for the table half. The question is a **comparison with the quote** (1 200,00 HT, a different figure, with a clause that later orders are invoiced separately): the answer does not mention the quote at all, and states that the documents were not needed. No model was called.
- Button "Demander à l'IA": clicking it only redraws the page ("blinks") and shows the same deterministic answer; no request goes to the model.
- Verdict: **FAIL**. BUG-03 (the router treated a mixed question as data-only), BUG-04 (the button is dead).

### Q24 - `Quel est le montant total pour le fournisseur mentionné dans cette lettre ?` (S41 zoom, S42, gemma2:2b; repeated at 18:34:21 in S43)

- Answer: `Cette colonne de factures-fournisseurs-2026.xlsx ne contient pas que des nombres : elle ne peut être ni additionnée ni comparée.` + columns + examples; "documents ... pas nécessaires". On the repeat: `Votre question précédente n'a pas pu être calculée non plus ...`.
- Check: expected 1 450 (the supplier named in the quote is MedSupply). The engine took `fournisseur` (text) as the column to sum instead of `montant` and refused. The answer text does not even say which column. Entity linking from the excerpt (the documented mechanism) was never attempted; no model call; the "Demander à l'IA" button is dead here too.
- Verdict: **FAIL**. BUG-05, BUG-06, BUG-04.

### Q25

Not a question (a statement in the catalogue). The owner noted it does not occur this way in real use. It stays covered only by the automated suite (`tests/mixed_answer.rs`, `an_altered_table_number_is_corrected_without_rewriting_the_answer`).

### Q26 - gateway stopped (`docker compose down`), question of Q24 re-asked at 18:34:21 (S43)

- The deterministic path answered as in Q24 (no gateway call was needed), then the left sidebar shows a red banner: `Cela n'a pas fonctionné. Aucune réponse de l'IA à l'adresse http://127.0.0.1:8080. Vérifiez que la machine est allumée et sur le même réseau.` + "Réessayer".
- The banner comes from the periodic health check (`App.tsx` renders `healthError` as an `ErrorBanner`), not from the question.
- The expected line "the document part could not be written" never appears because the mixed tier is never reached (BUG-03, BUG-05). The question used was the Q24 phrasing, not Q23.
- With both folder panels open, the banner pushes the sidebar past the window height and a second page scrollbar appears (UX-2, cosmetic).
- Verdict: **NOT-REACHED** for the feature under test; the deterministic half correctly kept working with the server down (a real, positive result).

---

## VIII - Nothing selected

### Q27 - `Rédige un e-mail de relance pour un fournisseur dont la facture n'est pas réglée.`

- `gemma2:2b`, 35s (S44, S45): `Pour créer un e-mail de relance, il faudrait que vous choisissiez un document avec les informations de la facture et du fournisseur.` with "Réponse sans vos documents."
- `ministral-3:3b`, 1m22s (S46 truncated by the scroll problem, S47 complete): a drafted e-mail: subject "Relance pour règlement de facture non payée", greeting `[Nom du fournisseur]`, the invoice line `[Facture N° : [X], Montant : [Y] €, Date : [Z]]`, a request for confirmation, a signature block `[Votre nom/prénom] [Votre poste]`.
- Check: the small model applied the instruction's last sentence ("if the question needs a specific document, say none is selected") to a drafting request and refused to help; the larger model drafted a usable template.
- Verdict: `gemma2:2b` **FAIL** (unhelpful), `ministral-3:3b` **PASS** (BUG-08 on scroll).

### Q28 - `Quel est le délai légal de conservation des dossiers médicaux en France ?`

- `gemma2:2b` (S48 view cut by scroll, S49 after scroll) and `ministral-3:3b` (S50, S51): both answered **`Aucun document sélectionné pour cette conversation.`** with "Réponse établie depuis votre dossier de documents, sans l'IA." and a "Demander à l'IA" button. Timestamps 18:59:37 and 19:05:06.
- Check: the question is general knowledge and should have reached the model; instead the folder-question router matched it ("Quel" = list intent, "dossiers" = folder subject) and answered "nothing selected". No model was called, so the model choice is irrelevant.
- Verdict: **FAIL** x2. BUG-07.

## Results by model

| Task | `gemma2:2b` | `ministral-3:3b` |
| --- | --- | --- |
| Quote extraction with the source selected (Q5, Q6, Q7, Q8) | Right on Q7; verbatim pasted quote on Q5; one fabricated claim on Q6; wrong figure on Q8-a | Exact on Q8-c (the only run) |
| Refusal when the source is not selected (Q8-d..f) | Right only without memory | Right, but leaked an internal label |
| Interpreting a data question (Q19-Q21) | No usable plan (40s, 17s) | No usable plan (1m25s, 1m12s) |
| Drafting without documents (Q27) | Refused | Drafted a template |
| Speed on this CPU | 17s-1m52s | 1m12s-3m05s; first call after a switch hit the gateway timeout |
