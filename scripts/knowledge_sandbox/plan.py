"""The plan of the library: which documents exist, in which order.

Order matters. The first 30 documents are the small library that started the whole programme, written by hand
to hold every trap; the next ones cover every designed person and organisation; the rest are drawn from the
long tail. The stream is sequential and uses its own random generator, so the first N documents are the same
whatever the total: a library of 1 000 starts with exactly the 150 of the core.
"""

from __future__ import annotations

import datetime as dt
import random
from typing import Callable

from . import docs as D
from .cast import Cast, Entity, fold

# A planned document: how to build it, and where it should land.
Spec = tuple[str, Callable[[D.Doc], None], dict]


def plan_documents(cast: Cast, seed: int, total: int) -> list[Spec]:
    rng = random.Random(f"plan-{seed}")
    specs: list[Spec] = []
    P = cast.by_id.get
    O = cast.org

    def add(kind: str, fn: Callable[[D.Doc], None], country: str = "FR", **flags) -> None:
        specs.append((kind, fn, {"country": country, **flags}))

    n_invoice = [13]
    n_contract = [40]

    def next_invoice() -> str:
        n_invoice[0] += 1
        return f"FAC-2026-{n_invoice[0]:03d}"

    def next_contract() -> str:
        n_contract[0] += 1
        return f"CTR-2026-{n_contract[0]:04d}"

    # ---- phase A: the thirty (hand written, every trap present) -----------------------------------------------
    add("lettre_confrere", lambda d: D.lettre_confrere(d, P("p-dupont-jean"), P("d-dupont-jean"), O("hug")), "FR", fmt="pdf", special="homonym")
    add("lettre_confrere", lambda d: D.lettre_confrere(d, P("p-dupont-jean-pierre"), P("d-martin-pierre"), O("change"), "mixed"), "FR", fmt="docx")
    add("resultats_labo", lambda d: D.resultats_labo(d, P("p-dupontet-jean"), O("cerballiance"), P("d-martin-paul")), "FR", fmt="pdf")
    add("lettre_patient", lambda d: D.lettre_patient(d, P("p-dupont-marie"), "plain"), "FR", fmt="txt")
    add("lettre_confrere", lambda d: D.lettre_confrere(d, P("p-dupont-jean"), P("d-gaillard-eric"), O("clinique-annecy"), "implicit"), "FR", fmt="pdf", special="implicit")
    add("mail", lambda d: D.mail(d, P("d-martin-paul"), P("o-marchetti"), "Planning de juillet",
        f"Bonjour Hélène, {d.person(P('d-martin-paul'), 'title_surname')} n'est pas disponible la semaine du 15. Pouvez-vous échanger votre créneau ?"), "FR", fmt="txt", special="ambiguous-martin")
    add("pv_reunion", lambda d: D.pv_reunion(d, [P("o-marchetti"), P("d-martin-paul"), P("s-colas"), P("s-bensaid")], True), "FR", fmt="docx")
    add("lettre_confrere", lambda d: D.lettre_confrere(d, P("p-martin-marie"), P("d-martin-pierre"), O("change"), "plain"), "FR", fmt="pdf", special="cross-border")
    add("courrier_caisse", lambda d: D.courrier_caisse(d, P("p-martin-marie"), O("helsana"), "plain"), "CH", fmt="pdf")
    add("courrier_caisse", lambda d: D.courrier_caisse(d, P("p-dupont-jean"), O("cpam74"), "plain"), "FR", fmt="pdf")
    add("facture_fournisseur", lambda d: D.facture_fournisseur(d, O("medsupply"), "FAC-2026-014", 4), "CH", fmt="pdf", special="fac-014")
    add("facture_fournisseur", lambda d: D.facture_fournisseur(d, O("papeterie-lac"), next_invoice(), 3), "FR", fmt="pdf")
    add("resultats_labo", lambda d: D.resultats_labo(d, P("p-favre-laurent"), O("unilabs"), P("d-favre-olivier")), "CH", fmt="pdf")
    add("lettre_confrere", lambda d: D.lettre_confrere(d, P("p-fabre-laurence"), P("d-gaillard-eric"), O("clinique-annecy"), "plain"), "FR", fmt="txt")
    add("courrier_caisse", lambda d: D.courrier_caisse(d, P("p-rochat-sophie"), O("css"), "plain"), "CH", fmt="pdf")
    add("contrat", lambda d: D.contrat(d, O("bureautique-leman"), next_contract(), "maintenance"), "CH", fmt="docx")
    add("note_interne", lambda d: D.note_interne(d, "photocopieuse"), "FR", fmt="txt")
    add("note_interne", lambda d: D.note_interne(d, "stock", P("s-colas")), "FR", fmt="md")
    add("lettre_confrere", lambda d: D.lettre_confrere(d, P("p-monnier-veronique"), P("d-bersier-marc"), O("hopital-nyon"), "plain"), "CH", fmt="scan-pdf", special="ocr")
    add("lettre_patient", lambda d: D.lettre_patient(d, P("p-meunier-veronique"), "initial"), "FR", fmt="jpg", special="ocr")
    add("transcription", lambda d: D.transcription(d, [P("o-marchetti"), P("d-martin-paul"), P("s-colas")]), "FR", fmt="vtt")
    add("lettre_confrere", lambda d: D.lettre_confrere(d, P("p-kowalski-andrzej"), P("d-haddad-karim"), O("hpps"), "plain"), "FR", fmt="odt")
    add("procedure", lambda d: D.procedure(d, "hygiene"), "FR", fmt="pdf")
    add("lettre_confrere", lambda d: D.lettre_confrere(d, P("p-cuendet-mathieu"), P("d-cuenoud-philippe"), O("clinique-source"), "plain"), "CH", fmt="pdf")
    add("ordonnance", lambda d: D.ordonnance(d, P("p-pittet-isabelle"), "plain"), "CH", fmt="png", special="ocr")
    add("courrier_avocat", lambda d: D.courrier_avocat(d, P("o-marchetti"), "DOS-2026-0057"), "CH", fmt="pdf", special="org-surname")
    add("courrier_pharmacie", lambda d: D.courrier_pharmacie(d, O("pharmacie-dupont"), P("p-dupont-jean")), "FR", fmt="pdf", special="org-surname")
    add("lettre_confrere", lambda d: D.lettre_confrere(d, P("p-girardin-fabienne"), P("d-vuagniaux-anne-sophie"), O("chuv"), "plain"), "CH", fmt="docx")
    add("lettre_confrere", lambda d: D.lettre_confrere(d, P("p-girard-frederic"), P("d-roche-bernard"), O("hcl"), "upper"), "FR", fmt="pdf")
    add("lettre_confrere", lambda d: D.lettre_confrere(d, P("p-lucas-roy"), P("d-martin-pierre"), O("change"), "plain"), "FR", fmt="md")
    add("lettre_confrere", lambda d: D.lettre_confrere(d, P("p-roy-lucas"), P("d-vuagniaux-anne-sophie"), O("chuv"), "upper"), "CH", fmt="pdf")

    # formats a real library holds that the product does not read, and files named after a person
    add("mail", lambda d: D.mail(d, P("s-colas"), P("o-marchetti"), "Commande de gants d'examen",
        "Bonjour, la commande de gants part demain matin. Pouvez-vous valider le devis joint ?", O("medsupply")), "FR", fmt="eml")
    add("presentation", lambda d: D.presentation(d, "Protection des données"), "FR", fmt="pptx")
    add("presentation", lambda d: D.presentation(d, "Accueil des patients"), "FR", fmt="odp")
    add("presentation", lambda d: D.presentation(d, "Gestes d'urgence"), "FR", fmt="ppt")
    add("lettre_confrere", lambda d: D.lettre_confrere(d, P("p-dupont-jean"), P("d-bersier-marc"), O("hopital-nyon"), "plain"), "CH",
        fmt="pdf", fixed_name="dupont-jean", fixed_folder="", special="named-after-person")
    add("horaires", lambda d: D.horaires_medecin(d, P("d-martin-pierre"), "au cabinet des Lilas", "mardi", "8 h à 12 h"), "FR",
        fmt="txt", fixed_name="pierre-martin", fixed_folder="", special="named-after-person")
    add("horaires", lambda d: D.horaires_medecin(d, P("d-martin-paul"), "au cabinet du Parc", "lundi", "14 h à 18 h"), "FR",
        fmt="txt", fixed_name="paul-martin", fixed_folder="", special="named-after-person")

    # spreadsheets saved among the documents: a library has them, the document pipeline does not read them
    add("commandes_tableau", lambda d: D.commandes_tableau(d, [O("medsupply"), O("papeterie-lac"), O("bureautique-leman")]), "FR",
        fmt="xlsx", special="misplaced-workbook")
    add("commandes_tableau", lambda d: D.commandes_tableau(d, [O("pharma-depot"), O("imprimerie-chablais")]), "CH",
        fmt="csv", special="misplaced-workbook")
    add("commandes_tableau", lambda d: D.commandes_tableau(d, [O("sig"), O("edf"), O("swisscom")]), "FR",
        fmt="xlsx", special="misplaced-workbook")

    # ---- phase B: every designed person and organisation appears at least twice ---------------------------------
    # Counting is done on the fly by the builder; here we only fix the order and the kinds. The builder asks
    # `coverage_specs` for more documents until nobody designed is under-represented.
    return specs


def coverage_specs(cast: Cast, rng: random.Random, seen: dict, next_invoice, next_contract) -> list[Spec]:
    """Documents for the designed names that appear fewer than twice so far. `seen` counts documents."""
    out: list[Spec] = []
    P, O = cast.by_id.get, cast.org
    swiss_hospitals = ["hug", "chuv", "clinique-source", "hopital-nyon", "hfr", "rhne", "hopital-valais"]
    french_hospitals = ["change", "chu-grenoble", "hcl", "clinique-annecy", "hpps", "clinique-alpes"]
    for e in cast.people:
        if not e.designed or e.role == "owner":
            continue
        need = (5 if e.weight >= 2.5 else 3 if e.weight >= 1.5 else 1) - seen.get(e.id, 0)
        for _ in range(max(0, need)):
            country = "CH" if e.country == "CH" else "FR"
            if e.role == "patient":
                kind_roll = rng.random()
                doctor = _pick_doctor(cast, rng, country)
                hospital = O(rng.choice(swiss_hospitals if country == "CH" else french_hospitals))
                if kind_roll < 0.4:
                    variant = rng.choice(["plain", "plain", "mixed", "upper", "weak", "no_accent"])
                    out.append(("lettre_confrere", _bind(D.lettre_confrere, e, doctor, hospital, variant), {"country": country, "fmt": None}))
                elif kind_roll < 0.6:
                    lab = O(rng.choice(["unilabs", "viollier"] if country == "CH" else ["cerballiance", "synlab"]))
                    out.append(("resultats_labo", _bind(D.resultats_labo, e, lab, doctor, "plain"), {"country": country, "fmt": None}))
                elif kind_roll < 0.8:
                    insurer = O(rng.choice(["helsana", "css", "groupe-mutuel", "sanitas", "swica", "assura"] if country == "CH" else ["cpam74", "cpam69", "mgen", "harmonie"]))
                    out.append(("courrier_caisse", _bind(D.courrier_caisse, e, insurer, "plain"), {"country": country, "fmt": None}))
                else:
                    out.append(("lettre_patient", _bind(D.lettre_patient, e, rng.choice(["initial", "plain", "upper", "typo"])), {"country": country, "fmt": None}))
            elif e.role == "doctor":
                patient = _pick_patient(cast, rng, e.country if e.country in ("FR", "CH") else "FR")
                hospital = O(rng.choice(swiss_hospitals if country == "CH" else french_hospitals))
                out.append(("lettre_confrere", _bind(D.lettre_confrere, patient, e, hospital, rng.choice(["plain", "mixed"])), {"country": country, "fmt": None}))
            elif e.role == "staff":
                out.append(("note_interne", _bind(D.note_interne, rng.choice(["nettoyage", "planning", "informatique", "stock"]), e), {"country": "FR", "fmt": None}))
    for org in cast.orgs:
        if org.id == "o-cabinet-voirons":
            continue
        need = 1 - seen.get(org.id, 0)
        for _ in range(max(0, need)):
            country = "CH" if org.country == "CH" else "FR"
            if org.kind == "supplier":
                if org.note == "supply" or org.note == "utilities":
                    out.append(("facture_fournisseur", _bind_x(D.facture_fournisseur, org, next_invoice), {"country": country, "fmt": None}))
                else:
                    out.append(("devis", _bind_x(D.devis, org, next_invoice), {"country": country, "fmt": None}))
            elif org.kind == "insurer" or org.kind == "admin":
                patient = _pick_patient(cast, rng, country)
                if org.kind == "insurer":
                    out.append(("courrier_caisse", _bind(D.courrier_caisse, patient, org, "plain"), {"country": country, "fmt": None}))
                else:
                    out.append(("attestation_assurance", _bind_x(D.attestation_assurance, org, next_contract), {"country": country, "fmt": None}))
            elif org.kind == "lab":
                out.append(("resultats_labo", _bind(D.resultats_labo, _pick_patient(cast, rng, country), org, _pick_doctor(cast, rng, country), "plain"), {"country": country, "fmt": None}))
            elif org.kind == "pharmacy":
                out.append(("courrier_pharmacie", _bind(D.courrier_pharmacie, org, _pick_patient(cast, rng, country)), {"country": country, "fmt": None}))
            elif org.kind == "hospital":
                out.append(("lettre_confrere", _bind(D.lettre_confrere, _pick_patient(cast, rng, country), _pick_doctor(cast, rng, country), org, "plain"), {"country": country, "fmt": None}))
    return out


def _bind(fn, *args):
    return lambda d: fn(d, *args)


def _bind_x(fn, org, numbers):
    return lambda d: fn(d, org, numbers())


def _weight(e: Entity) -> float:
    """The designed people come up more often than the long tail: a practice has regulars."""
    return max(e.weight, 0.01) * (3 if e.designed else 1)


def weighted(rng: random.Random, items: list[Entity]) -> Entity:
    total = sum(_weight(e) for e in items)
    roll = rng.random() * total
    acc = 0.0
    for e in items:
        acc += _weight(e)
        if roll <= acc:
            return e
    return items[-1]


def _pick_patient(cast: Cast, rng: random.Random, country: str) -> Entity:
    pool = [e for e in cast.people if e.role == "patient" and (e.country == country or e.country == "XX" or rng.random() < 0.05)]
    return weighted(rng, pool or cast.patients())


def _pick_doctor(cast: Cast, rng: random.Random, country: str) -> Entity:
    pool = [e for e in cast.people if e.role == "doctor" and e.country == country]
    return weighted(rng, pool or cast.doctors())


KIND_WEIGHTS = [
    ("lettre_confrere", 16), ("resultats_labo", 12), ("courrier_caisse", 10), ("facture_fournisseur", 9),
    ("devis", 3), ("contrat", 2), ("ordonnance", 6), ("certificat", 4), ("lettre_patient", 5),
    ("facture_patient", 4), ("demande_remboursement", 4), ("courrier_avocat", 1), ("convocation", 2),
    ("pv_reunion", 2), ("note_interne", 8), ("mail", 6), ("procedure", 3), ("attestation_assurance", 1),
    ("courrier_pharmacie", 2), ("releve_garde", 1.5), ("transcription", 1.5), ("presentation", 1.5),
    ("rapport_activite", 0.4),
]
VARIANTS = [
    ("plain", 55), ("mixed", 15), ("weak", 8), ("implicit", 4), ("initial", 4), ("upper", 6), ("typo", 2),
    ("no_accent", 2), ("surname", 4),
]


def _roll(rng: random.Random, table):
    total = sum(w for _, w in table)
    r = rng.random() * total
    acc = 0.0
    for item, w in table:
        acc += w
        if r <= acc:
            return item
    return table[-1][0]


def random_spec(cast: Cast, rng: random.Random, next_invoice, next_contract) -> Spec:
    kind = _roll(rng, KIND_WEIGHTS)
    O = cast.org
    swiss_hospitals = ["hug", "chuv", "clinique-source", "hopital-nyon", "hfr", "rhne", "hopital-valais"]
    french_hospitals = ["change", "chu-grenoble", "hcl", "clinique-annecy", "hpps", "clinique-alpes"]
    patient = weighted(rng, cast.patients())
    country = patient.country if patient.country in ("FR", "CH") else rng.choice(["FR", "CH"])
    doctor = _pick_doctor(cast, rng, country)
    variant = _roll(rng, VARIANTS)
    if kind == "lettre_confrere":
        hospital = O(rng.choice(swiss_hospitals if country == "CH" else french_hospitals))
        return (kind, _bind(D.lettre_confrere, patient, doctor, hospital, variant), {"country": country, "fmt": None})
    if kind == "resultats_labo":
        lab = O(rng.choice(["unilabs", "viollier"] if country == "CH" else ["cerballiance", "synlab"]))
        return (kind, _bind(D.resultats_labo, patient, lab, doctor, variant if variant in ("plain", "upper", "typo", "no_accent") else "plain"), {"country": country, "fmt": None})
    if kind == "courrier_caisse":
        insurer = O(rng.choice(["helsana", "css", "groupe-mutuel", "sanitas", "swica", "assura", "suva"] if country == "CH" else ["cpam74", "cpam69", "mgen", "harmonie"]))
        return (kind, _bind(D.courrier_caisse, patient, insurer, "plain"), {"country": country, "fmt": None})
    if kind == "facture_fournisseur":
        supplier = O(rng.choice(["medsupply", "papeterie-lac", "bureautique-leman", "pharma-depot", "imprimerie-chablais", "sig", "edf", "swisscom", "orange", "garage-martin"]))
        return (kind, _bind_x(D.facture_fournisseur, supplier, next_invoice), {"country": supplier.country, "fmt": None})
    if kind == "devis":
        supplier = O(rng.choice(["medsupply", "bureautique-leman", "imprimerie-chablais", "pharma-depot"]))
        return (kind, _bind_x(D.devis, supplier, next_invoice), {"country": supplier.country, "fmt": None})
    if kind == "contrat":
        supplier = O(rng.choice(["bureautique-leman", "swisscom", "orange", "sig", "edf"]))
        what = rng.choice(["maintenance du matériel", "nettoyage", "fourniture d'électricité", "télécommunications"])
        return (kind, lambda d, s=supplier, w=what: D.contrat(d, s, next_contract(), w), {"country": supplier.country, "fmt": None})
    if kind == "ordonnance":
        return (kind, _bind(D.ordonnance, patient, variant if variant in ("plain", "mixed", "upper") else "plain"), {"country": country, "fmt": None})
    if kind == "certificat":
        return (kind, _bind(D.certificat, patient, "plain"), {"country": "FR", "fmt": None})
    if kind == "lettre_patient":
        return (kind, _bind(D.lettre_patient, patient, variant if variant in ("initial", "plain", "upper", "typo") else "initial"), {"country": country, "fmt": None})
    if kind == "facture_patient":
        return (kind, _bind(D.facture_patient, patient, "plain"), {"country": "FR", "fmt": None})
    if kind == "demande_remboursement":
        swiss = weighted(rng, [e for e in cast.patients() if e.country == "CH"] or cast.patients())
        insurer = O(rng.choice(["helsana", "css", "groupe-mutuel", "sanitas", "swica", "assura"]))
        return (kind, _bind(D.demande_remboursement, swiss, insurer), {"country": "CH", "fmt": None})
    if kind == "courrier_avocat":
        return (kind, lambda d: D.courrier_avocat(d, cast.owner, f"DOS-2026-{rng.randint(10, 99):04d}"), {"country": "CH", "fmt": None})
    if kind == "convocation":
        who = [cast.owner, cast.by_id["d-martin-paul"], cast.by_id["s-colas"], cast.by_id["s-bensaid"]]
        topic = rng.choice(["renouvellement du contrat de maintenance", "organisation des permanences", "révision de la procédure d'accueil"])
        return (kind, lambda d: D.convocation(d, who, topic), {"country": "FR", "fmt": None})
    if kind == "pv_reunion":
        who = [cast.owner, cast.by_id["d-martin-paul"], cast.by_id["s-colas"]]
        return (kind, lambda d: D.pv_reunion(d, who, rng.random() < 0.3), {"country": "FR", "fmt": None})
    if kind == "note_interne":
        return (kind, _bind(D.note_interne, None, None), {"country": "FR", "fmt": None})
    if kind == "mail":
        other = weighted(rng, cast.doctors())
        subject = rng.choice(["Rendez-vous de la semaine prochaine", "Demande de renseignements", "Facture à régler", "Disponibilité pour une garde"])
        body = rng.choice([
            "Bonjour, pouvez-vous me confirmer le créneau de jeudi ? Merci d'avance.",
            "Bonjour, je vous écris au sujet de la facture en attente. Pourriez-vous nous indiquer la date prévue du règlement ?",
            "Bonjour, je serai absente la semaine prochaine. Le secrétariat vous répondra pour les urgences.",
        ])
        return (kind, lambda d, o=other: D.mail(d, o, cast.owner, subject, body), {"country": "FR", "fmt": None})
    if kind == "procedure":
        return (kind, lambda d: D.procedure(d, rng.choice(["hygiene", "dechets", "rgpd", "accueil", "froid", "incendie"])), {"country": "FR", "fmt": None})
    if kind == "attestation_assurance":
        insurer = O(rng.choice(["suva", "css", "helsana"]))
        return (kind, lambda d, i=insurer: D.attestation_assurance(d, i, next_contract()), {"country": "CH", "fmt": None})
    if kind == "courrier_pharmacie":
        return (kind, _bind(D.courrier_pharmacie, O(rng.choice(["pharmacie-dupont", "pharmacie-lac"])), None), {"country": "FR", "fmt": None})
    if kind == "releve_garde":
        who = [e for e in cast.people if e.role == "doctor" and e.designed][:5]
        return (kind, lambda d, w=who: D.releve_garde(d, w), {"country": "FR", "fmt": None})
    if kind == "transcription":
        return (kind, lambda d: D.transcription(d, [cast.owner, cast.by_id["d-martin-paul"]]), {"country": "FR", "fmt": None})
    if kind == "presentation":
        return (kind, lambda d: D.presentation(d, rng.choice(["Accueil des patients", "Protection des données", "Gestes d'urgence"])), {"country": "FR", "fmt": None})
    return (kind, lambda d: D.rapport_activite(d), {"country": "FR", "fmt": None})
