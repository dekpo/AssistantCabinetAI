"""Generate the knowledge-base validation sandbox (KB lot 11).

Fictional content only: every person, number and address is invented. The organisations are real public
bodies and companies named as organisations (hospitals, insurers, laboratories) or invented ones (suppliers).
No real patient, no real file.

    python scripts/gen-knowledge-sandbox.py                       # the versioned core: 200 documents
    python scripts/gen-knowledge-sandbox.py --documents 1000 --out data/knowledge-sandbox-1000

The output is deterministic: the same seed gives the same bytes, and the first N documents of a larger library
are the N documents of a smaller one (tiers 30, 100, 200, 500, 1000 are prefixes of the same stream).

Writes <out>/documents, <out>/data, and the ground truth (entities.json, documents.json, workbooks.json,
questions.json, manifest.json). Needs Pillow for the scanned pages (as scripts/gen-sandbox-fixtures.py does).
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from knowledge_sandbox import build  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SEED = 20261012


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--documents", type=int, default=200, help="number of documents (default 200, the versioned core)")
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    parser.add_argument("--extra-rows", type=int, default=0, help="more rows in the workbooks, for the large libraries")
    parser.add_argument("--out", type=Path, default=ROOT / "fixtures" / "knowledge-sandbox",
                        help="output folder (replaced if it exists)")
    args = parser.parse_args()
    manifest = build.build(args.out, args.documents, args.seed, args.extra_rows)
    print(json.dumps(manifest, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
