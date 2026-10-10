"""A read-only look at the NAMES the knowledge base holds in an index.sqlite3.

Unlike show_kb_tables.py, which prints counts only, this prints the names found in the analysed files,
the files that mention each one and how each was found (for a table: the sheet, the column, the first
row and a few sample rows, counted from 1 under the header), plus how the columns of the Data folder's
tables were read (and which were not, with the reason) and the links a table states between two columns. It exists for the human tests of the knowledge
base programme, which use the FICTIONAL fixtures only. Do not point it at an index built from real
files and paste the output anywhere: a list of the names found in a practice's files is personal data
(docs/PRIVACY-AND-SECURITY.md).

It opens the database read-only, so it cannot change anything and it does not migrate an old database.

    python show_kb_entities.py                                  # the application's own index
    python show_kb_entities.py path/to/index.sqlite3
    python show_kb_entities.py --look-for claire.martin@exemple.fr    # where does this text appear?

--look-for takes any text (repeat it for several). For each one it says whether the text appears in
the knowledge base tables (kb_*) and in the stored document text (the chunks table). The point of the
second answer is to be honest about a limit: the index keeps the text of every analysed document so
that questions can be answered, so a number that is written in a document is in the index in clear
even though the knowledge base stores only a keyed hash of it.
"""

import os
import re
import sqlite3
import sys
from pathlib import Path

APP_FOLDER = "com.assistantcabinetai.desktop"
SHADOW_SUFFIXES = ("_data", "_idx", "_content", "_docsize", "_config")
KEY_FILE = "knowledge-identifier.key"


def app_folder() -> Path:
    local = os.environ.get("LOCALAPPDATA")
    if local:
        return Path(local) / APP_FOLDER
    return Path.home() / "Library" / "Application Support" / APP_FOLDER


def reduced(text: str) -> str:
    """Letters and digits only, upper case: the form identifiers are compared in."""
    return re.sub(r"[^0-9A-Za-z]", "", text).upper()


def knowledge_tables(connection: sqlite3.Connection) -> list[str]:
    names = [
        row[0]
        for row in connection.execute("SELECT name FROM sqlite_master WHERE type = 'table'")
    ]
    return sorted(
        name for name in names if name.startswith("kb_") and not name.endswith(SHADOW_SUFFIXES)
    )


def describe_mention(path, kind, method, count, sheet, column, first_row, rows, role) -> str:
    """One mention. A table mention says where in the table: sheet, column, the first row and a
    few sample rows (rows are counted from 1 under the header), and the role the column gave."""
    if kind == "cells":
        place = f"{sheet} / {column}"
        rows_text = f", rows {rows}" if rows else ""
        role_text = f", role {role}" if role else ""
        return f"{path} [cells {place}, first row {first_row}{rows_text}{role_text}, {method}, x{count}]"
    return f"{path} [{kind}, {method}, x{count}]"


READ_REASONS = ("header_exact", "header_partial", "value_shape", "combined")


def show_columns(connection: sqlite3.Connection) -> None:
    """How the columns of the Data folder's tables were read (kb_column_semantics): first the ones
    whose values were read as entities, then the ones that were not, with the reason."""
    try:
        columns = connection.execute(
            "SELECT s.relative_path, c.sheet, c.column_name, c.semantic_type, COALESCE(c.subtype, ''), "
            "COALESCE(c.role, ''), c.confidence, c.reason "
            "FROM kb_column_semantics c JOIN kb_sources s ON s.source_id = c.source_id "
            "ORDER BY s.relative_path, c.sheet, c.column_name"
        ).fetchall()
    except sqlite3.Error:
        return
    read = [row for row in columns if row[7] in READ_REASONS and row[6] >= 0.6]
    unread = [row for row in columns if row not in read]
    print(f"\nColumns read as something: {len(read)}")
    for path, sheet, column, kind, subtype, role, confidence, reason in read:
        label = f"{kind}/{subtype}" if subtype else kind
        role_text = f" role {role}" if role else ""
        print(f"  {path} / {sheet} / {column:<22} {label:<26}{role_text}  {confidence:.2f}  {reason}")
    print(f"\nColumns NOT read, with the reason: {len(unread)}")
    for path, sheet, column, kind, subtype, role, confidence, reason in unread:
        hint = ""
        if reason == "module_off":
            hint = f"  (a module that is off would read it as {kind}{'/' + role if role else ''})"
        elif reason == "values_mismatch":
            hint = f"  (the header says {kind}, the values do not look like it; confidence {confidence:.2f})"
        elif reason == "number_or_date":
            hint = f"  (the header says {kind}, the column holds numbers or dates)"
        print(f"  {path} / {sheet} / {column:<22} {reason}{hint}")


def show_relations(connection: sqlite3.Connection) -> None:
    """Links a table states by two role columns (never from two names in one file)."""
    try:
        relations = connection.execute(
            "SELECT a.canonical_name, r.predicate, b.canonical_name, r.occurrence_count, s.relative_path "
            "FROM kb_relations r JOIN kb_entities a ON a.entity_id = r.subject "
            "JOIN kb_entities b ON b.entity_id = r.object "
            "LEFT JOIN kb_sources s ON s.source_id = r.source_id "
            "ORDER BY s.relative_path, a.canonical_name, b.canonical_name"
        ).fetchall()
    except sqlite3.Error:
        return
    print(f"\nRelations stated by a table: {len(relations)}")
    for subject, predicate, other, count, path in relations:
        print(f"  {subject}  --{predicate}-->  {other}   [{path}, x{count}]")


def show_surname_initial_aliases(connection: sqlite3.Connection) -> None:
    """The aliases of the people filed as 'Surname Initial' (Dupont J): their surname alone and
    'J. Dupont' are there, and the single letter is never an alias by itself."""
    try:
        rows = connection.execute(
            "SELECT e.canonical_name, a.normalized, a.kind "
            "FROM kb_aliases a JOIN kb_entities e ON e.entity_id = a.entity_id "
            "WHERE e.type_id = 'person' AND e.status IN ('candidate', 'active') "
            "AND e.normalized_name GLOB '* ?' ORDER BY e.canonical_name, a.normalized"
        ).fetchall()
    except sqlite3.Error:
        return
    people: dict[str, list[str]] = {}
    for name, normalized, kind in rows:
        people.setdefault(name, []).append(f"{normalized!r} ({kind})")
    print(f"\nAliases of people filed as surname + initial: {len(people)}")
    for name, aliases in people.items():
        print(f"  {name}: " + ", ".join(aliases))


def show_entities(connection: sqlite3.Connection) -> None:
    sources = connection.execute(
        "SELECT domain, relative_path, kb_version, gazetteer_epoch FROM kb_sources "
        "ORDER BY domain, relative_path"
    ).fetchall()
    print(f"Sources the knowledge base has read: {len(sources)}")
    for domain, path, version, epoch in sources:
        print(f"  {domain:<10} {path}   (extractor {version}, names list {epoch})")

    show_columns(connection)

    entities = connection.execute(
        "SELECT entity_id, type_id, COALESCE(subtype, ''), status, origin, canonical_name "
        "FROM kb_entities ORDER BY type_id, canonical_name"
    ).fetchall()
    print(f"\nEntities: {len(entities)}")
    print(f"  {'type':<36} {'status':<10} {'name (as shown)':<34} mentioned in")
    for entity_id, type_id, subtype, status, origin, name in entities:
        label = f"{type_id}/{subtype}" if subtype else type_id
        files = connection.execute(
            "SELECT s.relative_path, m.locator_kind, m.method, m.occurrence_count, m.sheet, "
            "m.column_name, m.first_row, m.sample_rows, m.role "
            "FROM kb_mentions m JOIN kb_sources s ON s.source_id = m.source_id "
            "WHERE m.entity_id = ? ORDER BY s.relative_path, m.sheet, m.column_name",
            (entity_id,),
        ).fetchall()
        where = "; ".join(
            describe_mention(path, kind, method, count, sheet, column, first_row, rows, role)
            for path, kind, method, count, sheet, column, first_row, rows, role in files
        )
        flag = "" if origin == "automatic" else f" ({origin})"
        print(f"  {label:<36} {status:<10} {name:<34} {where}{flag}")

    show_relations(connection)

    possible = connection.execute(
        "SELECT a.canonical_name, b.canonical_name, p.reason, p.state "
        "FROM kb_possible_matches p "
        "JOIN kb_entities a ON a.entity_id = p.entity_a JOIN kb_entities b ON b.entity_id = p.entity_b"
    ).fetchall()
    print(f"\nPossible matches (never merged): {len(possible)}")
    for first, second, reason, state in possible:
        print(f"  {first}  ~  {second}   [{reason}, {state}]")

    show_surname_initial_aliases(connection)

    print("\nkb_meta")
    for key, value in connection.execute("SELECT key, value FROM kb_meta ORDER BY key"):
        print(f"  {key:<24} {value!r}")


def look_for(connection: sqlite3.Connection, needles: list[str]) -> None:
    tables = knowledge_tables(connection)
    for needle in needles:
        lowered = needle.lower()
        squashed = reduced(needle)
        in_knowledge: list[str] = []
        for table in tables:
            cursor = connection.execute(f'SELECT * FROM "{table}"')
            for row in cursor:
                for value in row:
                    if isinstance(value, str) and (
                        lowered in value.lower() or (squashed and squashed in reduced(value))
                    ):
                        in_knowledge.append(table)
                        break
                else:
                    continue
                break
        chunks = connection.execute(
            "SELECT COUNT(*) FROM chunks WHERE lower(text) LIKE ?", (f"%{lowered}%",)
        ).fetchone()[0]
        print(f"\nText: {needle!r}")
        print(
            "  knowledge base tables (kb_*): "
            + (f"FOUND in {sorted(set(in_knowledge))}" if in_knowledge else "not found")
        )
        print(
            f"  stored document text (chunks): found in {chunks} chunk(s)"
            + ("  <- the text of the documents is in the index in clear" if chunks else "")
        )


def main(argv: list[str]) -> int:
    # Names have accents; a Windows console may not be in UTF-8 by default.
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    arguments = argv[1:]
    needles: list[str] = []
    path: Path | None = None
    position = 0
    while position < len(arguments):
        if arguments[position] == "--look-for" and position + 1 < len(arguments):
            needles.append(arguments[position + 1])
            position += 2
        else:
            path = Path(arguments[position])
            position += 1
    path = path or app_folder() / "index.sqlite3"
    if not path.is_file():
        print(f"No index at {path}")
        return 2
    connection = sqlite3.connect(f"{path.as_uri()}?mode=ro", uri=True)
    if not knowledge_tables(connection):
        print("No knowledge base table: this index was never opened by a build that has one.")
        return 0

    print(f"Index: {path}")
    key = path.parent / KEY_FILE
    print(f"Identifier key file beside the index: {'present' if key.is_file() else 'absent'} ({KEY_FILE})")
    print()
    if needles:
        look_for(connection, needles)
    else:
        show_entities(connection)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
