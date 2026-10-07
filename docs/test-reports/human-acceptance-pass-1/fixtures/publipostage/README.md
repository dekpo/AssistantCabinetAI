# Publipostage fixtures

Fictional. Used by the mail-merge tests and by the second human pass.

| File | What it is |
| --- | --- |
| `donnees_publipostage.xlsx` | **The workbook of the human pass** (the owner's): six order lines, three orders (CMD-2026-001: 2 lines; CMD-2026-002: 1 line, Mme Sophie Martin, Smartphone 5G; CMD-2026-003: 3 lines, M. Thomas Robert). Sum of `Total_Ligne` 1 683,3 |
| `donnees_publipostage.csv` | **A different, older sample used by the automated tests** (five lines; CMD-2026-002 is a chair). Do not put it in the data folder of a human pass together with the workbook: the two hold the same order numbers and the program would, correctly, not choose between them |
| `modele_lettre.docx` | Word template: `«No_Commande»`, `«Civilite»`, `«Prenom»`, `«Nom»` and, in one table row, `«Article»`, `«Quantite»`, `«Prix_Unitaire»`, `«Total_Ligne»` (every name is exactly a column) |
| `modele_lettre.txt` | Text template with `[Prix_U]` and `[Total]`, which are abbreviations of columns, not column names |
