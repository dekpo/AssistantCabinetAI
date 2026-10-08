"""Generate the fictional corpus for the Knowledge Base baseline measurement.

Standard library only, deterministic (the same output every run). Usage:

    python fixtures/kb-baseline/make_corpus.py <output-folder>

The output folder must not exist or must be empty. It receives:

    docs-10/   10 short letters
    docs-30/   the same 10, plus 20 more
    docs-50/   the same 30, plus 20 more
    data/      three semicolon-separated tables (invoices, appointments, clients)
    ground-truth.txt   what each question of the protocol should find (read it AFTER asking)

Everything is invented. No name, address or number belongs to a real person. The letters repeat a
letterhead and a signature on purpose: real letters do, and the measurement counts how many
chunks have byte-identical text across files. Keep `ground-truth.txt` out of any folder you
analyse.
"""

import random
import sys
from pathlib import Path

SEED = 20261008
TARGET = "Camille Vasseur"
# Letters (1-based, over the 50) that concern TARGET: two within the first 10, one more within 30,
# one more within 50. So the entity question has an answer at every size.
TARGET_LETTERS = (3, 8, 27, 44)

LETTERHEAD = (
    "Cabinet fictif Exemple, 12 rue des Lilas, 75000 Ville.\n"
    "Ouvert du lundi au vendredi, de 8 h 30 à 18 h 30. Téléphone : 01 00 00 00 00.\n"
)
SIGNATURE = (
    "\nCordialement,\nLe secrétariat du cabinet fictif Exemple.\n"
    "Document fictif, usage de test uniquement.\n"
)

PEOPLE = [
    "Alice Renard", "Bernard Lambert", "Chloé Marchand", "Damien Fournier", "Élise Perrot",
    "Florent Gaillard", "Gabrielle Mercier", "Hugo Delmas", "Inès Chevalier", "Julien Navarro",
    "Karine Lefort", "Louis Brunet", "Maëlle Roussel", "Nicolas Faure", "Océane Colin",
    "Paul Besnard", "Quentin Aubry", "Raphaëlle Simon", "Sébastien Dumas", "Thérèse Vidal",
    "Ulysse Garnier", "Valérie Morin", "William Blanc", "Yasmine Girard", "Zacharie Lemoine",
]

TOPICS = [
    ("cardiologie", "Compte rendu de consultation de cardiologie",
     "La consultation a porté sur le suivi d'une tension artérielle. Les chiffres relevés sont "
     "stables depuis le dernier contrôle. Un électrocardiogramme de repos a été réalisé et ne "
     "montre pas d'anomalie. Le traitement en cours est poursuivi sans changement."),
    ("biologie", "Résultats d'analyses biologiques",
     "Le bilan sanguin prélevé le matin à jeun est joint à ce courrier. L'hémoglobine glyquée est "
     "à 6,8 pour cent, la créatinine et les enzymes du foie sont dans les valeurs de référence. "
     "Un nouveau contrôle est proposé dans trois mois."),
    ("imagerie", "Compte rendu d'imagerie",
     "L'examen d'imagerie demandé a été réalisé sans incident. Les clichés ne montrent pas de "
     "lésion récente. Le compte rendu détaillé et les images sont à disposition au secrétariat "
     "sur simple demande."),
    ("administratif", "Courrier administratif",
     "Nous accusons réception de votre dossier. Il manque une pièce justificative récente pour "
     "le compléter. Merci de la transmettre au secrétariat avant la fin du mois afin que le "
     "traitement puisse reprendre."),
    ("rhumatologie", "Compte rendu de consultation de rhumatologie",
     "La personne se plaint de douleurs articulaires des mains, surtout le matin. L'examen "
     "clinique ne montre pas de gonflement. Des radiographies de contrôle sont prescrites et "
     "une nouvelle consultation est fixée dans six semaines."),
    ("convention", "Convention de remplacement",
     "La présente convention précise les dates du remplacement, les horaires assurés et la part "
     "des honoraires reversée au titulaire. Elle est établie en deux exemplaires et prend effet "
     "à la date de signature."),
]
# Two letters are a quote from the supplier MedSupply, so that a question mixing the documents and
# the invoices table has something to compare. They are in the first 10, hence in every folder.
QUOTE_LETTERS = (5, 9)
QUOTE = (
    "devis", "Devis MedSupply",
    "Le devis de la société MedSupply porte sur du matériel de bureau et des consommables. Le "
    "montant total est de 1 200,00 euros HT, en une seule somme. Les commandes complémentaires "
    "sont facturées à part.",
)


def letter(number: int, rng: random.Random) -> tuple[str, str]:
    """Return (file name, text) of letter `number` (1-based)."""
    person = TARGET if number in TARGET_LETTERS else rng.choice(PEOPLE)
    chosen = TOPICS[(number * 7 + rng.randrange(len(TOPICS))) % len(TOPICS)]
    topic, title, body = QUOTE if number in QUOTE_LETTERS else chosen
    day = (number * 3) % 28 + 1
    month = 1 + (number % 6)
    date = f"2026-{month:02d}-{day:02d}"
    extra = (
        f"Références du dossier : {topic.upper()}-{number:03d}. Le présent courrier concerne "
        f"{person}. Les éléments cités ci-dessus sont issus du dossier de {person} et ont été "
        "relus avant l'envoi. Aucune autre démarche n'est nécessaire de votre part à ce stade."
    )
    text = (
        LETTERHEAD
        + f"\nLe {date}\n\nObjet : {title} — {person}\n\n{body}\n\n{extra}\n"
        + SIGNATURE
    )
    return f"{date}_{topic}-{number:02d}.txt", text


def write_folder(folder: Path, letters: list[tuple[str, str]]) -> None:
    folder.mkdir(parents=True)
    for name, text in letters:
        (folder / name).write_text(text, encoding="utf-8")


def tables(root: Path, rng: random.Random) -> float:
    """Write the three tables; return the invoices total for MedSupply, for the ground truth."""
    root.mkdir(parents=True)
    suppliers = ["MedSupply", "Fournitures Dupont", "Papeterie Lefevre", "Lumière Services"]
    rows = ["date;fournisseur;montant"]
    medsupply_total = 0.0
    for index in range(40):
        day = index % 28 + 1
        amount = round(20 + rng.randrange(0, 60000) / 100, 2)
        if suppliers[index % 4] == "MedSupply":
            medsupply_total += amount
        rows.append(f"{day:02d}/03/2026;{suppliers[index % 4]};{amount:.2f}".replace(".", ","))
    (root / "factures-fournisseurs.csv").write_text("\n".join(rows) + "\n", encoding="utf-8")

    rooms = ["Salle 1", "Salle 2", "Salle 3"]
    rows = ["date;salle;personne;duree_min"]
    for index in range(30):
        day = index % 28 + 1
        rows.append(
            f"{day:02d}/03/2026;{rooms[index % 3]};{rng.choice(PEOPLE)};{15 + 5 * rng.randrange(0, 8)}"
        )
    (root / "rendez-vous-mars.csv").write_text("\n".join(rows) + "\n", encoding="utf-8")

    cities = ["Lyon", "Nantes", "Lille", "Dijon", "Brest"]
    rows = ["nom;ville;statut"]
    for index, person in enumerate(PEOPLE + [TARGET]):
        rows.append(f"{person};{cities[index % 5]};{'actif' if index % 4 else 'ancien'}")
    (root / "clients.csv").write_text("\n".join(rows) + "\n", encoding="utf-8")
    return round(medsupply_total, 2)


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    out = Path(sys.argv[1])
    if out.exists() and any(out.iterdir()):
        sys.exit(f"{out} is not empty: choose a new folder")
    rng = random.Random(SEED)
    letters = [letter(number, rng) for number in range(1, 51)]
    write_folder(out / "docs-10", letters[:10])
    write_folder(out / "docs-30", letters[:30])
    write_folder(out / "docs-50", letters)
    medsupply_total = tables(out / "data", rng)

    def holding(size: int) -> list[str]:
        return [name for name, text in letters[:size] if TARGET in text]

    truth = [
        "GROUND TRUTH - read it after asking, and keep this file out of any analysed folder.",
        "",
        f"Entity question, about {TARGET}:",
        *[f"  docs-{size}: {len(holding(size))} letters ({', '.join(holding(size))})" for size in (10, 30, 50)],
        "",
        "Entity-free question: every letter carries the same letterhead (opening hours Monday to",
        "Friday, 8:30 to 18:30; address 12 rue des Lilas), so the answer is the same in all of them.",
        "",
        "Each-document question: one sentence per letter; with 30 or 50 letters the answer",
        "cannot cover them all (the character budget runs out), and the line under the answer",
        "should say how many letters it rests on.",
        "",
        "Data question: factures-fournisseurs.csv has 40 rows over four suppliers (MedSupply,",
        "Fournitures Dupont, Papeterie Lefevre, Lumiere Services), 10 rows each.",
        "",
        "Mixed question: the invoices total for MedSupply is "
        + f"{medsupply_total:.2f}".replace(".", ",")
        + " euros; the two MedSupply quotes in the letters (2026-*_devis-05.txt and -09.txt)",
        "state 1 200,00 euros HT, in one sum. The two figures differ.",
    ]
    (out / "ground-truth.txt").write_text("\n".join(truth) + "\n", encoding="utf-8")
    print(f"Wrote {out} (docs-10, docs-30, docs-50, data, ground-truth.txt)")


if __name__ == "__main__":
    main()
