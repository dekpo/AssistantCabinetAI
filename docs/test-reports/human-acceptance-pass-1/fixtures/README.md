# Fixtures of HAP-1

Byte-identical copies of the files the owner used, so that the pass can be re-run or the findings
re-checked by another agent in another IDE or in the cloud.

| Copy here | Original location on the test workstation |
| --- | --- |
| `documents/` | `C:\Users\elise\AssistantCabinetAI\Docs\Test` |
| `data/` | `C:\Users\elise\AssistantCabinetAI\Data\Test` |

To re-run: point the Documents folder at a copy of `documents/` and the Data folder at a copy of `data/`
(each outside any cloud-synchronised folder, as the product requires), press **Analyser** on both cards,
and follow [../01-protocol.md](../01-protocol.md).

All content is fictional and marked as such inside each file. No real person, patient or supplier is
described (`AGENTS.md`, rules 2 and 6). The text below was extracted from the files with `pypdf`,
`python-docx` and `openpyxl`; accents are missing in the PDFs themselves (the generator wrote ASCII), which
is why several model answers quote unaccented text.

## documents/

### `bail-cabinet-2024.pdf` (1 page)

```text
BAIL PROFESSIONNEL - CABINET MEDICAL (document fictif, usage de test uniquement)
Entre les soussignes, M. Jean Fontaine, bailleur, d'une part, et Mme Camille Exemple, preneuse,
exercant a titre liberal, d'autre part, il a ete convenu ce qui suit :
Le bailleur donne a bail les locaux professionnels sis 14 rue des Tilleuls, 69003 Lyon, d'une
superficie de 85 metres carres, a usage exclusif de cabinet medical.
Le present bail est consenti et accepte pour une duree de NEUF ANS entiers et consecutifs, qui
commencera a courir le 1er avril 2024 pour se terminer le 31 mars 2033.
Le loyer annuel est fixe a 18 400 euros, payable mensuellement, revise chaque annee selon
l'indice national des loyers des activites tertiaires.
Fait a Lyon, le 15 mars 2024, en deux exemplaires originaux.
```

### `convention-remplacement-dr-martin.docx` (5 paragraphs, no table)

```text
Convention de remplacement (document fictif, usage de test uniquement)
Entre le Dr Camille Exemple, medecin titulaire du cabinet sis 14 rue des Tilleuls a Lyon, d'une part, et le Dr Antoine Martin, medecin remplacant inscrit a l'Ordre sous le numero fictif 69-9-99999, d'autre part.
Il est convenu que le Dr Antoine Martin assurera le remplacement du Dr Camille Exemple du lundi 6 juillet 2026 au vendredi 31 juillet 2026 inclus, pendant la periode de conges annuels de la titulaire.
Le remplacant percevra une retrocession d'honoraires fixee a 80 pour cent des honoraires encaisses pendant la duree du remplacement. Les 20 pour cent restants sont reverses au titulaire au titre de la mise a disposition du cabinet, du materiel et de la patientele.
Fait a Lyon, le 2 juin 2026, en trois exemplaires originaux dont un pour chacune des parties et un pour le Conseil de l'Ordre.
```

It does **not** mention the lease, its duration or its rent.

### `courrier-cpam-radiation.pdf` (1 page)

```text
CAISSE PRIMAIRE D'ASSURANCE MALADIE DU RHONE (document fictif, usage de test
uniquement)
Objet : radiation d'un assure du regime general
Madame, Monsieur,
Nous vous informons que l'assure(e) M. Hugo Exemple, numero de securite sociale fictif 1 85 03 69
123 456 78, a ete radie du regime general a compter du 1er fevrier 2026, suite a son affiliation au
regime d'un nouvel employeur.
En consequence, toute feuille de soins etablie pour cet assure a compter de cette date doit etre
adressee a sa nouvelle caisse de rattachement, dont les coordonnees figurent en piece jointe.
Nous vous prions d'agreer, Madame, Monsieur, l'expression de nos salutations distinguees.
Le service des affiliations, CPAM du Rhone (fictif).
```

### `devis-imprimante-medsupply.pdf` (1 page)

```text
MEDSUPPLY FOURNITURES MEDICALES (fournisseur fictif) - DEVIS N. DV-2026-0117
Devis etabli le 8 janvier 2026, valable 30 jours, a l'attention du Cabinet Exemple.
Imprimante laser multifonction A4, cartouches d'encre x4, ramette de papier x10, consommables
divers pour le secretariat du cabinet.
Montant total du devis : 1 200,00 euros HT.
Ce devis ne couvre que la commande initiale de janvier ; toute commande complementaire passee
au cours de l'annee fait l'objet d'une facturation separee aupres du fournisseur.
```

The only amount is the total, 1 200,00 euros HT. **There is no price for the printer alone**, which is what
Q8 probes.

### `2026/janvier/neurologie.pdf` (1 page)

```text
CABINET DE NEUROLOGIE (document fictif, usage de test uniquement)
Courrier concernant M. Hugo Exemple, consultation du 14 janvier 2026.
Monsieur Hugo Exemple est suivi depuis six semaines pour des cephalees d'allure tensionnelle,
sans signe d'alarme a l'examen clinique. Un bilan d'imagerie n'est pas juge necessaire a ce stade.
Bien confraternellement.
```

### `2026/mars/neurologie.pdf` (1 page)

```text
CABINET DE NEUROLOGIE (document fictif, usage de test uniquement)
Courrier concernant Mme Alice Exemple, consultation du 9 mars 2026.
Madame Alice Exemple presente des paresthesies des membres superieurs evoluant depuis trois
mois. Un electromyogramme des membres superieurs est propose en complement.
Bien confraternellement.
```

## data/

### `factures-fournisseurs-2026.xlsx` - sheet `Factures`, A1:C9, 8 data rows

| date | fournisseur | montant |
| --- | --- | --- |
| 2026-01-10 | MedSupply | 450 |
| 2026-01-22 | Fournitures Dupont | 180 |
| 2026-02-05 | MedSupply | 620 |
| 2026-02-14 | Papeterie Lefevre | 95 |
| 2026-02-28 | Fournitures Dupont | 210 |
| 2026-03-03 | MedSupply | 380 |
| 2026-03-11 | Papeterie Lefevre | 120 |
| 2026-03-19 | Fournitures Dupont | 160 |

Hand computation:

- Sum of `montant`: 450 + 180 + 620 + 95 + 210 + 380 + 120 + 160 = **2 215** (Q9).
- Maximum: **620** (Q10).
- MedSupply: 450 + 620 + 380 = **1 450**, 3 rows (Q11, Q12, Q18, Q23, Q24).
- Fournitures Dupont: 180 + 210 + 160 = **550**. Papeterie Lefevre: 95 + 120 = **215**.
- All rows are in the first quarter of 2026. No supplier is called "Alpha" and none is within edit distance 2 of "Alfa".
- The quote (1 200,00 HT) differs from the MedSupply invoices in the table (1 450): the comparison of Q23.

### `rdv-mars-2026.xlsx` - sheet `RDV`, A1:D11, 10 data rows

| date | salle | patient | duree_min |
| --- | --- | --- | --- |
| 2026-03-02 | Salle 1 | Dupont J | 20 |
| 2026-03-03 | Salle 2 | Martin A | 30 |
| 2026-03-04 | Salle 1 | Bernard L | 15 |
| 2026-03-05 | Salle 3 | Petit M | 25 |
| 2026-03-09 | Salle 1 | Dupont J | 20 |
| 2026-03-10 | Salle 2 | Girard P | 35 |
| 2026-03-11 | Salle 1 | Roux S | 25 |
| 2026-03-12 | Salle 3 | Fontaine R | 25 |
| 2026-03-16 | Salle 1 | Lambert T | 20 |
| 2026-03-20 | Salle 2 | Simon V | 30 |

Hand computation:

- `duree_min` total 245; mean **24,5** over 10 rows (Q15).
- Mondays: 2, 9 and 16 March 2026 -> **3** (Q13). (2 March 2026 is a Monday: 1 January 2026 is a Thursday and 60
  days later is a Monday.)
- Appointments from 9 to 15 March inclusive: 9, 10, 11, 12 -> **4** (Q14).
- Rows per room: Salle 1 = 5 (2, 4, 9, 11, 16 March), Salle 2 = 3 (3, 10, 20 March), Salle 3 = 2 (5, 12 March) (Q17).
- Total minutes per room: Salle 1 = 20+15+20+25+20 = 100; Salle 2 = 30+35+30 = 95; Salle 3 = 25+25 = **50**.
  The room with the least total is **Salle 3 (50)** (Q16).
- The patient `Martin A` contains the word "a", which collides with the French verb "a" of Q16 (BUG-02).
- The `duree_min` cell value 15 (Bernard L, 4 March) collides with the digits of "15/03/2026" in Q14 (BUG-02).

Neither workbook has a formula, a second sheet, or a column named `cumul`.
