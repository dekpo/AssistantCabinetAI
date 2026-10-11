"""Measure the score a reduced search reaches when it holds the answer and when it does not.

Written for lot 6 of the Knowledge Base programme (`lots/lot-06-targeted-scope.md`, "Measured"), to
choose `knowledge_widen_below_score`. It talks to the local gateway only (`/v1/embeddings`, the
embedding alias the gateway publishes), with **invented** passages and questions: no real document.

What the product does: a passage scores `cosine(question, passage)`, plus 0.2 when the full-text
index also matches it (`retrieval.rs`). When the knowledge base reduced the search to the files
that name an entity, the reduced search is repeated on the whole selection if its best score is
below the threshold. The threshold must therefore sit above the best score a reduced set reaches
when the answer is NOT in it, and below the score it reaches when the answer IS in it.

Run from the repository root (the gateway must be up; set GATEWAY to change the address):

    python docs/test-reports/knowledge-base-pass-1/lots/lot-06-widen-calibration.py
"""

import json
import math
import os
import sys
import urllib.request

GATEWAY = os.environ.get("GATEWAY", "http://127.0.0.1:8080")
ALIAS = os.environ.get("EMBEDDING_ALIAS", "")

# Invented passages, in the language of the pilot. Each belongs to a file.
PASSAGES = {
    "dupont-bilan": [
        "Docteur Jean Dupont recommande un bilan sanguin complet pour le suivi annuel de la patiente, "
        "avec dosage du cholesterol, de la glycemie a jeun et de la creatinine.",
        "Le Docteur Jean Dupont recoit le lundi et le jeudi de 9 heures a 12 heures, sur rendez-vous. "
        "Les urgences sont orientees vers le secretariat du cabinet.",
    ],
    "dupont-lettre": [
        "Courrier du Docteur Jean Dupont au confrere : je vous adresse Madame Exemple pour avis "
        "cardiologique, suite a des palpitations survenues a l'effort depuis trois semaines.",
        "Le traitement en cours associe un betabloquant a faible dose et une surveillance de la tension "
        "arterielle a domicile, avec un controle dans six semaines.",
    ],
    "martin-controle": [
        "Madame Claire Martin a ete recue pour une consultation de controle. La tension est stable, "
        "le poids est inchange et aucun nouveau symptome n'est signale depuis la derniere visite.",
        "Ordonnance de Madame Claire Martin : renouvellement du traitement habituel pour trois mois, "
        "avec une prise de sang de controle avant le prochain rendez-vous.",
    ],
    "notes-bureau": [
        "Note interne : la commande de fournitures de bureau est passee chez le fournisseur habituel, "
        "livraison prevue la semaine prochaine, facture a regler a trente jours.",
        "Rappel : le contrat de maintenance de la photocopieuse arrive a echeance en decembre, "
        "a renouveler avant la fin du mois de novembre.",
        "Compte rendu de la reunion d'equipe : organisation des permanences de l'ete, remplacement "
        "des collegues en conges et mise a jour des plannings.",
    ],
}

# (question, the file whose passage answers it, the file the question NAMES)
QUESTIONS = [
    ("Quel bilan sanguin recommande le Dr Dupont ?", "dupont-bilan", "dupont-bilan"),
    ("Quand le Docteur Dupont recoit-il les patients ?", "dupont-bilan", "dupont-bilan"),
    ("Pourquoi le Dr Dupont adresse-t-il Madame Exemple a un confrere ?", "dupont-lettre", "dupont-lettre"),
    ("Quel traitement est en cours selon le Docteur Dupont ?", "dupont-lettre", "dupont-lettre"),
    ("Comment va Claire Martin depuis sa derniere visite ?", "martin-controle", "martin-controle"),
    ("Que contient l'ordonnance de Claire Martin ?", "martin-controle", "martin-controle"),
    # The name points at one file, the answer is in another: the case the widening exists for.
    ("Le Dr Dupont a-t-il parle de la commande de fournitures de bureau ?", "notes-bureau", "dupont-bilan"),
    ("Le Dr Dupont a-t-il signale quelque chose sur la maintenance de la photocopieuse ?", "notes-bureau", "dupont-bilan"),
    ("Le Docteur Dupont participe-t-il aux permanences de l'ete ?", "notes-bureau", "dupont-bilan"),
    ("Claire Martin est-elle concernee par la reunion d'equipe ?", "notes-bureau", "martin-controle"),
    ("Que dit le Dr Dupont du traitement de Claire Martin ?", "martin-controle", "dupont-lettre"),
    ("Quelle tension a ete relevee pour Claire Martin selon le Dr Dupont ?", "martin-controle", "dupont-bilan"),
]


def post(path, body):
    request = urllib.request.Request(
        GATEWAY + path,
        data=json.dumps(body).encode("utf-8"),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=120) as response:
        return json.loads(response.read())


def alias():
    if ALIAS:
        return ALIAS
    with urllib.request.urlopen(GATEWAY + "/health", timeout=10) as response:
        return json.loads(response.read())["embedding_alias"]


def embed(model, texts):
    answer = post("/v1/embeddings", {"model": model, "input": texts})
    ordered = sorted(answer["data"], key=lambda item: item["index"])
    return [item["embedding"] for item in ordered]


def cosine(a, b):
    dot = sum(x * y for x, y in zip(a, b))
    norm = math.sqrt(sum(x * x for x in a)) * math.sqrt(sum(y * y for y in b))
    return dot / norm if norm else 0.0


# Questions about nothing the passages hold: the floor a related-looking score must clear.
UNRELATED = [
    "Quelle est la recette de la tarte aux pommes de ma grand-mere ?",
    "Qui a gagne le match de football hier soir ?",
    "Quel est le meilleur itineraire pour aller a Lisbonne en train ?",
    "Comment regler la pression des pneus d'une voiture ?",
    "Quelle est la capitale de l'Australie ?",
    "Que dit le Dr Dupont de la recette de la tarte aux pommes ?",
]


def main():
    model = alias()
    flat = [(file, text) for file, texts in PASSAGES.items() for text in texts]
    vectors = embed(model, [text for _, text in flat])
    questions = embed(model, [q for q, _, _ in QUESTIONS])
    unrelated = embed(model, UNRELATED)
    print(f"embedding alias: {model}; {len(flat)} passages; {len(QUESTIONS)} questions\n")
    floor = []
    for question, vector in zip(UNRELATED, unrelated):
        best = max(cosine(vector, v) for v in vectors)
        floor.append(best)
        print(f"{best:5.3f}  UNRELATED (best over every passage)  {question}")
    print(f"unrelated questions: min {min(floor):.3f}  max {max(floor):.3f}\n")

    good, bad = [], []
    for (question, answers_in, names), vector in zip(QUESTIONS, questions):
        scores = [(cosine(vector, v), file) for (file, _), v in zip(flat, vectors)]
        inside_answer = max(score for score, file in scores if file == answers_in)
        inside_named = max(score for score, file in scores if file == names)
        # The reduced set is the file the question names. It holds the answer when they are the same.
        if answers_in == names:
            good.append(inside_named)
        else:
            bad.append(inside_named)
        kind = "answer inside" if answers_in == names else "answer OUTSIDE"
        print(f"{inside_named:5.3f}  reduced set = {names:<16} {kind:<15} (best overall {max(s for s, _ in scores):5.3f})  {question}")

    print()
    print(f"reduced set HOLDS the answer : min {min(good):.3f}  median {sorted(good)[len(good)//2]:.3f}  max {max(good):.3f}  (n={len(good)})")
    print(f"reduced set LACKS the answer : min {min(bad):.3f}  median {sorted(bad)[len(bad)//2]:.3f}  max {max(bad):.3f}  (n={len(bad)})")
    print("A lexical hit adds 0.2 to either, so the threshold is read on the vector score alone and the")
    print("bonus is treated as a margin: a reduced set that holds the name always has the bonus.")
    gap = min(good) - max(bad)
    print(f"gap between the two groups: {gap:+.3f}")
    if gap > 0:
        print(f"a threshold between {max(bad):.3f} and {min(good):.3f} separates them on this sample")
    else:
        print("the two groups overlap on this sample: no threshold separates them")
    return 0


if __name__ == "__main__":
    sys.exit(main())
