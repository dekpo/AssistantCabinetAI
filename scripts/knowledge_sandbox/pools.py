"""Name, place and organisation pools, and valid-but-fictional identifier makers.

Everything here is invented or generic. No person is real. The organisations are of two kinds, as the
owner asked: real public bodies and companies named as organisations (hospitals, insurers, laboratories),
which is what a real library holds, and fictional ones (suppliers, firms). No real person is named.

Order matters: every list is iterated in this order, never as a set, so two runs give the same corpus.
"""

from __future__ import annotations

import random

# --------------------------------------------------------------------------------------------- given names

GIVEN_F_FR = [
    "Sophie", "Isabelle", "Nathalie", "Catherine", "Sylvie", "Valérie", "Christine", "Martine", "Françoise",
    "Monique", "Brigitte", "Florence", "Corinne", "Béatrice", "Stéphanie", "Céline", "Aurélie", "Émilie",
    "Camille", "Claire", "Julie", "Marion", "Laura", "Élodie", "Hélène", "Anne", "Agnès", "Laurence",
    "Véronique", "Dominique", "Marie-Claire", "Anne-Sophie", "Marie-Laure", "Charlotte", "Léa", "Manon",
    "Inès", "Océane", "Maëlle", "Gaëlle", "Noémie", "Zoé", "Louise", "Margaux", "Pauline", "Justine",
]
GIVEN_M_FR = [
    "Jean", "Pierre", "Michel", "Philippe", "Alain", "Patrick", "Daniel", "Bernard", "Christian", "Gérard",
    "Thierry", "Laurent", "Olivier", "Nicolas", "Stéphane", "Éric", "David", "Frédéric", "Pascal", "Sébastien",
    "Julien", "Maxime", "Thomas", "Antoine", "Alexandre", "Romain", "Guillaume", "Benoît", "Jérôme", "Yves",
    "Jean-Marc", "Jean-François", "Jean-Luc", "Pierre-Yves", "François", "Raphaël", "Étienne", "Hugo", "Lucas",
    "Mathieu", "Rémi", "Clément", "Vincent", "Gilles", "Didier", "Bruno", "Xavier", "Damien",
]
GIVEN_F_CH = [
    "Fabienne", "Christelle", "Marie-Claude", "Anne-Laure", "Séverine", "Murielle", "Nadine", "Sandra",
    "Carole", "Valentine", "Joëlle", "Katia", "Mélanie", "Rachel", "Sabrina", "Estelle", "Aline", "Ursula",
    "Yvonne", "Rosemarie", "Jacqueline", "Monique", "Danielle", "Chantal", "Laetitia",
]
GIVEN_M_CH = [
    "Grégoire", "Cédric", "Yann", "Sylvain", "Jean-Claude", "Pierre-Alain", "Daniel", "Marc", "Roland",
    "Kilian", "Loïc", "Fabrice", "Alexandre", "Raphaël", "Jérémie", "Nils", "Beat", "Urs", "Hans-Peter",
    "Christophe", "Dominique", "André", "Claude", "Jean-Daniel", "Jean-Pascal", "Blaise", "Gaël",
]
# Given names with an accent, an apostrophe or a particle, and foreign ones: the "names we are certain to
# meet" of a cross-border, multilingual practice.
GIVEN_F_OTHER = [
    "Fatima", "Aïcha", "Thi Lan", "Maria", "Elena", "Anna", "Zoë", "Åsa", "Leïla", "Amina", "Yasmine",
    "Ioana", "Mirjam", "Barbara", "Lucia", "Giulia", "Sara", "Nour", "Hana", "Elif",
]
GIVEN_M_OTHER = [
    "Mohamed", "Andrzej", "Ibrahim", "Giuseppe", "Sean", "Jörg", "Luca", "Dragan", "Karim", "Youssef",
    "Mehmet", "Mario", "António", "Rui", "Tomasz", "Viktor", "Florin", "Arben", "Besnik", "Samir",
]

# ------------------------------------------------------------------------------------------------ surnames

SURNAMES_FR = [
    "Bernard", "Dubois", "Thomas", "Robert", "Richard", "Petit", "Durand", "Leroy", "Moreau", "Simon",
    "Laurent", "Lefebvre", "Michel", "Garcia", "David", "Bertrand", "Roux", "Vincent", "Fournier", "Morel",
    "Girard", "André", "Lefèvre", "Mercier", "Dupuis", "Lambert", "Bonnet", "François", "Legrand", "Garnier",
    "Faure", "Rousseau", "Blanc", "Guerin", "Muller", "Henry", "Roussel", "Nicolas", "Perrin", "Morin",
    "Mathieu", "Clément", "Gauthier", "Dumont", "Lopez", "Fontaine", "Chevalier", "Robin", "Masson", "Gérard",
    "Boyer", "Denis", "Lemaire", "Duval", "Joly", "Gautier", "Roger", "Roche", "Roy", "Noël", "Meyer",
    "Lucas", "Meunier", "Perez", "Marchand", "Dufour", "Blanchard", "Barbier", "Brun", "Dumas", "Brunet",
    "Schmitt", "Leroux", "Colin", "Renard", "Arnaud", "Rolland", "Caron", "Aubert", "Giraud", "Leclerc",
    "Vidal", "Bourgeois", "Renaud", "Lemoine", "Picard", "Gaillard", "Lacroix", "Fabre", "Collet",
    "Deschamps", "Bailly", "Hervé", "Charpentier", "Langlois", "Guillot", "Berger", "Guillaume", "Jacob",
    "Perret", "Poirier", "Besson", "Pelletier", "Lebrun", "Humbert", "Dupuy", "Monnier", "Rey", "Maillard",
]
# Surnames of Haute-Savoie, Ain and the Jura foothills.
SURNAMES_SAVOIE = [
    "Dunand", "Duret", "Burnier", "Chappaz", "Mugnier", "Gaydon", "Pellet", "Vuagnoux", "Excoffier", "Million",
    "Favre-Bulle", "Ducrettet", "Bouvier", "Blondet", "Carrier", "Vulliet", "Cuvelier", "Ract-Madoux",
]
# Surnames of French-speaking Switzerland (Vaud, Genève, Fribourg, Valais, Neuchâtel, Jura).
SURNAMES_ROMAND = [
    "Rochat", "Cuendet", "Pittet", "Chappuis", "Favre", "Bovet", "Jaccard", "Girardin", "Zufferey", "Délèze",
    "Crettenand", "Vuilleumier", "Python", "Oberson", "Genoud", "Tschopp", "Bersier", "Vuagniaux", "Cuénoud",
    "Grandjean", "Mottier", "Rapin", "Piaget", "Cuche", "Challandes", "Gigon", "Progin", "Cottet", "Pillet",
    "Mudry", "Dubuis", "Bonvin", "Fellay", "Rappaz", "Berthoud", "Ducommun", "Jeanneret", "Matthey",
    "Perrenoud", "Huguenin", "Humair", "Voirol", "Cattin", "Queloz", "Juillerat", "Chételat", "Monnerat",
    "Comte", "Girod", "Meylan", "Cherix", "Gavillet", "Duc", "Bochud", "Currat", "Overney", "Sudan", "Savoy",
    "Dafflon", "Menoud", "Gremaud", "Mauron", "Rime", "Pasquier", "Dévaud", "Ruchat", "Mermoud", "Pernet",
    "Jaquier", "Cherpillod", "Bugnon", "Besençon", "Apothéloz", "Lavanchy", "Cretegny", "Chevalley",
]
# German-Swiss, Italian-Swiss and foreign names a romand or cross-border practice meets every day.
SURNAMES_OTHER = [
    "Müller", "Schneider", "Weber", "Meier", "Keller", "Brunner", "Baumann", "Gerber", "Zimmermann", "Lüthi",
    "Bernasconi", "Rossi", "Ferrari", "Bianchi", "Colombo", "Nguyen", "Tran", "Benali", "El Khoury",
    "Haddad", "Ben Salah", "Kowalski", "Nowak", "Dos Santos Ferreira", "Silva", "Pereira", "Costa",
    "Fernandes", "Gomes", "Demirci", "Yilmaz", "Kaya", "Petrović", "Hasanović", "Ibrahimi", "Krasniqi",
    "Berisha", "Hoxha", "Popescu", "Ionescu", "D'Amico", "N'Diaye", "O'Brien", "Le Gall", "de la Fontaine",
    "d'Aubigné", "Lindqvist", "Van den Berg", "De Smet", "Ben Ammar", "Diallo", "Traoré", "Camara",
]
# Surnames that are also common given names or common words: the ones a name finder confuses.
SURNAMES_TRAPS = ["Roy", "Lucas", "Thomas", "Pierre", "Jean", "Marie", "Robin", "Noël", "Martin", "Bernard", "Colin"]

TITLES_DOCTOR_FR = ["Dr", "Docteur"]

# ------------------------------------------------------------------------------------------------- places

# (commune, postal code, canton or department, country)
COMMUNES_FR = [
    ("Annemasse", "74100", "Haute-Savoie"), ("Annecy", "74000", "Haute-Savoie"),
    ("Thonon-les-Bains", "74200", "Haute-Savoie"), ("Saint-Julien-en-Genevois", "74160", "Haute-Savoie"),
    ("Cluses", "74300", "Haute-Savoie"), ("Bonneville", "74130", "Haute-Savoie"),
    ("Évian-les-Bains", "74500", "Haute-Savoie"), ("Sallanches", "74700", "Haute-Savoie"),
    ("Chamonix-Mont-Blanc", "74400", "Haute-Savoie"), ("Seynod", "74600", "Haute-Savoie"),
    ("Gaillard", "74240", "Haute-Savoie"), ("Ville-la-Grand", "74100", "Haute-Savoie"),
    ("Ferney-Voltaire", "01210", "Ain"), ("Gex", "01170", "Ain"), ("Divonne-les-Bains", "01220", "Ain"),
    ("Bourg-en-Bresse", "01000", "Ain"), ("Oyonnax", "01100", "Ain"), ("Lyon", "69003", "Rhône"),
    ("Villeurbanne", "69100", "Rhône"), ("Grenoble", "38000", "Isère"), ("Chambéry", "73000", "Savoie"),
    ("Besançon", "25000", "Doubs"), ("Pontarlier", "25300", "Doubs"),
]
COMMUNES_CH = [
    ("Lausanne", "1003", "VD"), ("Genève", "1201", "GE"), ("Nyon", "1260", "VD"), ("Morges", "1110", "VD"),
    ("Vevey", "1800", "VD"), ("Montreux", "1820", "VD"), ("Yverdon-les-Bains", "1400", "VD"),
    ("Neuchâtel", "2000", "NE"), ("La Chaux-de-Fonds", "2300", "NE"), ("Le Locle", "2400", "NE"),
    ("Fribourg", "1700", "FR"), ("Bulle", "1630", "FR"), ("Sion", "1950", "VS"), ("Sierre", "3960", "VS"),
    ("Martigny", "1920", "VS"), ("Monthey", "1870", "VS"), ("Delémont", "2800", "JU"), ("Porrentruy", "2900", "JU"),
    ("Carouge", "1227", "GE"), ("Lancy", "1212", "GE"), ("Meyrin", "1217", "GE"), ("Vernier", "1214", "GE"),
    ("Renens", "1020", "VD"), ("Pully", "1009", "VD"), ("Lutry", "1095", "VD"), ("Prilly", "1008", "VD"),
    ("Aigle", "1860", "VD"), ("Payerne", "1530", "VD"), ("Rolle", "1180", "VD"), ("Gland", "1196", "VD"),
    ("Coppet", "1296", "VD"), ("Saint-Imier", "2610", "BE"), ("Moutier", "2740", "BE"),
    ("Estavayer-le-Lac", "1470", "FR"), ("Châtel-Saint-Denis", "1618", "FR"), ("Plan-les-Ouates", "1228", "GE"),
]
STREETS_FR = [
    "rue de la République", "avenue de la Gare", "rue du Léman", "boulevard du Docteur Bouchet", "place de l'Hôtel de Ville",
    "chemin des Vignes", "avenue des Alpes", "rue du Mont-Blanc", "route de Genève", "rue des Écoles",
    "impasse des Lilas", "rue Pasteur", "avenue de Verdun", "rue Jean-Jaurès", "quai du Paquier",
]
STREETS_CH = [
    "Avenue de la Gare", "Rue du Rhône", "Chemin des Vignes", "Route de Lausanne", "Place de la Palud",
    "Rue de Carouge", "Avenue de Cour", "Chemin de la Source", "Rue du Lac", "Route de Chancy",
    "Rue de l'Hôpital", "Chemin du Bois-Gentil", "Avenue de Morges", "Rue de Bourg", "Grand-Rue",
]

# ------------------------------------------------------------------------------------------ organisations

# (key, canonical, short, country, kind, sector)
# `short` is an acronym or a usual short form, or "" when there is none.
ORGANISATIONS = [
    ("hug", "Hôpitaux universitaires de Genève", "HUG", "CH", "hospital", "health"),
    ("chuv", "Centre hospitalier universitaire vaudois", "CHUV", "CH", "hospital", "health"),
    ("clinique-source", "Clinique La Source", "", "CH", "hospital", "health"),
    ("hopital-nyon", "Hôpital de Nyon", "", "CH", "hospital", "health"),
    ("hfr", "Hôpital fribourgeois", "HFR", "CH", "hospital", "health"),
    ("rhne", "Réseau hospitalier neuchâtelois", "RHNe", "CH", "hospital", "health"),
    ("hopital-valais", "Hôpital du Valais", "", "CH", "hospital", "health"),
    ("chu-grenoble", "CHU Grenoble Alpes", "", "FR", "hospital", "health"),
    ("hcl", "Hospices Civils de Lyon", "HCL", "FR", "hospital", "health"),
    ("change", "Centre hospitalier Annecy Genevois", "CHANGE", "FR", "hospital", "health"),
    ("clinique-annecy", "Clinique Générale d'Annecy", "", "FR", "hospital", "health"),
    ("hpps", "Hôpital Privé Pays de Savoie", "", "FR", "hospital", "health"),
    ("cpam74", "CPAM de la Haute-Savoie", "CPAM 74", "FR", "insurer", "health"),
    ("cpam69", "CPAM du Rhône", "", "FR", "insurer", "health"),
    ("mgen", "MGEN", "", "FR", "insurer", "health"),
    ("harmonie", "Harmonie Mutuelle", "", "FR", "insurer", "health"),
    ("helsana", "Helsana Assurances SA", "Helsana", "CH", "insurer", "health"),
    ("css", "CSS Assurance", "CSS", "CH", "insurer", "health"),
    ("groupe-mutuel", "Groupe Mutuel", "", "CH", "insurer", "health"),
    ("sanitas", "Sanitas Assurance maladie", "Sanitas", "CH", "insurer", "health"),
    ("swica", "SWICA Assurance maladie", "SWICA", "CH", "insurer", "health"),
    ("assura", "Assura-Basis SA", "Assura", "CH", "insurer", "health"),
    ("oai-vd", "Office de l'assurance-invalidité du canton de Vaud", "OAI", "CH", "admin", "social"),
    ("ccvd", "Caisse cantonale vaudoise de compensation AVS", "CCVD", "CH", "admin", "social"),
    ("suva", "SUVA", "", "CH", "insurer", "health"),
    ("urssaf", "URSSAF Rhône-Alpes", "URSSAF", "FR", "admin", "social"),
    ("ars", "ARS Auvergne-Rhône-Alpes", "ARS", "FR", "admin", "health"),
    ("ordre-74", "Ordre des médecins de la Haute-Savoie", "", "FR", "admin", "health"),
    ("fmh", "Fédération des médecins suisses", "FMH", "CH", "admin", "health"),
    ("svm", "Société vaudoise de médecine", "SVM", "CH", "admin", "health"),
    ("unilabs", "Unilabs Genève", "Unilabs", "CH", "lab", "health"),
    ("viollier", "Laboratoire Viollier", "Viollier", "CH", "lab", "health"),
    ("cerballiance", "Cerballiance Alpes", "", "FR", "lab", "health"),
    ("synlab", "Laboratoire Synlab Rhône-Alpes", "Synlab", "FR", "lab", "health"),
    ("pharmacie-dupont", "Pharmacie Dupont", "", "FR", "pharmacy", "health"),
    ("pharmacie-lac", "Pharmacie du Lac SA", "", "CH", "pharmacy", "health"),
    ("medsupply", "MedSupply SA", "MedSupply", "CH", "supplier", "supply"),
    ("papeterie-lac", "Papeterie du Lac SARL", "", "FR", "supplier", "supply"),
    ("bureautique-leman", "Bureautique Léman SA", "", "CH", "supplier", "supply"),
    ("pharma-depot", "Pharma-Dépôt Romandie SA", "", "CH", "supplier", "supply"),
    ("imprimerie-chablais", "Imprimerie du Chablais SAS", "", "FR", "supplier", "supply"),
    ("garage-martin", "Garage Martin & Fils SARL", "", "FR", "supplier", "supply"),
    ("fiduciaire-rochat", "Fiduciaire Rochat SA", "", "CH", "supplier", "accounting"),
    ("etude-chappuis", "Étude Chappuis & Associés", "", "CH", "supplier", "legal"),
    ("sig", "Services industriels de Genève", "SIG", "CH", "supplier", "utilities"),
    ("edf", "EDF", "", "FR", "supplier", "utilities"),
    ("swisscom", "Swisscom (Suisse) SA", "Swisscom", "CH", "supplier", "utilities"),
    ("orange", "Orange SA", "", "FR", "supplier", "utilities"),
    ("dupont-immobilier", "Dupont Immobilier SA", "", "CH", "supplier", "property"),
    ("clinique-alpes", "Clinique des Alpes SA", "", "FR", "hospital", "health"),
    ("cabinet-voirons", "Cabinet médical des Voirons", "", "FR", "self", "health"),
]
ORG_BY_KEY = {row[0]: row for row in ORGANISATIONS}

# Micro-organisms and findings of laboratory reports: terms, not names. A question about one has to be
# answered from the whole selection, because the base reads no such term as an entity.
ORGANISMS = [
    "Escherichia coli", "Staphylococcus aureus", "Streptococcus pyogenes", "Klebsiella pneumoniae",
    "Pseudomonas aeruginosa", "Enterococcus faecalis", "Candida albicans", "Helicobacter pylori",
    "Haemophilus influenzae", "Streptococcus pneumoniae", "SARS-CoV-2", "virus influenza A",
]
MEDICATIONS = [
    "metformine", "ramipril", "atorvastatine", "paracétamol", "lévothyroxine", "amlodipine", "oméprazole",
    "bisoprolol", "sertraline", "ibuprofène", "amoxicilline", "salbutamol", "furosémide", "warfarine",
]

# ------------------------------------------------------------------------------ valid but fictional numbers


def nir(rng: random.Random, sex: str, year: int, month: int, department: str) -> str:
    """A 15-digit French social security number with its real control key, for an invented person."""
    commune = rng.randint(1, 450)
    order = rng.randint(1, 999)
    base = f"{1 if sex == 'M' else 2}{year % 100:02d}{month:02d}{department}{commune:03d}{order:03d}"
    key = 97 - (int(base) % 97)
    return f"{base[0]} {base[1:3]} {base[3:5]} {base[5:7]} {base[7:10]} {base[10:13]} {key:02d}"


def avs(rng: random.Random) -> str:
    """A Swiss AVS / AHV number 756.xxxx.xxxx.xC with its EAN-13 check digit, for an invented person."""
    digits = "756" + "".join(str(rng.randint(0, 9)) for _ in range(9))
    total = sum(int(d) * (1 if i % 2 == 0 else 3) for i, d in enumerate(digits))
    check = (10 - total % 10) % 10
    full = digits + str(check)
    return f"{full[0:3]}.{full[3:7]}.{full[7:11]}.{full[11:13]}"


def _mod97(text: str) -> int:
    value = ""
    for char in text:
        value += str(int(char, 36))
    return int(value) % 97


def iban_fr(rng: random.Random) -> str:
    bank = f"{rng.randint(10000, 30999)}"
    branch = f"{rng.randint(10000, 99999)}"
    account = "".join(str(rng.randint(0, 9)) for _ in range(11))
    key = 97 - (int(bank + branch + account + "00") % 97)
    bban = f"{bank}{branch}{account}{key:02d}"
    check = 98 - _mod97(bban + "FR00")
    raw = f"FR{check:02d}{bban}"
    return " ".join(raw[i:i + 4] for i in range(0, len(raw), 4))


def iban_ch(rng: random.Random) -> str:
    bank = f"{rng.randint(100, 899):05d}"
    account = "".join(str(rng.randint(0, 9)) for _ in range(12))
    bban = bank + account
    check = 98 - _mod97(bban + "CH00")
    raw = f"CH{check:02d}{bban}"
    return " ".join(raw[i:i + 4] for i in range(0, len(raw), 4))


def phone_fr(rng: random.Random) -> str:
    """The numbers the French regulator reserves for fiction: 01 99 00 xx xx and 06 39 98 xx xx."""
    if rng.random() < 0.5:
        return f"01 99 00 {rng.randint(0, 99):02d} {rng.randint(0, 99):02d}"
    return f"06 39 98 {rng.randint(0, 99):02d} {rng.randint(0, 99):02d}"


def phone_ch(rng: random.Random) -> str:
    """Swiss style, on the conventional fictional exchange 555."""
    area = rng.choice(["021", "022", "024", "026", "027", "032"])
    return f"{area} 555 {rng.randint(0, 99):02d} {rng.randint(0, 99):02d}"


def mobile_ch(rng: random.Random) -> str:
    return f"+41 79 555 {rng.randint(0, 99):02d} {rng.randint(0, 99):02d}"
