"""Document templates of the sandbox.

Every template builds a `Doc`: a title, paragraphs, and the list of everything it names. A person or an
organisation is written through `doc.person(...)`, `doc.org(...)` or `doc.ident(...)`, which return the exact
string to put in the text and record it. The ground truth is that record, never a reading of the text.

The prose is invented, in French, in the register of a general practice that works across the border between
Haute-Savoie and the canton of Geneva: letters from specialists, laboratory reports, health insurance letters
(France and Switzerland), supplier invoices, contracts, minutes, notes, e-mails.
"""

from __future__ import annotations

import datetime as dt
import random
from dataclasses import dataclass, field

from . import pools
from .cast import Cast, Entity, person_forms

MONTHS = [
    "janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre",
    "novembre", "décembre",
]


def fr_date(day: dt.date) -> str:
    return f"{'1er' if day.day == 1 else day.day} {MONTHS[day.month - 1]} {day.year}"


def numeric_date(day: dt.date) -> str:
    return f"{day.day:02d}.{day.month:02d}.{day.year}"


def money(amount: float, country: str) -> str:
    """French style `1 450,00 €`, Swiss style `CHF 1'450.00`."""
    whole, cents = divmod(round(amount * 100), 100)
    if country == "CH":
        grouped = f"{whole:,}".replace(",", "'")
        return f"CHF {grouped}.{cents:02d}"
    grouped = f"{whole:,}".replace(",", " ")
    return f"{grouped},{cents:02d} €"


@dataclass
class Doc:
    kind: str
    date: dt.date
    country: str  # FR | CH : the register of the document
    cast: Cast
    rng: random.Random
    title: str = ""
    paragraphs: list = field(default_factory=list)
    mentions: list = field(default_factory=list)
    about: list = field(default_factory=list)
    facts: dict = field(default_factory=dict)
    name_hint: str = ""  # words for the file name
    named_after: list = field(default_factory=list)  # entity ids whose surname the file name carries
    prefer: str = ""  # preferred format family: pdf | docx | txt | md | eml | vtt | slides | any
    needs_unicode: bool = False
    headers: dict = field(default_factory=dict)  # e-mail
    cues: list = field(default_factory=list)  # transcript
    slides: list = field(default_factory=list)  # presentation
    numbers: dict = field(default_factory=dict)  # figures the workbooks repeat (invoices, contracts)
    table: list = field(default_factory=list)  # a workbook saved in the documents folder (xlsx, csv)

    # -- recording ---------------------------------------------------------------------------------------

    def _record(self, entity: Entity, text: str, form: str) -> str:
        self.mentions.append({"entity": entity.id, "text": text, "form": form, "where": "text"})
        return text

    def person(self, e: Entity, form: str = "full") -> str:
        forms = person_forms(e)
        return self._record(e, forms[form], form)

    def org(self, o: Entity, form: str = "full") -> str:
        text = o.canonical if form == "full" or not o.short else o.short
        if form == "upper":
            text = o.canonical.upper()
        return self._record(o, text, "acronym" if text == o.short and o.short else form)

    def ident(self, kind: str, value: str, owner: Entity | None = None) -> str:
        entity = self.cast.identifier(kind, value, owner.id if owner else None)
        self.mentions.append({"entity": entity.id, "text": value, "form": kind, "where": "text"})
        return value

    def subject(self, e: Entity) -> None:
        if e.id not in self.about:
            self.about.append(e.id)

    def paragraph(self, text: str) -> None:
        self.paragraphs.append(text)


def choice(rng: random.Random, items):
    return items[rng.randrange(len(items))]


def letterhead(doc: Doc) -> str:
    """The practice's own letterhead: the user's own name and organisation, in every outgoing letter."""
    owner = doc.cast.owner
    org = doc.cast.own_org
    return (
        f"{doc.org(org)}\n"
        f"{doc.person(owner, 'title_full')}, médecine générale\n"
        "8 rue du Léman, 74100 Annemasse\n"
        "Tél. 01 99 00 41 20"
    )


def insurance_line(doc: Doc, patient: Entity) -> str:
    """The health insurance number a letter carries, by the patient's papers."""
    if doc.country == "CH" and "avs" in patient.ids:
        return f"N° AVS : {doc.ident('avs', patient.ids['avs'], patient)}"
    if "nir" in patient.ids:
        return f"N° de sécurité sociale : {doc.ident('nir', patient.ids['nir'], patient)}"
    if "avs" in patient.ids:
        return f"N° AVS : {doc.ident('avs', patient.ids['avs'], patient)}"
    return ""


def address_block(doc: Doc, e: Entity) -> str:
    town = e.commune
    if town and town[2] in {"VD", "GE", "NE", "FR", "VS", "JU", "BE"}:
        return f"{e.street}, {town[1]} {town[0]}"
    return f"{e.street}, {town[1]} {town[0]}" if town else ""


def name_in_text(doc: Doc, e: Entity, variant: str, first: bool) -> str:
    """How a letter writes its subject at one place. `variant` controls the shapes, so the library holds the
    full range: the full name, a title and a surname, an initial, the administrative upper-case order, a hyphen
    typed by mistake, the surname alone."""
    if variant == "plain":
        return doc.person(e, "full" if first else choice(doc.rng, ["full", "title_surname"]))
    if variant == "mixed":
        return doc.person(e, "title_full" if first else choice(doc.rng, ["title_surname", "full", "initial_surname"]))
    if variant in ("weak", "implicit"):
        return doc.person(e, "title_surname")
    if variant == "initial":
        return doc.person(e, "initial_surname")
    if variant == "upper":
        return doc.person(e, "reordered" if first else "title_surname")
    if variant == "typo":
        return doc.person(e, "hyphen_error" if first else "title_surname")
    if variant == "surname":
        return doc.person(e, "surname_only")
    if variant == "no_accent":
        return doc.person(e, "no_accent")
    return doc.person(e, "full")


REASONS = [
    "un suivi cardiologique", "un bilan endocrinologique", "un avis rhumatologique",
    "une consultation de dermatologie", "un bilan neurologique", "le suivi d'une hypertension artérielle",
    "des douleurs lombaires persistantes", "un contrôle postopératoire", "un bilan de santé annuel",
    "le suivi d'un diabète de type 2", "des troubles du sommeil", "des vertiges récurrents",
    "une surveillance thyroïdienne", "un bilan de la vue", "un avis ORL",
]
CLOSINGS = [
    "Je vous remercie de votre confiance et reste à votre disposition.",
    "Je reverrai le patient dans trois mois, sauf complication.",
    "Un nouveau rendez-vous est fixé au printemps.",
    "Je vous laisse poursuivre la prise en charge habituelle.",
    "N'hésitez pas à me contacter pour toute question.",
]
SALUTATIONS = ["Cher confrère,", "Chère consœur,", "Cher collègue,"]


# --------------------------------------------------------------------------------------------- templates


def lettre_confrere(doc: Doc, patient: Entity, specialist: Entity, org: Entity, variant: str = "plain",
                    with_number: bool = True) -> None:
    rng, cast = doc.rng, doc.cast
    owner = cast.owner
    doc.title = f"Compte rendu de consultation, {fr_date(doc.date)}"
    doc.subject(patient)
    doc.name_hint = "courrier-" + choice(rng, ["cardiologie", "endocrinologie", "rhumatologie", "neurologie", "orl", "dermatologie", "compte-rendu"])
    doc.named_after = [] if variant.startswith("implicit") else [patient.id]
    org_line = doc.org(org)
    if org.short and rng.random() < 0.35:
        # a letterhead often gives the acronym after the name
        org_line = f"{org_line} ({doc.org(org, 'acronym')})"
    phone_line = f"\nTél. {doc.ident('phone', specialist.ids['phone'], specialist)}" if "phone" in specialist.ids else ""
    sender = (
        f"{doc.person(specialist, 'title_full')}\n{org_line}\n"
        + (f"{specialist.street}, {specialist.commune[1]} {specialist.commune[0]}" if specialist.commune else "")
        + phone_line
    )
    doc.paragraph(sender)
    doc.paragraph(f"{doc.person(owner, 'title_full')}\n8 rue du Léman, 74100 Annemasse")
    doc.paragraph(f"{specialist.commune[0] if specialist.commune else 'Genève'}, le {fr_date(doc.date)}")
    head_variant = {"weak": "plain", "initial": "plain", "implicit": "weak", "implicit_initial": "initial"}.get(variant, variant)
    head = name_in_text(doc, patient, head_variant, True)
    reason = choice(rng, REASONS)
    doc.paragraph(f"Objet : {head}, né(e) en {patient.birth_year}, consultation pour {reason}.")
    if with_number and not variant.startswith("implicit"):
        line = insurance_line(doc, patient)
        if line:
            doc.paragraph(line)
    doc.paragraph(choice(rng, SALUTATIONS))
    mention = name_in_text(doc, patient, variant, False)
    doc.paragraph(
        f"J'ai reçu {mention} le {fr_date(doc.date - dt.timedelta(days=rng.randint(2, 20)))} pour {reason}. "
        f"L'examen clinique est rassurant et les résultats récents sont conformes à ce qui était attendu. "
        f"J'ai demandé un contrôle biologique et un courrier de synthèse sera adressé à la caisse."
    )
    medication = choice(rng, pools.MEDICATIONS)
    doc.paragraph(
        f"Le traitement par {medication} est poursuivi à la dose habituelle. "
        f"{name_in_text(doc, patient, variant if variant != 'plain' else 'plain', False)} "
        f"a été informé(e) des modalités de suivi et du calendrier des prochains rendez-vous."
    )
    doc.facts["next_visit"] = fr_date(doc.date + dt.timedelta(days=90))
    doc.paragraph(choice(rng, CLOSINGS) + f" Prochain rendez-vous : {doc.facts['next_visit']}.")
    doc.paragraph(f"Confraternellement,\n{doc.person(specialist, 'title_full')}")
    doc.prefer = "pdf"


def resultats_labo(doc: Doc, patient: Entity, lab: Entity, prescriber: Entity, variant: str = "plain") -> None:
    rng = doc.rng
    doc.title = f"Résultats d'analyses du {fr_date(doc.date)}"
    doc.subject(patient)
    doc.name_hint = "resultats-analyses"
    doc.named_after = [patient.id]
    doc.paragraph(f"{doc.org(lab)}\nDossier n° {rng.randint(10000, 99999)}")
    doc.paragraph(f"Patient : {name_in_text(doc, patient, 'upper' if variant == 'plain' else variant, True)}")
    doc.paragraph(f"Prescripteur : {doc.person(prescriber, 'title_full')}")
    line = insurance_line(doc, patient)
    if line:
        doc.paragraph(line)
    hba1c = round(rng.uniform(5.2, 8.4), 1)
    creat = rng.randint(58, 104)
    doc.facts["hba1c"] = f"{hba1c}".replace(".", ",") + " %"
    rows = [
        f"Hémoglobine glyquée : {doc.facts['hba1c']}",
        f"Créatinine : {creat} µmol/L",
        f"Cholestérol total : {rng.randint(40, 70) / 10:.1f} mmol/L".replace(".", ","),
        f"Glycémie à jeun : {rng.randint(40, 80) / 10:.1f} mmol/L".replace(".", ","),
    ]
    doc.paragraph("\n".join(rows))
    if rng.random() < 0.45:
        organism = choice(rng, pools.ORGANISMS)
        doc.facts["organism"] = organism
        doc.paragraph(
            f"Examen cytobactériologique des urines : culture positive à {organism} "
            f"(> 10^5 UFC/mL). Antibiogramme joint."
        )
    doc.paragraph(f"Résultats validés le {fr_date(doc.date)} par le laboratoire. Copie à {name_in_text(doc, patient, 'weak', False)}.")
    doc.prefer = "pdf"


def courrier_caisse(doc: Doc, patient: Entity, insurer: Entity, variant: str = "plain") -> None:
    rng = doc.rng
    swiss = insurer.country == "CH"
    doc.country = "CH" if swiss else "FR"
    doc.title = f"Décision de prise en charge, {fr_date(doc.date)}"
    doc.subject(patient)
    doc.name_hint = ("decompte-" if swiss else "courrier-") + insurer.org_key
    doc.named_after = [patient.id] if rng.random() < 0.5 else []
    doc.paragraph(f"{doc.org(insurer)}\nService des prestations\n{choice(rng, ['Case postale 2323, 1211 Genève 2', 'BP 280, 74008 Annecy cedex'])}")
    doc.paragraph(f"{name_in_text(doc, patient, 'plain', True)}\n{address_block(doc, patient)}")
    line = insurance_line(doc, patient)
    if line:
        doc.paragraph(line)
    reference = f"{rng.randint(100000, 999999)}"
    if swiss:
        police = f"{rng.randint(1, 9)}.{rng.randint(100, 999)}.{rng.randint(100, 999)}-{rng.randint(0, 9)}"
        doc.paragraph(f"Police n° {doc.ident('policy', police, patient)}")
        franchise = choice(rng, [300, 500, 1000, 1500, 2000, 2500])
        amount = round(rng.uniform(80, 940), 2)
        doc.facts["amount"] = money(amount, "CH")
        doc.paragraph(
            f"Madame, Monsieur,\nNous avons traité la facture de votre médecin du {fr_date(doc.date - dt.timedelta(days=rng.randint(10, 40)))}. "
            f"Montant pris en charge : {doc.facts['amount']}, après déduction de la franchise annuelle de CHF {franchise}.00 "
            f"et de la quote-part de 10 %. Le versement sera effectué sur votre compte dans les dix jours."
        )
        doc.paragraph("Ce décompte tient lieu de décision au sens de la LAMal. Une opposition est possible dans les 30 jours.")
    else:
        amount = round(rng.uniform(25, 410), 2)
        doc.facts["amount"] = money(amount, "FR")
        doc.paragraph(
            f"Madame, Monsieur,\nVotre dossier n° {reference} a été examiné. "
            f"Le remboursement de {doc.facts['amount']} sera versé sur le compte indiqué dans votre dossier. "
            f"Les indemnités journalières, le cas échéant, sont calculées sur les trois derniers mois de salaire."
        )
        doc.paragraph("Pour toute question, contactez votre agence en rappelant la référence de ce courrier.")
    doc.paragraph(f"Veuillez agréer, Madame, Monsieur, l'expression de nos salutations distinguées.\n{insurer.short or insurer.canonical}")
    doc.prefer = "pdf"


def facture_fournisseur(doc: Doc, supplier: Entity, number: str, lines: int = 3) -> None:
    rng = doc.rng
    country = supplier.country
    doc.country = country
    doc.title = f"Facture {number}"
    doc.name_hint = f"facture-{supplier.org_key}-{number.lower()}"
    doc.paragraph(f"{doc.org(supplier)}\n{choice(rng, pools.STREETS_CH if country == 'CH' else pools.STREETS_FR)} {rng.randint(1, 60)}")
    doc.paragraph(f"Facture n° {doc.ident('invoice', number)}\nDate : {fr_date(doc.date)}")
    doc.paragraph(f"Client : {doc.org(doc.cast.own_org)}, 8 rue du Léman, 74100 Annemasse")
    items = [
        ("Gants d'examen, boîte de 100", 8.9), ("Papier d'examen, rouleau", 4.2), ("Compresses stériles, lot de 50", 6.5),
        ("Solution hydroalcoolique 500 mL", 7.8), ("Abaisse-langue, boîte de 100", 5.1), ("Cartouches d'encre", 62.0),
        ("Ramette de papier A4", 5.9), ("Thermomètre frontal", 39.0), ("Tensiomètre de rechange", 74.0),
    ]
    total = 0.0
    out = []
    for _ in range(lines):
        label, price = choice(rng, items)
        qty = rng.randint(1, 12)
        total += price * qty
        out.append(f"{label} — {qty} × {money(price, country)} = {money(price * qty, country)}")
    doc.paragraph("\n".join(out))
    vat = round(total * (0.081 if country == "CH" else 0.20), 2)
    doc.numbers = {"kind": "invoice", "supplier": supplier.id, "number": number, "ht": total, "vat": vat, "date": doc.date}
    doc.facts["total_ht"] = money(total, country)
    doc.facts["total_ttc"] = money(total + vat, country)
    doc.paragraph(
        f"Total HT : {doc.facts['total_ht']}\nTVA : {money(vat, country)}\nTotal à payer : {doc.facts['total_ttc']}\n"
        f"Échéance : {fr_date(doc.date + dt.timedelta(days=30))}"
    )
    iban = pools.iban_ch(rng) if country == "CH" else pools.iban_fr(rng)
    doc.paragraph(f"Coordonnées bancaires : IBAN {doc.ident('iban', iban)}")
    doc.prefer = "pdf"


def devis(doc: Doc, supplier: Entity, number: str) -> None:
    rng = doc.rng
    country = supplier.country
    doc.country = country
    doc.title = f"Devis {number}"
    doc.name_hint = f"devis-{supplier.org_key}"
    doc.paragraph(f"{doc.org(supplier)}")
    amount = round(rng.uniform(300, 4800), 2)
    doc.facts["amount"] = money(amount, country)
    doc.paragraph(
        f"Devis n° {doc.ident('quote', number)} du {fr_date(doc.date)}\n"
        f"Destinataire : {doc.person(doc.cast.owner, 'title_full')}, {doc.org(doc.cast.own_org)}\n"
        f"Objet : {choice(rng, ['renouvellement du matériel de bureau', 'fourniture de consommables', 'remplacement d' + chr(39) + 'un poste informatique', 'aménagement de la salle d' + chr(39) + 'attente'])}.\n"
        f"Montant total : {doc.facts['amount']}. Validité du devis : trois mois."
    )
    doc.prefer = "pdf"


def contrat(doc: Doc, supplier: Entity, number: str, what: str) -> None:
    rng = doc.rng
    country = supplier.country
    doc.country = country
    doc.title = f"Contrat de {what}"
    doc.name_hint = f"contrat-{what.split()[0]}-{supplier.org_key}"
    annual = round(rng.uniform(400, 3600), 2)
    doc.facts["annual"] = money(annual, country)
    start = doc.date
    doc.numbers = {"kind": "contract", "supplier": supplier.id, "number": number, "annual": annual, "date": start, "end": start.replace(year=start.year + 1)}
    doc.facts["end"] = fr_date(start.replace(year=start.year + 1))
    doc.paragraph(f"CONTRAT N° {doc.ident('contract', number)}\nEntre {doc.org(supplier)} et {doc.org(doc.cast.own_org)}, représenté par {doc.person(doc.cast.owner, 'title_full')}.")
    doc.paragraph(
        f"Article 1 — Objet. Le prestataire assure {what} des locaux et du matériel du cabinet.\n"
        f"Article 2 — Durée. Le contrat prend effet le {fr_date(start)} pour une durée d'un an, soit jusqu'au {doc.facts['end']}, "
        f"renouvelable par tacite reconduction.\n"
        f"Article 3 — Prix. Le prix annuel est de {doc.facts['annual']}, payable à réception de facture.\n"
        f"Article 4 — Résiliation. Chaque partie peut résilier le contrat par lettre recommandée, trois mois avant l'échéance."
    )
    doc.paragraph("Fait en deux exemplaires, à Annemasse.")
    doc.prefer = "docx"


def ordonnance(doc: Doc, patient: Entity, variant: str = "plain") -> None:
    rng = doc.rng
    doc.title = f"Ordonnance du {fr_date(doc.date)}"
    doc.subject(patient)
    doc.name_hint = "ordonnance"
    doc.named_after = [patient.id] if rng.random() < 0.6 else []
    doc.paragraph(letterhead(doc))
    doc.paragraph(f"Patient : {name_in_text(doc, patient, variant, True)}, né(e) en {patient.birth_year}")
    meds = rng.sample(pools.MEDICATIONS, 2)
    doc.paragraph(
        f"{meds[0].capitalize()}, un comprimé matin et soir, pendant trois mois.\n"
        f"{meds[1].capitalize()}, un comprimé le soir, renouvelable une fois."
    )
    doc.paragraph(f"Fait à Annemasse, le {fr_date(doc.date)}.")
    doc.prefer = "pdf"


def certificat(doc: Doc, patient: Entity, variant: str = "plain") -> None:
    rng = doc.rng
    doc.title = "Certificat d'arrêt de travail"
    doc.subject(patient)
    doc.name_hint = "arret-travail"
    doc.named_after = [patient.id]
    days = rng.choice([3, 5, 7, 10, 14])
    doc.paragraph(letterhead(doc))
    doc.paragraph(
        f"Je soussignée, {doc.person(doc.cast.owner, 'title_full')}, certifie que l'état de santé de "
        f"{name_in_text(doc, patient, variant, True)} justifie un arrêt de travail de {days} jours à compter du {fr_date(doc.date)}."
    )
    doc.facts["days"] = str(days)
    doc.prefer = "pdf"


def lettre_patient(doc: Doc, patient: Entity, variant: str = "initial") -> None:
    rng = doc.rng
    doc.title = "Demande de copie de dossier"
    doc.subject(patient)
    doc.name_hint = "courrier-patient"
    doc.named_after = [patient.id] if rng.random() < 0.5 else []
    doc.paragraph(f"{address_block(doc, patient)}")
    if sum(map(ord, patient.id)) % 10 < 7 and "phone" in patient.ids and "email" in patient.ids:
        doc.paragraph(f"Tél. {doc.ident('phone', patient.ids['phone'], patient)} — {doc.ident('email', patient.ids['email'], patient)}")
    doc.paragraph(f"{doc.person(doc.cast.owner, 'title_full')}\n{doc.org(doc.cast.own_org)}\n8 rue du Léman, 74100 Annemasse")
    doc.paragraph(f"Objet : demande de rendez-vous et de copie de mon dossier. {patient.commune[0]}, le {fr_date(doc.date)}.")
    doc.paragraph(
        "Madame le Docteur,\nJe souhaiterais obtenir un rendez-vous dans le courant du mois prochain et recevoir une copie "
        "des derniers résultats d'analyses. Je me tiens à votre disposition pour toute précision."
    )
    sign = {"initial": "initial_surname", "plain": "full", "upper": "reordered", "weak": "title_surname", "typo": "hyphen_error"}.get(variant, "full")
    doc.paragraph(f"Avec mes salutations distinguées,\n{doc.person(patient, sign)}")
    doc.prefer = "any"


def courrier_avocat(doc: Doc, client: Entity, number: str) -> None:
    rng = doc.rng
    firm = doc.cast.org("etude-chappuis")
    doc.country = "CH"
    doc.title = "Courrier de l'étude"
    doc.name_hint = "courrier-etude-chappuis"
    doc.paragraph(f"{doc.org(firm)}\nRue de Bourg 12, 1003 Lausanne")
    doc.paragraph(f"{doc.person(doc.cast.owner, 'title_full')}\n{doc.org(doc.cast.own_org)}")
    landlord = doc.cast.org("dupont-immobilier")
    doc.paragraph(
        f"Dossier n° {doc.ident('case', number)}. Concerne : bail des locaux du cabinet, bailleur {doc.org(landlord)}.\n"
        f"Maître, Madame,\nNous avons examiné la clause d'indexation du loyer. Le bailleur propose une hausse de "
        f"{rng.choice([1.5, 2.0, 2.5, 3.0])} % à compter du {fr_date(doc.date + dt.timedelta(days=60))}. "
        f"Nous recommandons de la contester par écrit avant le {fr_date(doc.date + dt.timedelta(days=30))}."
    )
    doc.paragraph("Nous restons à votre disposition. Salutations distinguées, Me Chappuis")
    doc.prefer = "pdf"


def convocation(doc: Doc, attendees: list, topic: str) -> None:
    doc.title = f"Convocation à la réunion de cabinet du {fr_date(doc.date)}"
    doc.name_hint = "convocation-reunion"
    names = [doc.person(e, "title_surname" if e.role == "doctor" else "full") for e in attendees]
    doc.paragraph(
        f"Réunion de cabinet le {fr_date(doc.date)} à 12 h 30, salle de pause.\n"
        f"Participants : {', '.join(names)}.\nOrdre du jour : {topic}."
    )
    doc.prefer = "txt"


def pv_reunion(doc: Doc, attendees: list, ambiguous_martin: bool = False) -> None:
    rng = doc.rng
    cast = doc.cast
    doc.title = f"Procès-verbal de la réunion de cabinet du {fr_date(doc.date)}"
    doc.name_hint = "pv-reunion-cabinet"
    names = [doc.person(e, "title_full" if e.role == "doctor" else "full") for e in attendees]
    doc.paragraph(f"Présents : {', '.join(names)}.\nRédacteur : {doc.person(cast.by_id['s-colas'], 'full')}.")
    supplier = cast.org(rng.choice(["medsupply", "bureautique-leman", "papeterie-lac"]))
    decisions = [
        f"1. Matériel — le devis de {doc.org(supplier)} est accepté pour un montant de {money(rng.uniform(400, 2200), supplier.country)}.",
        f"2. Planning — les permanences de juillet sont réparties entre {doc.person(cast.by_id['d-martin-paul'], 'title_full')} et {doc.person(cast.owner, 'title_full')}.",
        "3. Hygiène — la procédure de nettoyage des salles d'examen est rappelée et affichée.",
    ]
    if ambiguous_martin:
        decisions.append("4. Formation — Dr Martin propose une demi-journée sur la gestion des rendez-vous manqués.")
        doc.mentions.append({"entity": "d-martin-paul", "text": "Dr Martin", "form": "title_surname", "where": "text"})
    doc.paragraph("\n".join(decisions))
    doc.paragraph(f"La prochaine réunion aura lieu le {fr_date(doc.date + dt.timedelta(days=60))}.")
    doc.prefer = "docx"


def note_interne(doc: Doc, topic: str | None = None, staff: Entity | None = None) -> None:
    rng = doc.rng
    topics = {
        "nettoyage": "Le nettoyage des salles d'examen est assuré chaque soir par l'entreprise de ménage. Merci de laisser les surfaces dégagées.",
        "stock": "Le stock de gants et de compresses est à vérifier le premier lundi du mois. La commande part le mardi matin.",
        "fermeture": "Le cabinet sera fermé le pont de l'Ascension. Les urgences sont orientées vers le centre de garde.",
        "photocopieuse": "Le contrat de maintenance de la photocopieuse arrive à échéance en décembre. À renouveler avant la fin novembre.",
        "planning": "Le planning des permanences du mois est affiché à l'accueil. Les échanges de créneaux se font avec l'accord du secrétariat.",
        "informatique": "Les sauvegardes du poste d'accueil sont faites chaque nuit. En cas de panne, appeler le prestataire informatique.",
    }
    key = topic or rng.choice(sorted(topics))
    doc.title = f"Note interne — {key}"
    doc.name_hint = f"note-interne-{key}"
    body = topics[key]
    if staff is not None:
        body += f" Contact : {doc.person(staff, 'full')}."
    doc.paragraph(body)
    doc.facts["topic"] = key
    doc.prefer = "txt"


def mail(doc: Doc, sender: Entity, recipient: Entity, subject: str, body: str, org: Entity | None = None) -> None:
    doc.title = f"Objet : {subject}"
    doc.name_hint = "message"
    doc.headers = {
        "From": f"{sender.canonical} <{sender.ids.get('email', 'contact@mail.example')}>",
        "To": f"{recipient.canonical} <{recipient.ids.get('email', 'contact@mail.example')}>",
        "Date": f"{doc.date.isoformat()}",
        "Subject": subject,
    }
    doc.mentions.append({"entity": sender.id, "text": sender.canonical, "form": "full", "where": "text"})
    doc.mentions.append({"entity": recipient.id, "text": recipient.canonical, "form": "full", "where": "text"})
    doc.paragraph(body)
    if org is not None:
        doc.paragraph(f"Cordialement,\n{doc.org(org)}")
    doc.prefer = "eml"


def demande_remboursement(doc: Doc, patient: Entity, insurer: Entity) -> None:
    rng = doc.rng
    doc.country = "CH"
    doc.title = "Demande de remboursement"
    doc.subject(patient)
    doc.name_hint = "demande-remboursement"
    doc.named_after = [patient.id]
    doc.paragraph(f"{name_in_text(doc, patient, 'upper', True)}\n{address_block(doc, patient)}")
    police = f"{rng.randint(1, 9)}.{rng.randint(100, 999)}.{rng.randint(100, 999)}-{rng.randint(0, 9)}"
    doc.paragraph(f"À l'attention de {doc.org(insurer)}\nPolice n° {doc.ident('policy', police, patient)}")
    line = insurance_line(doc, patient)
    if line:
        doc.paragraph(line)
    amount = round(rng.uniform(60, 640), 2)
    doc.facts["amount"] = money(amount, "CH")
    doc.paragraph(
        f"Madame, Monsieur,\nVeuillez trouver ci-joint la facture de mon médecin du {fr_date(doc.date - dt.timedelta(days=15))} "
        f"d'un montant de {doc.facts['amount']}. Je vous prie de bien vouloir la rembourser selon ma couverture LAMal. "
        f"Je précise que ma franchise a déjà été entamée cette année."
    )
    doc.prefer = "pdf"


def rapport_activite(doc: Doc) -> None:
    rng = doc.rng
    cast = doc.cast
    doc.title = f"Rapport d'activité {doc.date.year - 1}"
    doc.name_hint = "rapport-activite"
    doc.country = "FR"
    consultations = rng.randint(3800, 5200)
    doc.facts["consultations"] = str(consultations)
    doc.paragraph(f"{doc.org(cast.own_org)} — rapport d'activité de l'année {doc.date.year - 1}, rédigé par {doc.person(cast.owner, 'title_full')}.")
    doc.paragraph(
        f"Nombre de consultations : {consultations}. Taux de rendez-vous manqués : {rng.randint(3, 9)} %. "
        f"Part des patients résidant en Suisse : {rng.randint(18, 34)} %. "
        f"Les échanges avec {doc.org(cast.org('hug'))}, {doc.org(cast.org('change'))} et {doc.org(cast.org('cpam74'))} "
        f"restent les plus fréquents."
    )
    doc.paragraph(
        "Projets de l'année : renouvellement du parc informatique, révision de la procédure d'accueil et "
        "mise à jour du registre des traitements de données."
    )
    doc.prefer = "docx"


def procedure(doc: Doc, topic: str) -> None:
    banks = {
        "hygiene": ("Procédure d'hygiène des mains", "Le lavage des mains est réalisé avant et après chaque examen. La solution hydroalcoolique est utilisée en l'absence de point d'eau. Les ongles restent courts et sans vernis."),
        "dechets": ("Élimination des déchets d'activités de soins", "Les aiguilles et objets tranchants sont déposés dans un collecteur homologué. Le collecteur est fermé aux deux tiers de sa capacité et enlevé par le prestataire agréé."),
        "rgpd": ("Protection des données", "Les dossiers sont conservés sur un poste protégé par mot de passe. Aucun envoi de données personnelles ne se fait par messagerie non chiffrée. Toute demande d'accès est consignée au registre."),
        "accueil": ("Procédure d'accueil", "Le patient est accueilli par le secrétariat, qui vérifie son identité et sa couverture. En cas de doute sur une homonymie, un second élément (date de naissance, adresse) est demandé."),
        "froid": ("Chaîne du froid des vaccins", "Le réfrigérateur est relevé matin et soir, la température doit rester entre 2 et 8 degrés. Toute alarme est signalée au médecin responsable."),
        "incendie": ("Consignes incendie", "Les extincteurs sont contrôlés une fois par an. L'issue de secours donne sur la cour arrière et ne doit jamais être encombrée."),
    }
    title, body = banks[topic]
    doc.title = title
    doc.name_hint = f"procedure-{topic}"
    doc.paragraph(body)
    doc.paragraph(f"Procédure validée par {doc.person(doc.cast.owner, 'title_full')} le {fr_date(doc.date)}.")
    doc.facts["topic"] = topic
    doc.prefer = "any"


def attestation_assurance(doc: Doc, insurer: Entity, number: str) -> None:
    rng = doc.rng
    doc.country = insurer.country
    doc.title = "Attestation d'assurance responsabilité civile professionnelle"
    doc.name_hint = "attestation-assurance-rc"
    doc.paragraph(
        f"{doc.org(insurer)} atteste que {doc.person(doc.cast.owner, 'title_full')}, {doc.org(doc.cast.own_org)}, "
        f"est couverte par le contrat n° {doc.ident('contract', number)} au titre de la responsabilité civile professionnelle, "
        f"pour la période du {fr_date(doc.date)} au {fr_date(doc.date.replace(year=doc.date.year + 1))}."
    )
    doc.paragraph(f"Plafond de garantie : {money(rng.choice([3000000, 5000000, 8000000]), insurer.country)} par sinistre.")
    doc.prefer = "pdf"


def courrier_pharmacie(doc: Doc, pharmacy: Entity, patient: Entity | None = None) -> None:
    doc.title = "Courrier de la pharmacie"
    doc.name_hint = "courrier-pharmacie"
    doc.paragraph(f"{doc.org(pharmacy)}\n12 place de l'Hôtel de Ville, 74100 Annemasse")
    body = f"À l'attention de {doc.person(doc.cast.owner, 'title_full')}. Nous vous informons d'une rupture de stock sur {doc.rng.choice(pools.MEDICATIONS)} jusqu'à la fin du mois."
    if patient is not None:
        doc.subject(patient)
        body += f" Le traitement de {doc.person(patient, 'title_full')} devra être adapté."
    doc.paragraph(body)
    doc.prefer = "pdf"


def releve_garde(doc: Doc, doctors: list) -> None:
    doc.title = f"Planning de garde — {MONTHS[doc.date.month - 1]} {doc.date.year}"
    doc.name_hint = "planning-garde"
    lines = []
    for week, e in enumerate(doctors, start=1):
        lines.append(f"Semaine {week} : {doc.person(e, 'title_surname' if week % 2 else 'title_full')}")
    doc.paragraph("\n".join(lines))
    doc.prefer = "txt"


def transcription(doc: Doc, attendees: list, ambiguous: bool = True) -> None:
    rng = doc.rng
    cast = doc.cast
    doc.title = f"Transcription de la réunion du {fr_date(doc.date)}"
    doc.name_hint = "transcription-reunion"
    paul = cast.by_id["d-martin-paul"]
    marchetti = cast.owner
    lines = [
        (marchetti, "Bonjour à tous, commençons par le planning des permanences."),
        (paul, "Je peux prendre la semaine du quinze, mais pas celle du vingt-deux."),
        (cast.by_id["s-colas"], "Je note. Pour la commande de gants, le fournisseur a augmenté ses prix."),
        (marchetti, f"Dr Martin, pouvez-vous voir avec {doc.org(cast.org('medsupply'))} ?"),
        (paul, "Oui, je les appelle demain."),
    ]
    start = 0
    for speaker, text in lines:
        end = start + rng.randint(4, 12)
        label = speaker.canonical if speaker.role != "doctor" else f"{speaker.title or 'Dr'} {speaker.family}"
        doc.cues.append((f"00:{start // 60:02d}:{start % 60:02d}.000", f"00:{end // 60:02d}:{end % 60:02d}.000", label, text))
        doc.mentions.append({"entity": speaker.id, "text": label, "form": "full" if speaker.role != "doctor" else "title_surname", "where": "text"})
        start = end + 1
    doc.mentions.append({"entity": "d-martin-paul", "text": "Dr Martin", "form": "title_surname", "where": "text"})
    doc.prefer = "vtt"


def presentation(doc: Doc, topic: str) -> None:
    doc.title = f"Formation — {topic}"
    doc.name_hint = "formation-" + topic.split()[0].lower()
    owner = doc.person(doc.cast.owner, "title_full")
    doc.slides = [
        (topic, [f"Intervenant : {owner}", doc.org(doc.cast.org("ordre-74"))]),
        ("Objectifs", ["Connaître la procédure", "Savoir à qui s'adresser", f"Contact : {doc.org(doc.cast.org('ars'))}"]),
        ("Rappel", ["Vérifier l'identité du patient", "Consigner la demande au registre"]),
    ]
    doc.prefer = "slides"


def horaires_medecin(doc: Doc, doctor: Entity, place: str, weekday: str, hours: str) -> None:
    """A one-line schedule file, the kind a practice names after the person (`pierre-martin.txt`)."""
    doc.title = ""
    doc.name_hint = "horaires"
    doc.paragraph(f"{doc.person(doctor, 'title_full')} reçoit le {weekday}, de {hours}, {place}.")
    doc.facts["weekday"] = weekday
    doc.prefer = "txt"


def commandes_tableau(doc: Doc, suppliers: list) -> None:
    """A table of orders saved among the documents: a spreadsheet in the wrong folder. The document pipeline does
    not read it (the interface says « tableur, à placer dans le dossier des données »)."""
    rng = doc.rng
    doc.title = "Commandes du trimestre"
    doc.name_hint = "commandes"
    rows = [["Date", "Fournisseur", "Article", "Quantité", "Montant"]]
    for _ in range(12):
        supplier = choice(rng, suppliers)
        day = doc.date - dt.timedelta(days=rng.randint(0, 60))
        rows.append([day.isoformat(), doc.org(supplier), choice(rng, ["gants", "compresses", "papier", "encre", "thermomètre"]), rng.randint(1, 20), round(rng.uniform(20, 400), 2)])
    doc.table = rows
    doc.prefer = "table"


def facture_patient(doc: Doc, patient: Entity, variant: str = "plain") -> None:
    rng = doc.rng
    doc.title = f"Note d'honoraires {doc.date.year}"
    doc.subject(patient)
    doc.name_hint = "note-honoraires"
    doc.named_after = [patient.id]
    doc.paragraph(letterhead(doc))
    doc.paragraph(f"Patient : {name_in_text(doc, patient, variant, True)}")
    amount = rng.choice([30, 30, 55, 60, 100])
    doc.facts["amount"] = money(amount, "FR")
    doc.paragraph(f"Consultation du {fr_date(doc.date)} : {doc.facts['amount']}. Règlement par carte bancaire.")
    doc.prefer = "pdf"


# --------------------------------------------------------------------------------------------- long documents

_SENTENCES = {
    "hygiene": [
        "Le lavage des mains reste le geste le plus efficace pour limiter la transmission des infections.",
        "Les surfaces de contact sont désinfectées entre deux patients avec un produit virucide.",
        "Le matériel réutilisable est nettoyé, rincé, puis stérilisé selon un cycle validé et tracé.",
        "Les gants à usage unique sont changés entre chaque patient et jamais lavés.",
        "Le linge d'examen jetable est éliminé dans la filière des déchets assimilés aux ordures ménagères.",
    ],
    "dechets": [
        "Les déchets d'activités de soins à risque infectieux sont triés à la source, dans des emballages dédiés.",
        "Le collecteur d'objets piquants est fermé définitivement lorsqu'il atteint le trait de remplissage.",
        "L'enlèvement est assuré par un prestataire agréé, qui délivre un bordereau de suivi à conserver.",
        "Les médicaments périmés sont rapportés à la pharmacie partenaire et ne sont jamais jetés avec les déchets ménagers.",
    ],
    "donnees": [
        "Les dossiers des patients sont conservés pendant la durée légale, puis détruits de façon sécurisée.",
        "L'accès au poste de secrétariat est protégé par un mot de passe personnel, renouvelé régulièrement.",
        "Toute demande d'accès ou de rectification est consignée au registre et traitée dans le mois.",
        "Les sauvegardes sont chiffrées et conservées dans un second lieu, distinct du cabinet.",
        "Les transmissions vers les confrères passent par une messagerie sécurisée de santé.",
    ],
    "accueil": [
        "L'accueil vérifie l'identité du patient, sa couverture sociale et les coordonnées du médecin traitant.",
        "En cas de retard supérieur à vingt minutes, le rendez-vous est reporté si le planning l'exige.",
        "Les patients résidant en Suisse présentent leur carte d'assuré et leur numéro AVS lorsqu'il est demandé.",
        "Les homonymes font l'objet d'une vérification de la date de naissance avant tout enregistrement.",
    ],
    "facturation": [
        "La feuille de soins est établie à la fin de la consultation et transmise à l'organisme compétent.",
        "Les factures aux assurés suisses suivent le tarif en vigueur et mentionnent le numéro de police.",
        "Les impayés sont relancés après trente jours, puis transmis au comptable à soixante jours.",
        "Les justificatifs sont classés par mois, avec le numéro de facture en référence.",
    ],
    "urgences": [
        "Le chariot d'urgence est vérifié chaque mois : date de péremption, charge du défibrillateur, oxygène.",
        "En cas de malaise grave, le secrétariat compose le 15, ou le 144 pour les patients en Suisse.",
        "Chaque membre de l'équipe suit une remise à niveau aux gestes de premiers secours tous les deux ans.",
    ],
    "materiel": [
        "Le parc informatique est renouvelé tous les cinq ans, sur la base d'un devis comparatif.",
        "Les appareils de mesure sont étalonnés une fois par an par un organisme agréé.",
        "Les contrats de maintenance sont revus chaque année avant leur échéance, pour éviter la reconduction tacite.",
    ],
}


def long_document(doc: Doc, pages: int, needles: list, title: str, name_hint: str) -> None:
    """About `pages` pages of procedures, three paragraphs a page. `needles`: (page, sentence, [(entity, form,
    placeholder)]) planted on purpose, so a question has a known place to be answered from."""
    rng = doc.rng
    doc.title = title
    doc.name_hint = name_hint
    topics = sorted(_SENTENCES)
    chapter = 0
    for page in range(1, pages + 1):
        if page % 6 == 1:
            chapter += 1
            doc.paragraph(f"Chapitre {chapter} — {choice(rng, topics).capitalize()}")
        for slot in range(3):
            sentences = []
            for _ in range(rng.randint(9, 12)):
                topic = choice(rng, topics)
                sentences.append(choice(rng, _SENTENCES[topic]))
            text = " ".join(sentences)
            if slot == 0:
                for n_page, sentence, mentions in needles:
                    if n_page == page:
                        rendered = sentence
                        for entity_id, form, placeholder in mentions:
                            entity = doc.cast.by_id[entity_id]
                            writer = doc.org if entity.type == "organization" else doc.person
                            rendered = rendered.replace(placeholder, writer(entity, form))
                        text += " " + rendered
                        doc.facts[f"needle_page_{n_page}"] = sentence
            doc.paragraph(text)
    doc.prefer = "long"


def sentences_count() -> int:
    return sum(len(v) for v in _SENTENCES.values())
