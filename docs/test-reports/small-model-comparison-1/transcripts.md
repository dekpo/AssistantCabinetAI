# Transcripts, small-model comparison (pass 1)

9 October 2026 · verbatim copy of the conversations the owner pasted after the lot 2 human test, Part B. The conversation was cleared before every question. Order is the order of the paste. The Sources disclosure was not copied. Analysis: [README.md](README.md). The default alias that day was `qwen2.5:1.5b`; the model of an answer is the one named on its `Généré par` line, and an answer with no such line was produced by the product's own engine.

French is quoted as data. Spelling and missing accents are the models' and the fixtures'.

## Folder questions (answered without the AI)

**D1**

> Vous: Combien de fichiers au total ?
> Assistant: Votre dossier de documents contient 8 fichiers.
> Réponse établie depuis votre dossier de documents, sans l'IA.

**D4**

> Vous: Peux-tu me donner la liste des fichiers ?
> Assistant: Votre dossier de documents contient 8 fichiers :
> - 2026/janvier/neurologie.pdf — analysé
> - 2026/mars/neurologie.pdf — analysé
> - bail-cabinet-2024.pdf — analysé
> - convention-remplacement-dr-martin.docx — analysé
> - courrier-cpam-radiation.pdf — analysé
> - devis-imprimante-medsupply.pdf — analysé
> - modele_lettre.docx — analysé
> - modele_lettre.txt — analysé
> Réponse établie depuis votre dossier de documents, sans l'IA.

**D5, first turn**

> Vous: Que dit neurologie.pdf ?
> Assistant: Plusieurs fichiers s'appellent neurologie.pdf. Lequel voulez-vous ?
> - 2026/janvier/neurologie.pdf
> - 2026/mars/neurologie.pdf
> Réponse établie depuis votre dossier de documents, sans l'IA.

## JAN: `Que dit 2026/janvier/neurologie.pdf ?`

**gemma4:e2b** (2m20)

> Extrait de 2026/janvier/neurologie.pdf (page 1) :
> CABINET DE NEUROLOGIE (document fictif, usage de test uniquement)
> Courrier concernant M. Hugo Exemple, consultation du 14 janvier 2026.
> Monsieur Hugo Exemple est suivi depuis six semaines pour des cephalees d'allure tensionnelle, sans signe d'alarme a l'examen clinique. Un bilan d'imagerie n'est pas juge necessaire a ce stade.
> Généré par gemma4:e2b en 2m20s

**granite3.1-moe:1b** (32 s)

> 2026/janvier/neurologie.pdf dit :
> CABINET DE NEUROLOGIE (document fictif, usage de test uniquement)
> Courrier concernant M. Hugo Exemple, consultation du 14 janvier 2026.
> Monsieur Hugo Exemple est suivi depuis six semaines pour des cephalees d'allure tensionnelle,
> sans signe d'alarme a l'examen clinique. Un bilan d'imagerie n'est pas juge nécessaire a ce stade.
> Bien confraternellement.
> Généré par granite3.1-moe:1b en 32s

**granite3.1-moe:3b** (45 s)

> 2026/janvier/neurologie.pdf (page 1) :
> CABINET DE NEUROLOGIE (document fictif, usage de test uniquement)
> Courrier concernant M. Hugo Exemple, consultation du 14 janvier 2026.
> Monsieur Hugo Exemple est suivi depuis six semaines pour des cephalees d'allure tensionnelle,
> sans signe d'alarme à l'examen clinique. Un bilan d'imagerie n'est pas juge nécessaire à ce stade.
> Bien confraternellement.
> Généré par granite3.1-moe:3b en 45s

**ministral-3:3b** (1m37)

> Extrait de **2026/janvier/neurologie.pdf (page 1)** :
> - Courrier concernant **M. Hugo Exemple**.
> - Consultation du **14 janvier 2026**.
> - Suivi depuis **six semaines** pour des **céphalées d’allure tensionnelle**.
> - Pas de signe d’alarme à l’examen clinique.
> - Bilan d’imagerie **non jugé nécessaire** à ce stade.
> Généré par ministral-3:3b en 1m37s

**qwen2.5:1.5b** (42 s)

> Le document "2026/janvier/neurologie.pdf" contient un courrier concernant M. Hugo Exemple. L'étude fait savoir qu'il a été suivi depuis six semaines pour des cephalees d'allure tensionnelle sans signe d'alarme clinique. Un bilan d'imagerie n'est pas nécessaire à ce stade du traitement.
> Généré par qwen2.5:1.5b en 42s

**qwen3.5:2b** (1m41)

> Le document 2026/janvier/neurologie.pdf contient les informations suivantes :
> - CABINET DE NEUROLOGIE (extrait)
> - Consultation du 14 janvier 2026 concernant M. Hugo Exemple
> - Sans signe d'alarme à l'examen clinique sur des céphalées d'allure tensionnelle depuis six semaines
> - Un bilan d'imagerie n'est pas nécessaire au stade actuel (extrait)
> Cette consultation mentionne le cabinet de neurologie pour le médecin traitant et est rédigée en langue française. Le document indique également qu'il ne faut pas juger un diagnostic immédiat à ce stade.
> Généré par qwen3.5:2b en 1m41s

**qwen3:0.6b** (31 s)

> - The document discusses a neurology cabinet and M. Hugo Exemple's consultation for anxiety-related sleep disorders. It mentions six weeks of follow-up without clinical exam signs and an imaging assessment not needed at that stage.
> **Citation**: [1] 2026/janvier/neurologie.pdf.
> Généré par qwen3:0.6b en 31s

**qwen3:1.7b** (50 s)

> Le fichier 2026/janvier/neurologie.pdf est un document PDF (page 1) fictif utilisé pour des tests, décrivant une consultation de neurologie concernant Mr. Hugo Exemple avec des cephalees tensionnelles, sans examen clinique d'alerte et sans bilan d'imagerie. Il est indexé, lisible et situé dans le dossier 2026/janvier.
> Généré par qwen3:1.7b en 50s

## MAR: `Que dit 2026/mars/neurologie.pdf ?`

**gemma4:e2b** (2m14)

> Extrait de 2026/mars/neurologie.pdf (page 1) :
> CABINET DE NEUROLOGIE (document fictif, usage de test uniquement)
> Courrier concernant Mme Alice Exemple, consultation du 9 mars 2026.
> Madame Alice Exemple presente des paresthesies des membres superieurs evoluant depuis trois mois. Un electromyogramme des membres superieurs est propose en complement.
> Généré par gemma4:e2b en 2m14s

**granite3.1-moe:1b** (29 s)

> 2026/mars/neurologie.pdf dit "CABINET DE NEUROLOGIE (document fictif, usage de test uniquement)". Il mentionne une consultation du 9 mars 2026, dans laquelle il est question de paresthesies des membres superieurs evoluant depuis trois mois. Il a également un consultant propose un electromyogramme des membres superieurs.
> Généré par granite3.1-moe:1b en 29s

**granite3.1-moe:3b** (46 s)

> Le document extrait 2026/mars/neurologie.pdf contient un courrier concernant Mme Alice Exemple, consultation du 9 mars 2026. Ce courrier met en évidence des paresthesies des membres superieurs qui s'étaient développées depuis trois mois. Un électromyogramme des membres superieurs est proposé en complément. Bien confraternellement.
> [Note] Cette réponse a été obtenue en extrait du document 2026/mars/neurologie.pdf, correspondant au paragraphe 1 du document.
> Il est important de noter que toutes les informations présentées ici sont tirées des extraits du document et ne peuvent être utilisées comme un substitut direct aux informations fournies par le document lui-même. Les informations présentées ne sont pas exhaustives, et il est recommandé de consulter le document original pour obtenir une vision complète.
> Généré par granite3.1-moe:3b en 46s

**ministral-3:3b** (1m30)

> - **Date de la consultation** : 9 mars 2026.
> - **Patient concerné** : Mme Alice Exemple.
> - **Symptômes** : Paresthésies des membres supérieurs depuis trois mois.
> - **Proposition médicale** : Electromyogramme des membres supérieurs.
> Généré par ministral-3:3b en 1m30s

**qwen2.5:1.5b** (39 s)

> Le document "2026/mars/neurologie.pdf" contient un courrier concernant Mme Alice Exemple. Elle présente des paresthesies des membres superieurs qui ont commencé il y a trois mois. Un électroencéphalogramme des membres superieurs est recommandé comme complément de l'examen.
> Généré par qwen2.5:1.5b en 39s

**qwen3:0.6b** (28 s)

> Le document 2026/mars/neurologie.pdf contient :
> - Un cabinet de neurologie (fictif) ;
> - Un courrier concernant Madame Alice Exemple, consultation du 9 mars 2026 ;
> - Des paresthesies des membres superieurs et un électromyogramme proposé.
> Aucune information supplémentaire n'est fournie.
> Généré par qwen3:0.6b en 28s

**qwen3:1.7b** (59 s)

> Extrait du fichier 2026/mars/neurologie.pdf (page 1) :
> Ce document est un exemple fictif pour un cabinet de neurologie, concernant une consultation de Mme Alice Exemple (9 mars 2026) sur des paresthesies des membres supérieurs, avec un electromyogramme proposé en complément.
> Généré par qwen3:1.7b en 59s

## The wider battery, `qwen2.5:1.5b`

**D8** `Que dit BAIL-CABINET-2024 ?` (34 s)

> Le document BAIL-CABINET-2024 contient une convention de bail de 85 mètres carrés dans un local professionnel situé 14 rue des Tilleuls, 69003 Lyon, entre M. Jean Fontaine (bailleur) et Mme Camille Exemple (preneuse exerçant à titre libéral). Le bail est de dix-neuf ans, allant de 1er avril 2024 à 31 mars 2033, avec un loyer fixe à 18 400 euros par an, calculé à partir du taux national des loyers des activités tertiaires.
> Généré par qwen2.5:1.5b en 34s

**D9** `Que dit courrier-cpam-radiation ?` (30 s)

> Le courrier-cpam-radiation.pdf mentionne que l'assure(e) M. Hugo Exemple, numero de sécurité sociale fictif 1 85 03 69 123 456 78, a été radié du régime général à compter du 1er février 2026, suite à son affiliation au régime d'un nouvel employeur.
> Généré par qwen2.5:1.5b en 30s
> Le chiffre « 69 123 456 » ne figure dans aucun des extraits utilisés ni dans votre question : vérifiez-le dans vos documents.

**C1** `Que dit le courrier de la CPAM concernant la radiation ?` (43 s)

> Le courrier de la CPAM mentionne que l'assure(e) M. Hugo Exemple a été radié(e) du régime général à compter du 1er février 2026, suite à son affiliation au régime d'un nouvel employeur.
> Généré par qwen2.5:1.5b en 43s

**C3** `Quelle est la durée du bail du cabinet d'après le contrat ?` (45 s)

> La durée du bail du cabinet est de NEUF ANS entiers et consecutifs, qui commence le 1er avril 2024 et se termine le 31 mars 2033.
> Généré par qwen2.5:1.5b en 45s

**C6** `Quel est le prix de l'imprimante dans le devis MedSupply ?` (44 s)

> Le devis MedSupply mentionne que la montant total du devis est de 1 200,00 euros HT.
> Généré par qwen2.5:1.5b en 44s

**C7** same question, quote not selected (43 s)

> Je suis désolé, mais je ne suis pas en mesure de répondre à cette question en utilisant les informations contenues dans la liste des fichiers.
> Généré par qwen2.5:1.5b en 43s

**C9** `Quelle consultation a eu lieu le 9 mars 2026 ?` (41 s)

> La consultation de Mme Alice Exemple a eu lieu le 9 mars 2026.
> Généré par qwen2.5:1.5b en 41s

**C10** `Quel est le montant de l'assurance responsabilité civile du cabinet ?` (58 s)

> Le cabinet est facturé à la Sécurité Sociale pour son assurance responsabilité civile, mais la quantité exacte n'est pas établie dans la documentation fournie.
> [1] "Courrier-cpam-radiation.pdf" n'a pas d'informations sur la quantité d'assurance responsabilité civile du cabinet.
> [2] "Bail-cabinet-2024.pdf" ne contient aucune information relative à la responsabilité civile d'un cabinet.
> [3] "2026/mars/neurologie.pdf" ne mentionne pas d'assurance civile.
> [4] "2026/janvier/neurologie.pdf" n'a pas d'informations sur la quantité d'assurance civile.
> [5] "Convention-remplacement-dr-martin.docx" ne contient aucune information spécifique sur l'assurance civile.
> [6] "Devis-imprimante-medsupply.pdf" ne donne pas d'informations sur la quantité d'assurance civile.
> En conclusion, il semble que le montant de l'assurance responsabilité civile du cabinet n'est pas mentionné dans la documentation fournie.
> Généré par qwen2.5:1.5b en 58s

## Tables (answered by the engine, no AI)

**T1** `Quelle est la somme des montant ?`

> Somme de montant : **2 215**.
> Calculé sur 8 lignes.
> Réponse calculée depuis votre dossier des données, sans l'IA.

**T2** `Quel est le maximum de montant ?`

> Plus grande valeur de montant : **620**.
> Calculé sur 8 lignes.
> Réponse calculée depuis votre dossier des données, sans l'IA.

**T4** `Quel est le total de montant par fournisseur ?`

> Totaux de montant, par groupe :
> - MedSupply : 1 450
> - Fournitures Dupont : 550
> - Papeterie Lefevre : 215
> Calculé sur 8 lignes.
> Réponse calculée depuis votre dossier des données, sans l'IA.

**T5** `Combien de factures pour le fournisseur MedSupply ?`

> Nombre de lignes : **3**.
> Calculé sur 3 lignes.
> Compris comme : fournisseur = MedSupply.
> Réponse calculée depuis votre dossier des données, sans l'IA.

**T7** `Combien de factures pour Alfa ?`

> « Alfa » ne correspond à aucune valeur réelle de factures-fournisseurs-2026.xlsx.
> Ses colonnes sont : date, fournisseur, montant.
> Essayez de demander une somme, un minimum, un maximum, un nombre de lignes, la liste des valeurs d'une colonne, ou le groupe qui a le plus grand total.
> Par exemple : « Quelle est la somme de montant ? »
> Ou : « Quel(le) fournisseur a le plus de montant ? »
> Réponse calculée depuis votre dossier des données, sans l'IA.

**T9** `Combien de rendez-vous le lundi ?`

> Nombre de lignes : **3**.
> Calculé sur 3 lignes.
> Compris comme : date un lundi.
> Réponse calculée depuis votre dossier des données, sans l'IA.

**T10** `Combien de rendez-vous entre le 09/03/2026 et le 15/03/2026 ?`

> Nombre de lignes : **4**.
> Calculé sur 4 lignes.
> Compris comme : date entre 2026-03-09 et 2026-03-15.
> Réponse calculée depuis votre dossier des données, sans l'IA.

**T12** `Quelle salle a le moins de duree_min ?`

> Plus petit total duree_min par salle : **Salle 3**, avec **50**.
> Il s'agit du total par salle, additionné sur toutes les lignes, pas de la plus petite ligne isolée.
> Les premiers, sur 3 groupes en tout :
> - Salle 3 : 50
> - Salle 2 : 95
> - Salle 1 : 100
> Calculé sur 10 lignes.
> Réponse calculée depuis votre dossier des données, sans l'IA.

**T15** `Quelle est la somme des montant ?` (all three tables selected)

> Somme de montant : **2 215**.
> Calculé sur 8 lignes.
> Réponse calculée depuis votre dossier des données, sans l'IA.

**T16** `Combien de lignes ?` (all three tables selected), then the three buttons

> Plusieurs tableurs sont sélectionnés. Lequel voulez-vous ? Nommez-le dans la question :
> - factures-fournisseurs-2026.xlsx
> - publipostage/donnees_publipostage.csv
> - rdv-mars-2026.xlsx
> Réponse calculée depuis votre dossier des données, sans l'IA.

> Vous: Combien de lignes ? dans factures-fournisseurs-2026.xlsx
> Assistant: Nombre de lignes : **8**.
> Calculé sur 8 lignes.
> Compris comme : date en 2026.

> Vous: Combien de lignes ? dans publipostage/donnees_publipostage.csv
> Assistant: Nombre de lignes : **5**.
> Calculé sur 5 lignes.

> Vous: Combien de lignes ? dans rdv-mars-2026.xlsx
> Assistant: Nombre de lignes : **10**.
> Calculé sur 10 lignes.
> Compris comme : date en mars et date en 2026.

(each of the three answers ended with `Réponse calculée depuis votre dossier des données, sans l'IA.`)

## M1: `Le montant total facturé par MedSupply correspond-il à ce qui est indiqué dans le devis signé ?`

Every answer ends with the engine block:

> Calculé dans vos données (par le moteur, pas par l'IA) :
> Somme de montant : **1 450**.
> Calculé sur 3 lignes.
> Compris comme : fournisseur = MedSupply.

Text above the block, per model:

**gemma4:e2b** (2m28)

> Le montant total du devis indiqué est de 1 200,00 euros HT [1].
> La somme totale des factures est de 1 450 [Sheet Factures].
> Le montant total facturé par MedSupply ne correspond pas à ce qui est indiqué dans le devis signé.

**granite3.1-moe:1b** (26 s)

> Non, le montant total facturé par MedSupply ne correspond pas à ce qui est indiqué dans le devis signé. Le montant total facturé par MedSupply est fixé à 1 200,00 euros HT.

**granite3.1-moe:3b** (37 s)

> Le montant total facturé par MEDSUPPLY, comme indiqué dans le devis signé, est égal à 1 200,00 euros HT. Ce montant correspond à ce que le devis propose, sans inclure toute commande complémentaire réalisée l'année.

**ministral-3:3b** (1m25)

> - Le devis signé (DV-2026-0117) indique un montant total de **1 200,00 € HT**.
> - La somme des factures (extrait de la table) est de **1 450 €**.
> Non, le montant total facturé ne correspond pas au devis signé.

**qwen2.5:1.5b** (48 s)

> Oui, le montant total facturé par MedSupply correspond à ce qui est indiqué dans le devis signé. Selon le devis signé (montant total : 1 200,00 euros HT), le montant total facturé par MedSupply est de 1 200,00 euros HT.

**qwen3:0.6b** (35 s)

> Le montant total facturé par MedSupply est 1 200,00 euros, indiqué dans le devis signé. Le tableau indique 1 450 euros (computed from 3 rows), ce qui correspond à une incohérence entre les deux sources.

**qwen3:1.7b** (1m10)

> Le montant total facturé par MedSupply (1 450 €) ne correspond pas au montant indiqué dans le devis signé (1 200 € HT). Le tableau résultats indique une somme de 1 450 €, bien que le document mentionne un montant total de 1 200 €.

(each followed by `Généré par <model> en <time>` above the block)

## M2: `Quel est le montant total pour le fournisseur mentionné dans cette lettre ?`

Same engine block under every answer.

**gemma4:e2b** (2m59)

> Le montant total du devis est de 1 200,00 euros HT [1].

**granite3.1-moe:1b** (37 s)

> Le montant total pour le fournisseur mentionné dans cette lettre est de 1 200 € HT.

**granite3.1-moe:3b** (54 s)

> Le montant total pour le fournisseur mentionné dans cette lettre est de 1 200,00 euros HT.

**ministral-3:3b** (2m13)

> - Extrait [1] : montant total du devis = **1 200,0 € HT**.
> - Dossier des données : montant total des factures = **1 450 €** (non lié au devis cité).
> Le montant total pour le fournisseur **Medsupply** n’est pas précisé dans le devis.

**qwen2.5:1.5b** (29 s)

> Le montant total du devis pour MEDSUPPLY FOURNITURES MEDICALES mentionné dans cette lettre est de 1 200,00 euros HT.

**qwen3:0.6b** (40 s)

> Le montant total pour le fournisseur mentionné dans cette lettre est 1 200,00 euros HT.

**qwen3:1.7b** (57 s)

> Le montant total pour le fournisseur mentionné dans la lettre est détaillé dans le document [1], où le montant total est fixé à 1 200,00 euros HT. Le tableau résultats [Sheet Factures] montre une somme de 1 450 euros, mais ce dernier est calculé indépendamment et ne doit pas être utilisé pour établir le montant réel.

## M3 (engine only)

> Vous: Quelle est la somme des montant ?
> Assistant: Somme de montant : **2 215**.
> Calculé sur 8 lignes.
> Réponse calculée depuis votre dossier des données, sans l'IA.
> Réponse établie depuis vos tableaux ; les documents sélectionnés n'étaient pas nécessaires.

## N1: `Rédige un e-mail de relance pour un fournisseur dont la facture n'est pas réglée.`

Each answer ends with `Réponse sans vos documents.` and the `Généré par` line.

**gemma4:e2b** (5m34)

> Objet : Relance concernant la facture [Numéro de facture]
> Cher/Chère [Nom du contact ou du service comptabilité],
> Nous faisons suite à notre facture numéro [Numéro de facture], d'un montant de [Montant de la facture], dont la date d'échéance était le [Date d'échéance].
> À ce jour, nous n'avons pas encore reçu le règlement de cette facture.
> Nous vous prions de bien vouloir procéder au paiement dans les plus brefs délais. Si le paiement a déjà été effectué, veuillez ne pas tenir compte de ce courriel et nous transmettre la preuve de virement.
> Nous restons à votre disposition pour toute information complémentaire.
> Cordialement,
> [Votre Nom]
> [Votre Fonction]

**granite3.1-moe:1b** (47 s). The block below appeared **three times in a row** in the original, the second and third copies each introduced by a line `Réponse:`; shown once:

> Cher [Nom du fournisseur],
> Je vous écris pour répondre à votre facture de [date] pour [quantité] d'articles. Néanmoins, nous avons rencontré un problème avec la facturation et nous avons commis une erreur. Nous nous efforçons de rectifier cet erreur dès que possible.
> Nous voulons vous informer que nous avons reçu une demande de rectification de votre part concernant la facture [quantité] pour [date]. Nous vous suggérons de contacter le service clientèle de votre fournisseur pour discuter de cette demande et pour évaluer la façon de résoudre le problème.
> Nous vous remercions de votre compréhension et de votre collaboration pour résoudre ce problème.
> Cordialement,
> [Votre nom]
> [Réponse en français]

**granite3.1-moe:3b** (1m01)

> Subject : Relance pour la facture non réglée
> Cher [Nom du fournisseur],
> Nous avons reçu votre dernière facture pour [Nom du service ou du produit] et nous nous réjouissons de constater que vous avez effectué une transaction récente. Cependant, nous apprécions votre étroite collaboration et souhaitons vous assurer que toutes nos échéances sont honorées.
> Nous devons vous rappeler que notre dernière facture a été établie pour [Montant total de la facture], à [Date de facturation], et qu'elle doit être réglée dans les trois [Nombre de jours] prochains.
> Nous espérons que cette rappel sera suffisant pour que vous puissiez finaliser votre paiement. Si vous avez des questions ou des préoccupations concernant cette facture, veuillez nous contacter sans hésitation.
> Nous vous remercions d'avance pour votre attention et votre rapidité à résoudre ce point.
> Cordialement,
> [Votre nom]
> [Votre poste]
> [N° de téléphone ou adresse email]
> Dans ce cas, aucune facture n'est sélectionnée, et l'utilisateur peut choisir une facture dans la liste des documents.

**ministral-3:3b** (1m59)

> Objet : Relance pour règlement de la facture [N°XXXXX]
> Bonjour [Nom du fournisseur ou "Monsieur/Madame"],
> Nous vous prions de bien vouloir régulariser le paiement de la facture [N°XXXXX] échéant le [date d'échéance] relative à [détails du service/résumé du montant si pertinent].
> Veuillez nous confirmer par retour qu’elle a été payée ou nous indiquer les délais pour un règlement.
> Cordialement,
> [Votre nom/prénom]
> [Votre poste]
> [Nom de l’entreprise]

**qwen2.5:1.5b** (41 s)

> Bonjour,
> Je vous remercie pour la dernière facture que vous avez envoyée. Néanmoins, je me suis révélé avoir des difficultés à régler cette facture. J'aimerais que vous fassiez un effort pour régler cette facture en fin de mois.
> Cordialement,
> [Votre nom]

**qwen3:0.6b** (28 s)

> Aucun document n'a été attaché : la demande a été faite en sélection. Rédigez un exemplaire de résumé pour un fournisseur.

**qwen3:1.7b** (1m06)

> **Objet : Réclamation concernant la facture n°XXX**
> Cher [Nom du fournisseur],
> Nous vous rappelons que la facture n°XXX, issue du mois de [Date], est actuellement en attente de règlement. Le paiement est actuellement déchu, ce qui entraîne des conséquences administratives.
> Pour permettre le règlement, nous vous proposons une option de paiement planifié, à partir du [Date de début du plan]. Veuillez agréer l'approbation de cette proposition.
> Sachez que nous sommes disponibles pour discuter de solutions adaptées.
> Cordialement,
> [Votre nom]
> [Votre poste]
> [Votre contact]
