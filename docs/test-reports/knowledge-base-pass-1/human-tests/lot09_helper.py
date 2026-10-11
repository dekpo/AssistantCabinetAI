"""Does the file manipulations of the lot 9 human test, so nobody edits a file by hand.

Run it from the repository root with Python 3:

    python docs\\test-reports\\knowledge-base-pass-1\\human-tests\\lot09_helper.py <command>

It is the lot 8 helper under other names, so the two tests never share a backup or a test folder, with
two small spreadsheets added to the Data folder (the suggestions of lot 9 are made of them):

    status                  what the script sees: folders, mode, the test folders
    backup                  copy index.sqlite3, settings.json and the key file to the backup folder
                            "kb-backup-lot-09" (in your home folder)
    restore                 put them back (and remove a key that was not there before)
    prepare                 make a folder "kb-lot09" beside your Documents folder with the thirty-one
                            invented documents of lot 8, and a folder "kb-lot09-data" beside your Data
                            folder with two invented spreadsheets, "rdv.csv" and "factures.csv";
                            point the application's two folders at them; set the knowledge mode to
                            "suggest"; tick the setting "Masquer les noms dans les suggestions"
    fingerprint             remember the exact content of every file of both test folders (a SHA-256
                            for each one) in the backup folder
    verify-files            compare both folders with what "fingerprint" remembered: prints OK when
                            every file is byte for byte the same and nothing was added or removed
    write-fixtures <dir>    write the documents into <dir>\\docs and the spreadsheets into <dir>\\data and
                            change nothing else (used by the developer's own check; not part of the
                            human test)

Close the application first for every command that touches the settings or the index (the script
refuses otherwise; --even-if-running overrides that). Nothing here reads, prints or sends the content
of a document. The documents and the spreadsheets are invented.
"""

import hashlib
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import lot08_helper  # noqa: E402,F401  (sets the lot 8 names and the thirty-one documents)
import lot06_helper as base  # noqa: E402

base.BACKUP_NAME = "kb-backup-lot-09"
base.TEST_FOLDER_NAME = "kb-lot09"
DATA_FOLDER_NAME = "kb-lot09-data"
FINGERPRINT_FILE = "files-fingerprint-lot-09.txt"

# Dr Jean Dupont and Mrs Claire Martin are in the documents too; Alice Archer and Emma Evans are only
# in the table. Every date has one day above twelve, so the date column is a real column of dates.
WORKBOOKS = {
    "rdv.csv": (
        "date;patient;medecin;duree_min;montant\n"
        "02/03/2026;Claire Martin;Dr Jean Dupont;20;50\n"
        "03/03/2026;Alice Archer;Dr Jean Dupont;25;60\n"
        "13/03/2026;Claire Martin;Dr Jean Dupont;20;50\n"
        "17/03/2026;Emma Evans;Dr Paul Martin;45;80\n"
        "24/03/2026;Alice Archer;Dr Pierre Martin;30;60\n"
    ),
    "factures.csv": (
        "date;fournisseur;montant\n"
        "02/03/2026;MedSupply;450\n"
        "03/03/2026;PapeterieDuParc;180\n"
        "14/03/2026;MedSupply;620\n"
    ),
}


def write_fixtures(folder: Path) -> int:
    documents = folder / "docs"
    data = folder / "data"
    documents.mkdir(parents=True, exist_ok=True)
    data.mkdir(parents=True, exist_ok=True)
    files = base.fixtures()
    for name, text in files.items():
        (documents / name).write_text(text, encoding="utf-8")
    for name, text in WORKBOOKS.items():
        (data / name).write_text(text, encoding="utf-8", newline="\n")
    return len(files)


def documents_folder() -> Path:
    settings = base.read_settings()
    current = settings.get("workFolder")
    if not current or Path(current).name != base.TEST_FOLDER_NAME:
        raise FileNotFoundError("the Documents folder is not the test folder: run 'prepare' first")
    return Path(current)


def data_folder() -> Path:
    settings = base.read_settings()
    current = settings.get("dataFolder")
    if not current or Path(current).name != DATA_FOLDER_NAME:
        raise FileNotFoundError("the Data folder is not the test folder: run 'prepare' first")
    return Path(current)


def digests() -> dict[str, str]:
    found: dict[str, str] = {}
    for label, folder in (("docs", documents_folder()), ("data", data_folder())):
        for path in sorted(folder.rglob("*")):
            if path.is_file():
                name = f"{label}/{str(path.relative_to(folder)).replace(chr(92), '/')}"
                found[name] = hashlib.sha256(path.read_bytes()).hexdigest()
    return found


def fingerprint() -> int:
    found = digests()
    target = base.backup_dir()
    target.mkdir(parents=True, exist_ok=True)
    lines = [f"{name}  {digest}" for name, digest in found.items()]
    (target / FINGERPRINT_FILE).write_text("\n".join(lines) + "\n", encoding="utf-8")
    base.say(f"remembered {len(found)} files of {documents_folder()} and {data_folder()}")
    return 0


def verify_files() -> int:
    saved_file = base.backup_dir() / FINGERPRINT_FILE
    if not saved_file.is_file():
        return base.fail("run 'fingerprint' first")
    saved = dict(line.split("  ", 1) for line in saved_file.read_text(encoding="utf-8").splitlines() if line)
    now = digests()
    changed = sorted(name for name in saved if name in now and now[name] != saved[name])
    removed = sorted(name for name in saved if name not in now)
    added = sorted(name for name in now if name not in saved)
    base.say(f"{len(now)} files now, {len(saved)} remembered")
    for label, names in (("CHANGED", changed), ("REMOVED", removed), ("ADDED", added)):
        for name in names:
            base.say(f"{label}  {name}")
    if changed or removed or added:
        base.say("NOT THE SAME")
        return 1
    base.say("OK: every file is byte for byte the same, none added, none removed")
    return 0


def prepare() -> int:
    code = base.prepare()
    if code != 0:
        return code
    settings = base.read_settings()
    settings["knowledgeHideNamesInSuggestions"] = True
    base.write_settings(settings)
    current = settings.get("dataFolder")
    if not current:
        return base.fail(
            "no Data folder is chosen in the application yet: choose or create one on the Data "
            "card first, so that the test folder can be made beside it"
        )
    current_path = Path(current)
    folder = current_path if current_path.name == DATA_FOLDER_NAME else current_path.parent / DATA_FOLDER_NAME
    folder.mkdir(parents=True, exist_ok=True)
    for name, text in WORKBOOKS.items():
        (folder / name).write_text(text, encoding="utf-8", newline="\n")
    base.say(f"wrote {len(WORKBOOKS)} invented spreadsheets in {folder}")
    if current_path.name != DATA_FOLDER_NAME:
        (base.backup_dir() / "previous-data-folder.txt").write_text(current, encoding="utf-8")
        settings["dataFolder"] = str(folder)
        base.write_settings(settings)
    base.say(f'"dataFolder" is now {folder} (it was {current}; "restore" puts it back)')
    return 0


COMMANDS = {"status", "backup", "restore", "prepare", "fingerprint", "verify-files", "write-fixtures"}
base.COMMANDS["prepare"] = (prepare, True)
base.COMMANDS["fingerprint"] = (fingerprint, False)
base.COMMANDS["verify-files"] = (verify_files, False)

if __name__ == "__main__":
    arguments = [a for a in sys.argv[1:] if a != "--even-if-running"]
    if not arguments or arguments[0] not in COMMANDS:
        print(__doc__)
        sys.exit(2)
    if arguments[0] == "write-fixtures":
        if len(arguments) != 2:
            sys.exit(base.fail("usage: write-fixtures <folder>"))
        base.say(f"wrote {write_fixtures(Path(arguments[1]))} invented documents and {len(WORKBOOKS)} spreadsheets in {arguments[1]}")
        sys.exit(0)
    sys.exit(base.main(sys.argv))
