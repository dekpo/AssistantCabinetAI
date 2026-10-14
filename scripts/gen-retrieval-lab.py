"""Generate the retrieval lab's sample set (phase 0, `docs/RETRIEVAL-LAB.md`): a few dozen documents, small on purpose
so that an Analyse on the tower takes minutes, with a fact planted in each and questions worded so that the
distinctive word of the answer is often NOT in the question.

Fictional content only: every person, number and reference is invented. Same output shape as the knowledge sandbox
(`documents/`, `documents.json`, `questions.json`, `entities.json`, `workbooks.json`, `manifest.json`), so the retrieval
lab test reads it through `ACAI_SANDBOX_DIR`. Each question adds `fact_pages` (the page that carries the fact, for
PDF) and `kind` (the family it tests).

    python scripts/gen-retrieval-lab.py                       # the versioned synthetic set, fixtures/retrieval-lab
    python scripts/gen-retrieval-lab.py --books-dir "C:\\Users\\elise\\Downloads" --out data/retrieval-lab-full

With `--books-dir` it also cuts a run of pages out of two French textbooks (the anatomy manual of Marcel Pirard and the
medical microbiology course of Dr Ayad Wissem) into `livres/`. The books are never committed: their rights are not
stated in the files, so the full set goes under `data/`, which git ignores. Needs `pypdf` for that part only.
The output is deterministic.
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import sys
import textwrap
import unicodedata
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import retrieval_lab_bulk as bulk  # noqa: E402
from knowledge_sandbox import writers  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
LINES_PER_PAGE = 46
WIDTH = 92
MAX_FILES = 50


def squash(text: str) -> str:
    """The way the lab looks for a fact in a passage: accents, case and punctuation ignored."""
    folded = "".join(c for c in unicodedata.normalize("NFD", text) if not 0x300 <= ord(c) <= 0x36F)
    return " ".join(w for w in re.split(r"[\W_]+", folded.lower()) if w)


# ------------------------------------------------------------------------------------------------ the documents
# (path, format, title, paragraphs, [(kind, question, [facts])]) ; facts must be written in the paragraphs.
# kind: "keyword" the question reuses a distinctive word of the sentence; "paraphrase" it does not.

SHORT: list[tuple] = [
    ("courriers/2026-03-04_lettre-caisse-ferrand.pdf", "pdf", "Caisse maladie Helvia - Décompte de prestations", [
        "Madame Odile Ferrand, rue des Alpes 14, 1201 Genève.",
        "Madame, nous avons bien reçu votre demande du mois de février. Nous avons pris en charge un montant de CHF 212.40 "
        "pour vos soins ambulatoires, déduction faite de la franchise.",
        "Nous vous prions d'agréer, Madame, nos salutations distinguées. Le service des prestations."],
     [("paraphrase", "Combien l'assurance a-t-elle remboursé à Odile Ferrand ?", ["212.40"])]),
    ("courriers/2026-03-11_lettre-caisse-rochat.pdf", "pdf", "Caisse maladie Helvia - Décompte de prestations", [
        "Madame Sophie Rochat, chemin des Vignes 3, 1004 Lausanne.",
        "Madame, nous avons bien reçu votre demande du mois de février. Nous avons pris en charge un montant de CHF 148.90 "
        "pour vos soins ambulatoires, déduction faite de la franchise.",
        "Nous vous prions d'agréer, Madame, nos salutations distinguées. Le service des prestations."],
     [("paraphrase", "Quel remboursement Sophie Rochat a-t-elle reçu de sa caisse ?", ["148.90"])]),
    ("courriers/2026-05-27_lettre-caisse-praz.pdf", "pdf", "Caisse maladie Helvia - Décompte de prestations", [
        "Madame Yvonne Praz, avenue de la Gare 22, 1260 Nyon.",
        "Madame, nous avons bien reçu votre demande du mois d'avril. Nous avons pris en charge un montant de CHF 96.30 "
        "pour vos soins ambulatoires, déduction faite de la franchise.",
        "Nous vous prions d'agréer, Madame, nos salutations distinguées. Le service des prestations."],
     [("paraphrase", "Quelle somme la caisse a-t-elle versée à Yvonne Praz ?", ["96.30"])]),
    ("courriers/2026-04-02_resultat-laboratoire-bonvin.docx", "docx", "Laboratoire Cantonal - Analyse de sang", [
        "Patient : Laurent Bonvin, né en 1968. Prélèvement du 1er avril 2026.",
        "Créatinine : 84 µmol/L (valeurs de référence 60 à 110 µmol/L).",
        "Sodium : 140 mmol/L. Potassium : 4.2 mmol/L. Aucune anomalie notable."],
     [("keyword", "Quelle est la créatinine de Laurent Bonvin ?", ["84 µmol/L"])]),
    ("resultats/2026-04-09_resultat-glycemie-moret.txt", "txt", "Résultat d'analyse", [
        "Patient : Pierre Moret. Prélèvement du 8 avril 2026.",
        "Glycémie à jeun : 5.8 mmol/L (valeurs de référence 3.9 à 6.1 mmol/L).",
        "Interprétation : dans la norme."],
     [("paraphrase", "Quel est le taux de sucre dans le sang de Pierre Moret à jeun ?", ["5.8 mmol/L"])]),
    ("resultats/2026-07-08_resultat-glycemie-jaquet.txt", "txt", "Résultat d'analyse", [
        "Patient : Raymond Jaquet. Prélèvement du 7 juillet 2026.",
        "Glycémie à jeun : 6.9 mmol/L (valeurs de référence 3.9 à 6.1 mmol/L).",
        "Interprétation : au-dessus de la norme, contrôle conseillé."],
     [("paraphrase", "Quel est le taux de sucre dans le sang de Raymond Jaquet ?", ["6.9 mmol/L"])]),
    ("comptes-rendus/2026-04-15_compte-rendu-reunion-equipe.txt", "txt", "Compte rendu de la réunion d'équipe", [
        "Présents : toute l'équipe du secrétariat et des soins.",
        "Il a été décidé de déplacer la réunion d'équipe au mardi à 12h15, dans la salle commune.",
        "Prochain point : les remplacements de l'été."],
     [("paraphrase", "Quand l'équipe se réunit-elle désormais ?", ["mardi à 12h15"])]),
    ("comptes-rendus/2026-06-24_compte-rendu-reunion-formation.txt", "txt", "Compte rendu de la réunion de formation", [
        "Présents : la responsable de la formation et deux assistantes.",
        "Il a été décidé de proposer une demi-journée de formation au printemps, le jeudi à 14h00, en salle B.",
        "Prochain point : le budget des formations."],
     [("paraphrase", "Quand est proposée la demi-journée de perfectionnement ?", ["jeudi à 14h00"])]),
    ("comptes-rendus/2026-05-06_compte-rendu-reunion-materiel.docx", "docx", "Compte rendu de la réunion matériel", [
        "Présents : la direction, le responsable du matériel.",
        "Le renouvellement de l'autoclave est reporté à septembre 2027. Le budget prévu est de CHF 9 800.",
        "Le contrat de maintenance reste inchangé."],
     [("keyword", "Quel budget est prévu pour l'autoclave ?", ["CHF 9 800"])]),
    ("factures/2026-05-12_devis-informatique-leman.pdf", "pdf", "Devis - Informatique Léman SA", [
        "Devis n° D-5521 pour le renouvellement de deux postes de travail et d'un serveur.",
        "Total HT : CHF 3 170.00. TVA : CHF 250.00. Total TTC : CHF 3 420.00.",
        "Validité du devis : trois mois."],
     [("keyword", "Quel est le montant total du devis informatique de Léman ?", ["CHF 3 420.00"])]),
    ("factures/2026-07-22_devis-informatique-alpina.pdf", "pdf", "Devis - Alpina Systèmes SA", [
        "Devis n° A-0877 pour le renouvellement de deux postes de travail et d'un serveur.",
        "Total HT : CHF 2 760.00. TVA : CHF 220.00. Total TTC : CHF 2 980.00.",
        "Validité du devis : deux mois."],
     [("keyword", "Quel est le montant total du devis informatique d'Alpina ?", ["CHF 2 980.00"])]),
    ("courriers/2026-05-20_lettre-medecin-conseil-vuillemin.pdf", "pdf", "Cabinet du médecin-conseil", [
        "Madame Madeleine Vuillemin, route de Berne 8, 1700 Fribourg.",
        "Madame, je vous confirme le rendez-vous du 24 juin 2026 à 14h30 pour l'examen demandé par votre assureur.",
        "Veuillez vous munir de vos pièces d'identité."],
     [("paraphrase", "Quand est la consultation de Madeleine Vuillemin ?", ["24 juin 2026"])]),
    ("courriers/2026-06-03_lettre-arret-travail-gaillard.docx", "docx", "Certificat d'arrêt de travail", [
        "Monsieur Hugo Gaillard, employé de commerce.",
        "Je certifie que son état de santé justifie un arrêt de travail de 14 jours à compter du 2 juin 2026.",
        "Reprise prévue sous réserve d'un nouveau contrôle."],
     [("paraphrase", "Combien de temps Hugo Gaillard est-il en incapacité de travailler ?", ["14 jours"])]),
    ("courriers/2026-06-10_lettre-vaccination-martinet.txt", "txt", "Carnet de vaccination", [
        "Madame Noémie Martinet, née en 1990.",
        "Le vaccin contre le tétanos a été administré ce jour. Le rappel est prévu dans dix ans.",
        "Aucune réaction locale signalée."],
     [("paraphrase", "Quand Noémie Martinet doit-elle refaire son vaccin antitétanique ?", ["dans dix ans"])]),
    ("resultats/2026-06-17_resultat-cholesterol-dubois-alice.pdf", "pdf", "Laboratoire Cantonal - Bilan lipidique", [
        "Patiente : Alice Dubois, née en 1975. Prélèvement du 16 juin 2026.",
        "Cholestérol LDL : 3.4 mmol/L. Cholestérol HDL : 1.5 mmol/L.",
        "Interprétation : à surveiller."],
     [("paraphrase", "Quel est le mauvais cholestérol d'Alice Dubois ?", ["3.4 mmol/L"])]),
    ("resultats/2026-06-17_resultat-cholesterol-dubois-charles.pdf", "pdf", "Laboratoire Cantonal - Bilan lipidique", [
        "Patient : Charles Dubois, né en 1971. Prélèvement du 16 juin 2026.",
        "Cholestérol LDL : 4.1 mmol/L. Cholestérol HDL : 1.1 mmol/L.",
        "Interprétation : au-dessus de la cible."],
     [("paraphrase", "Quel est le mauvais cholestérol de Charles Dubois ?", ["4.1 mmol/L"])]),
    ("contrats/2026-07-01_contrat-nettoyage-proprex.pdf", "pdf", "Contrat de nettoyage - Proprex SA", [
        "Le présent contrat est conclu pour une durée de 24 mois, renouvelable.",
        "Le prix est de CHF 640 par mois, payable à la fin de chaque mois.",
        "Résiliation par lettre recommandée, trois mois avant l'échéance."],
     [("paraphrase", "Quelle est la mensualité du contrat de ménage ?", ["CHF 640"])]),
    ("contrats/2026-07-15_attestation-assurance-rc.docx", "docx", "Attestation d'assurance responsabilité civile", [
        "L'assureur atteste que la responsabilité civile professionnelle est couverte jusqu'au 31 décembre 2026.",
        "Prime annuelle responsabilité civile : CHF 1 150.",
        "Somme assurée : CHF 5 000 000."],
     [("keyword", "Quelle est la prime annuelle de l'assurance de responsabilité civile ?", ["CHF 1 150"])]),
    ("courriers/2026-08-04_facture-transport-aebi.pdf", "pdf", "Facture de transport sanitaire", [
        "Monsieur Gilbert Aebi, rue du Lac 5, 1800 Vevey.",
        "Le transport en ambulance du 3 août 2026 a été facturé CHF 385. Franchise non comprise.",
        "Merci de régler sous trente jours."],
     [("paraphrase", "Combien a coûté le trajet en ambulance de Gilbert Aebi ?", ["CHF 385"])]),
    ("comptes-rendus/2026-08-19_note-formation-hygiene.md", "md", "Note de service", [
        "La formation à l'hygiène des mains aura lieu le 9 octobre 2026 en salle B.",
        "La présence de tout le personnel est demandée."],
     [("keyword", "Quand a lieu la formation à l'hygiène des mains ?", ["9 octobre 2026"])]),
]


def nir_key(body13: str) -> str:
    return f"{97 - (int(body13) % 97):02d}"


NIR_BODY = "2850578006084"
NIR_SPACED = f"2 85 05 78 006 084 {nir_key(NIR_BODY)}"

IDENTIFIERS: list[tuple] = [
    ("factures/2026-02-03_facture-fac-2026-0417.pdf", "pdf", "Facture", [
        "Facture n° FAC-2026-0417 du 3 février 2026. Client : Cabinet des Eaux-Vives.",
        "Fourniture de matériel de bureau. Total à payer : CHF 275.00, paiement à trente jours."],
     [("identifier", "Quel est le total de la facture 2026-0417 ?", ["CHF 275.00"])]),
    ("factures/2026-02-17_facture-fac-2026-0418.pdf", "pdf", "Facture", [
        "Facture n° FAC-2026-0418 du 17 février 2026. Client : Cabinet des Eaux-Vives.",
        "Fourniture de matériel de bureau. Total à payer : CHF 310.50, paiement à trente jours."],
     [("identifier", "Quel est le montant de la facture FAC-2026-0418 ?", ["CHF 310.50"])]),
    ("paiements/2026-03-02_ordre-de-paiement.txt", "txt", "Ordre de paiement", [
        "Bénéficiaire : Pharma-Dépôt Romandie SA. IBAN : CH93 0076 2011 6238 5295 7.",
        "Motif : livraison de consommables du mois de février."],
     [("identifier", "Quel document mentionne l'IBAN CH9300762011623852957 ?", ["Pharma-Dépôt Romandie"])]),
    ("patients/2026-03-09_dossier-administratif-praz.docx", "docx", "Dossier administratif", [
        "Patiente : Yvonne Praz, avenue de la Gare 22, 1260 Nyon.",
        "Numéro AVS : 756.1234.5678.97. Caisse maladie : Helvia."],
     [("identifier", "À qui appartient le numéro AVS 7561234567897 ?", ["Yvonne Praz"])]),
    ("patients/2026-03-23_dossier-administratif-pellet.pdf", "pdf", "Dossier administratif", [
        "Patient : Sylvain Pellet, rue de la Paix 9, 74000 Annecy.",
        f"Numéro d'assurance-maladie : {NIR_SPACED}. Caisse : CPAM de la Haute-Savoie."],
     [("identifier", f"Quel patient a le numéro d'assurance-maladie {NIR_BODY}{nir_key(NIR_BODY)} ?", ["Sylvain Pellet"])]),
    ("courriers/2026-04-28_reference-sinistre.txt", "txt", "Déclaration de sinistre", [
        "Notre référence : SIN-7781-B. Dégât des eaux au cabinet du 20 avril 2026.",
        "La franchise applicable à ce sinistre est de CHF 500."],
     [("identifier", "Quel est le montant de la franchise du sinistre SIN-7781-B ?", ["CHF 500"])]),
]

PROCEDURE_STEPS = [
    "Accueillir la personne et vérifier son identité.",
    "Demander la carte d'assuré et la photocopier.",
    "Vérifier les coordonnées et le médecin traitant.",
    "Ouvrir le dossier administratif.",
    "Expliquer le déroulement de la consultation.",
    "Proposer un questionnaire d'accueil.",
    "Remettre le formulaire de consentement signé au secrétariat avant la consultation.",
    "Informer la personne du délai d'attente.",
    "Prévenir le médecin de l'arrivée.",
    "Remettre les documents de sortie.",
]

STRUCTURED: list[tuple] = [
    ("procedures/2026-09-14_procedure-accueil.docx", "docx", "Procédure d'accueil d'un nouveau patient",
     ["Les étapes à suivre sont les suivantes."] + [f"{n}. {step}" for n, step in enumerate(PROCEDURE_STEPS, 1)],
     [("list_item", "Quelle est la septième étape de l'accueil d'un nouveau patient ?",
       ["Remettre le formulaire de consentement"])]),
    ("procedures/2026-09-21_politique-de-conservation.md", "md", "Politique de conservation des documents", [
        "## 1. Objet\nCette politique fixe les règles de conservation.",
        "## 2. Champ d'application\nElle s'applique à tout le personnel.",
        "## 3. Responsabilité\nLa direction en est responsable.",
        "## 4. Conservation des dossiers : vingt ans\nVoir la règle ci-dessus.",
        "## 5. Destruction\nLa destruction est consignée."],
     [("heading", "Combien de temps conserve-t-on les dossiers ?", ["vingt ans"])]),
]

ABSENT = [
    "Combien l'assurance a-t-elle remboursé à Valérie Nusbaumer ?",
    "Quel est le total de la facture FAC-2026-0999 ?",
    "Quel est le mauvais cholestérol de Bruno Aeschlimann ?",
    "Quand a lieu la formation à la radioprotection ?",
    "Quelle est la prime annuelle d'assurance du véhicule ?",
    "Quel budget est prévu pour le scanner ?",
    "À qui appartient le numéro AVS 7569999999991 ?",
    "Quels sont les horaires d'ouverture du secrétariat ?",
]

# --- the table split by a page break: the header is the last line of page 1, the rows start page 2.
TABLE_HEADER = f"{'Prestation':<34}{'Tarif de base (CHF)':<26}Participation patient (CHF)"
TABLE_ROWS = [
    ("Consultation initiale", "80.00", "62.00"),
    ("Radiographie du poignet", "120.00", "84.50"),
    ("Echographie abdominale", "160.00", "117.30"),
    ("Electrocardiogramme", "55.00", "41.80"),
    ("Prise de sang de contrôle", "35.00", "27.40"),
]


def table_lines() -> list[str]:
    intro = [
        "Tarif des prestations du cabinet - édition de septembre 2026",
        "",
        "Les tarifs ci-dessous sont indiqués en francs suisses. Ils sont valables pour tous les assurés "
        "ayant une couverture de base. Les prestations sont facturées selon le tarif en vigueur. "
        "La participation du patient comprend la franchise et la quote-part, selon son contrat. "
        "Les rendez-vous manqués sans annulation préalable de vingt-quatre heures sont facturés. "
        "Pour toute question sur une facture, veuillez contacter le secrétariat pendant les heures d'ouverture. "
        "Les prestations non prévues dans cette liste font l'objet d'un devis écrit. "
        "Les prix peuvent être modifiés au 1er janvier de chaque année.",
    ]
    intro = [line for block in intro for line in (textwrap.wrap(block, 88) or [""])]
    lines = list(intro)
    while len(lines) < LINES_PER_PAGE - 3:
        lines.append("Voir les conditions générales du cabinet pour le détail des modalités de facturation.")
    lines += ["", "Tableau des prestations", TABLE_HEADER]
    assert len(lines) == LINES_PER_PAGE, len(lines)
    for name, base, share in TABLE_ROWS:
        lines.append(f"{name:<34}{base:<26}{share}")
    lines.append("")
    lines.append("Fin du tableau. Les montants sont arrondis au centime.")
    return lines


TABLE_QUESTIONS = [
    ("table_break", "Quelle est la part à payer par le patient pour une radiographie du poignet ?", ["84.50"]),
    ("table_break", "Combien le patient doit-il payer pour un électrocardiogramme, et sous quelle colonne ?",
     ["Participation patient", "41.80"]),
    ("table_break", "Quel est le tarif de base de l'échographie abdominale ?", ["160.00"]),
]
TABLE_PATH = "tarifs/2026-09-01_tarif-des-prestations.pdf"

# --- the two textbook extracts: (file, source pdf, first page, last page, [(kind, question, [facts], original page)])
BOOKS = [
    ("livres/anatomie-circulation-extrait.pdf", "anatomie-et-physiologie-humaines-marcel-pirard-4717.pdf", 101, 130, [
        ("book", "À quel rythme le nœud sinu-atrial se dépolarise-t-il spontanément ?", ["70 à 80"], 110),
        ("book", "Quelle est la fréquence naturelle du stimulateur du cœur ?", ["70 à 80"], 110),
        ("book", "Quel volume de sang chaque ventricule éjecte-t-il à chaque battement ?", ["70 à 90 ml"], 111),
        ("book", "Combien de molécules d'hémoglobine un globule rouge contient-il ?", ["280 millions"], 103),
        ("book", "Quelle proportion de la population porte l'antigène Rh ?", ["85", "antigènes Rh"], 124),
    ]),
    ("livres/microbiologie-urines-extrait.pdf", "microbiologie-medicale-dr-ayad-wissem-4721.pdf", 50, 79, [
        ("book", "Quel est le seuil normal de globules blancs dans l'urine ?", ["10 000 leucocytes"], 52),
        ("book", "Pendant combien de temps incube-t-on une uroculture ?", ["18 à 24 heures"], 56),
        ("book", "À combien de bactéries par millilitre correspond une colonie ?", ["1000 bactéries"], 55),
        ("book", "Combien de temps dure la coloration primaire au violet de gentiane ?",
         ["violet de gentiane pendant 30 secondes"], 53),
        ("book", "Quelle est la proportion de bactéries Gram négatif dans une selle normale ?",
         ["60 % de bactéries Gram"], 60),
        ("book", "Combien d'espèces de bactéries compte une flore normale ?", ["400 espèces"], 62),
    ]),
]
BOOK_ABSENT = "Quelle est la posologie de l'amoxicilline chez l'enfant ?"


# ------------------------------------------------------------------------------------------------- the writers


def page_of(paragraphs: list[str], title: str, fact: str) -> int:
    """The PDF page that holds `fact`, with the pagination `write_pdf_text` applies."""
    lines = [title, ""] + writers._wrap(paragraphs, WIDTH) if title else writers._wrap(paragraphs, WIDTH)
    for number, line in enumerate(lines):
        if squash(fact) in squash(line):
            return number // LINES_PER_PAGE + 1
    window = squash(" ".join(lines))
    assert squash(fact) in window, f"the fact {fact!r} is not in the document"
    return 1


def write_document(out: Path, path: str, fmt: str, title: str, paragraphs: list[str]) -> int:
    target = out / "documents" / path
    target.parent.mkdir(parents=True, exist_ok=True)
    if fmt == "pdf":
        return writers.write_pdf_text(target, title, paragraphs, LINES_PER_PAGE, WIDTH)
    if fmt == "docx":
        writers.write_docx(target, title, paragraphs)
    elif fmt == "txt":
        writers.write_text(target, title, paragraphs)
    elif fmt == "md":
        writers.write_markdown(target, title, paragraphs)
    else:
        raise ValueError(fmt)
    return 1


class Builder:
    def __init__(self, out: Path) -> None:
        self.out = out
        self.documents: list[dict] = []
        self.questions: list[dict] = []

    def add_document(self, path: str, fmt: str, kind: str, pages: int) -> None:
        self.documents.append({
            "index": len(self.documents) + 1, "path": path, "format": fmt, "extension": fmt, "kind": kind,
            "readable": True, "ocr": False, "pages": pages, "tier": len(self.documents) + 1, "special": "",
            "about": [], "mentions": [], "terms": [], "ocr_pages": [], "facts": {},
        })

    def add_question(self, kind: str, text: str, relevant: list[str], facts: list[str], pages: list[int]) -> None:
        self.questions.append({
            "id": f"L{len(self.questions) + 1:03d}", "category": kind, "text": text, "entities": [], "name_entities": [],
            "relevant_documents": relevant, "relevant_readable": relevant, "relevant_workbooks": [],
            "selection": {"documents": "all", "workbooks": "none"},
            "expect": {"resolution": "none", "between": [], "contains": facts}, "min_tier": 0,
            "follow_up_of": None, "carry_over": [], "kind": kind, "fact_pages": pages,
        })

    def short(self, entries: list[tuple], kind: str) -> None:
        for path, fmt, title, paragraphs, questions in entries:
            pages = write_document(self.out, path, fmt, title, paragraphs)
            self.add_document(path, fmt, kind, pages)
            for q_kind, text, facts in questions:
                for fact in facts:
                    assert squash(fact) in squash(" ".join(paragraphs) + " " + title), (path, fact)
                where = sorted({page_of(paragraphs, title, f) for f in facts}) if fmt == "pdf" else [1]
                self.add_question(q_kind, text, [path], facts, where)

    def table(self) -> None:
        lines = table_lines()
        assert len(writers._wrap(["\n".join(lines)], WIDTH)) == len(lines), "a table line was wrapped"
        target = self.out / "documents" / TABLE_PATH
        target.parent.mkdir(parents=True, exist_ok=True)
        pages = writers.write_pdf_text(target, "", ["\n".join(lines)], LINES_PER_PAGE, WIDTH)
        header_page = lines.index(TABLE_HEADER) // LINES_PER_PAGE + 1
        first_row_page = next(i for i, line in enumerate(lines) if line.startswith(TABLE_ROWS[0][0])) // LINES_PER_PAGE + 1
        assert (header_page, first_row_page) == (1, 2), (header_page, first_row_page)
        self.add_document(TABLE_PATH, "pdf", "table_break", pages)
        for kind, text, facts in TABLE_QUESTIONS:
            self.add_question(kind, text, [TABLE_PATH], facts, [2])

    def books(self, books_dir: Path, books: list) -> None:
        from pypdf import PdfReader, PdfWriter  # imported here: only the books need it

        for path, source, first, last, questions in books:
            reader = PdfReader(books_dir / source)
            writer = PdfWriter()
            for number in range(first - 1, last):
                writer.add_page(reader.pages[number])
            target = self.out / "documents" / path
            target.parent.mkdir(parents=True, exist_ok=True)
            with target.open("wb") as handle:
                writer.write(handle)
            self.add_document(path, "pdf", "book_extract", last - first + 1)
            texts = [(reader.pages[n].extract_text() or "") for n in range(first - 1, last)]
            for kind, text, facts, original in questions:
                page_text = re.sub(r"\s+", "", squash(texts[original - first]))
                for fact in facts:
                    # the extractor may split a word; compare without spaces
                    assert re.sub(r"\s+", "", squash(fact)) in page_text, (path, fact, original)
                self.add_question(kind, text, [path], facts, [original - first + 1])

    def bulk(self, total: int) -> None:
        taken = {"Odile Ferrand", "Sophie Rochat", "Yvonne Praz", "Laurent Bonvin", "Pierre Moret", "Raymond Jaquet",
                 "Madeleine Vuillemin", "Hugo Gaillard", "Noémie Martinet", "Alice Dubois", "Charles Dubois",
                 "Gilbert Aebi", "Sylvain Pellet"}
        for path, fmt, title, paragraphs, questions in bulk.Bulk(20261013, taken).documents(total):
            pages = write_document(self.out, path, fmt, title, paragraphs)
            self.add_document(path, fmt, "bulk", pages)
            for q_kind, text, facts in questions:
                for fact in facts:
                    assert squash(fact) in squash(" ".join(paragraphs) + " " + title), (path, fact)
                where = sorted({page_of(paragraphs, title, f) for f in facts}) if fmt == "pdf" else [1]
                self.add_question(q_kind, text, [path], facts, where)

    def finish(self, absent_extra: list, limit: int) -> dict:
        for text in ABSENT + absent_extra:
            self.add_question("absent", text, [], [], [])
        assert len(self.documents) <= limit, f"{len(self.documents)} files: more than the {limit} allowed"
        manifest = {
            "seed": 0, "documents": len(self.documents), "workbooks": 0, "questions": len(self.questions),
            "generated_by": "scripts/gen-retrieval-lab.py", "book_extracts": sum(1 for d in self.documents if d["kind"] == "book_extract"),
            "by_format": {f: sum(1 for d in self.documents if d["format"] == f) for f in sorted({d["format"] for d in self.documents})},
            "pages_total": sum(d["pages"] for d in self.documents),
        }
        for name, data in (("documents.json", self.documents), ("questions.json", self.questions),
                           ("entities.json", []), ("workbooks.json", []), ("manifest.json", manifest)):
            (self.out / name).write_text(json.dumps(data, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        return manifest


def build(out: Path, books_dir: Path | None, documents: int | None = None) -> dict:
    """`documents`: the size of the big set (templated documents fill what the handwritten ones and the books leave)."""
    if out.exists():
        shutil.rmtree(out)
    (out / "documents").mkdir(parents=True)
    builder = Builder(out)
    builder.short(SHORT, "short_text")
    builder.short(IDENTIFIERS, "identifier")
    builder.table()
    builder.short(STRUCTURED, "structured")
    absent_extra: list = []
    if books_dir is not None:
        books = BOOKS
        if documents:
            books = [(f, src, first, bulk.WIDER_LAST_PAGE.get(src, last), qs) for f, src, first, last, qs in BOOKS]
            books += bulk.EXTRA_BOOKS
        builder.books(books_dir, books)
        absent_extra = bulk.BOOK_ABSENT_BIG if documents else [BOOK_ABSENT]
    if documents:
        builder.bulk(max(0, documents - len(builder.documents)))
    return builder.finish(absent_extra, documents or MAX_FILES)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--out", type=Path, default=ROOT / "fixtures" / "retrieval-lab")
    parser.add_argument("--books-dir", type=Path, help="folder holding the textbook PDFs (adds the extracts)")
    parser.add_argument("--documents", type=int, help="the big set: fill up to this many files (needs --books-dir)")
    args = parser.parse_args()
    manifest = build(args.out, args.books_dir, args.documents)
    print(json.dumps(manifest, ensure_ascii=False))


if __name__ == "__main__":
    main()
