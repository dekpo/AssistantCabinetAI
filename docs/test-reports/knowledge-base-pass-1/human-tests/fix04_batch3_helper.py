"""Writes the invented files of the fix 4, batch 3 human test, so nobody edits a file by hand.

Run it from the repository root with Python 3, after `lot11_helper.py prepare 30` (the test folders must exist):

    python docs\\test-reports\\knowledge-base-pass-1\\human-tests\\fix04_batch3_helper.py files

`files` copies three invented workbooks into "kb-lot11-data" and two invented documents (a "neurologie.txt" in
2026\\mars and another in 2026\\janvier) into "kb-lot11". It only adds files in the test folders; the settings and the
index are untouched (the backup and the restore are `lot11_helper.py backup` and `lot11_helper.py restore`).
Nothing here reads, prints or sends the content of a real document. The files are invented.
"""

import shutil
import sys
from pathlib import Path

import lot11_helper as base

FILES = Path(__file__).resolve().parent / "fix-04-batch-3-files"


def files() -> int:
    settings = base.read_settings()
    docs = base.docs_folder(settings)
    data = settings.get("dataFolder")
    if docs is None or not docs.is_dir() or not data or Path(data).name != base.DATA_NAME or not Path(data).is_dir():
        return base.fail("run 'lot11_helper.py prepare 30' first: the files are written in " + base.DOCS_NAME + " and " + base.DATA_NAME)
    for source in sorted((FILES / "data").glob("*.csv")):
        shutil.copy2(source, Path(data) / source.name)
        base.say(f"wrote {source.name} in {data}")
    for source in sorted((FILES / "docs").rglob("*.txt")):
        target = docs / source.relative_to(FILES / "docs")
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
        base.say(f"wrote {source.relative_to(FILES / 'docs')} in {docs}")
    base.say("Start the application, press Analyser on the Documents card and on the Data card.")
    return 0


def main(argv: list[str]) -> int:
    if argv[1:] == ["files"]:
        return files()
    base.say(__doc__ or "")
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
