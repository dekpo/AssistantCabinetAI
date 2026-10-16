"""Guard for `scripts/gen-knowledge-sandbox.py`: the default profile writes the same bytes as before the
`demo` profile existed.

The figures already published (`docs/test-reports/knowledge-base-pass-1/`) are computed on this library, so a
change to the generator that moves one byte of it is a defect. The digests below were taken on the generator as
it stood on 16 October 2026, before the `demo` profile was added.

    python -m unittest scripts.test_gen_knowledge_sandbox            (from the repository root)
    ACAI_SKIP_SLOW=1 python -m unittest ...                          (skips the 1 000-document run)

A digest is the SHA-256 of the sorted list of `relative path, SHA-256 of the content`. A `.json` or `.md` file
is hashed with Unix line ends, because Python writes text with the platform's line ends and the digest must be
the same on Windows and on macOS.
"""

from __future__ import annotations

import hashlib
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "scripts" / "gen-knowledge-sandbox.py"

# `python scripts/gen-knowledge-sandbox.py` (the versioned core: 200 documents)
DIGEST_CORE_200 = "da12e6c8f8fedd45ac8ba582e5a4271ee64132edad4fb5c65aa8246fff24e161"
# `python scripts/gen-knowledge-sandbox.py --documents 1000`
DIGEST_1000 = "9a4efef03923654928b353053f3f636a3f7df302410fd0d4c18a93e3e98c694c"

TEXT_EXTENSIONS = {".json", ".md"}


def tree_digest(folder: Path) -> str:
    rows = []
    for path in sorted(p for p in folder.rglob("*") if p.is_file()):
        content = path.read_bytes()
        if path.suffix in TEXT_EXTENSIONS:
            content = content.replace(b"\r\n", b"\n")
        rows.append(f"{path.relative_to(folder).as_posix()},{hashlib.sha256(content).hexdigest()}")
    return hashlib.sha256("\n".join(rows).encode("utf-8")).hexdigest()


def generate(out: Path, *arguments: str) -> None:
    subprocess.run([sys.executable, str(SCRIPT), *arguments, "--out", str(out)], check=True,
                   stdout=subprocess.DEVNULL, cwd=ROOT)


class DefaultProfileIsUnchanged(unittest.TestCase):
    def test_the_core_of_200_documents_is_the_same_bytes_as_before_the_demo_profile(self):
        with tempfile.TemporaryDirectory() as first, tempfile.TemporaryDirectory() as second:
            generate(Path(first) / "lib", "--documents", "200")
            generate(Path(second) / "lib", "--documents", "200")
            digest = tree_digest(Path(first) / "lib")
            self.assertEqual(digest, tree_digest(Path(second) / "lib"), "two runs give the same bytes")
            self.assertEqual(digest, DIGEST_CORE_200)

    @unittest.skipIf(os.environ.get("ACAI_SKIP_SLOW") == "1", "the 1 000-document run takes about a minute")
    def test_the_library_of_1000_documents_is_the_same_bytes_as_before_the_demo_profile(self):
        with tempfile.TemporaryDirectory() as folder:
            generate(Path(folder) / "lib", "--documents", "1000")
            self.assertEqual(tree_digest(Path(folder) / "lib"), DIGEST_1000)


if __name__ == "__main__":
    unittest.main()
