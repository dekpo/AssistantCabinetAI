"""The documents the `demo` profile adds to the sandbox's own stream: long transcripts (WebVTT and SubRip, hundreds
of cues) and slide decks (PowerPoint and OpenDocument, several slides), each with facts planted behind a code that
exists nowhere else, so a question about it has exactly one answer.

The format of a document is decided by `Wheel`: a fixed, evenly spread schedule, so that any stretch of the stream
holds every format and a small slice shows the same variety as the whole library.
"""

from __future__ import annotations

import datetime as dt
import random

from knowledge_sandbox import docs as D
from knowledge_sandbox.cast import Cast

# Share of the stream, in hundredths, for the documents drawn after the sandbox's own thirty-nine hand-written
# ones. The formats of the product's new readers (rtf, eml, odt, vtt, srt, odp, pptx) are all there.
WEIGHTS = {
    "pdf": 28, "docx": 14, "txt": 7, "md": 3, "odt": 6, "rtf": 5, "eml": 8, "vtt": 4, "srt": 4,
    "odp": 3, "pptx": 4, "scan-pdf": 4, "jpg": 2, "png": 1,
}
# A format whose documents are built for it (cues, slides); every other format takes an ordinary document.
BUILT_FOR_FORMAT = {"vtt", "srt", "odp", "pptx"}


class Wheel:
    """Smooth weighted round robin: each step adds every weight to its credit and takes the format with the most,
    which then pays the total. Deterministic, and evenly spread: in any stretch of 100 steps each format appears
    within one of its share."""

    def __init__(self, weights: dict | None = None):
        self.weights = dict(weights or WEIGHTS)
        self.total = sum(self.weights.values())
        self.credit = {fmt: 0 for fmt in self.weights}
        self.sequence: list[str] = []

    def format_at(self, step: int) -> str:
        """The format of the `step`-th draw (1-based)."""
        while len(self.sequence) < step:
            for fmt, weight in self.weights.items():
                self.credit[fmt] += weight
            best = max(self.weights, key=lambda fmt: (self.credit[fmt], -list(self.weights).index(fmt)))
            self.credit[best] -= self.total
            self.sequence.append(best)
        return self.sequence[step - 1]


# --------------------------------------------------------------------------------------------- transcripts

NEUTRAL = [
    "On reprend le point sur l'organisation de la semaine prochaine.",
    "Je note que le planning des permanences n'a pas changé.",
    "Quelqu'un a-t-il des remarques sur le compte rendu précédent ?",
    "Le standard a reçu plusieurs appels pour des renouvellements d'ordonnance.",
    "Il faudra penser à vérifier les dates de péremption dans l'armoire.",
    "Je propose que l'on garde le même créneau pour la prochaine réunion.",
    "Les nouveaux arrivants ont bien reçu le livret d'accueil.",
    "Je m'occupe de relancer les courriers restés sans réponse.",
    "Pas d'objection, on passe au point suivant.",
    "Je vais vérifier cela auprès du secrétariat et je vous tiens informés.",
    "Le matériel de la salle d'examen a été contrôlé hier.",
    "Nous avons de nouveau eu un retard sur la livraison du courrier.",
    "Je rappelle que les clés de l'armoire restent au secrétariat.",
    "Cela me semble raisonnable, mais je préfère que l'on en reparle lundi.",
    "Il y a eu moins de rendez-vous manqués ce mois-ci.",
    "Merci de signaler toute anomalie dès que vous la constatez.",
    "Je suis d'accord avec cette organisation, elle simplifie l'accueil.",
    "Pouvez-vous envoyer le tableau à tout le monde avant jeudi ?",
    "Le téléphone a sonné plusieurs fois pendant la pause, il faudra y penser.",
    "On garde ce fonctionnement jusqu'à la fin du trimestre.",
    "Je note l'action et je la reporte dans le suivi.",
    "Le nettoyage des locaux se passe bien depuis le changement de prestataire.",
    "Pensez à éteindre les postes en partant, c'est une consigne de la direction.",
    "Je n'ai rien à ajouter sur ce sujet.",
    "Très bien, je prends la parole sur le point suivant.",
]
MEETINGS = [
    "point d'équipe", "revue des commandes", "organisation des gardes", "revue qualité", "point fournisseurs",
    "préparation de la semaine", "bilan mensuel",
]
TOPICS = [
    "Hygiène et gestion des déchets", "Protection des données", "Accueil des patients", "Gestes d'urgence",
    "Chaîne du froid", "Facturation et tiers payant", "Sécurité incendie", "Gestion des plaintes",
]


class Codes:
    """Codes planted in documents, each issued once: nothing else in the library can carry the same one."""

    def __init__(self) -> None:
        self.counters: dict = {}

    def next(self, prefix: str, start: int = 1) -> str:
        n = self.counters.get(prefix, start - 1) + 1
        self.counters[prefix] = n
        return f"{prefix}-{n:04d}"


def stamp(seconds: int) -> str:
    return f"{seconds // 3600:02d}:{seconds % 3600 // 60:02d}:{seconds % 60:02d}.000"


def long_transcript(doc: D.Doc, cast: Cast, codes: Codes, cues_wanted: int) -> None:
    """A meeting of `cues_wanted` cues. Two facts are planted, each in one cue, each behind a code of its own."""
    rng = doc.rng
    owner = cast.owner
    others = [cast.by_id["d-martin-paul"], cast.by_id["s-colas"], cast.by_id["s-bensaid"]]
    speakers = [owner] + others
    meeting = rng.choice(MEETINGS)
    doc.title = f"Transcription du {meeting} du {D.fr_date(doc.date)}"
    doc.name_hint = "transcription-" + meeting.split()[0].lower()
    order_code = codes.next("BC-2026", 400)
    case_code = codes.next("DOS-2026", 700)
    amount = D.money(rng.randint(180, 9800) + rng.choice([0, 0.5, 0.2, 0.9]), doc.country)
    closing = doc.date + dt.timedelta(days=rng.randint(20, 80))
    doc.facts.update({
        "meeting": meeting, "order_code": order_code, "order_amount": amount,
        "case_code": case_code, "case_closing": D.fr_date(closing),
    })
    planted = {
        cues_wanted // 3: (owner, f"Le bon de commande {order_code} est validé pour un montant de {amount}."),
        (2 * cues_wanted) // 3: (others[1], f"Le dossier {case_code} sera clos le {D.fr_date(closing)}."),
    }
    clock = rng.randint(0, 600)
    for number in range(cues_wanted):
        if number in planted:
            speaker, text = planted[number]
        else:
            speaker, text = rng.choice(speakers), rng.choice(NEUTRAL)
        end = clock + rng.randint(3, 11)
        label = speaker.canonical if speaker.role != "doctor" else f"{speaker.title or 'Dr'} {speaker.family}"
        doc.cues.append((stamp(clock), stamp(end), label, text))
        clock = end + rng.randint(0, 3)
    for speaker in speakers:
        label = speaker.canonical if speaker.role != "doctor" else f"{speaker.title or 'Dr'} {speaker.family}"
        doc.mentions.append({"entity": speaker.id, "text": label, "form": "title_surname" if speaker.role == "doctor" else "full", "where": "text"})
    doc.prefer = "vtt"


# ------------------------------------------------------------------------------------------------ decks


def deck(doc: D.Doc, cast: Cast, codes: Codes, slides_wanted: int) -> None:
    """A training deck of `slides_wanted` slides. The facts are on the second slide and later, never on the first."""
    rng = doc.rng
    topic = rng.choice(TOPICS)
    module = codes.next("MOD", 100)
    speaker = rng.choice([cast.owner, cast.by_id["d-martin-paul"], cast.by_id["s-bensaid"]])
    label = speaker.canonical if speaker.role != "doctor" else f"{speaker.title or 'Dr'} {speaker.family}"
    session = doc.date + dt.timedelta(days=rng.randint(10, 90))
    cost = D.money(rng.randint(40, 480) + rng.choice([0, 0.5]), doc.country)
    seats = rng.randint(6, 24)
    doc.title = f"Formation : {topic} (module {module})"
    doc.name_hint = "formation-" + topic.split()[0].lower()
    doc.facts.update({
        "topic": topic, "module": module, "session_date": D.fr_date(session), "cost_per_person": cost,
        "seats": str(seats), "speaker": label,
    })
    doc.mentions.append({"entity": speaker.id, "text": label, "form": "title_full" if speaker.role == "doctor" else "full", "where": "text"})
    fixed = [
        (doc.title, [f"Intervenant : {label}", "Cabinet de groupe des Voirons"]),
        ("Objectifs", [f"Référence du module : {module}", "Connaître la procédure", "Savoir à qui s'adresser"]),
        ("Organisation", [f"Date de la session : {D.fr_date(session)}", f"Nombre de places : {seats}"]),
        ("Coût", [f"Coût par participant : {cost}", "Facturé au cabinet après la session"]),
    ]
    filler = [
        ("Rappel", ["Vérifier l'identité de la personne", "Consigner la demande au registre", "Informer le secrétariat"]),
        ("Cas pratiques", ["Un appel au standard", "Un courrier reçu sans destinataire", "Une demande de copie"]),
        ("Points de vigilance", ["Ne pas laisser de document sur le comptoir", "Fermer la session en quittant le poste"]),
        ("Questions", ["Temps d'échange", "Retours d'expérience du cabinet"]),
        ("Pour aller plus loin", ["Fiche de procédure interne", "Contact du référent du cabinet"]),
        ("Évaluation", ["Questionnaire de fin de session", "Retours à transmettre au secrétariat"]),
        ("Calendrier", ["Prochaine session au trimestre suivant", "Inscription auprès du secrétariat"]),
        ("Synthèse", ["Les trois gestes à retenir", "Les interlocuteurs en cas de doute"]),
    ]
    doc.slides = fixed + filler[: max(0, slides_wanted - len(fixed))]
    doc.prefer = "slides"
