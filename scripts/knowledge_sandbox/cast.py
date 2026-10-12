"""The cast of the sandbox: invented people and the organisations they deal with.

A small hand-written core (the confusables the owner asked for: the Dupont and Martin families, sound-alike
surnames, a doctor who has a patient's name, an organisation that carries a surname, names with particles,
apostrophes, accents, hyphens, and names from French-speaking Switzerland and from abroad) plus a bulk of
generated people so a library of a thousand documents has a long tail of names.

Every person has a stable `id`. The text writers record, for each mention they put in a document, the id and
the exact string written; the ground truth is built from those records, never from a reading of the text.
"""

from __future__ import annotations

import random
import re
import unicodedata
from dataclasses import dataclass, field

from . import pools


def fold(text: str) -> str:
    """Lowercase, accents removed, apostrophes and hyphens read as spaces. Used for uniqueness only."""
    decomposed = unicodedata.normalize("NFD", text)
    stripped = "".join(ch for ch in decomposed if not unicodedata.combining(ch))
    return re.sub(r"[^a-z0-9]+", " ", stripped.lower()).strip()


def ascii_keep_case(text: str) -> str:
    decomposed = unicodedata.normalize("NFD", text)
    return "".join(ch for ch in decomposed if not unicodedata.combining(ch))


def slug(text: str) -> str:
    return fold(text).replace(" ", "-")


@dataclass
class Entity:
    id: str
    type: str  # person | organization | identifier
    canonical: str
    # persons
    given: str = ""
    family: str = ""
    sex: str = ""  # F | M
    role: str = ""  # patient | doctor | staff | owner | org | identifier
    country: str = ""  # FR | CH | XX (abroad, living in either)
    title: str = ""  # Dr | Dre | Pr | ""
    commune: tuple = ()  # (name, postal code, canton or department)
    street: str = ""
    birth_year: int = 0
    ids: dict = field(default_factory=dict)  # nir, avs, iban, phone, email
    # organisations
    org_key: str = ""
    short: str = ""
    kind: str = ""
    # shared
    homonym_group: str = ""
    designed: bool = False
    weight: float = 1.0
    note: str = ""

    def public(self) -> dict:
        """What the ground truth file keeps."""
        row = {
            "id": self.id,
            "type": self.type,
            "canonical": self.canonical,
            "role": self.role,
            "country": self.country,
        }
        if self.type == "person":
            row.update(given=self.given, family=self.family, title=self.title, sex=self.sex)
            row["forms"] = person_forms(self)
            row["identifiers"] = {k: v for k, v in sorted(self.ids.items()) if k in ("nir", "avs", "iban", "email", "phone")}
            if self.commune:
                row["commune"] = self.commune[0]
        if self.type == "organization":
            row.update(short=self.short, kind=self.kind)
            row["forms"] = org_forms(self)
        if self.homonym_group:
            row["homonym_group"] = self.homonym_group
        if self.note:
            row["note"] = self.note
        return row


def initials(given: str) -> str:
    """`Jean` is `J.`, `Jean-Pierre` is `J.-P.`, `Thi Lan` is `T. L.`."""
    if "-" in given:
        return "-".join(part[0] + "." for part in given.split("-"))
    if " " in given:
        return " ".join(part[0] + "." for part in given.split())
    return given[0] + "."


def initial_letter(given: str) -> str:
    return given[0]


def person_forms(e: Entity) -> dict:
    """Every shape in which a text may write this person. The keys are the `form` names of the ground truth."""
    g, f = e.given, e.family
    forms = {
        "full": f"{g} {f}",
        "reordered": f"{f.upper()} {g}",
        "upper": f"{g} {f}".upper(),
        "no_accent": ascii_keep_case(f"{g} {f}"),
        "initial_surname": f"{initials(g)} {f}",
        "surname_initial": f"{f} {initial_letter(g)}",
        "surname_only": f,
        "hyphen_error": f"{g}-{f}" if " " not in g else f"{g.replace(' ', '-')}-{f}",
    }
    if e.role in ("doctor", "owner"):
        title = e.title or "Dr"
        forms["title_full"] = f"{title} {g} {f}"
        forms["title_surname"] = f"{title} {f}"
    else:
        civility = "Monsieur" if e.sex == "M" else "Madame"
        short = "M." if e.sex == "M" else "Mme"
        forms["title_full"] = f"{civility} {g} {f}"
        forms["title_surname"] = f"{short} {f}"
    return forms


def org_forms(e: Entity) -> dict:
    forms = {"full": e.canonical}
    if e.short:
        forms["acronym"] = e.short
    forms["upper"] = e.canonical.upper()
    return forms


def person(
    pid: str, given: str, family: str, sex: str, role: str, country: str, commune: tuple = (),
    birth_year: int = 0, title: str = "", homonym_group: str = "", note: str = "", weight: float = 1.0,
    designed: bool = True,
) -> Entity:
    return Entity(
        id=pid, type="person", canonical=f"{given} {family}", given=given, family=family, sex=sex, role=role,
        country=country, title=title, commune=commune, birth_year=birth_year, homonym_group=homonym_group,
        note=note, weight=weight, designed=designed,
    )


def commune(name: str) -> tuple:
    for row in pools.COMMUNES_FR + pools.COMMUNES_CH:
        if row[0] == name:
            return row
    raise KeyError(name)


# ----------------------------------------------------------------------------------------- the designed core


def designed_people() -> list[Entity]:
    p = person
    people = [
        # --- the practice -------------------------------------------------------------------------------
        p("o-marchetti", "Hélène", "Marchetti", "F", "owner", "FR", commune("Annemasse"), 1976, "Dr",
          note="the user's own name: in every letter's letterhead (selectivity test)", weight=0),
        p("s-colas", "Sandrine", "Colas", "F", "staff", "FR", commune("Annemasse"), 1983, weight=0.6),
        p("s-bensaid", "Nadia", "Bensaïd", "F", "staff", "FR", commune("Gaillard"), 1990, weight=0.4),
        p("s-cottet", "Mélanie", "Cottet", "F", "staff", "CH", commune("Nyon"), 1985, weight=0.3,
          note="accountant of the fiduciary"),
        # --- the Dupont family: the trap of the original brief --------------------------------------------
        p("p-dupont-jean", "Jean", "Dupont", "M", "patient", "FR", commune("Annemasse"), 1958, weight=3,
          homonym_group="H-jean-dupont", note="patient; a cardiologist of Geneva has the same name"),
        p("d-dupont-jean", "Jean", "Dupont", "M", "doctor", "CH", commune("Genève"), 1964, "Dr", weight=1.5,
          homonym_group="H-jean-dupont", note="cardiologist (HUG); same name as the patient Jean Dupont"),
        p("p-dupont-jean-pierre", "Jean-Pierre", "Dupont", "M", "patient", "FR", commune("Cluses"), 1971, weight=2),
        p("p-dupontet-jean", "Jean", "Dupontet", "M", "patient", "FR", commune("Thonon-les-Bains"), 1949, weight=1.5,
          note="surname one syllable longer than Dupont"),
        p("p-dupont-marie", "Marie", "Dupont", "F", "patient", "FR", commune("Annemasse"), 1962, weight=2),
        # --- the Martin family ------------------------------------------------------------------------------
        p("d-martin-paul", "Paul", "Martin", "M", "doctor", "FR", commune("Annemasse"), 1969, "Dr", weight=2.5,
          note="associate GP of the practice; `Dr Martin` alone is ambiguous"),
        p("d-martin-pierre", "Pierre", "Martin", "M", "doctor", "FR", commune("Annecy"), 1961, "Dr", weight=2,
          note="cardiologist, Annecy"),
        p("p-martin-marie", "Marie", "Martin", "F", "patient", "FR", commune("Gaillard"), 1975, weight=2,
          note="French resident, works in Geneva: has a social security number AND an AVS number"),
        p("p-martin-claire", "Claire", "Martin", "F", "patient", "FR", commune("Lyon"), 1988, weight=1.5),
        p("p-martin-luc", "Luc", "Martin", "M", "patient", "FR", commune("Annecy"), 1944, weight=1.2),
        # --- sound-alike and look-alike surnames ------------------------------------------------------------------
        p("p-favre-laurent", "Laurent", "Favre", "M", "patient", "CH", commune("Genève"), 1966, weight=1.5),
        p("p-fabre-laurence", "Laurence", "Fabre", "F", "patient", "FR", commune("Annecy"), 1970, weight=1.2),
        p("d-favre-olivier", "Olivier", "Favre", "M", "doctor", "CH", commune("Genève"), 1967, "Dr", weight=1,
          note="ENT; shares the surname Favre with a patient"),
        p("p-monnier-veronique", "Véronique", "Monnier", "F", "patient", "CH", commune("Nyon"), 1959, weight=1.3),
        p("p-meunier-veronique", "Véronique", "Meunier", "F", "patient", "FR", commune("Thonon-les-Bains"), 1961, weight=1.3),
        p("p-girardin-fabienne", "Fabienne", "Girardin", "F", "patient", "CH", commune("Delémont"), 1968, weight=1),
        p("p-girard-frederic", "Frédéric", "Girard", "M", "patient", "FR", commune("Annecy"), 1974, weight=1),
        p("p-lucas-roy", "Lucas", "Roy", "M", "patient", "FR", commune("Annemasse"), 1983, weight=1,
          note="given name and surname are both usual first names; Roy Lucas is another person"),
        p("p-roy-lucas", "Roy", "Lucas", "M", "patient", "CH", commune("Lausanne"), 1979, weight=1),
        p("p-thomas-pierre", "Thomas", "Pierre", "M", "patient", "FR", commune("Annecy"), 1990, weight=1),
        p("p-pierre-thomas", "Pierre", "Thomas", "M", "patient", "FR", commune("Seynod"), 1955, weight=1),
        # --- French-speaking Switzerland ----------------------------------------------------------------------
        p("p-rochat-sophie", "Sophie", "Rochat", "F", "patient", "CH", commune("Lausanne"), 1982, weight=1.6),
        p("p-cuendet-mathieu", "Mathieu", "Cuendet", "M", "patient", "CH", commune("Morges"), 1991, weight=1.3),
        p("p-pittet-isabelle", "Isabelle", "Pittet", "F", "patient", "CH", commune("Nyon"), 1973, weight=1.3),
        p("p-chappuis-nathalie", "Nathalie", "Chappuis", "F", "patient", "CH", commune("Vevey"), 1969, weight=1.2),
        p("p-maillard-pascal", "Pascal", "Maillard", "M", "patient", "CH", commune("Fribourg"), 1955, weight=1.1),
        p("p-bovet-celine", "Céline", "Bovet", "F", "patient", "CH", commune("Bulle"), 1986, weight=1),
        p("p-jaccard-alain", "Alain", "Jaccard", "M", "patient", "CH", commune("Yverdon-les-Bains"), 1948, weight=1),
        p("p-deleze-christelle", "Christelle", "Délèze", "F", "patient", "CH", commune("Sion"), 1979, weight=1),
        p("p-crettenand-gregoire", "Grégoire", "Crettenand", "M", "patient", "CH", commune("Martigny"), 1967, weight=1),
        p("p-vuilleumier-marie-claude", "Marie-Claude", "Vuilleumier", "F", "patient", "CH", commune("Saint-Imier"), 1952, weight=1),
        p("p-python-jean-claude", "Jean-Claude", "Python", "M", "patient", "CH", commune("Fribourg"), 1940, weight=1),
        p("p-oberson-pierre-alain", "Pierre-Alain", "Oberson", "M", "patient", "CH", commune("Bulle"), 1963, weight=1),
        p("p-genoud-anne-laure", "Anne-Laure", "Genoud", "F", "patient", "CH", commune("Châtel-Saint-Denis"), 1990, weight=1),
        p("p-tschopp-raphael", "Raphaël", "Tschopp", "M", "patient", "CH", commune("Porrentruy"), 1984, weight=1),
        p("p-zufferey-stephane", "Stéphane", "Zufferey", "M", "patient", "CH", commune("Sierre"), 1972, weight=1),
        # --- names from elsewhere, particles, apostrophes, hyphens ------------------------------------------------
        p("p-benali-fatima", "Fatima", "Benali", "F", "patient", "CH", commune("Vernier"), 1985, weight=1.2),
        p("p-el-khoury-mohamed", "Mohamed", "El Khoury", "M", "patient", "FR", commune("Lyon"), 1977, weight=1),
        p("p-nguyen-thi-lan", "Thi Lan", "Nguyen", "F", "patient", "CH", commune("Genève"), 1993, weight=1),
        p("p-kowalski-andrzej", "Andrzej", "Kowalski", "M", "patient", "FR", commune("Annemasse"), 1960, weight=1),
        p("p-dos-santos-ferreira-maria", "Maria", "Dos Santos Ferreira", "F", "patient", "CH", commune("Lausanne"), 1972, weight=1),
        p("p-demirci-ibrahim", "Ibrahim", "Demirci", "M", "patient", "CH", commune("Renens"), 1981, weight=1),
        p("p-petrovic-elena", "Elena", "Petrović", "F", "patient", "CH", commune("Meyrin"), 1987, weight=1),
        p("p-damico-giuseppe", "Giuseppe", "D'Amico", "M", "patient", "FR", commune("Thonon-les-Bains"), 1950, weight=1),
        p("p-ndiaye-aicha", "Aïcha", "N'Diaye", "F", "patient", "FR", commune("Villeurbanne"), 1995, weight=1),
        p("p-obrien-sean", "Sean", "O'Brien", "M", "patient", "CH", commune("Coppet"), 1968, weight=1),
        p("p-muller-jorg", "Jörg", "Müller", "M", "patient", "CH", commune("Neuchâtel"), 1957, weight=1),
        p("p-hasanovic-anna", "Anna", "Hasanović", "F", "patient", "CH", commune("Genève"), 1989, weight=1),
        p("p-bernasconi-luca", "Luca", "Bernasconi", "M", "patient", "CH", commune("Vevey"), 1974, weight=1),
        p("p-le-gall-laurence", "Laurence", "Le Gall", "F", "patient", "FR", commune("Annecy"), 1964, weight=1),
        p("p-de-la-fontaine-matthieu", "Matthieu", "de la Fontaine", "M", "patient", "FR", commune("Chambéry"), 1978, weight=1),
        p("p-daubigne-camille", "Camille", "d'Aubigné", "F", "patient", "FR", commune("Annemasse"), 1992, weight=1),
        p("p-luthi-zoe", "Zoë", "Lüthi", "F", "patient", "CH", commune("Lausanne"), 1998, weight=1),
        p("p-lindqvist-asa", "Åsa", "Lindqvist", "F", "patient", "CH", commune("Montreux"), 1980, weight=1),
        # --- doctors and specialists -----------------------------------------------------------------------------
        p("d-vuagniaux-anne-sophie", "Anne-Sophie", "Vuagniaux", "F", "doctor", "CH", commune("Lausanne"), 1972, "Dre", weight=1.6,
          note="endocrinologist, CHUV; signs `Dre` as is usual in Switzerland"),
        p("d-bersier-marc", "Marc", "Bersier", "M", "doctor", "CH", commune("Nyon"), 1965, "Dr", weight=1.4),
        p("d-cuenoud-philippe", "Philippe", "Cuénoud", "M", "doctor", "CH", commune("Lausanne"), 1962, "Dr", weight=1.2),
        p("d-grandjean-yann", "Yann", "Grandjean", "M", "doctor", "CH", commune("Genève"), 1970, "Dr", weight=1.2),
        p("d-rapin-jean-luc", "Jean-Luc", "Rapin", "M", "doctor", "CH", commune("Lausanne"), 1958, "Pr", weight=1),
        p("d-mottier-sylvain", "Sylvain", "Mottier", "M", "doctor", "CH", commune("Nyon"), 1976, "Dr", weight=0.8),
        p("d-haddad-karim", "Karim", "Haddad", "M", "doctor", "FR", commune("Annemasse"), 1973, "Dr", weight=1.3,
          note="radiologist"),
        p("d-pasquier-sandra", "Sandra", "Pasquier", "F", "doctor", "CH", commune("Genève"), 1980, "Dre", weight=0.8,
          note="paediatrician"),
        p("d-gaillard-eric", "Éric", "Gaillard", "M", "doctor", "FR", commune("Annecy"), 1968, "Dr", weight=0.9,
          note="dermatologist; Gaillard is also a commune next to Annemasse"),
        p("d-roche-bernard", "Bernard", "Roche", "M", "doctor", "FR", commune("Lyon"), 1960, "Dr", weight=0.8,
          note="biologist"),
    ]
    return people


# ---------------------------------------------------------------------------------------------- the bulk


def bulk_people(rng: random.Random, taken: set, count: int) -> list[Entity]:
    """Generated people. Surnames repeat on purpose (a real library has several Bernard and several
    Favre), given-name and surname pairs never do."""
    people: list[Entity] = []
    surname_pools = [
        (pools.SURNAMES_FR, 0.30, "FR"),
        (pools.SURNAMES_SAVOIE, 0.05, "FR"),
        (pools.SURNAMES_ROMAND, 0.45, "CH"),
        (pools.SURNAMES_OTHER, 0.20, "XX"),
    ]
    attempts = 0
    while len(people) < count and attempts < count * 40:
        attempts += 1
        roll = rng.random()
        acc = 0.0
        chosen = surname_pools[0]
        for entry in surname_pools:
            acc += entry[1]
            if roll <= acc:
                chosen = entry
                break
        surnames, _, origin = chosen
        sex = "F" if rng.random() < 0.55 else "M"
        if origin == "CH":
            given_pool = (pools.GIVEN_F_CH + pools.GIVEN_F_FR) if sex == "F" else (pools.GIVEN_M_CH + pools.GIVEN_M_FR)
        elif origin == "XX":
            given_pool = (pools.GIVEN_F_OTHER + pools.GIVEN_F_FR) if sex == "F" else (pools.GIVEN_M_OTHER + pools.GIVEN_M_FR)
        else:
            given_pool = pools.GIVEN_F_FR if sex == "F" else pools.GIVEN_M_FR
        given = rng.choice(given_pool)
        family = rng.choice(surnames)
        key = fold(f"{given} {family}")
        if key in taken:
            continue
        taken.add(key)
        country = "CH" if origin == "CH" else ("FR" if origin == "FR" else rng.choice(["FR", "CH"]))
        town = rng.choice(pools.COMMUNES_CH if country == "CH" else pools.COMMUNES_FR)
        index = len(people) + 1
        role = "doctor" if rng.random() < 0.13 else "patient"
        title = ""
        if role == "doctor":
            title = "Dre" if (sex == "F" and country == "CH" and rng.random() < 0.7) else "Dr"
        # a long tail: a few names come up in many documents, most in one or two
        weight = 1.0 / (1 + index * 0.012) + 0.15
        people.append(
            Entity(
                id=f"{'d' if role == 'doctor' else 'p'}-g{index:04d}", type="person", canonical=f"{given} {family}",
                given=given, family=family, sex=sex, role=role, country=country, title=title, commune=tuple(town),
                birth_year=rng.randint(1936, 2006), weight=weight,
            )
        )
    return people


def finish_people(rng: random.Random, people: list[Entity]) -> None:
    """Give every person a street, an e-mail address and the numbers their country's papers carry."""
    for e in people:
        if e.type != "person":
            continue
        swiss_town = e.commune and e.commune[2] in {"VD", "GE", "NE", "FR", "VS", "JU", "BE"}
        street = rng.choice(pools.STREETS_CH if swiss_town else pools.STREETS_FR)
        number = rng.randint(1, 80)
        e.street = f"{street} {number}" if swiss_town else f"{number} {street}"
        local = f"{slug(e.given).replace('-', '.')}.{slug(e.family).replace('-', '.')}"
        e.ids["email"] = f"{local}@mail.example"
        if e.role in ("patient", "owner", "staff"):
            department = "74"
            if e.commune and e.commune[1][:2].isdigit() and not swiss_town:
                department = e.commune[1][:2]
            month = rng.randint(1, 12)
            if not swiss_town or e.id == "p-martin-marie":
                e.ids["nir"] = pools.nir(rng, e.sex or "F", e.birth_year or 1970, month, department)
            if swiss_town or e.id == "p-martin-marie":
                e.ids["avs"] = pools.avs(rng)
            e.ids["phone"] = pools.phone_ch(rng) if swiss_town else pools.phone_fr(rng)
            if rng.random() < 0.5:
                e.ids["iban"] = pools.iban_ch(rng) if swiss_town else pools.iban_fr(rng)
        else:
            e.ids["phone"] = pools.phone_ch(rng) if swiss_town else pools.phone_fr(rng)


def organisations() -> list[Entity]:
    rows = []
    for key, canonical, short, country, kind, sector in pools.ORGANISATIONS:
        rows.append(
            Entity(
                id=f"o-{key}", type="organization", canonical=canonical, role="org", country=country,
                org_key=key, short=short, kind=kind, designed=True, weight=1.0,
                note=sector,
            )
        )
    return rows


class Cast:
    """Everything the documents can name."""

    def __init__(self, seed: int, bulk: int = 420):
        rng = random.Random(f"cast-{seed}")
        self.people: list[Entity] = designed_people()
        taken = {fold(e.canonical) for e in self.people}
        self.people += bulk_people(rng, taken, bulk)
        finish_people(rng, self.people)
        self.orgs: list[Entity] = organisations()
        self.by_id: dict[str, Entity] = {e.id: e for e in self.people + self.orgs}
        self.identifiers: dict[str, Entity] = {}  # created by documents, keyed by value
        self.surname_count: dict[str, int] = {}
        for e in self.people:
            if e.type == "person":
                self.surname_count[fold(e.family)] = self.surname_count.get(fold(e.family), 0) + 1
        self.owner = self.by_id["o-marchetti"]
        self.own_org = self.by_id["o-cabinet-voirons"]

    def shared_surname(self, e: Entity) -> bool:
        return self.surname_count.get(fold(e.family), 0) > 1

    def patients(self) -> list[Entity]:
        return [e for e in self.people if e.role == "patient"]

    def doctors(self) -> list[Entity]:
        return [e for e in self.people if e.role == "doctor"]

    def org(self, key: str) -> Entity:
        return self.by_id[f"o-{key}"]

    def identifier(self, kind: str, value: str, owner: str | None = None) -> Entity:
        entity = self.identifiers.get(value)
        if entity is None:
            entity = Entity(
                id=f"i-{slug(value)}", type="identifier", canonical=value, role="identifier", kind=kind,
                note=owner or "",
            )
            self.identifiers[value] = entity
            self.by_id[entity.id] = entity
        return entity
