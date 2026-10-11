# Human test - lot 8, The knowledge management dialog

Written for the owner, who runs it. English, with the French strings of the interface quoted as data. **Every command is written in full and can be pasted as it is** (`.cursor/rules/kb-programme.mdc`, "Exact commands in a human test" and "A human test is a script, not a list").

| | |
| --- | --- |
| Branch to be on | `feat/kb-management-dialog` |
| Build | From `apps\desktop`, `pnpm tauri dev` |
| Services | **The AI machine must be up**, for the analyses (they embed the documents) and for the one question of the smoke test: the address in « Réglages » must answer. With the tower of the local network, nothing to do; on the local machine, `docker compose up -d` from the repository root |
| Duration | Preparation 8 minutes; Part A 5; Part B 6; Part C 5; Part D 7; Part E 7; Part F 3; Part G 5; Part H 4; smoke test 4; final restore 2. About 55 minutes. You can stop after any part: each one ends with a stated state, and the restore at the end works from anywhere |
| Visible in the application? | **Yes, this lot is the dialog itself.** Reading an entity and where it comes from, adding a name, merging and unmerging, separating two people with one name, deleting a false positive and restoring it, resetting the knowledge, undo |

## What this proves, and what it does not

It proves, in the running application, that **everything the knowledge base holds can be seen with its origin** (which file, which page or which cells, what it relied on); that **a name added by hand is used by the next Analyse**; that **a merge can be undone and a split is not undone by a re-reading**; that **a false positive deleted by hand does not come back when the documents are read again, and comes back when restored**; that the two resets do what they say; and that **no file of the folder is touched** (a script compares every file, byte for byte, before and after).

It does not prove anything about spreadsheets: the Data folder is empty on purpose (Part E of lot 5's test covers them). It also does not prove that the model answers better: nothing here changes an answer.

**What the agent ran, and what only you can see.** The agent ran every automated suite (numbers in the lot report), every helper command of this protocol in order on a temporary copy of the settings and the index (`ACAI_CONFIG_DIR`, `ACAI_DATA_DIR`, `ACAI_BACKUP_DIR`), and the whole sequence of Parts A to H through the same Rust code the dialog calls, on the same thirty-one invented documents (`cargo test --test knowledge_manage_walk -- --ignored --nocapture`): the figures and the order of the lists below come from there. **The agent did not open the application window and clicked nothing**: every label and every click here is unverified on screen until you run it. The text of the dialog was read from the catalogue; the passage shown beside each mention is the beginning of the document, so it will read like the documents below.

## How to read this protocol

- **One line = one command**, typed in the order given, from `C:\Users\elise\Documents\CURSOR\AssistantCabinetAI`.
- Each part begins with **« You start with »** and ends with **« You leave with »**. If your state is not the one written, stop and say so: do not guess.
- **Nothing is undone before the very last step**, « Final restore ». The dialog's own undo button is part of what is tested (Parts C and D), not a way to put the test right.
- Close and reopen the dialog as written: a **« Fermer »** on the dialog goes back to the settings window behind it, and a second **« Fermer »** closes the settings.
- Two different things are called « Annuler » on screen: the **button at the top right of the dialog's strip** (« Annuler : la fusion », which undoes an operation) and the **cancel button of a confirmation window**. The step says which.

## The commands you will use (copy them)

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot08_helper.py status
python docs\test-reports\knowledge-base-pass-1\human-tests\lot08_helper.py fingerprint
python docs\test-reports\knowledge-base-pass-1\human-tests\lot08_helper.py verify-files
python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_entities.py
```

`lot08_helper.py` changes files and settings (close the application first, it refuses otherwise; `fingerprint`, `verify-files` and `status` may run with the application open). `show_kb_entities.py` prints the names the knowledge base holds, read-only.

## The thirty-one documents

`lot08_helper.py prepare` writes them in a folder `kb-lot08` beside your Documents folder (invented, French; the thirty of lots 6 and 7 plus one):

- **Dr Jean Dupont**: `dupont-bilan-2026.txt`, `dupont-lettre-confrere.txt`, `dupont-horaires.txt`. And `rappel-m-dupont.txt`, which only says « M. Dupont » (so it is not linked to him).
- **Mrs Claire Martin**: `martin-controle.txt`, `martin-ordonnance.txt`. **Dr Paul Martin**: `paul-martin.txt` (and « cabinet du Parc »). **Dr Pierre Martin**: `pierre-martin.txt` (and « cabinet des Lilas »).
- Twenty-two notes: `note-01.txt` to `note-21.txt` and `note-photocopieuse.txt`.
- **`note-jeannot.txt`**: « mardi matin, Jeannot est passé pour un contrôle de routine, sans rendez-vous. » Nobody knows who Jeannot is until Part B.

## Preparation

**You start with:** the application closed, your own settings (your Documents folder chosen), the AI machine up.

1. One line at a time:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot08_helper.py backup
python docs\test-reports\knowledge-base-pass-1\human-tests\lot08_helper.py status
python docs\test-reports\knowledge-base-pass-1\human-tests\lot08_helper.py prepare
```

   `backup` copies your index, settings and identifier key to `C:\Users\elise\kb-backup-lot-08`. `prepare` makes `kb-lot08`, writes the thirty-one documents in it, points the Documents folder at it, **points the Data folder at an empty folder `kb-lot08-data`**, sets the mode to `suggest` and switches the timing log on. Your own documents and spreadsheets are never touched; `restore` puts your folders back.
2. Start the application:

```text
cd apps\desktop
pnpm tauri dev
```

3. **Empty the knowledge base of what your own earlier analyses taught it**, so that every number below is yours to compare. On the card « Dossier des documents », click **« Reset »**; the window « Effacer l'analyse ? » opens: click **« Reset »** in it. Do the same on the card « Dossier des données » (« Effacer l'analyse des données ? »). **Expected:** both cards say that nothing is analysed. Your files are not touched.
4. On the card « Dossier des documents », click **« Analyser »**. **Expected:** thirty-one documents analysed, and a line **« 6 personnes et organisations repérées, dont 0 déjà connue. »**
5. Run:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_entities.py
python docs\test-reports\knowledge-base-pass-1\human-tests\lot08_helper.py fingerprint
```

   **Expected from the first:** four people **active** (`Claire Martin`, `Jean Dupont`, `Paul Martin`, `Pierre Martin`) and two organisations only **candidates** (`cabinet des Lilas`, `cabinet du Parc`). The second prints `remembered 31 files of ...kb-lot08`.

**You leave with:** the application running on `kb-lot08`, mode `suggest`, both folders analysed (31 documents, an empty Data folder), six entities, the thirty-one files remembered by `fingerprint`.

## Part A - Open the dialog, find an entity, read where it comes from

**You start with:** what Preparation left.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| A1 | Click **« Réglages »** (title bar). In the group **« Connaissances »**, find the field **« Gérer les connaissances »** and click **« Gérer… »** | A large window titled **« Connaissances »** opens over the settings. Under the title: a sentence saying that nothing here modifies, moves or deletes a file. Then a strip: **« 6 entités, 11 mentions, 31 documents lus, 0 tableur lu. »** and a button **« Rien à annuler »** (greyed). No sentence about documents to read again. Four tabs: **Entités** (selected), **À vérifier**, **Types et rôles**, **Réinitialiser** | No such field; another count; a note about documents to read again |
| A2 | Look at the list of the **Entités** tab, on the left | Six rows, in this order: **cabinet des Lilas** (« Organisation · 1 mention · 1 fichier », tag **À confirmer**), **cabinet du Parc** (same), **Claire Martin** (« Personne · 2 mentions · 2 fichiers »), **Jean Dupont** (« Personne · 3 mentions · 3 fichiers »), **Paul Martin** (« Personne · 2 mentions · 1 fichier »), **Pierre Martin** (same). Above the list: **« 6 résultats »**. On the right: « Choisissez une entité dans la liste pour voir d'où elle vient. » | A seventh row; a person tagged À confirmer |
| A3 | In **« Rechercher un nom »** type `dupont` | After a short pause, **one** row: **Jean Dupont**, and « 1 résultat » | Nothing; Paul or Claire listed |
| A4 | Click the row **Jean Dupont** | The sheet opens on the right. Title **Jean Dupont**, under it « Personne · 3 mentions · 3 fichiers » and two tags: **Confirmée** and **Repérée par l'application**. Buttons: **Modifier**, **Fusionner avec…**, **Séparer…**, **Supprimer** | Missing buttons |
| A5 | Read the section **« Noms reconnus »** | Five names, in this order: **Docteur Dupont** (tag « Avec un titre »), **Dupont** (« Nom de famille seul »), **Dupont Jean** (« Ordre inversé »), **J. Dupont** (« Initiale »), **Jean Dupont** (« Forme du nom »). None has a button « Retirer ce nom »: the application found them, you cannot remove them here | A « Retirer ce nom » button |
| A6 | Read the section **« D'où vient cette entité »** | Three documents, in this order: `dupont-bilan-2026.txt`, `dupont-horaires.txt`, `dupont-lettre-confrere.txt`, each tagged **Document**. Under each: **« page 1, passage 1 »**, a quoted passage that contains « Jean Dupont » (for the first: « Docteur Jean Dupont Cabinet de médecine générale Bilan sangu… »), « Repéré par : nom déjà connu de la base. Confiance : 90 %. » and **« Nom trouvé : « Jean Dupont » »**. **`rappel-m-dupont.txt` is not there** (it only says « M. Dupont ») | `rappel-m-dupont.txt` listed; a file name that is not a Dupont document; a machine word such as `chunk` or `gazetteer_hit` |
| A7 | Select all of the search box and type `martin`, then click **Claire Martin** | Three rows (Claire, Paul, Pierre Martin). The sheet of Claire Martin lists `martin-controle.txt` and `martin-ordonnance.txt`, each « page 1, passage 1 » | |
| A8 | Empty the search box. Tick **« Seulement ce qui mérite un coup d'œil : à confirmer, correspondance à trancher, vu une seule fois »** | Two rows: **cabinet des Lilas** and **cabinet du Parc** (the two guesses). Untick the box: six rows again | Another row |
| A9 | Click the tab **« Types et rôles »** | « Types d'entité »: **Identifiant** 0, **Objet** 0, **Lieu** 0, **Organisation** 2, **Personne** 4, **Terme** 0 « entités » (each tagged « type de base »); « Rôles » (seven); « Modules de vocabulaire »: **Vocabulaire de base**, **Santé, France**; and the list of available modules with **Santé, France** « activé » and **Santé, Suisse**, **Droit, France**, **Comptabilité, France** « non activé (Réglages) ». Nothing can be edited | An edit control |
| A10 | Click the tab **« À vérifier »** | « Correspondances possibles »: **« Aucune correspondance à trancher. »** Under « État de la bibliothèque »: « Entités vues une seule fois : 2 », « Entités à confirmer (repérées sans appui sûr) : 2 », « Documents où aucun nom n'a été repéré : 23 », « Tableurs dont aucune colonne n'a été lue ... : 0 », « Correspondances possibles à trancher : 0 », « Personnes et organisations citées dans des documents mais dans aucun tableur : 0 » | Other numbers |
| A11 | Click the tab **« Entités »** | Back on the list, where you left it | The search or the sheet lost |

**You leave with:** the dialog open over the settings, the tab « Entités », the search box empty, the sheet of **Claire Martin** open, nothing changed.

## Part B - Add a name by hand, and see the next Analyse use it

**You start with:** what Part A left.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| B1 | Search `dupont`, click **Jean Dupont**. In **« Nouveau nom »** type `JD` (do **not** click Ajouter) | A line appears under the field: **« Un nom de moins de 3 lettres est retrouvé dans les documents, mais pas dans une question. »** Also « Ce nom sera cherché dans vos documents à la prochaine analyse. » | No warning for two letters |
| B2 | Replace `JD` by `Jeannot` and click **« Ajouter »** | The field empties. In « Noms reconnus » a sixth name, **Jeannot**, tagged **Ajouté à la main**, with a button **« Retirer ce nom »**. The strip now says **« 31 documents seront relus à la prochaine analyse, à partir du texte déjà conservé : l'IA n'est pas sollicitée. »** and the button reads **« Annuler : l'ajout d'un nom »**. Nothing else moved: the sheet still lists three documents | The documents are read at once (a progress bar); no note |
| B3 | Click **« Fermer »** on the dialog, then **« Fermer »** on the settings. On the card « Dossier des documents » click **« Analyser »** | The analysis ends quickly (nothing is sent to the AI: the text is already stored). The summary says **« 6 personnes et organisations repérées, dont 6 déjà connues. »** and **« 31 documents déjà analysés relus pour y repérer les noms, sans solliciter l'IA. »**, then a button **« Voir les connaissances »** | No such button; an error |
| B4 | Click **« Voir les connaissances »**. Search `dupont`, click **Jean Dupont** | The dialog opens (from the card this time). The strip: **« 6 entités, 13 mentions, 31 documents lus, 0 tableur lu. »**, no note about documents to read again. The sheet: « Personne · 5 mentions · 4 fichiers ». « D'où vient cette entité » now has a fourth document, **`note-jeannot.txt`**, with **two** lines: « page 1, passage 1 » (« Nom trouvé : « Jeannot » ») and **« dans le nom du fichier »** (« Repéré par : mot du nom du fichier. Confiance : 50 %. » — the file is called `note-jeannot.txt`) | Four mentions only; `note-jeannot.txt` missing |

**You leave with:** the dialog open (opened from the card), tab « Entités », the sheet of Jean Dupont open, a name `Jeannot` on him, both folders analysed, 31 files unchanged.

## Part C - Merge two entities, then separate them again

**You start with:** what Part B left.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| C1 | Search `martin`. Click **Paul Martin**, then **« Fusionner avec… »**. In **« Chercher l'autre entité »** type `pierre` | A choice list with one line: **Pierre Martin**. Under the field: « Seules les entités du même type peuvent être fusionnées. » (An organisation can never be offered for a person.) | An organisation in the list |
| C2 | Click **Pierre Martin** | Two round buttons under **« Garder le nom de »**: **Paul Martin** (selected) and **Pierre Martin** (they are seen as often, so the older one is kept by default). A button **« Fusionner »** | |
| C3 | Click **« Fusionner »** | A confirmation window: **« Fusionner ces deux entités ? »**, text **« « Pierre Martin » devient un autre nom de « Paul Martin ». Rien n'est déplacé : vous pourrez défaire la fusion. »**. Click **« Fusionner »** in it | The merge happens without the question |
| C4 | Read the sheet that opens | **Paul Martin**: « Personne · 4 mentions · 2 fichiers ». A section **« Fusionnées dans cette entité »** with a button **Pierre Martin**. « Noms reconnus » lists the names of both, those of Pierre tagged **« Vient d'une entité fusionnée ici »**. « D'où vient » lists `paul-martin.txt` and `pierre-martin.txt`, two lines each (« page 1, passage 1 » and « dans le nom du fichier »). The strip: **5 entités**, the note about 31 documents, and the button **« Annuler : la fusion »** | |
| C5 | In the list's **« Afficher »** menu choose **« Entités fusionnées »** | One row: **Pierre Martin**, tag **Fusionnée**, « 0 mention » (its mentions are read on the survivor). Click it | |
| C6 | Read the note on its sheet, then click **« Défaire la fusion »** | The note says **« Cette entité est fusionnée dans « Paul Martin » : ses noms et ses mentions se lisent sur la fiche de celle-ci. »** with **« Ouvrir « Paul Martin » »**. After the click, the sheet becomes an ordinary one (tags **Confirmée**, **Repérée par l'application**). The strip: **6 entités**, button **« Annuler : la fin d'une fusion »** | The note still there |
| C7 | Set **« Afficher »** back to **« Entités actives »** (search `martin`) | Three rows again: Claire Martin (2 mentions, 2 fichiers), **Paul Martin** and **Pierre Martin** (2 mentions, 1 fichier each) | A « Fusionnée » row |

**You leave with:** the dialog open, « Entités actives », search `martin`, six active entities, nothing merged, the last operation « la fin d'une fusion ».

## Part D - Two people with one name: separate the mentions

**You start with:** what Part C left. (Pretend `martin-ordonnance.txt` is about another Claire Martin.)

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| D1 | Search `claire`, click **Claire Martin**, then **« Séparer… »** | A form: an explanation (« Utile quand deux personnes différentes portent le même nom... »), a field **« Nom de la nouvelle entité »** already filled with **`Claire Martin 2`**, « Cochez au moins une mention, mais pas toutes. » A **tick box appears before each mention** in « D'où vient cette entité » | No tick boxes |
| D2 | Tick the box of the mention under **`martin-ordonnance.txt`** (the second document) | « 1 cochée(s) ». The button **« Séparer les mentions cochées »** becomes available. (Tick both boxes: it stays unavailable — all of them cannot move; untick the first one again) | The button available with none or both ticked |
| D3 | Click **« Séparer les mentions cochées »** | A confirmation window: **« Séparer 1 mention(s) de « Claire Martin » ? »**, « Elles passent à une nouvelle entité, « Claire Martin 2 ». Vous pourrez annuler cette séparation juste après... ». Click **« Séparer »** | |
| D4 | Read the sheet that opens | **Claire Martin 2**, « Personne · 1 mention · 1 fichier », tags **Confirmée** and **Saisie à la main**. One document, `martin-ordonnance.txt`: « page 1, passage 1 », **« Saisi à la main »** and **« Séparé à la main d'une autre entité »**. Its only name is « Claire Martin 2 ». The strip: **7 entités**, the note about 31 documents, **« Annuler : la séparation »** | |
| D5 | Search `claire` | Two rows: **Claire Martin** (« 1 mention · 1 fichier ») and **Claire Martin 2** (« 1 mention · 1 fichier », tag **Saisie à la main**) | |
| D6 | **« Fermer »**, **« Fermer »**, click **« Analyser »** on the Documents card, then **« Voir les connaissances »**. Search `claire` | The summary said **« 31 documents déjà analysés relus ... »**. The two rows are **unchanged**: the re-reading did not put the mention back. Strip: 7 entités | **Claire Martin** back at 2 mentions, or « Claire Martin 2 » gone |
| D7 | Click the strip's button **« Annuler : la séparation »** | The search now shows **one** row: **Claire Martin**, « 2 mentions · 2 fichiers »; « Claire Martin 2 » is gone. Strip: **6 entités**; the button now reads **« Annuler : la fin d'une fusion »** (the operation before) | |

**You leave with:** the dialog open (from the card), search `claire`, six entities, a note about 31 documents to read again, nothing separated.

## Part E - Delete a false positive; it does not come back; restore it

**You start with:** what Part D left. `cabinet des Lilas` is a guess of the application (a place name taken for an organisation); here it plays the false positive.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| E1 | Search `lilas`, click **cabinet des Lilas**, then **« Supprimer »** | A confirmation window: **« Supprimer « cabinet des Lilas » ? »**, « L'entité disparaît des listes et l'analyse ne la recrée pas. Vos fichiers ne sont pas touchés. Vous pourrez la restaurer dans « Entités supprimées ». » Click **« Supprimer »** (red) | |
| E2 | Read the sheet | A note: **« Cette entité a été supprimée : elle n'apparaît plus et l'analyse ne la recrée pas. »** with **« Restaurer »**. The strip: **5 entités**, button **« Annuler : la suppression »** | |
| E3 | Set **« Afficher »** to **« Entités supprimées »** | One row: **cabinet des Lilas**, tag **Supprimée**, « 0 mention » | |
| E4 | **« Fermer »**, **« Fermer »**, click **« Analyser »**, then **« Voir les connaissances »** | The summary says « 31 documents déjà analysés relus ... » and **« 5 personnes et organisations repérées, dont 5 déjà connues. »** (the deleted name is not counted: it was read again and ignored) | A sixth entity; `cabinet des Lilas` among the detected |
| E5 | Search `lilas` with **« Afficher »** on **« Entités actives »** | **« Aucune entité ne correspond. »** With **« Entités supprimées »**: `cabinet des Lilas` is still there | The name is back among the active ones |
| E6 | In **« Entités supprimées »**, click it, then **« Restaurer »** | The note goes. Tags **À confirmer** and **Saisie à la main** (a restored entity is yours from then on). « Organisation · 0 mention · 0 fichier ». The strip: **6 entités** and the note about 31 documents | |
| E7 | **« Fermer »**, **« Fermer »**, **« Analyser »**, **« Voir les connaissances »**. (**« Afficher »** is back on **« Entités actives »**: the dialog starts fresh each time it opens) Search `lilas` | One row: **cabinet des Lilas**, « Organisation · **1 mention · 1 fichier** » (it is found again in `pierre-martin.txt`), tags **À confirmer** and **Saisie à la main**. Strip: **« 6 entités, 13 mentions, 31 documents lus, 0 tableur lu. »**, no note | The mention not back |

**You leave with:** the dialog open (from the card), « Entités actives », six entities (`cabinet des Lilas` restored), the last operation « la restauration », no note about documents to read again.

## Part F - Files untouched, so far

**You start with:** what Part E left. The application may stay open.

1. Run:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot08_helper.py verify-files
```

   **Expected:** `31 files now, 31 remembered`, then **`OK: every file is byte for byte the same, none added, none removed`**. Merges, splits, deletions and name additions have not touched a file of your folder.

**You leave with:** the same.

## Part G - Reset the knowledge, keeping what you typed

**You start with:** what Part E left (dialog open, six entities, `Jeannot` on Jean Dupont, `cabinet des Lilas` restored).

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| G1 | Click the tab **« Réinitialiser »** | An introduction saying that the base holds only names and places, never your files, and that the two **« Reset »** buttons of the folder cards empty each folder's analysis while this one empties the knowledge base. Two blocks: **« Ce que les fichiers ont appris »** (button **« Réinitialiser, en gardant mes saisies… »**) and **« Tout, y compris mes saisies »** (red button **« Tout réinitialiser… »**) | |
| G2 | Click **« Réinitialiser, en gardant mes saisies… »** | A confirmation window: **« Réinitialiser ce que les fichiers ont appris ? »**. First click its cancel button **« Annuler »**: nothing happens. Click the first button again, then **« Réinitialiser »** | The reset without the question |
| G3 | Read the line « La base de connaissances a été réinitialisée. », then the tab **« Entités »** (search box empty) | **Two** rows: **cabinet des Lilas** (« 0 mention », kept: you restored it) and **Jean Dupont** (« 0 mention », kept: it holds the name Jeannot you typed). Strip: **« 2 entités, 0 mention, 0 document lu, 0 tableur lu. »**, the note **« 31 documents seront relus ... »**, and the strip's button greyed and reading **« Ne peut pas être annulé ici : la réinitialisation »** (a reset is a barrier: nothing before it can be undone) | The button active; four people still listed |
| G4 | **« Fermer »**, **« Fermer »**, **« Analyser »**, **« Voir les connaissances »** | The summary says « 6 personnes et organisations repérées, dont 2 déjà connues. » and « 31 documents déjà analysés relus ... ». The list: six rows again; **Jean Dupont** « 5 mentions · 4 fichiers » (Jeannot found again), **cabinet des Lilas** « 1 mention · 1 fichier » tagged **Saisie à la main** | `Jeannot` forgotten |
| G5 | Set **« Afficher »** to **« Entités supprimées »**, look, then set it back to **« Entités actives »** | No row: the deletion of Part E was undone by the restoration. (Had you left `cabinet des Lilas` deleted, it would still be here after this reset: a reset keeps the tombstones.) | |

**You leave with:** the dialog open, six entities back, `Jeannot` still on Jean Dupont, the strip without a note.

## Part H - Reset everything

**You start with:** what Part G left.

| # | Do | Expected | If you see ... it is a bug |
| --- | --- | --- | --- |
| H1 | Tab **« Réinitialiser »**, click **« Tout réinitialiser… »** | A confirmation window **« Tout réinitialiser ? »** whose text lists what goes (entities found, entities typed, names added, details, deleted entities, history) and says it cannot be undone. Click **« Tout réinitialiser »** | |
| H2 | Tab **« Entités »** (search box empty, **« Afficher »** on **« Entités actives »**) | **« La base ne contient encore aucune entité. Lancez une analyse ... »** Strip: **« 0 entité, 0 mention, 0 document lu, 0 tableur lu. »** and the note about 31 documents | A row left |
| H3 | **« Fermer »**, **« Fermer »**, **« Analyser »**, **« Voir les connaissances »**, search `dupont`, click **Jean Dupont** | The summary: **« 6 personnes et organisations repérées, dont 0 déjà connue. »** The sheet: « Personne · **3 mentions** · 3 fichiers », **five** names, **no `Jeannot`**, no `note-jeannot.txt`. The knowledge base was rebuilt from the stored text, with no request to the AI | `Jeannot` still there |
| H4 | Click **« Fermer »** on the dialog and **« Fermer »** on the settings | Both closed | |

Then run both:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\show_kb_entities.py
python docs\test-reports\knowledge-base-pass-1\human-tests\lot08_helper.py verify-files
```

   **Expected:** the first lists the same six entities as in the Preparation (four people active, two organisations candidates); the second ends with **`OK: every file is byte for byte the same, none added, none removed`**.

**You leave with:** both dialogs closed, the application running, the Preparation's state.

## Smoke test (the product as before)

**You start with:** what Part H left.

1. In the chat, with « Documents utilisés » set to **« Tous les documents »** (tick it if it is not), ask `Que dit le Docteur Jean Dupont du bilan sanguin ?` **Expected:** an answer, and « Sources » listing `dupont-bilan-2026.txt`. **No** line « Recherche ciblée » under it (`suggest`).
2. On the card « Dossier des documents », click **« Reset »**, then **« Reset »** in the confirmation. **Expected:** the card says nothing is analysed. Click **« Analyser »** again: thirty-one documents analysed. A folder Reset removes the people with their documents (nothing in the base is yours any more), so the summary says **« 6 personnes et organisations repérées, dont 0 déjà connue. »**
3. Ask the same question again. **Expected:** the same kind of answer.

**You leave with:** the Documents folder analysed again.

## Final restore (the only one)

1. Close the application (the window, then the terminal where `pnpm tauri dev` runs: `Ctrl+C`).
2. Run:

```text
python docs\test-reports\knowledge-base-pass-1\human-tests\lot08_helper.py restore
```

   **Expected:** « restored index.sqlite3 », « restored settings.json » (and the key file if there was one). Your own Documents folder, Data folder, index and settings are back; `kb-lot08` and `kb-lot08-data` can be deleted whenever you want.

## Results table

| step | expected | observed | OK? | screenshot |
| --- | --- | --- | --- | --- |
| Preparation 3 | both cards empty | | | |
| Preparation 4 | 6 personnes et organisations repérées, dont 0 déjà connue | | | |
| A1 | strip 6 entités, 11 mentions, 31 documents lus, 0 tableur lu | | | |
| A2 | six rows in order, candidates tagged | | | |
| A5 | five names, none removable | | | |
| A6 | three documents, page 1 passage 1, no `rappel-m-dupont.txt` | | | |
| A9 | types and roles read-only | | | |
| A10 | 2, 2, 23, 0, 0, 0 | | | |
| B1 | warning under three letters | | | |
| B2 | Jeannot added by hand, note about 31 documents | | | |
| B3 | 31 documents relus, link Voir les connaissances | | | |
| B4 | 5 mentions, 4 fichiers, note-jeannot.txt twice | | | |
| C3 | confirmation before the merge | | | |
| C4 | 4 mentions, Pierre listed as merged in | | | |
| C6 | Défaire la fusion | | | |
| D2 | split only with some, not all | | | |
| D4 | Claire Martin 2, manual, separated | | | |
| D6 | split not undone by Analyse | | | |
| D7 | Annuler : la séparation | | | |
| E1 | confirmation before the deletion | | | |
| E4 | 5 detected, not the deleted one | | | |
| E5 | not among the active ones | | | |
| E7 | restored, 1 mention back | | | |
| F | OK byte for byte | | | |
| G2 | confirmation, cancel works | | | |
| G3 | two rows kept, nothing to undo | | | |
| G4 | Jeannot found again | | | |
| H3 | rebuilt, no Jeannot | | | |
| H5 | six entities, files OK | | | |
| Smoke | answer with Sources, no targeting line | | | |

## Findings

Bugs found go to `docs/test-reports/knowledge-base-pass-1/defects-register.md` at once and are fixed on this branch before the merge. Ideas go to the lot report's open questions.
