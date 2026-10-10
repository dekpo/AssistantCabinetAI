"""A read-only look at the NAMES the knowledge base holds in an index.sqlite3.

Unlike show_kb_tables.py, which prints counts only, this prints the names found in the analysed files,
the files that mention each one and how each was found. It exists for the human tests of the knowledge
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


def show_entities(connection: sqlite3.Connection) -> None:
    sources = connection.execute(
        "SELECT domain, relative_path, kb_version, gazetteer_epoch FROM kb_sources "
        "ORDER BY domain, relative_path"
    ).fetchall()
    print(f"Sources the knowledge base has read: {len(sources)}")
    for domain, path, version, epoch in sources:
        print(f"  {domain:<10} {path}   (extractor {version}, names list {epoch})")

    entities = connection.execute(
        "SELECT entity_id, type_id, COALESCE(subtype, ''), status, origin, canonical_name "
        "FROM kb_entities ORDER BY type_id, canonical_name"
    ).fetchall()
    print(f"\nEntities: {len(entities)}")
    print(f"  {'type':<36} {'status':<10} {'name (as shown)':<34} mentioned in")
    for entity_id, type_id, subtype, status, origin, name in entities:
        label = f"{type_id}/{subtype}" if subtype else type_id
        files = connection.execute(
            "SELECT s.relative_path, m.locator_kind, m.method, m.occurrence_count "
            "FROM kb_mentions m JOIN kb_sources s ON s.source_id = m.source_id "
            "WHERE m.entity_id = ? ORDER BY s.relative_path",
            (entity_id,),
        ).fetchall()
        where = "; ".join(
            f"{path} [{kind}, {method}, x{count}]" for path, kind, method, count in files
        )
        flag = "" if origin == "automatic" else f" ({origin})"
        print(f"  {label:<36} {status:<10} {name:<34} {where}{flag}")

    possible = connection.execute(
        "SELECT a.canonical_name, b.canonical_name, p.reason, p.state "
        "FROM kb_possible_matches p "
        "JOIN kb_entities a ON a.entity_id = p.entity_a JOIN kb_entities b ON b.entity_id = p.entity_b"
    ).fetchall()
    print(f"\nPossible matches (never merged): {len(possible)}")
    for first, second, reason, state in possible:
        print(f"  {first}  ~  {second}   [{reason}, {state}]")

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
