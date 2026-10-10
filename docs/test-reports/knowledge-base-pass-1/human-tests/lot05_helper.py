"""Does the file manipulations of the lot 5 human test, so nobody edits a file by hand.

Run it from the repository root with Python 3 (the same Python as show_kb_tables.py):

    python docs\\test-reports\\knowledge-base-pass-1\\human-tests\\lot05_helper.py <command>

Close the application first for every command that touches the settings, the index or the key (the
script checks and refuses while the application runs; --even-if-running overrides that).

Commands

    status                  what the script sees: folders, mode, packs, key, the test data folder
    backup                  copy index.sqlite3, settings.json and the key file to a backup folder
    restore                 put them back (and remove a key that was not there before)
    prepare-data            make a folder "kb-lot05" beside your Data folder, copy the three fictional
                            workbooks of the first human pass into it, and point the application's Data
                            folder at it (the previous setting is kept in the backup and comes back with
                            restore)
    mode off                set "knowledgeMode" in settings.json: off | suggest | auto
    mode suggest
    edit-workbook           in the test data folder, replace the patient "Simon V" by "Garnier C" in
                            rdv-mars-2026.xlsx (the original is kept)
    restore-workbook        put the original back
    hide-workbook           move factures-fournisseurs-2026.xlsx out of the test data folder
    restore-hidden          move it back
    age-aliases             (fix 3, surname then initial) put the aliases of "Dupont J" back the way
                            the alias maker of lot 5 left them (a surname "J", an initial "D. J"), and
                            forget the possible matches and the alias generation, so the next Analyse
                            has an old base to correct. Index only; reads and sends no document
    stop-ai                 docker compose stop   (the AI machine, so nothing can answer a request)
    start-ai                docker compose up -d

Nothing here reads, prints or sends the content of a workbook. The test uses the FICTIONAL fixtures
only (docs/test-reports/human-acceptance-pass-1/fixtures).
"""

import json
import os
import shutil
import sqlite3
import subprocess
import sys
import zipfile
from pathlib import Path

APP_FOLDER = "com.assistantcabinetai.desktop"
KEY_FILE = "knowledge-identifier.key"
INDEX_FILE = "index.sqlite3"
SETTINGS_FILE = "settings.json"
PROCESS_NAME = "assistant-cabinet-ai.exe"
BACKUP_NAME = "kb-backup-lot-05"
TEST_FOLDER_NAME = "kb-lot05"
RDV = "rdv-mars-2026.xlsx"
INVOICES = "factures-fournisseurs-2026.xlsx"
MAILING = "donnees_publipostage.csv"
REPLACE_FROM = "Simon V"
REPLACE_TO = "Garnier C"

# Where the three fictional workbooks are, relative to the repository root.
FIXTURES = (
    ("docs/test-reports/human-acceptance-pass-1/fixtures/data", RDV),
    ("docs/test-reports/human-acceptance-pass-1/fixtures/data", INVOICES),
    ("docs/test-reports/human-acceptance-pass-1/fixtures/publipostage", MAILING),
)


def config_dir() -> Path:
    override = os.environ.get("ACAI_CONFIG_DIR")
    if override:
        return Path(override)
    roaming = os.environ.get("APPDATA")
    if roaming:
        return Path(roaming) / APP_FOLDER
    return Path.home() / "Library" / "Application Support" / APP_FOLDER


def data_dir() -> Path:
    override = os.environ.get("ACAI_DATA_DIR")
    if override:
        return Path(override)
    local = os.environ.get("LOCALAPPDATA")
    if local:
        return Path(local) / APP_FOLDER
    return Path.home() / "Library" / "Application Support" / APP_FOLDER


def backup_dir() -> Path:
    override = os.environ.get("ACAI_BACKUP_DIR")
    return Path(override) if override else Path.home() / BACKUP_NAME


def repository_root() -> Path:
    override = os.environ.get("ACAI_REPOSITORY")
    return Path(override) if override else Path(__file__).resolve().parents[4]


def say(text: str = "") -> None:
    print(text)


def fail(text: str) -> int:
    print(f"ERROR: {text}")
    return 1


def application_running() -> bool:
    if os.name != "nt":
        return False
    try:
        out = subprocess.run(
            ["tasklist", "/FI", f"IMAGENAME eq {PROCESS_NAME}"],
            capture_output=True,
            text=True,
            check=False,
        ).stdout
    except OSError:
        return False
    return PROCESS_NAME.lower() in out.lower()


def read_settings() -> dict:
    path = config_dir() / SETTINGS_FILE
    if not path.is_file():
        raise FileNotFoundError(f"no {SETTINGS_FILE} at {path}: start the application once first")
    # utf-8-sig: tolerate a byte-order mark if another program saved the file; this script writes none.
    return json.loads(path.read_text(encoding="utf-8-sig"))


def write_settings(settings: dict) -> None:
    path = config_dir() / SETTINGS_FILE
    path.write_text(json.dumps(settings, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def test_data_folder() -> Path:
    """The folder `prepare-data` made, wherever the settings point now."""
    folder = read_settings().get("dataFolder")
    if not folder:
        raise FileNotFoundError("no Data folder is chosen in the application yet: run prepare-data")
    path = Path(folder)
    if path.name != TEST_FOLDER_NAME:
        raise FileNotFoundError(
            f"the Data folder is {path}, not a {TEST_FOLDER_NAME} folder: run prepare-data first, "
            "so that this script never touches your own workbooks"
        )
    if not path.is_dir():
        raise FileNotFoundError(f"the test data folder {path} does not exist: run prepare-data")
    return path


# ------------------------------------------------------------------------------------ commands


def status() -> int:
    say(f"Settings folder : {config_dir()}")
    say(f"Index folder    : {data_dir()}")
    say(f"Backup folder   : {backup_dir()} ({'exists' if backup_dir().is_dir() else 'not made yet'})")
    say(f"Application     : {'RUNNING' if application_running() else 'closed'}")
    try:
        settings = read_settings()
        say(f"knowledgeMode   : {settings.get('knowledgeMode', '(absent: suggest)')}")
        say(f"knowledgePacks  : {settings.get('knowledgePacks', '(absent: health-fr)')}")
        say(f"Documents folder: {settings.get('workFolder')}")
        data = settings.get("dataFolder")
        marker = "  <- the test folder" if data and Path(data).name == TEST_FOLDER_NAME else ""
        say(f"Data folder     : {data}{marker}")
        if data and Path(data).is_dir() and Path(data).name == TEST_FOLDER_NAME:
            names = sorted(p.name for p in Path(data).iterdir() if p.is_file())
            say(f"  holds         : {', '.join(names) if names else '(nothing)'}")
    except FileNotFoundError as error:
        say(f"Settings        : {error}")
    say(f"Index           : {'present' if (data_dir() / INDEX_FILE).is_file() else 'absent'}")
    say(f"Identifier key  : {'present' if (data_dir() / KEY_FILE).is_file() else 'absent'}")
    return 0


def backup() -> int:
    target = backup_dir()
    target.mkdir(parents=True, exist_ok=True)
    pairs = [
        (data_dir() / INDEX_FILE, target / INDEX_FILE),
        (config_dir() / SETTINGS_FILE, target / SETTINGS_FILE),
        (data_dir() / KEY_FILE, target / KEY_FILE),
    ]
    marker = target / "key-was-absent.txt"
    for source, copy in pairs:
        if source.is_file():
            shutil.copy2(source, copy)
            say(f"saved   {source.name}")
        else:
            say(f"absent  {source.name} (nothing to save)")
    if not (data_dir() / KEY_FILE).is_file():
        marker.write_text("no key file when the backup was made\n", encoding="utf-8")
    elif marker.exists():
        marker.unlink()
    say(f"Backup in {target}")
    return 0


def restore() -> int:
    source = backup_dir()
    if not source.is_dir():
        return fail(f"no backup folder at {source}: run 'backup' first")
    data_dir().mkdir(parents=True, exist_ok=True)
    config_dir().mkdir(parents=True, exist_ok=True)
    for name, folder in ((INDEX_FILE, data_dir()), (SETTINGS_FILE, config_dir())):
        saved = source / name
        if saved.is_file():
            shutil.copy2(saved, folder / name)
            say(f"restored {name}")
    key_target = data_dir() / KEY_FILE
    saved_key = source / KEY_FILE
    if saved_key.is_file():
        shutil.copy2(saved_key, key_target)
        say(f"restored {KEY_FILE}")
    elif key_target.is_file():
        key_target.unlink()
        say(f"removed {KEY_FILE} (there was none when the backup was made)")
    return 0


def prepare_data() -> int:
    settings = read_settings()
    current = settings.get("dataFolder")
    if not current:
        return fail(
            "no Data folder is chosen in the application yet: choose or create one on the Data card "
            "first, so that the test folder can be made beside it"
        )
    current_path = Path(current)
    if current_path.name == TEST_FOLDER_NAME:
        folder = current_path
    else:
        folder = current_path.parent / TEST_FOLDER_NAME
    if not backup_dir().is_dir():
        return fail("run 'backup' first: the Data folder setting is about to change")
    folder.mkdir(parents=True, exist_ok=True)
    root = repository_root()
    for relative, name in FIXTURES:
        source = root / relative / name
        if not source.is_file():
            return fail(f"the fixture {source} is missing")
        shutil.copy2(source, folder / name)
        say(f"copied  {name}")
    if current_path.name != TEST_FOLDER_NAME:
        (backup_dir() / "previous-data-folder.txt").write_text(current, encoding="utf-8")
        settings["dataFolder"] = str(folder)
        write_settings(settings)
        say(f'"dataFolder" is now {folder} (it was {current}; "restore" puts it back)')
    else:
        say(f"The test folder is already the Data folder: {folder}")
    say("Start the application now.")
    return 0


def set_mode(value: str) -> int:
    if value not in ("off", "suggest", "auto"):
        return fail("mode must be off, suggest or auto")
    settings = read_settings()
    settings["knowledgeMode"] = value
    write_settings(settings)
    say(f'knowledgeMode is now "{value}"')
    say("Start the application now.")
    return 0


def edit_workbook() -> int:
    folder = test_data_folder()
    document = folder / RDV
    if not document.is_file():
        return fail(f"{RDV} is not in {folder}")
    keep = backup_dir() / "held-files"
    keep.mkdir(parents=True, exist_ok=True)
    original = keep / (RDV + ".original")
    if not original.exists():
        shutil.copy2(document, original)
    replaced = 0
    temporary = document.with_suffix(".xlsx.tmp")
    with zipfile.ZipFile(original) as source, zipfile.ZipFile(
        temporary, "w", zipfile.ZIP_DEFLATED
    ) as target:
        for item in source.infolist():
            data = source.read(item.filename)
            if item.filename == "xl/worksheets/sheet1.xml":
                text = data.decode("utf-8")
                marker = f"<t>{REPLACE_FROM}</t>"
                replaced = text.count(marker)
                data = text.replace(marker, f"<t>{REPLACE_TO}</t>").encode("utf-8")
            target.writestr(item, data)
    if replaced == 0:
        temporary.unlink()
        return fail(f'"{REPLACE_FROM}" was not found in {RDV}: nothing changed')
    temporary.replace(document)
    say(f'{RDV}: "{REPLACE_FROM}" replaced by "{REPLACE_TO}" {replaced} time(s).')
    say("Now press Analyser on the Data card.")
    return 0


def restore_workbook() -> int:
    original = backup_dir() / "held-files" / (RDV + ".original")
    if not original.is_file():
        return fail("no original was kept: nothing to restore")
    shutil.copy2(original, test_data_folder() / RDV)
    say(f"{RDV} is back as it was.")
    return 0


def hide_workbook() -> int:
    folder = test_data_folder()
    workbook = folder / INVOICES
    if not workbook.is_file():
        return fail(f"{INVOICES} is not in {folder}")
    keep = backup_dir() / "held-files"
    keep.mkdir(parents=True, exist_ok=True)
    shutil.move(str(workbook), str(keep / INVOICES))
    say(f"{INVOICES} moved out of the test data folder (kept in {keep}).")
    say("Now press Analyser on the Data card.")
    return 0


def restore_hidden() -> int:
    held = backup_dir() / "held-files" / INVOICES
    if not held.is_file():
        return fail("the workbook is not being held: nothing to put back")
    shutil.move(str(held), str(test_data_folder() / INVOICES))
    say(f"{INVOICES} is back in the test data folder.")
    return 0


def age_aliases() -> int:
    """Make the index look like one the alias maker of lot 5 filled, for the fix 3 human test."""
    index = data_dir() / INDEX_FILE
    if not index.is_file():
        return fail(f"no index at {index}: start the application and press Analyser first")
    connection = sqlite3.connect(index)
    try:
        row = connection.execute(
            "SELECT entity_id FROM kb_entities WHERE canonical_name = ? AND type_id = 'person'",
            ("Dupont J",),
        ).fetchone()
        if row is None:
            return fail(
                "'Dupont J' is not in the knowledge base: press Analyser on the Data card first, "
                "with the three fictional workbooks of the test folder"
            )
        entity = row[0]
        with connection:
            connection.execute(
                "DELETE FROM kb_aliases WHERE entity_id = ? AND normalized <> 'dupont j'", (entity,)
            )
            for display, normalized, kind in (
                ("J Dupont", "j dupont", "reordered"),
                ("D. J", "d j", "initial"),
                ("J", "j", "partial_surname"),
            ):
                connection.execute(
                    "INSERT INTO kb_aliases (entity_id, display, normalized, phonetic_key, kind, "
                    "origin, confidence) VALUES (?, ?, ?, '', ?, 'automatic', 0.6)",
                    (entity, display, normalized, kind),
                )
            connection.execute("DELETE FROM kb_possible_matches")
            connection.execute("DELETE FROM kb_meta WHERE key = 'alias_generation'")
    finally:
        connection.close()
    say("The aliases of 'Dupont J' are the old ones again (a surname 'J', an initial 'D. J'),")
    say("no possible match is recorded, and the alias generation is forgotten.")
    say("Now start the application and press Analyser on the Data card.")
    return 0


def docker(*arguments: str) -> int:
    try:
        result = subprocess.run(["docker", "compose", *arguments], cwd=repository_root(), check=False)
    except OSError:
        return fail("docker was not found on this computer")
    return result.returncode


COMMANDS = {
    "status": (status, False),
    "backup": (backup, True),
    "restore": (restore, True),
    "prepare-data": (prepare_data, True),
    "edit-workbook": (edit_workbook, False),
    "restore-workbook": (restore_workbook, False),
    "hide-workbook": (hide_workbook, False),
    "restore-hidden": (restore_hidden, False),
    "age-aliases": (age_aliases, True),
}


def main(argv: list[str]) -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    arguments = [a for a in argv[1:] if a != "--even-if-running"]
    force = "--even-if-running" in argv
    if not arguments:
        print(__doc__)
        return 2
    name = arguments[0]
    try:
        if name == "mode":
            if len(arguments) != 2:
                return fail("usage: mode off|suggest|auto")
            if application_running() and not force:
                return fail("close the application first (or add --even-if-running)")
            return set_mode(arguments[1])
        if name == "stop-ai":
            return docker("stop")
        if name == "start-ai":
            return docker("up", "-d")
        if name not in COMMANDS:
            return fail(f"unknown command {name!r}; run it without argument for the list")
        action, needs_closed_app = COMMANDS[name]
        if needs_closed_app and application_running() and not force:
            return fail("close the application first (or add --even-if-running)")
        return action()
    except FileNotFoundError as error:
        return fail(str(error))


if __name__ == "__main__":
    sys.exit(main(sys.argv))
