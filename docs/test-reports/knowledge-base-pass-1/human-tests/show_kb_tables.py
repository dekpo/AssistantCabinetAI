"""A read-only look at an index.sqlite3: what the index holds and what the knowledge base holds.

Prints table names and row COUNTS only. It never prints a file name, a passage or a name, and it opens
the database read-only, so it cannot change anything and it does not migrate an old database.

    python show_kb_tables.py                      # the application's own index
    python show_kb_tables.py path/to/index.sqlite3

Used by the human tests of the knowledge base programme (docs/test-reports/knowledge-base-pass-1).
"""

import os
import sqlite3
import sys
from pathlib import Path

INDEX_TABLES = ("documents", "chunks", "tabular_inventories", "tabular_workbooks")
# The full-text index of names keeps shadow tables named after it; they are not tables of ours.
SHADOW_SUFFIXES = ("_data", "_idx", "_content", "_docsize", "_config")
APP_FOLDER = "com.assistantcabinetai.desktop"


def default_path() -> Path:
    local = os.environ.get("LOCALAPPDATA")
    if local:
        return Path(local) / APP_FOLDER / "index.sqlite3"
    return Path.home() / "Library" / "Application Support" / APP_FOLDER / "index.sqlite3"


def count(connection: sqlite3.Connection, table: str) -> int:
    return connection.execute(f'SELECT COUNT(*) FROM "{table}"').fetchone()[0]


def main(argv: list[str]) -> int:
    path = Path(argv[1]) if len(argv) > 1 else default_path()
    if not path.is_file():
        print(f"No index at {path}")
        return 2
    connection = sqlite3.connect(f"{path.as_uri()}?mode=ro", uri=True)

    print(f"Index: {path}")
    print(f"Size : {path.stat().st_size} bytes")
    journal = connection.execute("PRAGMA journal_mode").fetchone()[0]
    beside = [suffix for suffix in ("-wal", "-shm") if Path(f"{path}{suffix}").exists()]
    print(f"Journal mode: {journal}; files beside the index: {beside or 'none'}")

    print("\nThe index's own tables (row counts)")
    names = {
        row[0]
        for row in connection.execute("SELECT name FROM sqlite_master WHERE type IN ('table', 'view')")
    }
    for table in INDEX_TABLES:
        print(f"  {table:<22} {count(connection, table) if table in names else 'absent'}")

    knowledge = sorted(
        name
        for name in names
        if name.startswith("kb_") and not name.endswith(SHADOW_SUFFIXES)
    )
    if not knowledge:
        print("\nNo knowledge base table: this index was never opened by a build that has one.")
        return 0

    print("\nThe knowledge base (row counts)")
    for name in knowledge:
        print(f"  {name:<22} {count(connection, name)}")

    print("\nkb_meta")
    for key, value in connection.execute("SELECT key, value FROM kb_meta ORDER BY key"):
        print(f"  {key:<20} {value!r}")

    print("\nSources by folder and extractor version (counts only)")
    rows = connection.execute(
        "SELECT domain, kb_version, COUNT(*) FROM kb_sources GROUP BY domain, kb_version "
        "ORDER BY domain, kb_version"
    ).fetchall()
    for domain, version, total in rows or []:
        print(f"  {domain:<10} kb_version={version}  {total}")
    if not rows:
        print("  (none)")

    violations = connection.execute("PRAGMA foreign_key_check").fetchall()
    print(f"\nForeign key violations: {len(violations)}")
    return 0 if not violations else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
