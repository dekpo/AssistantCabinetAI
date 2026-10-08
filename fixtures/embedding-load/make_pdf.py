"""Builds `rapport-fictif-42-pages.pdf`: invented French-looking prose with a real text layer.

Fictional on purpose (AGENTS.md: no real document). Deterministic: the same seed gives the same
file, so a measurement can be repeated. Needs `reportlab` (`pip install reportlab`).
"""

import random
from pathlib import Path

from reportlab.lib.pagesizes import A4
from reportlab.pdfgen import canvas

PAGES = 42
SEED = 20261007
#: A sentence that exists nowhere else, so a log can be searched for it (it must never be found).
MARKER = "Le heron cobalt traverse la mediatheque de Saint-Quasimodo a minuit."

SUBJECTS = ["La commission", "Le comite de lecture", "Madame Verdier", "Le service technique",
            "L'association des voisins", "Monsieur Lacombe", "Le bureau regional", "L'atelier municipal"]
VERBS = ["examine", "reporte", "confirme", "prepare", "conteste", "archive", "relit", "valide"]
OBJECTS = ["le calendrier des reunions", "la liste des fournitures", "le plan de la salle commune",
           "le budget previsionnel de l'exercice", "la convention de partage des locaux",
           "le registre des interventions", "le compte rendu de la derniere seance",
           "la proposition d'horaires pour le printemps"]
TAILS = ["avant la fin du mois", "sans changement de fond", "apres un dernier echange par courrier",
         "en tenant compte des remarques recues", "conformement aux usages de la maison",
         "dans l'attente d'une reponse ecrite", "pour la prochaine assemblee"]


def sentence(rng: random.Random) -> str:
    return f"{rng.choice(SUBJECTS)} {rng.choice(VERBS)} {rng.choice(OBJECTS)} {rng.choice(TAILS)}."


def main() -> None:
    rng = random.Random(SEED)
    target = Path(__file__).with_name("rapport-fictif-42-pages.pdf")
    pdf = canvas.Canvas(str(target), pagesize=A4)
    pdf.setTitle("Rapport fictif de 42 pages")
    for page in range(1, PAGES + 1):
        pdf.setFont("Helvetica-Bold", 12)
        pdf.drawString(50, 800, f"Section {page} - note de service fictive")
        pdf.setFont("Helvetica", 8)
        y = 780
        sentences = [sentence(rng) for _ in range(30)]
        if page == 7:
            sentences.insert(20, MARKER)
        line = ""
        for item in sentences:
            if len(line) + len(item) + 1 > 120:
                pdf.drawString(50, y, line)
                y -= 9.5
                line = item
                if y < 40:
                    break
            else:
                line = f"{line} {item}".strip()
        pdf.showPage()
    pdf.save()


if __name__ == "__main__":
    main()
