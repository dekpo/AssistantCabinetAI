"""Questions and what they must find.

Every question carries its category (the groups of the lot 11 brief), the selection it is asked in, the entities
it is *really* about (the generator knows, because it wrote the text), the documents and workbooks that hold the
answer, and what the knowledge base is expected to do with it. Nothing here is judged by eye: the automated run
derives every figure from this file and `documents.json`.

`expect.resolution`:

    resolved    one clear entity: the base may narrow, and must keep every relevant file it can link
    homonym     two people with exactly the same full name: the base files them as one, the user splits them
    ambiguous   several entities fit the words: the base must NOT narrow (invariant of lot 6)
    possible    only a spelling variant matches: the base may suggest, must not narrow on it alone
    none        nothing the base knows: the old path on the whole selection runs
    named_file  the question names a file: never narrowed (I2)
    folder      a question about the folder itself, answered without the documents
    tolerant    the words may or may not be linked; whatever happens, no relevant file may be lost if it narrows
"""

from __future__ import annotations

import random

from .cast import Cast, Entity, fold, person_forms

KEYWORDS = [
    "LAMal", "franchise", "quote-part", "photocopieuse", "incendie", "aiguilles", "hygiène", "lavage des mains",
    "chaîne du froid", "armoire à pharmacie", "homonymie", "Escherichia coli", "Staphylococcus aureus",
    "Streptococcus pyogenes", "Klebsiella pneumoniae", "Candida albicans", "SARS-CoV-2", "tacite reconduction",
    "indemnités journalières", "responsabilité civile", "Genève", "Lausanne", "Nyon", "Annecy", "Lyon", "Fribourg",
    "Neuchâtel", "Morges", "Vevey", "Sion", "Thonon-les-Bains", "Annemasse", "metformine", "ramipril",
]


def docs_naming(records: list, ids: list, text_only: bool = False) -> list:
    wanted = set(ids)
    out = []
    for r in records:
        hit = any(
            m["entity"] in wanted and (m["where"] == "text" or not text_only)
            for m in r["mentions"]
        ) or any(a in wanted for a in r["about"])
        if hit:
            out.append(r["path"])
    return out


def readable_only(records_by_path: dict, paths: list) -> list:
    return [p for p in paths if records_by_path[p]["readable"]]


def books_naming(books: list, ids: list) -> list:
    wanted = set(ids)
    return [b.path for b in books if b.readable and any(m["entity"] in wanted for m in b.mentions)]


def build_questions(library, books) -> list:
    cast: Cast = library.cast
    records = library.records
    by_path = {r["path"]: r for r in records}
    tier_of = {r["path"]: r["tier"] for r in records}
    rng = random.Random(f"questions-{library.seed}")
    questions: list = []
    counter = {"n": 0}

    def add(category: str, text: str, entities=(), name_entities=None, resolution="resolved", between=(),
            selection=None, contains=(), relevant=None, workbooks=None, follow_up_of=None, carry_over=(),
            note="", expected_value=None, risk="") -> dict | None:
        ids = list(entities)
        name_ids = list(name_entities) if name_entities is not None else ids
        if relevant is None:
            relevant = docs_naming(records, name_ids) if name_ids else []
        relevant_readable = readable_only(by_path, relevant)
        if workbooks is None:
            workbooks = books_naming(books, name_ids) if name_ids else []
        min_tier = min([tier_of[p] for p in relevant_readable] + [10 ** 6]) if relevant_readable else 30
        if resolution in ("resolved", "homonym", "tolerant") and not relevant_readable and not workbooks and category not in ("unknown",):
            return None
        counter["n"] += 1
        question = {
            "id": f"Q{counter['n']:03d}",
            "category": category,
            "text": text,
            "entities": ids,
            "name_entities": name_ids,
            "relevant_documents": relevant,
            "relevant_readable": relevant_readable,
            "relevant_workbooks": workbooks,
            "selection": selection or {"documents": "all", "workbooks": "all"},
            "expect": {"resolution": resolution, "between": list(between), "contains": list(contains)},
            "min_tier": min_tier if min_tier < 10 ** 6 else 30,
            "follow_up_of": follow_up_of,
            "carry_over": list(carry_over),
        }
        if note:
            question["note"] = note
        if risk:
            question["risk"] = risk
        if expected_value is not None:
            question["expected_value"] = expected_value
        questions.append(question)
        return question

    people = {e.id: e for e in cast.people}
    forms = {e.id: person_forms(e) for e in cast.people if e.type == "person"}
    org = cast.org

    def exists(eid: str) -> bool:
        return bool(docs_naming(records, [eid]))

    # ---- entity-free ------------------------------------------------------------------------------------------
    add("entity_free", "Combien de documents ai-je ?", resolution="folder")
    add("entity_free", "Quel est le dernier courrier reçu ?", resolution="none")
    add("entity_free", "Quels sont les horaires d'ouverture du cabinet ?", resolution="none", relevant=[])
    for text, word in (
        ("Que dit la procédure sur le lavage des mains ?", "lavage des mains"),
        ("Comment éliminer les aiguilles usagées ?", "aiguilles"),
        ("Quelles sont les consignes en cas d'incendie ?", "incendie"),
        ("Qu'est-ce qui est prévu pour la chaîne du froid des vaccins ?", "chaîne du froid"),
        ("Que dit le contrat de la photocopieuse ?", "photocopieuse"),
    ):
        paths = [r["path"] for r in records if word in r.get("terms", [])]
        if paths:
            add("entity_free", text, resolution="none", relevant=paths, note="the answer is a topic, not a name")
    for text, word in (
        ("Quels documents mentionnent Escherichia coli ?", "Escherichia coli"),
        ("Quels résultats parlent de Staphylococcus aureus ?", "Staphylococcus aureus"),
        ("Y a-t-il un compte rendu qui cite Klebsiella pneumoniae ?", "Klebsiella pneumoniae"),
        ("Quels documents parlent de la LAMal ?", "LAMal"),
        ("Quels courriers évoquent la franchise annuelle ?", "franchise"),
        ("Quels documents viennent de Genève ?", "Genève"),
        ("Quels courriers sont liés à Lausanne ?", "Lausanne"),
        ("Y a-t-il des documents qui parlent de Nyon ?", "Nyon"),
        ("Quels documents mentionnent Annecy ?", "Annecy"),
    ):
        paths = [r["path"] for r in records if word in r.get("terms", [])]
        if paths:
            add("term", text, resolution="none", relevant=paths, note="an organism, a word or a place: no entity in the base")

    # ---- persons (patients and doctors), the plain full name --------------------------------------------------------
    person_ids = [
        "p-dupont-marie", "p-dupontet-jean", "p-dupont-jean-pierre", "p-martin-marie", "p-martin-claire", "p-martin-luc",
        "p-favre-laurent", "p-fabre-laurence", "p-monnier-veronique", "p-meunier-veronique", "p-girardin-fabienne",
        "p-girard-frederic", "p-lucas-roy", "p-roy-lucas", "p-thomas-pierre", "p-pierre-thomas", "p-rochat-sophie",
        "p-cuendet-mathieu", "p-pittet-isabelle", "p-chappuis-nathalie", "p-maillard-pascal", "p-bovet-celine",
        "p-jaccard-alain", "p-deleze-christelle", "p-crettenand-gregoire", "p-vuilleumier-marie-claude",
        "p-python-jean-claude", "p-oberson-pierre-alain", "p-genoud-anne-laure", "p-tschopp-raphael",
        "p-zufferey-stephane", "p-benali-fatima", "p-el-khoury-mohamed", "p-nguyen-thi-lan", "p-kowalski-andrzej",
        "p-dos-santos-ferreira-maria", "p-demirci-ibrahim", "p-petrovic-elena", "p-damico-giuseppe",
        "p-ndiaye-aicha", "p-obrien-sean", "p-muller-jorg", "p-hasanovic-anna", "p-bernasconi-luca",
        "p-le-gall-laurence", "p-de-la-fontaine-matthieu", "p-daubigne-camille", "p-luthi-zoe", "p-lindqvist-asa",
    ]
    for pid in person_ids:
        e = people[pid]
        if not exists(pid):
            continue
        template = rng.choice([
            "Quels documents concernent {n} ?", "Que dit le courrier de {n} ?", "Résume le dossier de {n}.",
            "Y a-t-il des résultats d'analyses pour {n} ?", "Quels documents parlent de {n} ?",
        ])
        add("person", template.format(n=e.canonical), [pid], note="the full name of a patient")

    # ---- the Dupont family and the homonym ---------------------------------------------------------------------------
    jean = ["p-dupont-jean", "d-dupont-jean"]
    add("homonym", "Quels documents concernent Jean Dupont ?", ["p-dupont-jean"], name_entities=jean, resolution="homonym",
        note="a patient and a cardiologist of Geneva have exactly this name; the base files them as one person")
    add("homonym", "Que dit le courrier du Dr Jean Dupont ?", ["d-dupont-jean"], name_entities=jean, resolution="homonym")
    add("homonym", "Quel est le prochain rendez-vous de Jean Dupont ?", ["p-dupont-jean"], name_entities=jean, resolution="homonym",
        contains=[r["facts"]["next_visit"] for r in records if "p-dupont-jean" in r["about"] and "next_visit" in r["facts"]][:1])
    add("person", "Quels documents concernent Jean-Pierre Dupont ?", ["p-dupont-jean-pierre"], note="a longer given name")
    add("person", "Que dit le dossier de Jean Dupontet ?", ["p-dupontet-jean"], note="a surname one syllable longer than Dupont")
    add("person", "Que dit le dossier de Marie Dupont ?", ["p-dupont-marie"])
    add("person", "Résume les courriers de Madame Marie Martin.", ["p-martin-marie"], note="has a social security number AND an AVS number")
    add("person", "Quels documents concernent Claire Martin ?", ["p-martin-claire"])

    # ---- aliases ------------------------------------------------------------------------------------------------------
    dupont_all = ["p-dupont-jean", "d-dupont-jean", "p-dupont-jean-pierre"]
    add("alias", "Que dit J. Dupont ?", ["p-dupont-jean"], name_entities=dupont_all, resolution="ambiguous",
        between=dupont_all, note="the initial J fits Jean, Jean-Pierre and the cardiologist")
    add("alias", "Que dit DUPONT Jean ?", ["p-dupont-jean"], name_entities=jean, resolution="homonym",
        note="the administrative order surname first")
    add("alias", "Que dit Jean-Dupont ?", ["p-dupont-jean"], name_entities=jean, resolution="tolerant",
        note="a hyphen typed between the given name and the surname")
    add("alias", "Que dit Dupont J ?", ["p-dupont-jean"], name_entities=dupont_all, resolution="tolerant",
        note="the form of the workbooks, surname then initial")
    add("alias", "Que dit M. Dupont ?", ["p-dupont-jean"], name_entities=dupont_all + ["p-dupont-marie"], resolution="ambiguous",
        between=dupont_all, note="a weak title and a shared surname")
    add("alias", "Que dit Dupont ?", ["p-dupont-jean"], name_entities=dupont_all + ["p-dupont-marie"], resolution="ambiguous",
        between=dupont_all + ["p-dupont-marie"], note="the surname alone")
    add("alias", "Que dit Mme Martin ?", ["p-martin-marie"], name_entities=["p-martin-marie", "p-martin-claire"],
        resolution="ambiguous", between=["p-martin-marie", "p-martin-claire"])
    add("alias", "Que dit Favre ?", ["p-favre-laurent"], name_entities=["p-favre-laurent", "d-favre-olivier"],
        resolution="ambiguous", between=["p-favre-laurent", "d-favre-olivier"], note="a patient and an ENT specialist")
    for pid in ("p-damico-giuseppe", "p-ndiaye-aicha", "p-obrien-sean", "p-daubigne-camille", "p-de-la-fontaine-matthieu", "p-petrovic-elena", "p-dos-santos-ferreira-maria"):
        if exists(pid):
            e = people[pid]
            from .cast import ascii_keep_case

            text = ascii_keep_case(e.canonical) if pid in ("p-petrovic-elena", "p-daubigne-camille") else e.canonical
            add("alias", f"Que dit le dossier de {text} ?", [pid], note="particle, apostrophe or accent in the name" )

    # ---- practitioners --------------------------------------------------------------------------------------------------
    add("practitioner", "Quel courrier concerne le Dr Martin ?", ["d-martin-paul"], name_entities=["d-martin-paul", "d-martin-pierre"],
        resolution="ambiguous", between=["d-martin-paul", "d-martin-pierre"], note="two doctors Martin in the library")
    add("practitioner", "Qu'a dit le Dr Martin ?", ["d-martin-paul"], name_entities=["d-martin-paul", "d-martin-pierre"],
        resolution="ambiguous", between=["d-martin-paul", "d-martin-pierre"])
    for did, text in (
        ("d-martin-pierre", "Quels courriers viennent du Dr Pierre Martin ?"),
        ("d-martin-paul", "Quels documents concernent le Dr Paul Martin ?"),
        ("d-vuagniaux-anne-sophie", "Que dit la Dre Vuagniaux ?"),
        ("d-vuagniaux-anne-sophie", "Quels courriers a écrit la Dre Anne-Sophie Vuagniaux ?"),
        ("d-bersier-marc", "Quels courriers viennent du Dr Marc Bersier ?"),
        ("d-cuenoud-philippe", "Que dit le Dr Cuénoud ?"),
        ("d-grandjean-yann", "Quels documents concernent le Dr Yann Grandjean ?"),
        ("d-rapin-jean-luc", "Que dit le Pr Rapin ?"),
        ("d-haddad-karim", "Quels courriers viennent du Dr Haddad ?"),
        ("d-gaillard-eric", "Que dit le Dr Gaillard ?"),
        ("d-favre-olivier", "Que dit le Dr Olivier Favre ?"),
        ("d-roche-bernard", "Quels documents concernent le Dr Roche ?"),
    ):
        if exists(did):
            add("practitioner", text, [did], note="title and surname" if "Dr" in text and len(text.split()) < 6 else "")
    if exists("o-marchetti"):
        add("practitioner", "Quels documents concernent le Dr Hélène Marchetti ?", ["o-marchetti"], resolution="tolerant",
            note="the user's own name is in every letter: an entity that is not discriminating")

    # ---- organisations ----------------------------------------------------------------------------------------------------
    for key, text in (
        ("hug", "Quels documents concernent les Hôpitaux universitaires de Genève ?"),
        ("hug", "Quels documents concernent HUG ?"),
        ("chuv", "Quels courriers viennent du CHUV ?"),
        ("chuv", "Quels documents concernent le Centre hospitalier universitaire vaudois ?"),
        ("cpam74", "Quels courriers de la CPAM de la Haute-Savoie avons-nous ?"),
        ("helsana", "Quels documents concernent Helsana ?"),
        ("css", "Quels courriers viennent de la CSS Assurance ?"),
        ("groupe-mutuel", "Que dit le Groupe Mutuel ?"),
        ("medsupply", "Quelles factures viennent de MedSupply ?"),
        ("papeterie-lac", "Quels documents concernent la Papeterie du Lac ?"),
        ("bureautique-leman", "Quels contrats avons-nous avec Bureautique Léman ?"),
        ("unilabs", "Quels résultats viennent d'Unilabs ?"),
        ("cerballiance", "Quels résultats viennent de Cerballiance ?"),
        ("change", "Quels courriers viennent du Centre hospitalier Annecy Genevois ?"),
        ("clinique-source", "Quels documents concernent la Clinique La Source ?"),
        ("urssaf", "Quels documents concernent l'URSSAF ?"),
        ("ars", "Quels documents concernent l'ARS Auvergne-Rhône-Alpes ?"),
        ("suva", "Quels documents concernent la SUVA ?"),
        ("etude-chappuis", "Quels courriers viennent de l'Étude Chappuis & Associés ?"),
    ):
        if exists(f"o-{key}"):
            add("organization", text, [f"o-{key}"], note="acronym only" if text.endswith("HUG ?") else "")
    for key, text, note in (
        ("pharmacie-dupont", "Quels documents concernent la Pharmacie Dupont ?", "an organisation that carries a surname of the library"),
        ("dupont-immobilier", "Que dit Dupont Immobilier SA ?", "an organisation that carries a surname of the library"),
        ("garage-martin", "Quelles factures viennent du Garage Martin & Fils ?", "an organisation that carries a surname of the library"),
    ):
        if exists(f"o-{key}"):
            add("org_vs_person", text, [f"o-{key}"], note=note)

    # ---- multi-entity --------------------------------------------------------------------------------------------------------
    pairs = []
    for r in records:
        people_in = [m["entity"] for m in r["mentions"] if m["where"] == "text" and m["entity"].startswith(("p-", "d-")) and m["entity"] != "o-marchetti"]
        uniq = list(dict.fromkeys(people_in))
        if len(uniq) >= 2 and r["readable"]:
            pairs.append((uniq[0], uniq[1], r["path"]))
    seen_pairs = set()
    for a, b, path in pairs:
        if (a, b) in seen_pairs or a in jean or b in jean:
            continue
        seen_pairs.add((a, b))
        ea, eb = cast.by_id[a], cast.by_id[b]
        both = [p for p in docs_naming(records, [a]) if p in docs_naming(records, [b])]
        add("multi_entity", f"Quelle correspondance existe entre {ea.canonical} et {eb.canonical} ?", [a, b], relevant=both,
            note="both names in the question: the files that hold both", resolution="resolved")
        if len(seen_pairs) >= 6:
            break

    # ---- identifiers ------------------------------------------------------------------------------------------------------------
    for number in ("FAC-2026-014",):
        ent = cast.identifiers.get(number)
        if ent is not None:
            add("identifier", f"Montre la facture {number}", [ent.id], contains=[a for r in records if ent.id in [m["entity"] for m in r["mentions"]] for a in [r["facts"].get("total_ttc", "")] if a][:1])
    invoice_ids = [e for e in cast.identifiers.values() if e.kind == "invoice"][:4]
    for ent in invoice_ids[1:4]:
        add("identifier", f"Quel est le montant de la facture {ent.canonical} ?", [ent.id],
            contains=[a for r in records if ent.id in [m["entity"] for m in r["mentions"]] for a in [r["facts"].get("total_ttc", "")] if a][:1])
    for ent in [e for e in cast.identifiers.values() if e.kind == "contract"][:3]:
        add("identifier", f"Que dit le contrat {ent.canonical} ?", [ent.id])
    for ent in [e for e in cast.identifiers.values() if e.kind == "avs"][:3]:
        add("identifier", f"À qui appartient le numéro AVS {ent.canonical} ?", [ent.id], note="a personal identifier: stored as a keyed hash")
    for ent in [e for e in cast.identifiers.values() if e.kind == "nir"][:3]:
        add("identifier", f"À qui appartient le numéro de sécurité sociale {ent.canonical} ?", [ent.id], note="a personal identifier: stored as a keyed hash")
    for ent in [e for e in cast.identifiers.values() if e.kind == "policy"][:2]:
        add("identifier", f"Quel document porte la police n° {ent.canonical} ?", [ent.id])
    for ent in [e for e in cast.identifiers.values() if e.kind == "iban"][:2]:
        add("identifier", f"Quelle facture porte l'IBAN {ent.canonical} ?", [ent.id], note="an IBAN: stored as a keyed hash")
    for ent in [e for e in cast.identifiers.values() if e.kind == "email"][:2]:
        add("identifier", f"Quel courrier vient de l'adresse {ent.canonical} ?", [ent.id], note="an e-mail address: stored as a keyed hash")
    for ent in [e for e in cast.identifiers.values() if e.kind == "phone"][:2]:
        add("identifier", f"À qui est le numéro de téléphone {ent.canonical} ?", [ent.id], note="a telephone number")
    add("unknown", "Montre la facture FAC-2026-999", resolution="none", note="a number that exists nowhere")

    # ---- unknown entities ------------------------------------------------------------------------------------------------------
    for text in (
        "Que dit le dossier de Madame Zénobie Quarante ?",
        "Quels documents concernent Hyacinthe Vaudroz ?",
        "Que dit le Dr Ernest Blondeau ?",
        "Quels courriers viennent de la Boulangerie Clochette SARL ?",
        "Quels documents concernent Léopold Tschumi-Bauer ?",
        "Que dit la clinique des Mimosas ?",
    ):
        add("unknown", text, resolution="none", relevant=[], note="a name that is nowhere in the library: the answer is that the documents do not say")

    # ---- named files ----------------------------------------------------------------------------------------------------------------
    for r in records:
        if r["special"] in ("named-after-person", "long") or r["index"] in (2, 8, 11):
            if r["readable"]:
                add("named_file", f"Que dit {r['path'].split('/')[-1]} ?", resolution="named_file", relevant=[r["path"]],
                    note="the file name carries a person's name" if r["special"] == "named-after-person" else "")
    add("named_file", "Que dit le fichier dupont-jean.pdf ?", ["p-dupont-jean"], resolution="named_file",
        relevant=[r["path"] for r in records if r["path"] == "dupont-jean.pdf"], risk="KBD-18")

    # ---- long documents -----------------------------------------------------------------------------------------------------------------
    for r in records:
        for key, needle in r.get("needles", {}).items():
            if "question" in needle:
                add("long_document", needle["question"], [], resolution="none", relevant=[r["path"]], contains=needle["contains"],
                    note=f"planted on page {needle['page'] or '?'} of a {r['pages']}-page document")

    # ---- scans (OCR) ----------------------------------------------------------------------------------------------------------------------
    for r in records:
        if r["ocr"] and r["about"] and len(r["about"]) == 1:
            e = cast.by_id[r["about"][0]]
            if e.type == "person" and e.designed:
                add("ocr", f"Que dit le courrier de {e.canonical} ?", [e.id], relevant=docs_naming(records, [e.id]),
                    note=f"the answer is in a scanned document ({r['format']}); text comes from OCR")
        if sum(1 for q in questions if q["category"] == "ocr") >= 6:
            break

    # ---- data (workbooks) ---------------------------------------------------------------------------------------------------------------------
    rdv = next((b for b in books if b.path == "rendez-vous-2026.xlsx"), None)
    if rdv is not None:
        by_entity: dict = {}
        for m in rdv.mentions:
            if m["column"] == "Patient":
                by_entity.setdefault((m["entity"], m["sheet"]), []).append((m["form"], len(m["rows"])))
        for (eid, sheet), rows in sorted(by_entity.items()):
            total = sum(n for _, n in rows)
            if total >= 2 and eid.startswith("p-"):
                e = cast.by_id[eid]
                add("data", f"Combien de rendez-vous pour {e.canonical} en {sheet.lower()} ?", [eid], relevant=[],
                    workbooks=["rendez-vous-2026.xlsx"], expected_value=total,
                    note="the cells write the name in several forms (full, surname-given, surname-initial)")
            if sum(1 for q in questions if q["category"] == "data") >= 7:
                break
    add("data", "Combien de lignes dans factures-fournisseurs-2026.xlsx ?", resolution="none", workbooks=["factures-fournisseurs-2026.xlsx"], relevant=[],
        note="a count of rows in a named workbook")
    add("data", "Quelle est la somme des montants HT pour MedSupply SA ?", ["o-medsupply"], relevant=[],
        workbooks=["factures-fournisseurs-2026.xlsx"], note="a supplier column")
    add("data", "Combien de patients de Fribourg dans patients-ch.xlsx ?", resolution="none", relevant=[], workbooks=["patients-ch.xlsx"],
        note="a commune column: places are not entities in the base")

    # ---- mixed (documents and workbooks) -------------------------------------------------------------------------------------------------------------
    for eid, text in (
        ("p-dupont-jean", "Compare les rendez-vous de septembre de Jean Dupont avec la correspondance qui le concerne."),
        ("p-martin-marie", "Que disent les documents et le tableau des rendez-vous à propos de Marie Martin ?"),
        ("p-rochat-sophie", "Quels documents et quels rendez-vous concernent Sophie Rochat ?"),
    ):
        add("mixed", text, [eid], name_entities=jean if eid == "p-dupont-jean" else [eid], resolution="homonym" if eid == "p-dupont-jean" else "resolved",
            risk="KBD-21..24 can intervene before the knowledge base is consulted")

    # ---- follow-ups ----------------------------------------------------------------------------------------------------------------------------------------
    first = add("person", "Que dit le courrier de Sophie Rochat ?", ["p-rochat-sophie"], note="first question of a conversation")
    if first:
        add("follow_up", "Et son numéro AVS ?", ["p-rochat-sophie"], follow_up_of=first["id"], carry_over=["p-rochat-sophie"], resolution="tolerant",
            note="a short question that names nobody: read as a follow-up of the previous answer")
        notes = [r["path"] for r in records if r["kind"] == "note_interne"]
        add("follow_up", "Que contiennent les notes internes ?", [], follow_up_of=first["id"], carry_over=["p-rochat-sophie"], resolution="none",
            relevant=notes, risk="KBD-19",
            note="a NEW short question that names nobody; the previous person must not narrow it")

    # ---- safety: an explicit selection that leaves a relevant file out --------------------------------------------------------
    readable_paths = [r["path"] for r in records if r["readable"]]
    for pid in ("p-rochat-sophie", "p-martin-marie", "p-dupont-marie", "p-favre-laurent", "p-cuendet-mathieu", "p-monnier-veronique",
                "p-benali-fatima", "p-girard-frederic", "p-petrovic-elena", "p-muller-jorg", "d-martin-pierre", "d-vuagniaux-anne-sophie"):
        wanted = readable_only(by_path, docs_naming(records, [pid]))
        if len(wanted) < 3:
            continue
        left_out = wanted[-1]
        kept = wanted[:-1]
        others = [p for p in readable_paths if p not in wanted]
        noise = [others[(i * 7 + 3) % len(others)] for i in range(14)]
        noise = list(dict.fromkeys(noise))
        selected = sorted(set(kept + noise))
        e = cast.by_id[pid]
        add("leak", f"Quels documents concernent {e.canonical} ?" if e.role == "patient" else f"Que dit {e.title or 'Dr'} {e.family} ?",
            [pid], relevant=kept, workbooks=[],
            selection={"documents": {"paths": selected}, "workbooks": "none"},
            note=f"explicit selection of {len(selected)} files; the relevant file {left_out} is NOT ticked and must never be read")
        questions[-1]["left_out"] = [left_out]
    # a very small selection: nothing should change
    for pid in ("p-rochat-sophie", "p-cuendet-mathieu", "p-benali-fatima"):
        wanted = readable_only(by_path, docs_naming(records, [pid]))
        if len(wanted) >= 2:
            selected = sorted(set(wanted[:2] + [p for p in readable_paths if p not in wanted][:2]))
            e = cast.by_id[pid]
            add("small_scope", f"Que dit le courrier de {e.canonical} ?", [pid], relevant=wanted[:2], workbooks=[],
                selection={"documents": {"paths": selected}, "workbooks": "none"}, resolution="resolved",
                note="four files selected: the base must leave the search alone (fewer than 8 files)")

    # ---- appended last, so the ids of every question above never move ---------------------------------------------------------
    # two suppliers use the invoice number FAC-2026-014 (a clash a real library has): the number alone names two files
    fac = cast.identifiers.get("FAC-2026-014")
    medsupply_docs = [r["path"] for r in records if "o-medsupply" in [m["entity"] for m in r["mentions"]] and r["kind"] == "facture_fournisseur"]
    if fac is not None and medsupply_docs:
        add("identifier", "Quel est le total TTC de la facture FAC-2026-014 de MedSupply ?", [fac.id, "o-medsupply"], relevant=medsupply_docs[:1],
            contains=[r["facts"]["total_ttc"] for r in records if r["path"] == medsupply_docs[0]][:1],
            note="the number FAC-2026-014 exists twice (MedSupply and Papeterie du Lac): the supplier says which")

    return questions
