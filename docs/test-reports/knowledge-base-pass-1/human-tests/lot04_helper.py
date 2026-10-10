"""Does the file manipulations of the lot 4 human test, so nobody edits a file by hand.

Run it from the repository root with Python 3 (the same Python as show_kb_tables.py):

    python docs\\test-reports\\knowledge-base-pass-1\\human-tests\\lot04_helper.py <command>

Close the application first for every command that touches the settings, the index or the key (the
script checks and refuses while the application runs; --even-if-running overrides that).

Commands

    status                  what the script sees: folders, mode, key, files
    backup                  copy index.sqlite3, settings.json and the key file to a backup folder
    restore                 put them back (and remove a key that was not there before)
    mode off                set "knowledgeMode" in settings.json: off | suggest | auto
    mode suggest
    edit-convention         in the Documents folder, replace "Antoine Martin" by "Benoit Lambert" in
                            convention-remplacement-dr-martin.docx (the original is kept)
    restore-convention      put the original back
    hide-cpam               move courrier-cpam-radiation.pdf out of the Documents folder
    restore-cpam            move it back
    add-identifiers         create essai-identifiants.txt (an invented e-mail address and IBAN)
    remove-identifiers      delete it
    add-swiss               create essai-suisse.txt (an invented Swiss AVS number and IBAN)
    remove-swiss            delete it
    delete-key              delete knowledge-identifier.key (the index keeps working; it is made again)
    stop-ai                 docker compose stop   (the AI machine, so nothing can answer a request)
    start-ai                docker compose up -d

Nothing here reads, prints or sends the content of a document. The test uses the FICTIONAL fixtures
only.
"""

import json
import os
import shutil
import subprocess
import sys
import zipfile
from pathlib import Path

APP_FOLDER = "com.assistantcabinetai.desktop"
KEY_FILE = "knowledge-identifier.key"
INDEX_FILE = "index.sqlite3"
SETTINGS_FILE = "settings.json"
PROCESS_NAME = "assistant-cabinet-ai.exe"
BACKUP_NAME = "kb-backup-lot-04"
CONVENTION = "convention-remplacement-dr-martin.docx"
CPAM = "courrier-cpam-radiation.pdf"
IDENTIFIERS_FILE = "essai-identifiants.txt"
SWISS_FILE = "essai-suisse.txt"
REPLACE_FROM = "Antoine Martin"
REPLACE_TO = "Benoit Lambert"

# Invented. The address is on a domain reserved for examples; the IBAN is the documentation example.
IDENTIFIERS_TEXT = (
    "Contact : claire.martin@exemple.fr\n"
    "Règlement : IBAN FR76 3000 6000 0112 3456 7890 189 avant la fin du mois.\n"
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
    return Path(__file__).resolve().parents[4]


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


def documents_folder() -> Path:
    folder = read_settings().get("workFolder")
    if not folder:
        raise FileNotFoundError("no Documents folder is chosen in the application yet")
    path = Path(folder)
    if not path.is_dir():
        raise FileNotFoundError(f"the Documents folder {path} does not exist")
    return path


def find_in_folder(folder: Path, name: str) -> Path | None:
    for candidate in folder.rglob(name):
        if candidate.is_file():
            return candidate
    return None


# ------------------------------------------------------------------------------------ commands


def status() -> int:
    say(f"Settings folder : {config_dir()}")
    say(f"Index folder    : {data_dir()}")
    say(f"Backup folder   : {backup_dir()} ({'exists' if backup_dir().is_dir() else 'not made yet'})")
    say(f"Application     : {'RUNNING' if application_running() else 'closed'}")
    try:
        settings = read_settings()
        say(f"knowledgeMode   : {settings.get('knowledgeMode', '(absent: suggest)')}")
        if "knowledgePacks" in settings:
            say(f"knowledgePacks  : {settings['knowledgePacks']}")
        say(f"Documents folder: {settings.get('workFolder')}")
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


def set_mode(value: str) -> int:
    if value not in ("off", "suggest", "auto"):
        return fail("mode must be off, suggest or auto")
    path = config_dir() / SETTINGS_FILE
    settings = read_settings()
    settings["knowledgeMode"] = value
    path.write_text(json.dumps(settings, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    say(f'knowledgeMode is now "{value}" in {path}')
    say("Start the application now.")
    return 0


def edit_convention() -> int:
    folder = documents_folder()
    document = find_in_folder(folder, CONVENTION)
    if document is None:
        return fail(f"{CONVENTION} is not in {folder}")
    keep = backup_dir() / "held-files"
    keep.mkdir(parents=True, exist_ok=True)
    original = keep / (CONVENTION + ".original")
    if not original.exists():
        shutil.copy2(document, original)
    replaced = 0
    temporary = document.with_suffix(".docx.tmp")
    with zipfile.ZipFile(original) as source, zipfile.ZipFile(
        temporary, "w", zipfile.ZIP_DEFLATED
    ) as target:
        for item in source.infolist():
            data = source.read(item.filename)
            if item.filename == "word/document.xml":
                text = data.decode("utf-8")
                replaced = text.count(REPLACE_FROM)
                data = text.replace(REPLACE_FROM, REPLACE_TO).encode("utf-8")
            target.writestr(item, data)
    if replaced == 0:
        temporary.unlink()
        return fail(f'"{REPLACE_FROM}" was not found in one piece in the document: nothing changed')
    temporary.replace(document)
    say(f'{CONVENTION}: "{REPLACE_FROM}" replaced by "{REPLACE_TO}" {replaced} time(s).')
    say("Now press Analyser in the application.")
    return 0


def restore_convention() -> int:
    original = backup_dir() / "held-files" / (CONVENTION + ".original")
    if not original.is_file():
        return fail("no original was kept: nothing to restore")
    document = find_in_folder(documents_folder(), CONVENTION)
    if document is None:
        return fail(f"{CONVENTION} is not in the Documents folder")
    shutil.copy2(original, document)
    say(f"{CONVENTION} is back as it was.")
    return 0


def hide_cpam() -> int:
    folder = documents_folder()
    letter = find_in_folder(folder, CPAM)
    if letter is None:
        return fail(f"{CPAM} is not in {folder}")
    keep = backup_dir() / "held-files"
    keep.mkdir(parents=True, exist_ok=True)
    relative = letter.relative_to(folder)
    (keep / "cpam-location.txt").write_text(str(relative), encoding="utf-8")
    shutil.move(str(letter), str(keep / CPAM))
    say(f"{CPAM} moved out of the Documents folder (kept in {keep}).")
    say("Now press Analyser in the application.")
    return 0


def restore_cpam() -> int:
    keep = backup_dir() / "held-files"
    held = keep / CPAM
    where = keep / "cpam-location.txt"
    if not held.is_file() or not where.is_file():
        return fail("the letter is not being held: nothing to put back")
    destination = documents_folder() / where.read_text(encoding="utf-8")
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.move(str(held), str(destination))
    say(f"{CPAM} is back in {destination.parent}.")
    return 0


SWISS_TEXT = (
    "Patiente : Madame Rose Marchand\n"
    "N° AVS 756.1234.5678.97\n"
    "Règlement : IBAN CH93 0076 2011 6238 5295 7\n"
)


def add_swiss() -> int:
    path = documents_folder() / SWISS_FILE
    path.write_text(SWISS_TEXT, encoding="utf-8")
    say(f"Created {path}")
    say("Now press Analyser in the application.")
    return 0


def remove_swiss() -> int:
    path = documents_folder() / SWISS_FILE
    if path.is_file():
        path.unlink()
        say(f"Deleted {path}. Press Analyser to forget it.")
    else:
        say("There was no such file.")
    return 0


def add_identifiers() -> int:
    path = documents_folder() / IDENTIFIERS_FILE
    path.write_text(IDENTIFIERS_TEXT, encoding="utf-8")
    say(f"Created {path}")
    say("Now press Analyser in the application.")
    return 0


def remove_identifiers() -> int:
    path = documents_folder() / IDENTIFIERS_FILE
    if path.is_file():
        path.unlink()
        say(f"Deleted {path}. Press Analyser to forget it.")
    else:
        say("There was no such file.")
    return 0


def delete_key() -> int:
    path = data_dir() / KEY_FILE
    if path.is_file():
        path.unlink()
        say(f"Deleted {path}. The application makes a new one at the next Analyse.")
    else:
        say("There was no key file.")
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
    "edit-convention": (edit_convention, False),
    "restore-convention": (restore_convention, False),
    "hide-cpam": (hide_cpam, False),
    "restore-cpam": (restore_cpam, False),
    "add-identifiers": (add_identifiers, False),
    "remove-identifiers": (remove_identifiers, False),
    "add-swiss": (add_swiss, False),
    "remove-swiss": (remove_swiss, False),
    "delete-key": (delete_key, True),
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
