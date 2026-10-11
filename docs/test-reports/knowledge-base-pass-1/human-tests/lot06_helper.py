"""Does the file manipulations of the lot 6 human test, so nobody edits a file by hand.

Run it from the repository root with Python 3 (the same Python as show_kb_tables.py):

    python docs\\test-reports\\knowledge-base-pass-1\\human-tests\\lot06_helper.py <command>

Close the application first for every command that touches the settings, the index or the log (the
script checks and refuses while the application runs; --even-if-running overrides that).

Commands

    status                  what the script sees: folders, mode, timing log, the test documents folder
    backup                  copy index.sqlite3, settings.json and the key file to a backup folder
    restore                 put them back (and remove a key that was not there before)
    prepare                 make a folder "kb-lot06" beside your Documents folder with thirty invented
                            documents, point the application's Documents folder at it, set the
                            knowledge mode to "suggest" and switch the timing log on (the previous
                            settings are kept in the backup and come back with restore)
    mode off                set "knowledgeMode" in settings.json: off | suggest | auto
    mode suggest
    mode auto
    min-scope <n>           set "knowledgeMinScopeFiles" (the number of selected files under which the
                            base leaves a side alone; the application's own value is 8). "min-scope
                            default" puts it back
    clear-log               empty the questions' record: delete retrieval-timings.jsonl and the rows of
                            kb_diagnostics (they hold numbers and codes only; no knowledge is touched)
    write-fixtures <dir>    write the thirty invented documents into <dir> and change nothing else
                            (used by the developer's own check; not part of the human test)

Nothing here reads, prints or sends the content of a document. The documents are invented.
"""

import json
import os
import shutil
import sqlite3
import subprocess
import sys
from pathlib import Path

APP_FOLDER = "com.assistantcabinetai.desktop"
KEY_FILE = "knowledge-identifier.key"
INDEX_FILE = "index.sqlite3"
SETTINGS_FILE = "settings.json"
TIMING_LOG = "retrieval-timings.jsonl"
PROCESS_NAME = "assistant-cabinet-ai.exe"
BACKUP_NAME = "kb-backup-lot-06"
TEST_FOLDER_NAME = "kb-lot06"

# Thirty invented documents: eight that name people, twenty-two plain notes. One person is called
# Dupont (three documents and a reminder that only says "M. Dupont"), three people are called Martin.
NAMED = {
    "dupont-bilan-2026.txt": (
        "Docteur Jean Dupont\nCabinet de médecine générale\n\n"
        "Bilan sanguin du 12 mars 2026.\n"
        "Cholestérol total : 2,1 grammes par litre. Glycémie à jeun : 0,95 gramme par litre. "
        "Le Docteur Jean Dupont recommande un contrôle dans six mois.\n"
    ),
    "dupont-lettre-confrere.txt": (
        "Docteur Jean Dupont\n\nCher confrère,\n"
        "Je vous adresse Madame Exemple pour un avis cardiologique, à la suite de palpitations "
        "survenues à l'effort depuis trois semaines.\nBien cordialement, Docteur Jean Dupont\n"
    ),
    "dupont-horaires.txt": (
        "Le Docteur Jean Dupont reçoit le lundi et le jeudi, de 9 h à 12 h, sur rendez-vous.\n"
        "Les urgences sont orientées vers le secrétariat du cabinet.\n"
    ),
    "rappel-m-dupont.txt": (
        "Rappel : M. Dupont est attendu mardi à 10 h pour le suivi de son traitement.\n"
    ),
    "martin-controle.txt": (
        "Madame Claire Martin a été reçue le 3 mars 2026 pour une consultation de contrôle. "
        "La tension est stable : 12/8. Aucun nouveau symptôme n'est signalé.\n"
    ),
    "martin-ordonnance.txt": (
        "Ordonnance de Madame Claire Martin : renouvellement du traitement habituel pour trois mois, "
        "avec une prise de sang avant le prochain rendez-vous.\n"
    ),
    "paul-martin.txt": "Docteur Paul Martin reçoit le lundi, de 14 h à 18 h, au cabinet du Parc.\n",
    "pierre-martin.txt": "Docteur Pierre Martin reçoit le mardi, de 8 h à 12 h, au cabinet des Lilas.\n",
}
NOTES = 21
PHOTOCOPIER = (
    "note-photocopieuse.txt",
    "Note interne : le contrat de maintenance de la photocopieuse arrive à échéance en décembre. "
    "À renouveler avant la fin du mois de novembre.\n",
)


def fixtures() -> dict:
    files = dict(NAMED)
    for number in range(1, NOTES + 1):
        files[f"note-{number:02d}.txt"] = (
            f"Note interne numéro {number} : commande de fournitures de bureau, planning des "
            "permanences et entretien du matériel du cabinet.\n"
        )
    files[PHOTOCOPIER[0]] = PHOTOCOPIER[1]
    return files


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


def write_fixtures(folder: Path) -> int:
    folder.mkdir(parents=True, exist_ok=True)
    files = fixtures()
    for name, text in files.items():
        (folder / name).write_text(text, encoding="utf-8")
    return len(files)


# ------------------------------------------------------------------------------------ commands


def status() -> int:
    say(f"Settings folder : {config_dir()}")
    say(f"Index folder    : {data_dir()}")
    say(f"Backup folder   : {backup_dir()} ({'exists' if backup_dir().is_dir() else 'not made yet'})")
    say(f"Application     : {'RUNNING' if application_running() else 'closed'}")
    try:
        settings = read_settings()
        say(f"knowledgeMode   : {settings.get('knowledgeMode', '(absent: suggest)')}")
        say(f"writeTimingLog  : {settings.get('writeTimingLog', '(absent: false)')}")
        folder = settings.get("workFolder")
        marker = "  <- the test folder" if folder and Path(folder).name == TEST_FOLDER_NAME else ""
        say(f"Documents folder: {folder}{marker}")
        if folder and Path(folder).is_dir() and Path(folder).name == TEST_FOLDER_NAME:
            say(f"  holds         : {sum(1 for p in Path(folder).iterdir() if p.is_file())} files")
    except FileNotFoundError as error:
        say(f"Settings        : {error}")
    log = data_dir() / TIMING_LOG
    lines = sum(1 for _ in log.open(encoding="utf-8")) if log.is_file() else 0
    say(f"Timing log      : {'present' if log.is_file() else 'absent'} ({lines} line(s))")
    say(f"Index           : {'present' if (data_dir() / INDEX_FILE).is_file() else 'absent'}")
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


def prepare() -> int:
    settings = read_settings()
    current = settings.get("workFolder")
    if not current:
        return fail(
            "no Documents folder is chosen in the application yet: choose or create one on the "
            "Documents card first, so that the test folder can be made beside it"
        )
    current_path = Path(current)
    folder = current_path if current_path.name == TEST_FOLDER_NAME else current_path.parent / TEST_FOLDER_NAME
    if not backup_dir().is_dir():
        return fail("run 'backup' first: the Documents folder setting is about to change")
    count = write_fixtures(folder)
    say(f"wrote {count} invented documents in {folder}")
    if current_path.name != TEST_FOLDER_NAME:
        (backup_dir() / "previous-work-folder.txt").write_text(current, encoding="utf-8")
        settings["workFolder"] = str(folder)
    settings["knowledgeMode"] = "suggest"
    settings["writeTimingLog"] = True
    write_settings(settings)
    say(f'"workFolder" is now {folder} (it was {current}; "restore" puts it back)')
    say('"knowledgeMode" is "suggest" and "writeTimingLog" is true.')
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


def set_min_scope(value: str) -> int:
    settings = read_settings()
    if value == "default":
        settings.pop("knowledgeMinScopeFiles", None)
        say('"knowledgeMinScopeFiles" removed: the default value (8) applies')
    else:
        if not value.isdigit() or int(value) < 1:
            return fail("min-scope takes a whole number of 1 or more, or the word default")
        settings["knowledgeMinScopeFiles"] = int(value)
        say(f'"knowledgeMinScopeFiles" is now {int(value)}')
    write_settings(settings)
    say("Start the application now.")
    return 0


def clear_log() -> int:
    log = data_dir() / TIMING_LOG
    if log.is_file():
        log.unlink()
        say(f"deleted {TIMING_LOG}")
    else:
        say(f"no {TIMING_LOG} to delete")
    index = data_dir() / INDEX_FILE
    if index.is_file():
        connection = sqlite3.connect(index)
        try:
            with connection:
                exists = connection.execute(
                    "SELECT 1 FROM sqlite_master WHERE name = 'kb_diagnostics'"
                ).fetchone()
                if exists:
                    deleted = connection.execute("DELETE FROM kb_diagnostics").rowcount
                    say(f"emptied kb_diagnostics ({deleted} row(s))")
                else:
                    say("kb_diagnostics does not exist yet")
        finally:
            connection.close()
    return 0


COMMANDS = {
    "status": (status, False),
    "backup": (backup, True),
    "restore": (restore, True),
    "prepare": (prepare, True),
    "clear-log": (clear_log, True),
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
                return fail("usage: mode off | suggest | auto")
            if application_running() and not force:
                return fail("close the application first (or add --even-if-running)")
            return set_mode(arguments[1])
        if name == "min-scope":
            if len(arguments) != 2:
                return fail("usage: min-scope <whole number> | default")
            if application_running() and not force:
                return fail("close the application first (or add --even-if-running)")
            return set_min_scope(arguments[1])
        if name == "write-fixtures":
            if len(arguments) != 2:
                return fail("usage: write-fixtures <folder>")
            say(f"wrote {write_fixtures(Path(arguments[1]))} invented documents in {arguments[1]}")
            return 0
        if name not in COMMANDS:
            return fail(f"unknown command {name!r}; run without arguments for the list")
        action, closes_application = COMMANDS[name]
        if closes_application and application_running() and not force:
            return fail("close the application first (or add --even-if-running)")
        return action()
    except FileNotFoundError as error:
        return fail(str(error))


if __name__ == "__main__":
    sys.exit(main(sys.argv))
