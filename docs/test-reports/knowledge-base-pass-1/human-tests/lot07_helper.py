"""Does the file manipulations of the lot 7 human test, so nobody edits a file by hand.

Run it from the repository root with Python 3:

    python docs\\test-reports\\knowledge-base-pass-1\\human-tests\\lot07_helper.py <command>

It is the lot 6 helper under other names, so the two tests never share a backup or a test folder:

    status                  what the script sees: folders, mode, timing log, the test documents folder
    backup                  copy index.sqlite3, settings.json and the key file to the backup folder
                            "kb-backup-lot-07" (in your home folder)
    restore                 put them back (and remove a key that was not there before)
    prepare                 make a folder "kb-lot07" beside your Documents folder with the same thirty
                            invented documents as lot 6, point the application's Documents folder at
                            it, point the Data folder at an empty folder "kb-lot07-data" beside it,
                            set the knowledge mode to "suggest" and switch the timing log on
    clear-log               empty the questions' record (retrieval-timings.jsonl and kb_diagnostics)
    write-fixtures <dir>    write the thirty documents into <dir> and change nothing else (used by the
                            developer's own check; not part of the human test)

The knowledge mode and the thresholds are changed in the application itself this time, in the
settings window; nothing here edits them. Close the application first for every command that touches
the settings, the index or the log (the script refuses otherwise; --even-if-running overrides that).
Nothing here reads, prints or sends the content of a document. The documents are invented.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import lot06_helper as base  # noqa: E402

base.BACKUP_NAME = "kb-backup-lot-07"
base.TEST_FOLDER_NAME = "kb-lot07"

COMMANDS = {"status", "backup", "restore", "prepare", "clear-log", "write-fixtures"}
EMPTY_DATA_FOLDER = "kb-lot07-data"


def prepare_with_an_empty_data_folder() -> int:
    """The lot 6 preparation, and the Data folder pointed at an empty folder beside it. The owner's
    first run (11 October 2026) had her own Data folder analysed, so the name picker also offered
    the names of those workbooks (Dupont J, Sophie Martin, Clavier Mecanique) and "Add to my
    selection" ticked a workbook: not what the protocol describes. The previous setting comes back
    with restore, which puts settings.json back."""
    code = base.prepare()
    if code != 0:
        return code
    settings = base.read_settings()
    current = settings.get("dataFolder")
    if not current:
        base.say("no Data folder is chosen: nothing to point at an empty folder")
        return 0
    current_path = Path(current)
    folder = current_path if current_path.name == EMPTY_DATA_FOLDER else current_path.parent / EMPTY_DATA_FOLDER
    folder.mkdir(parents=True, exist_ok=True)
    if current_path.name != EMPTY_DATA_FOLDER:
        (base.backup_dir() / "previous-data-folder.txt").write_text(current, encoding="utf-8")
        settings["dataFolder"] = str(folder)
        base.write_settings(settings)
    base.say(f'"dataFolder" is now {folder}, an empty folder (it was {current}; "restore" puts it back)')
    return 0


base.COMMANDS["prepare"] = (prepare_with_an_empty_data_folder, True)

if __name__ == "__main__":
    arguments = [a for a in sys.argv[1:] if a != "--even-if-running"]
    if not arguments or arguments[0] not in COMMANDS:
        print(__doc__)
        sys.exit(2)
    sys.exit(base.main(sys.argv))
