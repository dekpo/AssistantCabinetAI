"""Does the file manipulations of the lot 8 human test, so nobody edits a file by hand.

Run it from the repository root with Python 3:

    python docs\\test-reports\\knowledge-base-pass-1\\human-tests\\lot08_helper.py <command>

It is the lot 7 helper under other names, so the two tests never share a backup or a test folder, plus
two commands that prove no file of the test folder was touched:

    status                  what the script sees: folders, mode, the test documents folder
    backup                  copy index.sqlite3, settings.json and the key file to the backup folder
                            "kb-backup-lot-08" (in your home folder)
    restore                 put them back (and remove a key that was not there before)
    prepare                 make a folder "kb-lot08" beside your Documents folder with the thirty
                            invented documents of lots 6 and 7 plus one more, "note-jeannot.txt", point
                            the application's Documents folder at it, point the Data folder at an empty
                            folder "kb-lot08-data" beside it, set the knowledge mode to "suggest"
    fingerprint             remember the exact content of every file of "kb-lot08" (a SHA-256 for each
                            one) in the backup folder
    verify-files            compare the folder with what "fingerprint" remembered: prints OK when every
                            file is byte for byte the same and nothing was added or removed
    write-fixtures <dir>    write the thirty-one documents into <dir> and change nothing else (used by
                            the developer's own check; not part of the human test)

Close the application first for every command that touches the settings or the index (the script
refuses otherwise; --even-if-running overrides that). Nothing here reads, prints or sends the content
of a document. The documents are invented.
"""

import hashlib
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import lot07_helper  # noqa: E402,F401  (sets the lot 7 names, replaced just below)
import lot06_helper as base  # noqa: E402

base.BACKUP_NAME = "kb-backup-lot-08"
base.TEST_FOLDER_NAME = "kb-lot08"

# A nickname no document explains: nobody knows who Jeannot is until a name is added by hand.
EXTRA = (
    "note-jeannot.txt",
    "mardi matin, Jeannot est passé pour un contrôle de routine, sans rendez-vous.\n",
)
FINGERPRINT_FILE = "files-fingerprint.txt"
EMPTY_DATA_FOLDER = "kb-lot08-data"

_lot06_fixtures = base.fixtures


def fixtures() -> dict:
    files = _lot06_fixtures()
    files[EXTRA[0]] = EXTRA[1]
    return files


base.fixtures = fixtures


def test_folder() -> Path:
    settings = base.read_settings()
    current = settings.get("workFolder")
    if not current:
        raise FileNotFoundError("no Documents folder is chosen: run 'prepare' first")
    folder = Path(current)
    if folder.name != base.TEST_FOLDER_NAME:
        raise FileNotFoundError(f"the Documents folder is {folder}, not the test folder: run 'prepare' first")
    return folder


def digest_of(folder: Path) -> dict[str, str]:
    return {
        str(path.relative_to(folder)).replace("\\", "/"): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(folder.rglob("*"))
        if path.is_file()
    }


def fingerprint() -> int:
    folder = test_folder()
    digests = digest_of(folder)
    target = base.backup_dir()
    target.mkdir(parents=True, exist_ok=True)
    lines = [f"{name}  {digest}" for name, digest in digests.items()]
    (target / FINGERPRINT_FILE).write_text("\n".join(lines) + "\n", encoding="utf-8")
    base.say(f"remembered {len(digests)} files of {folder}")
    return 0


def verify_files() -> int:
    folder = test_folder()
    saved_file = base.backup_dir() / FINGERPRINT_FILE
    if not saved_file.is_file():
        return base.fail("run 'fingerprint' first")
    saved = dict(line.split("  ", 1) for line in saved_file.read_text(encoding="utf-8").splitlines() if line)
    now = digest_of(folder)
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


def prepare_with_an_empty_data_folder() -> int:
    code = base.prepare()
    if code != 0:
        return code
    settings = base.read_settings()
    current = settings.get("dataFolder")
    if not current:
        base.say("no Data folder is chosen: nothing to point at an empty folder")
        return 0
    current_path = Path(current)
    folder = (
        current_path
        if current_path.name == EMPTY_DATA_FOLDER
        else current_path.parent / EMPTY_DATA_FOLDER
    )
    folder.mkdir(parents=True, exist_ok=True)
    if current_path.name != EMPTY_DATA_FOLDER:
        (base.backup_dir() / "previous-data-folder.txt").write_text(current, encoding="utf-8")
        settings["dataFolder"] = str(folder)
        base.write_settings(settings)
    base.say(f'"dataFolder" is now {folder}, an empty folder (it was {current}; "restore" puts it back)')
    return 0


COMMANDS = {"status", "backup", "restore", "prepare", "fingerprint", "verify-files", "write-fixtures"}
base.COMMANDS["prepare"] = (prepare_with_an_empty_data_folder, True)
base.COMMANDS["fingerprint"] = (fingerprint, False)
base.COMMANDS["verify-files"] = (verify_files, False)

if __name__ == "__main__":
    arguments = [a for a in sys.argv[1:] if a != "--even-if-running"]
    if not arguments or arguments[0] not in COMMANDS:
        print(__doc__)
        sys.exit(2)
    sys.exit(base.main(sys.argv))
