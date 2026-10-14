"""The big retrieval-lab set: five more textbook extracts and several hundred templated documents.

Used by `gen-retrieval-lab.py --documents N`. Fictional content only. The templated documents share a handful of
templates on purpose: hundreds of near-identical letters, results and invoices that differ by a name and a figure are
what a retrieval has to tell apart at the scale of a real folder. A question names one person or one number.
"""

from __future__ import annotations

import random
import unicodedata

# (file, source pdf, first page, last page, [(kind, question, [facts], original page)])
EXTRA_BOOKS = [
    ("livres/diagnostic-medecine-generale-extrait.pdf",
     "aide-au-diagnostic-en-medecine-generale-fred-f-ferri-jacques-wagner-ballon-6223.pdf", 33, 232, [
         ("book", "Entre quelles valeurs varie l'indice de pression systolique en cas de claudication ?",
          ["claudication", "0,5 à 0,8"], 54),
         ("book", "Dans quelle proportion des cas la biopsie rectale est-elle positive pour l'amylose ?",
          ["plus de 60 % des cas", "amyloïdes"], 34),
         ("book", "Quelle épaisseur de fenêtre utilise-t-on pour la tomodensitométrie haute résolution du thorax ?",
          ["1 à 1,5 mm"], 45),
     ]),
    ("livres/soins-infirmiers-extrait.pdf",
     "soins-infirmiers-techniques-generales-et-speciales-universite-de-kisangani-5105.pdf", 85, 187, [
         ("book", "Combien de liquide perd-on par évaporation sans s'en rendre compte en vingt-quatre heures ?",
          ["pertes insensibles", "800", "1000 cc"], 85),
         ("book", "Quelle quantité de solution verse-t-on dans l'entonnoir pour un lavage d'estomac ?",
          ["400 à 500 cc"], 108),
         ("book", "Quand recueille-t-on le sixième tube lors du tubage gastrique fractionné ?", ["75 minutes"], 111),
         ("book", "Quelle est la longueur de la sonde de Canton ?", ["3 m de long"], 113),
         ("book", "À quelle hauteur place-t-on l'irrigateur pour un lavement ?", ["30 - 40 cm"], 118),
     ]),
    ("livres/synthese-medecine-generale-extrait.pdf",
     "synthese-medecine-generale-jean-sebastien-lambert-6224.pdf", 85, 195, [
         ("book", "Quelle température ambiante maximale faut-il pour aider à faire baisser la fièvre d'un enfant ?",
          ["ambiante maximale de 20"], 103),
         ("book", "Combien de jours avant l'éruption l'enfant est-il déjà contagieux ?",
          ["5 jours avant le début"], 107),
         ("book", "Quelle part des patients décède dans le premier mois ?", ["29 % dans le premier mois"], 122),
         ("book", "Quand le risque familial est-il considéré comme normal ?", ["aucun membre atteint"], 136),
         ("book", "Depuis quand les femmes de quarante à quarante-neuf ans ne sont-elles plus convoquées au dépistage ?",
          ["Depuis 2015"], 137),
     ]),
    ("livres/immunologie-extrait.pdf",
     "cours-d-immunologie-fondamentale-pr-balaka-ekwalanga-6574.pdf", 121, 265, [
         ("book", "En quelle année a-t-on découvert le système du complément ?", ["découvert en 1890"], 157),
         ("book", "Combien de composants compte le complément ?", ["9 composants"], 158),
         ("book", "Quelles sont les deux sortes de chaînes légères des immunoglobulines ?",
          ["2 sortes de chaînes L"], 187),
         ("book", "Combien de domaines constants porte la chaîne lourde gamma ?", ["3 domaines constants"], 192),
         ("book", "Combien de types d'autophagie existe-t-il ?", ["3 types d autophagie"], 146),
     ]),
    ("livres/300-diagnostics-extrait.pdf",
     "300-diagnostics-en-pratique-medicale-courante-ibrahim-marroun-paul-legendre-6227.pdf", 421, 600, [
         ("book", "À partir de quel taux d'amylase évoque-t-on une pancréatite aiguë ?",
          ["amylasémie", "3 fois la normale"], 421),
         ("book", "Pendant combien de temps dure la quadrithérapie bismuthée ?", ["pendant 14 jours"], 437),
         ("book", "Quelle proportion des sarcoïdoses guérissent spontanément ?",
          ["80 %", "sarcoïdoses régressent"], 488),
         ("book", "Combien de fausses couches spontanées consécutives précoces sont retenues comme critère ?",
          ["3 fausses couches spontanées consécutives"], 522),
     ]),
]

# The two books of the small set, read further in the big set (the first page, hence every page number, is unchanged).
WIDER_LAST_PAGE = {"anatomie-et-physiologie-humaines-marcel-pirard-4717.pdf": 170,
                   "microbiologie-medicale-dr-ayad-wissem-4721.pdf": 115}

BOOK_ABSENT_BIG = [
    "Quelle est la dose de warfarine recommandée chez l'adulte ?",
    "Comment prévenir la poliomyélite chez l'enfant ?",
    "Quelle est la durée d'une séance de radiothérapie ?",
    "Quel est le débit de dialyse chez un adulte ?",
]

FIRST_F = ["Alice", "Amélie", "Anaïs", "Béatrice", "Camille", "Carole", "Céline", "Chantal", "Claire", "Corinne",
           "Delphine", "Dominique", "Elisabeth", "Emilie", "Fabienne", "Florence", "Francine", "Gabrielle", "Hélène",
           "Isabelle", "Jacqueline", "Josiane", "Julie", "Laure", "Lucie", "Margaux", "Martine", "Muriel", "Nadine",
           "Océane", "Pascale", "Patricia", "Raphaëlle", "Sabine", "Simone", "Solange", "Sylvie", "Thérèse",
           "Valentine", "Véronique"]
FIRST_M = ["Adrien", "Alain", "André", "Arnaud", "Bastien", "Bernard", "Cédric", "Christian", "Claude", "Daniel",
           "Denis", "Didier", "Edouard", "Eric", "Fabrice", "Frédéric", "Gaétan", "Gérard", "Guillaume", "Henri",
           "Jacques", "Jérôme", "Julien", "Laurent", "Léon", "Lucas", "Marcel", "Mathieu", "Maurice", "Nicolas",
           "Olivier", "Patrick", "Philippe", "Raymond", "Régis", "Sébastien", "Stéphane", "Thierry", "Vincent",
           "Xavier"]
LAST = ["Aeby", "Amiguet", "Bachmann", "Baud", "Beguin", "Berthoud", "Bessard", "Blanc", "Bochud", "Bonjour",
        "Borel", "Bourquin", "Burnier", "Cachin", "Chappuis", "Chevalley", "Cherix", "Clerc", "Cornaz", "Cuendet",
        "Dedominici", "Delacrétaz", "Demierre", "Deriaz", "Duruz", "Favre", "Fivaz", "Gaudard", "Genoud", "Gillieron",
        "Golay", "Grandjean", "Gremaud", "Guignard", "Haenni", "Jaccard", "Jordan", "Karlen", "Krebs", "Lador",
        "Lavanchy", "Leuba", "Magnin", "Maillard", "Margot", "Mayor", "Menoud", "Mermoud", "Monnard", "Monod",
        "Nicod", "Oberson", "Pasche", "Perret", "Pidoux", "Pittet", "Progin", "Quartier", "Reymond", "Rieder",
        "Rod", "Rossier", "Salamin", "Savary", "Siegrist", "Stucki", "Tinguely", "Tornare", "Udry", "Vaucher",
        "Vernez", "Viret", "Volet", "Voruz", "Wuthrich", "Zbinden", "Zurcher", "Arnold", "Brandt", "Cottier"]
TEAMS = ["des soins", "de l'accueil", "de la pharmacie", "du laboratoire", "de la radiologie", "de la comptabilité",
         "du secrétariat", "de la logistique", "de la maintenance", "de la formation", "de la pédiatrie",
         "de la cardiologie", "de la gériatrie", "de la maternité", "des urgences", "de la dermatologie",
         "de la neurologie", "de l'oncologie", "de la psychiatrie", "de la rééducation"]
SITES = ["du site nord", "du site sud", "du site est", "du site ouest"]
DAYS = ["lundi", "mardi", "mercredi", "jeudi", "vendredi"]
MONTHS = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre"]
VACCINES = ["contre le tétanos", "contre la diphtérie", "contre l'hépatite B", "contre la grippe", "contre le zona"]


def ascii_slug(text: str) -> str:
    folded = "".join(c for c in unicodedata.normalize("NFD", text) if not 0x300 <= ord(c) <= 0x36F)
    return "".join(c if c.isalnum() else "-" for c in folded.lower()).strip("-")


def ean13(body12: str) -> str:
    total = sum(int(d) * (3 if i % 2 else 1) for i, d in enumerate(body12))
    return body12 + str((10 - total % 10) % 10)


def avs_number(rng: random.Random) -> tuple[str, str]:
    digits = ean13("756" + "".join(str(rng.randint(0, 9)) for _ in range(9)))
    return digits, f"{digits[:3]}.{digits[3:7]}.{digits[7:11]}.{digits[11:]}"


class Bulk:
    """Yields `(path, fmt, title, paragraphs, [(kind, question, [facts])])` tuples, one document each."""

    def __init__(self, seed: int, taken_names: set[str]) -> None:
        self.rng = random.Random(seed)
        self.names = set(taken_names)
        self.numbers = set()
        self.invoice = 1000
        self.count = 0

    def person(self) -> tuple[str, str]:
        while True:
            female = self.rng.random() < 0.5
            first = self.rng.choice(FIRST_F if female else FIRST_M)
            name = f"{first} {self.rng.choice(LAST)}"
            if name not in self.names:
                self.names.add(name)
                return name, "Madame" if female else "Monsieur"

    def amount(self, low: int, high: int) -> str:
        while True:
            value = f"{self.rng.randint(low, high)}.{self.rng.randint(0, 99):02d}"
            if value not in self.numbers:
                self.numbers.add(value)
                return value

    def date(self) -> tuple[str, str]:
        month = self.rng.randint(1, 10)
        day = self.rng.randint(1, 28)
        return f"2026-{month:02d}-{day:02d}", f"{day} {MONTHS[month - 1]} 2026"

    def make(self, family: int, n: int) -> tuple:
        name, civility = self.person()
        iso, written = self.date()
        slug = ascii_slug(name)
        k = n // 10  # the rank inside the family, so that the choice does not follow the family's parity
        ask = k % 3 == 0  # about one document in three carries a question
        para = k % 2 == 0
        month = iso[:7]
        if family == 0:
            value = self.amount(80, 990)
            q = [("paraphrase" if para else "keyword",
                  f"Combien l'assurance a-t-elle remboursé à {name} ?" if para else
                  f"Quel montant la caisse a-t-elle pris en charge pour {name} ?", [value])] if ask else []
            return (f"courriers/{month}/{iso}_decompte-caisse-{slug}.pdf", "pdf", "Caisse maladie Helvia - Décompte", [
                f"{civility} {name}, rue des Alpes {self.rng.randint(1, 80)}, 1200 Genève.",
                f"Nous avons pris en charge un montant de CHF {value} pour vos soins ambulatoires du mois dernier, "
                "déduction faite de la franchise.",
                "Nous vous prions d'agréer nos salutations distinguées. Le service des prestations."], q)
        if family == 1:
            value = f"{self.rng.randint(35, 95) / 10:.1f}"
            q = [("paraphrase", f"Quel est le taux de sucre dans le sang de {name} à jeun ?", [f"{value} mmol/L"])] \
                if ask else []
            return (f"resultats/{month}/{iso}_glycemie-{slug}.txt", "txt", "Résultat d'analyse", [
                f"Patient : {name}. Prélèvement du {written}.",
                f"Glycémie à jeun : {value} mmol/L (valeurs de référence 3.9 à 6.1 mmol/L).",
                "Interprétation à discuter avec le médecin traitant."], q)
        if family == 2:
            value = self.rng.randint(45, 140)
            q = [("keyword", f"Quelle est la créatinine de {name} ?", [f"{value} µmol/L"])] if ask else []
            return (f"resultats/{month}/{iso}_creatinine-{slug}.docx", "docx", "Laboratoire Cantonal - Analyse de sang", [
                f"Patient : {name}. Prélèvement du {written}.",
                f"Créatinine : {value} µmol/L (valeurs de référence 60 à 110 µmol/L).",
                "Sodium et potassium dans la norme."], q)
        if family == 3:
            self.invoice += 1
            number = f"FAC-2026-{self.invoice:04d}"
            value = self.amount(100, 4000)
            q = []
            if ask:
                q = [("identifier", f"Quel est le total de la facture {number} ?" if para else
                      f"Quel est le montant de la facture 2026-{self.invoice:04d} ?", [f"CHF {value}"])]
            return (f"factures/{month}/{iso}_facture-{number.lower()}.pdf", "pdf", "Facture", [
                f"Facture n° {number} du {written}. Client : {name}.",
                f"Fourniture de prestations diverses. Total à payer : CHF {value}, paiement à trente jours."], q)
        if family == 4:
            team = f"{self.rng.choice(TEAMS)} {self.rng.choice(SITES)}"
            while team in self.numbers:
                team = f"{self.rng.choice(TEAMS)} {self.rng.choice(SITES)}"
            self.numbers.add(team)
            slot = f"{self.rng.choice(DAYS)} à {self.rng.randint(8, 17)}h{self.rng.choice(['00', '15', '30', '45'])}"
            q = [("paraphrase", f"Quand se réunit désormais l'équipe {team} ?", [slot])] if ask else []
            return (f"comptes-rendus/{month}/{iso}_reunion-{ascii_slug(team)}.txt", "txt",
                    f"Compte rendu de la réunion de l'équipe {team}", [
                        f"Présents : les membres de l'équipe {team}.",
                        f"Il a été décidé de déplacer la réunion d'équipe au {slot}, dans la salle commune.",
                        "Prochain point : les remplacements."], q)
        if family == 5:
            days = self.rng.randint(3, 28)
            q = [("paraphrase", f"Combien de temps {name} est-il en incapacité de travailler ?", [f"{days} jours"])] \
                if ask else []
            return (f"courriers/{month}/{iso}_arret-travail-{slug}.docx", "docx", "Certificat d'arrêt de travail", [
                f"{civility} {name}.",
                f"Je certifie que son état de santé justifie un arrêt de travail de {days} jours à compter du {written}.",
                "Reprise sous réserve d'un nouveau contrôle."], q)
        if family == 6:
            hour = f"{self.rng.randint(8, 17)}h{self.rng.choice(['00', '20', '40'])}"
            q = [("paraphrase", f"Quand est la consultation de {name} ?", [f"{written} à {hour}"])] if ask else []
            return (f"courriers/{month}/{iso}_rendez-vous-{slug}.pdf", "pdf", "Cabinet médical - Rendez-vous", [
                f"{civility} {name}, route de Berne {self.rng.randint(1, 60)}, 1700 Fribourg.",
                f"Je vous confirme votre rendez-vous du {written} à {hour} pour la consultation demandée."], q)
        if family == 7:
            value = self.amount(120, 900)
            q = [("paraphrase", f"Combien a coûté le trajet en ambulance de {name} ?", [f"CHF {value}"])] if ask else []
            return (f"factures/{month}/{iso}_transport-{slug}.pdf", "pdf", "Facture de transport sanitaire", [
                f"{civility} {name}, rue du Lac {self.rng.randint(1, 40)}, 1800 Vevey.",
                f"Le transport en ambulance du {written} a été facturé CHF {value}. Franchise non comprise."], q)
        if family == 8:
            raw, dotted = avs_number(self.rng)
            q = [("identifier", f"À qui appartient le numéro AVS {raw} ?", [name])] if ask else []
            return (f"patients/{month}/{iso}_dossier-{slug}.docx", "docx", "Dossier administratif", [
                f"Patient : {name}, né en {self.rng.randint(1935, 2005)}.",
                f"Numéro AVS : {dotted}. Caisse maladie : Helvia."], q)
        years = self.rng.randint(5, 10)
        vaccine = self.rng.choice(VACCINES)
        q = [("paraphrase", f"Quand {name} doit-il refaire son vaccin {vaccine} ?", [f"dans {years} ans"])] if ask else []
        return (f"courriers/{month}/{iso}_vaccination-{slug}.txt", "txt", "Carnet de vaccination", [
            f"{civility} {name}.",
            f"Le vaccin {vaccine} a été administré le {written}. Le rappel est prévu dans {years} ans."], q)

    def documents(self, total: int):
        for n in range(total):
            yield self.make(n % 10, n)
