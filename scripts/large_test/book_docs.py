"""The 150 questions on documents (15 in the slice). See `book.py` for the shape of a question and `features.py`
for the features. Each generator only adds a question when the data makes its answer certain: one document for a
person and a kind, a code that exists once, a term counted from the text."""

from __future__ import annotations

import re
from collections import defaultdict

from knowledge_sandbox import docs as D
from knowledge_sandbox import questions as KQ
from knowledge_sandbox.cast import fold, person_forms

from . import book as B

CAPS_FULL = {
    "D01": 8, "D02": 10, "D03": 11, "D04": 6, "D05": 12, "D06": 4, "D07": 2, "D08": 6, "D09": 6, "D10": 6,
    "D11": 8, "D12": 6, "D13": 3, "D14": 10, "D15": 6, "D16": 6, "D17": 6, "D18": 6, "D19": 6, "D20": 6,
    "D21": 6, "D22": 6, "D23": 4,
}
CAPS_SLICE = {
    "D01": 1, "D02": 1, "D03": 1, "D05": 1, "D09": 1, "D11": 1, "D12": 1, "D14": 1,
    "D16": 1, "D17": 1, "D18": 1, "D19": 1, "D20": 1, "D21": 1, "D22": 1,
}
READER_FEATURE = {"vtt": "D16", "srt": "D17", "rtf": "D18", "eml": "D19", "odt": "D20", "odp": "D21", "pptx": "D22"}

MONTHS = D.MONTHS
DATE_PATTERN = r"\b\d{1,2}(?:er)? (?:" + "|".join(MONTHS) + r") 20\d\d\b"
REFUSAL_ROAD = ["global_retrieval", "error", "named_file", "every_document"]


# ------------------------------------------------------------------------------------------------ parsing


def iso_of_french(text: str) -> str:
    """`24 octobre 2026` or `1er mai 2027` as `2026-10-24`."""
    match = re.match(r"(\d{1,2})(?:er)? (\w+) (\d{4})", text.strip())
    month = MONTHS.index(match.group(2)) + 1
    return f"{match.group(3)}-{month:02d}-{int(match.group(1)):02d}"


def amount_of(text: str) -> float:
    """`CHF 1'109.32` or `1 450,00 €` as a number."""
    digits = re.sub(r"[^\d.,]", "", text.replace("'", "").replace(" ", "").replace(" ", ""))
    if "," in digits:
        digits = digits.replace(".", "").replace(",", ".")
    return float(digits)


def date_spellings(iso: str) -> list:
    year, month, day = iso.split("-")
    return [f"{int(day)} {MONTHS[int(month) - 1]} {year}", f"{day}/{month}/{year}", f"{day}.{month}.{year}"]


# ----------------------------------------------------------------------------------------------- the index


class Index:
    def __init__(self, b: B.Builder):
        self.b = b
        self.by_patient_kind: dict = defaultdict(list)
        for r in b.records:
            if len(r["about"]) == 1:
                self.by_patient_kind[(r["about"][0], r["kind"])].append(r)
        self.by_kind: dict = defaultdict(list)
        for r in b.records:
            self.by_kind[r["kind"]].append(r)

    def unique(self, record: dict) -> bool:
        """The only document of its kind about this person."""
        return len(record["about"]) == 1 and len(self.by_patient_kind[(record["about"][0], record["kind"])]) == 1

    def lookalikes(self, pid: str) -> list:
        """Other people of the cast who share the surname (a patient, a doctor): what is said of them must not
        reach this person."""
        cast = self.b.cast
        family = fold(cast.by_id[pid].family)
        return [e.id for e in cast.people if e.id != pid and fold(e.family) == family]


# ----------------------------------------------------------------------------- one fact about one person


def person_fact(b: B.Builder, index: Index, r: dict):
    """(text, expect, patient id, route) for a document that carries a fact about one person, or None."""
    if not index.unique(r) or r["ocr"] and r["kind"] != "lettre_confrere":
        return None
    pid = r["about"][0]
    person = b.cast.by_id[pid]
    if person.role != "patient":
        return None
    name = person_forms(person)["full"]
    facts = r["facts"]
    if r["kind"] == "lettre_confrere" and "next_visit" in facts:
        text = f"Quel est le prochain rendez-vous de {name} d'après le courrier du confrère ?"
        expect = {"date": iso_of_french(facts["next_visit"])}
    elif r["kind"] == "resultats_labo" and "hba1c" in facts:
        text = f"Quel est le taux d'hémoglobine glyquée de {name} dans ses résultats d'analyses ?"
        expect = {"value": amount_of(facts["hba1c"].replace("%", ""))}
    elif r["kind"] == "courrier_caisse" and "amount" in facts:
        text = f"Quel montant a été pris en charge pour {name} d'après le décompte de sa caisse ?"
        expect = {"value": amount_of(facts["amount"])}
    elif r["kind"] == "facture_patient" and "amount" in facts:
        text = f"Quel est le montant de la facture d'honoraires de {name} ?"
        expect = {"value": amount_of(facts["amount"])}
    elif r["kind"] == "certificat" and "days" in facts:
        text = f"Combien de jours d'arrêt figurent dans le certificat de {name} ?"
        expect = {"patterns": [rf"\b{facts['days']}\s*jours?\b"]}
    else:
        return None
    expect["sources_any"] = [r["path"]]
    return text, expect, pid


def leak_guard(b: B.Builder, index: Index, r: dict, pid: str, expect: dict) -> list:
    """The facts of the people who look like this one, when they differ from this one's: they must not appear."""
    forbidden = []
    for other in index.lookalikes(pid):
        for d in index.by_patient_kind.get((other, r["kind"]), []):
            for key in ("next_visit", "hba1c", "amount"):
                if key in d["facts"] and key in r["facts"] and d["facts"][key] != r["facts"][key]:
                    forbidden.append(d["facts"][key])
    return sorted(set(forbidden))[:2]


# ---------------------------------------------------------------------------------------------- generators


def person_facts(b: B.Builder, index: Index) -> None:
    """D01, D02, and the person questions of the readers D18, D19, D20."""
    pools = defaultdict(list)
    for r in b.records:
        if not r["readable"]:
            continue
        built = person_fact(b, index, r)
        if built is None:
            continue
        pools[r["format"]].append((r, built))
    rng = b.rng
    # The new readers first: a few questions on a format that exists nowhere else in the book.
    for fmt in ("rtf", "eml", "odt"):
        for r, (text, expect, pid) in rng.sample(pools[fmt], k=min(len(pools[fmt]), 12)):
            lookalike = bool(index.lookalikes(pid))
            expect = dict(expect, must_not=leak_guard(b, index, r, pid, expect) if lookalike else [])
            b.add("documents", READER_FEATURE[fmt], text, expect=expect, route=["global_retrieval"], documents=[r["path"]], fmt=fmt)
    # Then the ordinary formats: people with a look-alike are the D02 questions, the others D01.
    ordinary = [(r, built) for fmt, items in sorted(pools.items()) if fmt in ("pdf", "docx", "txt", "md") for r, built in items]
    rng.shuffle(ordinary)
    ordinary.sort(key=lambda item: 0 if index.lookalikes(item[1][2]) else 1)
    for r, (text, expect, pid) in ordinary:
        lookalike = bool(index.lookalikes(pid))
        expect = dict(expect, must_not=leak_guard(b, index, r, pid, expect) if lookalike else [])
        b.add("documents", "D02" if lookalike else "D01", text, expect=expect, route=["global_retrieval"], documents=[r["path"]], fmt=r["format"])
    # D01 gets the rest of its quota from people with no look-alike.
    rest = [(r, built) for r, built in ordinary if not index.lookalikes(built[2])]
    for r, (text, expect, pid) in rest:
        b.add("documents", "D01", text, expect=dict(expect, must_not=[]), route=["global_retrieval"], documents=[r["path"]], fmt=r["format"])


def identifiers(b: B.Builder, index: Index) -> None:
    """D03: an invoice, a contract, a social-security number; D23 takes an amount from the same invoices."""
    numbers: dict = defaultdict(list)
    for r in b.records:
        for m in r["mentions"]:
            if m["form"] in ("invoice", "contract", "avs", "nir") and m["where"] == "text":
                numbers[(m["form"], m["text"])].append(r)
    for (form, value), docs in sorted(numbers.items()):
        distinct = {d["path"] for d in docs}
        if len(distinct) != 1 or not docs[0]["readable"] or docs[0]["ocr"]:
            continue
        r = docs[0]
        if form == "invoice" and "total_ttc" in r["facts"]:
            expect = {"value": amount_of(r["facts"]["total_ttc"]), "sources_any": [r["path"]]}
            b.add("documents", "D03", f"Quel est le total TTC de la facture {value} ?", expect=expect, route=["global_retrieval"], documents=[r["path"]], fmt=r["format"])
        elif form == "contract" and "end" in r["facts"]:
            expect = {"date": iso_of_french(r["facts"]["end"]), "sources_any": [r["path"]]}
            b.add("documents", "D03", f"Jusqu'à quelle date court le contrat {value} ?", expect=expect, route=["global_retrieval"], documents=[r["path"]], fmt=r["format"])
        elif form in ("avs", "nir"):
            owner = b.cast.identifiers[value].note if value in b.cast.identifiers else ""
            if not owner or owner not in b.cast.by_id:
                continue
            family = b.cast.by_id[owner].family
            label = "numéro AVS" if form == "avs" else "numéro de sécurité sociale"
            expect = {"facts": [[family]], "sources_any": [r["path"]]}
            b.add("documents", "D03", f"À qui appartient le {label} {value} ?", expect=expect, route=["global_retrieval"], documents=[r["path"]], fmt=r["format"])
    # Amounts (D23): the net total of a quote or the pre-tax total of an invoice.
    for r in b.records:
        if not r["readable"] or r["ocr"]:
            continue
        if r["kind"] == "facture_fournisseur" and "total_ht" in r["facts"]:
            number = next((m["text"] for m in r["mentions"] if m["form"] == "invoice"), None)
            if number and len(numbers[("invoice", number)]) == 1:
                expect = {"value": amount_of(r["facts"]["total_ht"]), "sources_any": [r["path"]]}
                b.add("documents", "D23", f"Quel est le total hors taxes de la facture {number} ?", expect=expect, route=["global_retrieval"], documents=[r["path"]], fmt=r["format"])


def organisations(b: B.Builder, index: Index) -> None:
    """D04: a contract by its supplier, an acronym, an organisation that carries a surname."""
    by_supplier: dict = defaultdict(list)
    for r in index.by_kind["contrat"]:
        for m in r["mentions"]:
            if m["entity"].startswith("o-") and m["entity"] not in ("o-cabinet-voirons", "o-marchetti"):
                by_supplier[m["entity"]].append(r)
    for supplier, docs in sorted(by_supplier.items()):
        unique = {d["path"]: d for d in docs}
        if len(unique) == 1 and docs[0]["readable"] and "end" in docs[0]["facts"]:
            r = docs[0]
            name = b.cast.by_id[supplier].canonical
            expect = {"date": iso_of_french(r["facts"]["end"]), "sources_any": [r["path"]]}
            b.add("documents", "D04", f"Jusqu'à quelle date court le contrat avec {name} ?", expect=expect, route=["global_retrieval"], documents=[r["path"]], fmt=r["format"])
    # An organisation that carries a surname: the pharmacy, not the patient.
    for r in index.by_kind["courrier_pharmacie"]:
        pharmacy = next((m["entity"] for m in r["mentions"] if m["entity"].startswith("o-pharmacie")), None)
        if pharmacy and r["readable"] and not r["ocr"] and len(index.by_kind["courrier_pharmacie"]) <= 2:
            name = b.cast.by_id[pharmacy].canonical
            b.add("documents", "D04", f"Que dit le courrier de la {name} ?", expect={"sources_any": [r["path"]]}, route=["global_retrieval"], documents=[r["path"]], human=True, fmt=r["format"])
            break
    # An acronym: the letter of a hospital, by its initials.
    for r in index.by_kind["lettre_confrere"]:
        if not (index.unique(r) and r["readable"] and not r["ocr"] and "next_visit" in r["facts"]):
            continue
        hospital = next((m for m in r["mentions"] if m["entity"] in ("o-hug", "o-chuv") and m["where"] == "text"), None)
        pid = r["about"][0]
        if hospital and b.cast.by_id[pid].role == "patient":
            acronym = b.cast.by_id[hospital["entity"]].short
            name = person_forms(b.cast.by_id[pid])["full"]
            expect = {"date": iso_of_french(r["facts"]["next_visit"]), "sources_any": [r["path"]]}
            b.add("documents", "D04", f"Quel est le prochain rendez-vous de {name} d'après la lettre du {acronym} ?", expect=expect, route=["global_retrieval"], documents=[r["path"]], fmt=r["format"])


KEYWORD_SENTENCES = [
    "Quels documents parlent de {x} ?", "Quels documents mentionnent {x} ?", "Y a-t-il des documents qui parlent de {x} ?",
    "Combien de documents mentionnent {x} ?", "Quels documents évoquent {x} ?",
]


def set_questions(b: B.Builder) -> None:
    """D05 (a word), D06 (an identifier typed in another spelling)."""
    for k, word in enumerate(KQ.KEYWORDS):
        plain = b.docs_containing(word)
        scanned = b.docs_containing(word, with_ocr=True)
        if not 1 <= len(plain) <= 400:
            continue
        sentence = KEYWORD_SENTENCES[k % len(KEYWORD_SENTENCES)]
        expect = {"listing": plain, "listing_optional": scanned, "total": len(plain)}
        b.add("documents", "D05", sentence.format(x=word), expect=expect, route=["set_search"], documents=[], fmt="")
    spelled = 0
    for ident in sorted(b.cast.identifiers.values(), key=lambda e: e.canonical):
        if ident.kind not in ("avs", "nir"):
            continue
        holders = [r["path"] for r in b.records if r["readable"] and not r["ocr"]
                   and any(m["entity"] == ident.id and m["where"] == "text" for m in r["mentions"])]
        if not holders:
            continue
        compact = re.sub(r"\D", "", ident.canonical)
        typed = compact if spelled % 2 == 0 else ident.canonical
        b.add("documents", "D06", f"Quels documents contiennent {typed} ?",
              expect={"listing": holders, "listing_optional": [], "total": len(holders)}, route=["set_search"], documents=[])
        spelled += 1


def folder_questions(b: B.Builder) -> None:
    """D07 (the rank) and D08 (the folder itself). The truths are counted from the generated files."""
    records = b.records
    analysed = [r for r in records if r["readable"]]
    letters = [r for r in analysed if "courrier" in r["path"]]
    if letters:
        newest = max(r["date"] for r in letters)
        oldest = min(r["date"] for r in letters)
        b.add("documents", "D07", "Quel est le dernier courrier reçu ?",
              expect={"first": [r["path"] for r in letters if r["date"] == newest], "date": newest},
              route=["folder_answer:ranked_file"], documents=[])
        b.add("documents", "D07", "Quel est le courrier le plus ancien ?",
              expect={"first": [r["path"] for r in letters if r["date"] == oldest], "date": oldest},
              route=["folder_answer:ranked_file"], documents=[])
    scans = sum(1 for r in analysed if r["ocr"])
    b.add("documents", "D08", "Combien de documents ai-je ?", expect={"count_range": [len(analysed) - scans, len(analysed)]},
          route=["folder_answer:indexed_count"], documents=[],
          note="Every readable document, minus the scans the text recognition cannot read.")
    b.add("documents", "D08", "Combien de fichiers y a-t-il dans le dossier ?", expect={"count": len(records)}, route=["folder_answer:file_count"], documents=[])
    b.add("documents", "D08", "Combien de fichiers sont illisibles ?", expect={"count_range": [0, scans]}, route=["folder_answer:unreadable_count"], documents=[],
          note="Only scans can be unreadable here; how many depends on the text recognition. A format nobody reads is not counted as unreadable.")
    for noun, extensions in (("PDF", {"pdf"}), ("documents Word", {"docx"}), ("fichiers texte", {"txt"})):
        count = sum(1 for r in records if r["extension"] in extensions)
        text = f"Combien de {noun} ai-je ?" if noun != "fichiers texte" else "Combien de fichiers texte ai-je ?"
        b.add("documents", "D08", text, expect={"count": count}, route=["folder_answer:extension_count"], documents=[])


def named_files(b: B.Builder, index: Index) -> None:
    """D09: `Que dit le fichier X ?` for one short file with a fact, in a different format each time."""
    seen_formats: set = set()
    candidates = [r for r in b.records if r["readable"] and not r["ocr"] and r["facts"] and r["format"] not in ("vtt", "srt", "pptx", "odp")]
    b.rng.shuffle(candidates)
    for r in candidates:
        if r["format"] in seen_formats:
            continue
        key, value = next(iter(r["facts"].items()))
        if key in ("topic", "weekday"):
            facts = [[value]]
            expect = {"facts": facts, "sources_all": [r["path"]]}
        elif key in ("next_visit", "end"):
            expect = {"date": iso_of_french(value), "sources_all": [r["path"]]}
        else:
            amount = r["facts"].get("total_ttc") or r["facts"].get("amount") or r["facts"].get("annual")
            if not amount:
                continue
            expect = {"value": amount_of(amount), "sources_all": [r["path"]]}
        name = r["path"].split("/")[-1]
        if b.add("documents", "D09", f"Que dit le fichier {name} ?", expect=expect, route=["named_file"], documents=[r["path"]], fmt=r["format"]):
            seen_formats.add(r["format"])


def follow_ups(b: B.Builder, index: Index) -> None:
    """D10: a second question that names nobody, about the person of the first. It asks for a fact of another kind of
    document of the same person; the first question is added to the book when it is not there yet."""
    wording = {
        "lettre_confrere": "Et son prochain rendez-vous d'après le courrier du confrère ?",
        "resultats_labo": "Et son taux d'hémoglobine glyquée dans ses résultats d'analyses ?",
        "courrier_caisse": "Et le montant pris en charge d'après le décompte de sa caisse ?",
        "certificat": "Et le nombre de jours d'arrêt de son certificat ?",
        "facture_patient": "Et le montant de sa facture d'honoraires ?",
    }
    patients = sorted({pid for (pid, kind) in index.by_patient_kind if kind in wording})
    b.rng.shuffle(patients)
    for pid in patients:
        if b.feature_count.get("D10", 0) >= b.feature_cap.get("D10", 0):
            return
        usable = {}
        for kind in wording:
            docs = index.by_patient_kind.get((pid, kind), [])
            if len(docs) == 1 and docs[0]["readable"] and not docs[0]["ocr"] and person_fact(b, index, docs[0]):
                usable[kind] = docs[0]
        if len(usable) < 2 or b.cast.by_id[pid].role != "patient":
            continue
        kinds = sorted(usable)
        first, second = usable[kinds[0]], usable[kinds[1]]
        text, expect, _ = person_fact(b, index, first)
        lookalike = bool(index.lookalikes(pid))
        expect = dict(expect, must_not=leak_guard(b, index, first, pid, expect) if lookalike else [])
        parent = next((q for q in b.questions if q["text"] == text and q["files"]["documents"] == [first["path"]]), None)
        if parent is None:
            parent = b.add("documents", "D02" if lookalike else "D01", text, expect=expect, route=["global_retrieval"], documents=[first["path"]], fmt=first["format"])
        if parent is None:
            continue
        built = person_fact(b, index, second)
        b.add("documents", "D10", wording[kinds[1]], expect=dict(built[1], must_not=[]), route=["global_retrieval"], documents=[second["path"]],
              follow_up_of=parent["id"], fmt=second["format"])


def absent(b: B.Builder, index: Index) -> None:
    """D11: the documents do not hold it."""
    known = {fold(e.canonical) for e in b.cast.people}
    for name in ("Zénobie Quarante", "Hyacinthe Vaudroz", "Ernest Blondeau", "Léopold Tschumi-Bauer", "Gwenaëlle Brizard", "Octave Pellicier"):
        assert fold(name) not in known
        b.add("documents", "D11", f"Quel est le prochain rendez-vous de {name} ?", expect={"refusal": True, "must_not_patterns": [DATE_PATTERN]},
              route=REFUSAL_ROAD, documents=[])
    # A person who has a laboratory result but no letter, whose look-alike has a letter: the date must not cross.
    for r in index.by_kind["resultats_labo"]:
        if not index.unique(r) or b.cast.by_id[r["about"][0]].role != "patient":
            continue
        pid = r["about"][0]
        if index.by_patient_kind.get((pid, "lettre_confrere")):
            continue
        for other in index.lookalikes(pid):
            letters = [d for d in index.by_patient_kind.get((other, "lettre_confrere"), []) if "next_visit" in d["facts"]]
            if letters:
                name = person_forms(b.cast.by_id[pid])["full"]
                forbidden = sum((date_spellings(iso_of_french(d["facts"]["next_visit"])) for d in letters), [])
                b.add("documents", "D11", f"Quel est le prochain rendez-vous de {name} ?",
                      expect={"refusal": True, "must_not": forbidden, "must_not_patterns": [DATE_PATTERN]}, route=REFUSAL_ROAD, documents=[],
                      note="The look-alike has a letter with a date; this person has none.")
                break
    b.add("documents", "D11", "Montre la facture FAC-2026-999", expect={"refusal": True}, route=REFUSAL_ROAD, documents=[])
    b.add("documents", "D11", "Quel est le code du réseau Wi-Fi du cabinet ?", expect={"refusal": True}, route=REFUSAL_ROAD, documents=[])
    b.add("documents", "D11", "Quelle est la capitale de l'Australie ?", expect={"refusal": True}, route=REFUSAL_ROAD, documents=[],
          note="General knowledge: with documents ticked, the answer must come from them.")


def selections(b: B.Builder, index: Index) -> None:
    """D12: a file that is not ticked is never read. D13: a handful of files ticked."""
    readable = [r for r in b.records if r["readable"] and not r["ocr"] and r["format"] in ("pdf", "docx", "txt")]
    letters = [r for r in index.by_kind["lettre_confrere"] if index.unique(r) and r["readable"] and not r["ocr"] and "next_visit" in r["facts"]
               and b.cast.by_id[r["about"][0]].role == "patient"]
    for r in letters:
        pid = r["about"][0]
        family = fold(b.cast.by_id[pid].family)
        others = [d["path"] for d in b.records if d is not r and pid in d["about"] and d["readable"] and not d["ocr"]]
        fillers = [d["path"] for d in readable if pid not in d["about"] and family not in b.folded.get(d["path"], "")]
        if len(fillers) < 12:
            continue
        name = person_forms(b.cast.by_id[pid])["full"]
        start = (r["index"] * 7) % (len(fillers) - 12)
        picked = fillers[start:start + 12]
        spellings = date_spellings(iso_of_french(r["facts"]["next_visit"]))
        if len(others) >= 1 and b.count("documents") < b.quota["documents"]:
            selection = {"documents": {"paths": sorted(set(others + picked))}, "workbooks": "none"}
            b.add("documents", "D12", f"Quel est le prochain rendez-vous de {name} d'après le courrier du confrère ?",
                  expect={"must_not": spellings, "must_not_patterns": [], "route_only": False}, route=["global_retrieval", "error"], selection=selection,
                  documents=[], note="The letter that holds the date is not ticked.")
        selection = {"documents": {"paths": sorted({r["path"], *picked[:3]})}, "workbooks": "none"}
        b.add("documents", "D13", f"Quel est le prochain rendez-vous de {name} d'après le courrier du confrère ?",
              expect={"date": iso_of_french(r["facts"]["next_visit"]), "sources_any": [r["path"]]}, route=["global_retrieval"], selection=selection,
              documents=[r["path"]], note="Four files ticked, one of them the letter.")


def long_documents(b: B.Builder) -> None:
    """D14: a sentence planted on one page of a document of 100 pages or more."""
    for r in b.records:
        for needle in r.get("needles", {}).values():
            if "question" not in needle:
                continue
            expect = {"facts": [[c] for c in needle["contains"]], "sources_any": [r["path"]]}
            if r["format"] == "pdf" and needle["page"]:
                expect["page"] = needle["page"]
            b.add("documents", "D14", needle["question"], expect=expect, route=["global_retrieval"], documents=[r["path"]], fmt=r["format"],
                  note=f"Planted on page {needle['page'] or '?'} of {r['pages']}.")


def scans(b: B.Builder, index: Index) -> None:
    """D15: the same person questions, on scanned letters."""
    for r in b.records:
        if r["ocr"] and r["kind"] == "lettre_confrere" and index.unique(r) and "next_visit" in r["facts"] and b.cast.by_id[r["about"][0]].role == "patient":
            name = person_forms(b.cast.by_id[r["about"][0]])["full"]
            expect = {"date": iso_of_french(r["facts"]["next_visit"]), "sources_any": [r["path"]]}
            b.add("documents", "D15", f"Quel est le prochain rendez-vous de {name} d'après le courrier du confrère ?", expect=expect,
                  route=["global_retrieval"], documents=[r["path"]], human=True, fmt=r["format"],
                  note="The text comes from the local text recognition; it may be misread.")


def transcripts(b: B.Builder) -> None:
    """D16 (WebVTT) and D17 (SubRip): the order and the case planted in each transcript."""
    for fmt in ("vtt", "srt"):
        feature = READER_FEATURE[fmt]
        for r in [x for x in b.records if x["format"] == fmt and "order_code" in x["facts"]]:
            f = r["facts"]
            b.add("documents", feature, f"Dans la transcription de la réunion, quel est le montant du bon de commande {f['order_code']} ?",
                  expect={"value": amount_of(f["order_amount"]), "sources_any": [r["path"]]}, route=["global_retrieval"], documents=[r["path"]], fmt=fmt)
            b.add("documents", feature, f"À quelle date sera clos le dossier {f['case_code']} ?",
                  expect={"date": iso_of_french(f["case_closing"]), "sources_any": [r["path"]]}, route=["global_retrieval"], documents=[r["path"]], fmt=fmt)


def decks(b: B.Builder) -> None:
    """D21 (OpenDocument) and D22 (PowerPoint): a fact on slide 1, 3 or 4, and the slide it is on."""
    kinds = ["cost", "date", "speaker", "seats", "cost", "date"]
    for fmt in ("odp", "pptx"):
        feature = READER_FEATURE[fmt]
        pool = [x for x in b.records if x["format"] == fmt and "module" in x["facts"]]
        for k, r in enumerate(pool):
            f = r["facts"]
            kind = kinds[k % len(kinds)]
            if kind == "cost":
                text, expect = f"Quel est le coût par participant du module {f['module']} ?", {"value": amount_of(f["cost_per_person"]), "page": 4}
            elif kind == "date":
                text, expect = f"À quelle date a lieu la session du module {f['module']} ?", {"date": iso_of_french(f["session_date"]), "page": 3}
            elif kind == "speaker":
                text, expect = f"Qui est l'intervenant du module {f['module']} ?", {"facts": [[f["speaker"].split()[-1]]], "page": 1}
            else:
                text, expect = f"Combien de places y a-t-il dans le module {f['module']} ?", {"patterns": [rf"\b{f['seats']}\b"], "page": 3}
            expect["sources_any"] = [r["path"]]
            b.add("documents", feature, text, expect=expect, route=["global_retrieval"], documents=[r["path"]], fmt=fmt)


def add_all(b: B.Builder) -> None:
    caps = CAPS_SLICE if b.slice else CAPS_FULL
    b.feature_cap = caps
    index = Index(b)
    # Deterministic engines first (their answers need no model), then the questions that read a document.
    folder_questions(b)
    set_questions(b)
    transcripts(b)
    decks(b)
    long_documents(b)
    named_files(b, index)
    identifiers(b, index)
    organisations(b, index)
    follow_ups(b, index)
    person_facts(b, index)
    scans(b, index)
    selections(b, index)
    absent(b, index)
