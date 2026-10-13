"""Does the file manipulations of the lot 11 human test, so nobody edits a file by hand.

Run it from the repository root with Python 3 (the same Python as show_kb_tables.py). On Windows:

    python docs\\test-reports\\knowledge-base-pass-1\\human-tests\\lot11_helper.py <command>

On macOS, the same command with its own spelling (the settings and the index live in
~/Library/Application Support/com.assistantcabinetai.desktop):

    python3 docs/test-reports/knowledge-base-pass-1/human-tests/lot11_helper.py <command>

Close the application first for the commands that touch the settings or the index (backup, restore, prepare, mode,
clear-log): the script checks and refuses while the application runs (--even-if-running overrides that). The commands
that only write files in the test folder (tier, lifecycle) and the read-only ones (status, names, report, questions)
may run with the application open.

Commands

    status                      what the script sees: folders, mode, log, the test library
    backup                      copy index.sqlite3, settings.json and the key file to a backup folder
    restore                     put them back (and remove a key that was not there before)
    generate                    write the 1 000-document library to data\\knowledge-sandbox-1000 (once; ~10 s)
    prepare <size>              make "kb-lot11" (Documents) and "kb-lot11-data" (Data) beside your current
                                Documents folder with the first <size> documents of the sandbox (30, 100, 200,
                                500 or 1000) and its workbooks, point the application at them, set the
                                knowledge mode to "suggest" and switch the timing log on
    swiss-file                  write two invented amount tables in "kb-lot11-data": patients-suisse.csv (amounts
                                "CHF 1'450.00", dates 14.10.2026) and patients-france.csv (amounts "1 450,00 EUR",
                                dates 14/10/2026), the same ten rows in both; Analyse the Data card afterwards
    extra-files                 swiss-file, plus the invented files of the fix 4 batch 3 test: three small workbooks
                                (rdv-medecins.csv, rdv-export-titre.csv, rdv-dates-ambigues.csv) in "kb-lot11-data" and
                                two documents (2026/janvier/neurologie.txt, 2026/mars/neurologie.txt) in "kb-lot11";
                                Analyse both cards afterwards
    tier <size>                 grow or shrink the test library to <size> documents (adds or removes files in
                                kb-lot11 only), the way a real library grows
    mode off | suggest | auto   set "knowledgeMode" in settings.json
    questions <list>            print the questions of a list exactly as to be typed: ab, safety, follow-up,
                                lifecycle, daily
    lifecycle add               write a new letter about a person the library has never seen
    lifecycle add-spaced        write a letter whose name has spaces and an accent (Analyse renames it)
    lifecycle modify            rewrite the surname in one existing letter
    lifecycle delete            delete one existing letter
    lifecycle undo              put back the letter that "modify" or "delete" changed (kb-lot11 only)
    names                       count the entities of the base by type and status, with a fingerprint of the
                                list: run it before and after a question to see that nothing was created
    report                      what the questions recorded: the figures of the release gate and the timings
                                by mode (read only; numbers and codes, no question text)
    clear-log                   empty the questions' record: delete retrieval-timings.jsonl and the rows of
                                kb_diagnostics (numbers and codes only; no knowledge is touched)

Nothing here reads, prints or sends the content of a real document. The library is invented.
"""

import hashlib
import json
import os
import shutil
import sqlite3
import statistics
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[4]
APP_FOLDER = "com.assistantcabinetai.desktop"
KEY_FILE = "knowledge-identifier.key"
INDEX_FILE = "index.sqlite3"
SETTINGS_FILE = "settings.json"
TIMING_LOG = "retrieval-timings.jsonl"
PROCESS_NAME = "assistant-cabinet-ai.exe"  # Windows; macOS and Linux use the name without ".exe"
FILES = Path(__file__).resolve().parent  # the invented files of the test live beside this script
BACKUP_NAME = "kb-backup-lot-11"
DOCS_NAME = "kb-lot11"
DATA_NAME = "kb-lot11-data"
CORE = REPO / "fixtures" / "knowledge-sandbox"
LARGE = REPO / "data" / "knowledge-sandbox-1000"
STATE_FILE = ".lot11-state.json"  # inside kb-lot11: which tier the library is at

# the letters the lifecycle commands write and change
NEW_PERSON = "Sidonie Quenneville"
NEW_LETTER = "courrier-quenneville-nouveau.txt"
SPACED_LETTER = "Courrier Quenneville été 2026.txt"
CHANGE_SURNAME = "Quéméner"


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
        # macOS and Linux: the process is named without ".exe". Not walked on a Mac yet (Part M of the protocol).
        try:
            result = subprocess.run(["pgrep", "-x", PROCESS_NAME.removesuffix(".exe")], capture_output=True, check=False)
        except OSError:
            return False
        return result.returncode == 0
    try:
        out = subprocess.run(["tasklist", "/FI", f"IMAGENAME eq {PROCESS_NAME}"], capture_output=True, text=True, check=False).stdout
    except OSError:
        return False
    return PROCESS_NAME.lower() in out.lower()


def read_settings() -> dict:
    path = config_dir() / SETTINGS_FILE
    if not path.is_file():
        raise FileNotFoundError(f"no {SETTINGS_FILE} at {path}: start the application once first")
    return json.loads(path.read_text(encoding="utf-8-sig"))


def write_settings(settings: dict) -> None:
    (config_dir() / SETTINGS_FILE).write_text(json.dumps(settings, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def docs_folder(settings: dict | None = None) -> Path | None:
    settings = settings or read_settings()
    current = settings.get("workFolder")
    return Path(current) if current and Path(current).name == DOCS_NAME else None


def library_for(size: int) -> Path:
    return CORE if size <= 200 else LARGE


def load_json(library: Path, name: str):
    return json.loads((library / name).read_text(encoding="utf-8"))


# ------------------------------------------------------------------------------------------ the library on disk


def copy_range(library: Path, docs: Path, low: int, high: int) -> int:
    """Copy the documents whose position in the stream is in (low, high] into the test folder."""
    copied = 0
    for record in load_json(library, "documents.json"):
        if low < record["index"] <= high:
            source = library / "documents" / record["path"]
            target = docs / record["path"]
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
            copied += 1
    return copied


def remove_above(library: Path, docs: Path, size: int) -> int:
    removed = 0
    for record in load_json(library, "documents.json"):
        if record["index"] > size:
            target = docs / record["path"]
            if target.is_file():
                target.unlink()
                removed += 1
    for folder in sorted((p for p in docs.rglob("*") if p.is_dir()), key=lambda p: len(p.parts), reverse=True):
        if not any(folder.iterdir()):
            folder.rmdir()
    return removed


def write_state(docs: Path, size: int, library: Path) -> None:
    (docs.parent / (DOCS_NAME + STATE_FILE)).write_text(json.dumps({"size": size, "library": str(library)}), encoding="utf-8")


def read_state(docs: Path) -> dict:
    path = docs.parent / (DOCS_NAME + STATE_FILE)
    return json.loads(path.read_text(encoding="utf-8")) if path.is_file() else {}


def describe(library: Path, size: int) -> None:
    records = [r for r in load_json(library, "documents.json") if r["index"] <= size]
    readable = sum(1 for r in records if r["readable"])
    say(f"  {len(records)} documents: {readable} the application reads, {len(records) - readable} it does not read (formats: "
        + ", ".join(sorted({r['format'] for r in records if not r['readable']})) + ")")
    say(f"  scans (OCR): {sum(1 for r in records if r['ocr'])}; PDF pages: {sum(r['pages'] for r in records if r['format'] in ('pdf', 'scan-pdf'))}")


# --------------------------------------------------------------------------------------------------- commands


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
        say(f"Documents folder: {folder}{'  <- the test folder' if docs_folder(settings) else ''}")
        say(f"Data folder     : {settings.get('dataFolder')}")
        docs = docs_folder(settings)
        if docs and docs.is_dir():
            state = read_state(docs)
            say(f"  holds         : {sum(1 for p in docs.rglob('*') if p.is_file())} files (library tier {state.get('size', '?')})")
    except FileNotFoundError as error:
        say(f"Settings        : {error}")
    log = data_dir() / TIMING_LOG
    lines = sum(1 for _ in log.open(encoding="utf-8")) if log.is_file() else 0
    say(f"Timing log      : {'present' if log.is_file() else 'absent'} ({lines} line(s))")
    say(f"Index           : {'present' if (data_dir() / INDEX_FILE).is_file() else 'absent'}")
    say(f"Core library    : {'present' if CORE.is_dir() else 'MISSING'} ({CORE})")
    say(f"Large library   : {'present' if LARGE.is_dir() else 'not generated (run: generate)'} ({LARGE})")
    return 0


def backup() -> int:
    target = backup_dir()
    target.mkdir(parents=True, exist_ok=True)
    for source, copy in [
        (data_dir() / INDEX_FILE, target / INDEX_FILE),
        (config_dir() / SETTINGS_FILE, target / SETTINGS_FILE),
        (data_dir() / KEY_FILE, target / KEY_FILE),
    ]:
        if source.is_file():
            shutil.copy2(source, copy)
            say(f"saved   {source.name}")
        else:
            say(f"absent  {source.name} (nothing to save)")
    marker = target / "key-was-absent.txt"
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
    if (source / KEY_FILE).is_file():
        shutil.copy2(source / KEY_FILE, key_target)
        say(f"restored {KEY_FILE}")
    elif key_target.is_file():
        key_target.unlink()
        say(f"removed {KEY_FILE} (there was none when the backup was made)")
    say("The test folders kb-lot11 and kb-lot11-data are still on disk: delete them when you wish (they hold only invented files).")
    return 0


def generate() -> int:
    if LARGE.is_dir() and (LARGE / "manifest.json").is_file():
        say(f"already there: {LARGE}")
        return 0
    command = [sys.executable, str(REPO / "scripts" / "gen-knowledge-sandbox.py"), "--documents", "1000", "--extra-rows", "40", "--out", str(LARGE)]
    say("running: " + " ".join(command))
    result = subprocess.run(command, check=False)
    return result.returncode


def prepare(size_text: str) -> int:
    if not size_text.isdigit() or int(size_text) not in (30, 100, 200, 500, 1000):
        return fail("prepare takes a size: 30, 100, 200, 500 or 1000")
    size = int(size_text)
    library = library_for(size)
    if not (library / "manifest.json").is_file():
        return fail(f"the library {library} does not exist" + (": run 'generate' first" if library == LARGE else ""))
    settings = read_settings()
    current = settings.get("workFolder")
    if not current:
        return fail("no Documents folder is chosen in the application yet: choose or create one on the Documents card first, so that the test folders can be made beside it")
    if not backup_dir().is_dir():
        return fail("run 'backup' first: the folders in the settings are about to change")
    base = Path(current).parent if Path(current).name != DOCS_NAME else Path(current).parent
    docs, data = base / DOCS_NAME, base / DATA_NAME
    if docs.exists() or data.exists():
        shutil.rmtree(docs, ignore_errors=True)
        shutil.rmtree(data, ignore_errors=True)
    docs.mkdir(parents=True)
    data.mkdir(parents=True)
    copied = copy_range(library, docs, 0, size)
    workbooks = load_json(library, "workbooks.json")
    for book in workbooks:
        shutil.copy2(library / "data" / book["path"], data / book["path"])
    say(f"wrote {copied} invented documents in {docs}")
    describe(library, size)
    say(f"wrote {len(workbooks)} invented workbooks in {data} (formats: " + ", ".join(sorted({b['format'] for b in workbooks})) + ")")
    if Path(current).name != DOCS_NAME:
        (backup_dir() / "previous-work-folder.txt").write_text(current, encoding="utf-8")
    settings["workFolder"] = str(docs)
    settings["dataFolder"] = str(data)
    settings["knowledgeMode"] = "suggest"
    settings["writeTimingLog"] = True
    settings["knowledgePacks"] = ["health-fr", "health-ch"]
    write_settings(settings)
    write_state(docs, size, library)
    say(f'"workFolder" is now {docs}; "dataFolder" is now {data}')
    say('"knowledgeMode" is "suggest", "writeTimingLog" is true and both health modules (France and Switzerland) are on. ("restore" puts your settings back.)')
    say("Start the application now.")
    return 0


SWISS_AMOUNTS = (
    "Date;Patient;Montant\n"
    "14.10.2026;Sophie Rochat;CHF 1'450.00\n"
    "02.10.2026;Mathieu Cuendet;CHF 380.50\n"
    "21.10.2026;Isabelle Pittet;CHF 1'020.00\n"
    "05.09.2026;Nathalie Chappuis;CHF 95.00\n"
    "18.09.2026;Pascal Maillard;CHF 2'310.25\n"
    "30.09.2026;C\u00e9line Bovet;CHF 560.00\n"
    "12.10.2026;Alain Jaccard;CHF 75.00\n"
    "26.10.2026;Christelle D\u00e9l\u00e8ze;CHF 640.80\n"
    "09.10.2026;Gr\u00e9goire Crettenand;CHF 1'999.99\n"
    "15.09.2026;Marie-Claude Vuilleumier;CHF 410.00\n"
)
FRENCH_AMOUNTS = (
    "Date;Patient;Montant\n"
    "14/10/2026;Jean Dupont;1 450,00 \u20ac\n"
    "02/10/2026;Marie Martin;380,50 \u20ac\n"
    "21/10/2026;Claire Martin;1 020,00 \u20ac\n"
    "05/09/2026;Luc Martin;95,00 \u20ac\n"
    "18/09/2026;Laurence Fabre;2 310,25 \u20ac\n"
    "30/09/2026;Fr\u00e9d\u00e9ric Girard;560,00 \u20ac\n"
    "12/10/2026;Lucas Roy;75,00 \u20ac\n"
    "26/10/2026;Pierre Thomas;640,80 \u20ac\n"
    "09/10/2026;Thomas Pierre;1 999,99 \u20ac\n"
    "15/09/2026;Marie Dupont;410,00 \u20ac\n"
)


def swiss_file() -> int:
    settings = read_settings()
    data = settings.get("dataFolder")
    if not data or Path(data).name != DATA_NAME or not Path(data).is_dir():
        return fail("run 'prepare <size>' first: the amount tables are written in " + DATA_NAME)
    (Path(data) / "patients-suisse.csv").write_text(SWISS_AMOUNTS, encoding="utf-8", newline="\n")
    (Path(data) / "patients-france.csv").write_text(FRENCH_AMOUNTS, encoding="utf-8", newline="\n")
    say(f"wrote patients-suisse.csv and patients-france.csv in {data} (invented patients, ten rows each)")
    say("Start the application, press Analyser on the Data card.")
    return 0


def extra_files() -> int:
    """The Swiss and French amount tables, and the invented files of the fix 4 batch 3 test, in one command."""
    settings = read_settings()
    docs = docs_folder(settings)
    data = settings.get("dataFolder")
    if docs is None or not docs.is_dir() or not data or Path(data).name != DATA_NAME or not Path(data).is_dir():
        return fail("run 'prepare <size>' first: the files are written in " + DOCS_NAME + " and " + DATA_NAME)
    sources = sorted((FILES / "lot-11-files" / "data").glob("*.csv")) + sorted((FILES / "fix-04-batch-3-files" / "data").glob("*.csv"))
    for source in sources:
        shutil.copy2(source, Path(data) / source.name)
        say(f"wrote {source.name} in {data}")
    for source in sorted((FILES / "fix-04-batch-3-files" / "docs").rglob("*.txt")):
        relative = source.relative_to(FILES / "fix-04-batch-3-files" / "docs")
        target = docs / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
        say(f"wrote {relative.as_posix()} in {docs}")
    say("Start the application, press Analyser on the Documents card and on the Data card.")
    return 0


def tier(size_text: str) -> int:
    if not size_text.isdigit() or int(size_text) not in (30, 100, 200, 500, 1000):
        return fail("tier takes a size: 30, 100, 200, 500 or 1000")
    size = int(size_text)
    docs = docs_folder()
    if docs is None or not docs.is_dir():
        return fail("run 'prepare <size>' first")
    state = read_state(docs)
    current = int(state.get("size", 0))
    library = library_for(max(size, current))
    if str(library) != state.get("library") and current:
        library = Path(state["library"]) if size <= current else library
    if size > current:
        added = copy_range(library, docs, current, size)
        say(f"added {added} documents: the library goes from {current} to {size}")
    elif size < current:
        removed = remove_above(library, docs, size)
        say(f"removed {removed} documents: the library goes from {current} to {size}")
    else:
        say(f"already at {size}")
    write_state(docs, size, library)
    describe(library, size)
    say("Start the application and press Analyser.")
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


def find_record(name_part: str) -> dict | None:
    docs = docs_folder()
    if docs is None:
        return None
    library = Path(read_state(docs).get("library", CORE))
    for record in load_json(library, "documents.json"):
        if name_part in record["path"] and (docs / record["path"]).is_file():
            return record
    return None


def remember(undo: Path, change: dict) -> None:
    done = json.loads(undo.read_text(encoding="utf-8")) if undo.is_file() else []
    done.append(change)
    undo.write_text(json.dumps(done), encoding="utf-8")


def lifecycle(action: str) -> int:
    docs = docs_folder()
    if docs is None or not docs.is_dir():
        return fail("run 'prepare <size>' first")
    undo = docs.parent / (DOCS_NAME + ".undo.json")
    if action == "add":
        target = docs / "courriers-entrants" / NEW_LETTER
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(
            f"Madame {NEW_PERSON}\n12 rue des Acacias, 74100 Annemasse\n\nObjet : demande de rendez-vous.\n"
            f"Madame {NEW_PERSON} souhaite un rendez-vous de suivi dans le courant du mois d'octobre 2026.\n"
            "Elle rappelle que sa caisse maladie est la CSS Assurance.\n",
            encoding="utf-8",
        )
        remember(undo, {"action": "create", "path": str(target)})
        say(f"wrote {target.relative_to(docs)} (a person the library has never seen: {NEW_PERSON})")
        return 0
    if action == "add-spaced":
        target = docs / SPACED_LETTER
        target.write_text(f"Madame {NEW_PERSON} confirme son rendez-vous du 14 octobre 2026.\n", encoding="utf-8")
        remember(undo, {"action": "create", "path": str(target)})
        say(f"wrote {target.name} (spaces and an accent in the name: the next Analyse renames it)")
        return 0
    if action == "modify":
        library = Path(read_state(docs).get("library", CORE))
        entities = {e["id"]: e for e in load_json(library, "entities.json")}
        candidates = [r for r in load_json(library, "documents.json")
                      if r["format"] == "txt" and r["kind"] == "lettre_confrere" and len(r["about"]) == 1 and (docs / r["path"]).is_file()]
        if not candidates:
            return fail("no text letter about one patient in the library at this size")
        record = candidates[0]
        person = entities[record["about"][0]]
        given, _, family = person["canonical"].partition(" ")
        path = docs / record["path"]
        text = path.read_text(encoding="utf-8")
        remember(undo, {"action": "modify", "path": str(path), "text": text})
        path.write_text(text.replace(family, CHANGE_SURNAME), encoding="utf-8")
        say(f"changed {record['path']}: {person['canonical']} -> {given} {CHANGE_SURNAME}")
        return 0
    if action == "delete":
        candidates = [r for r in load_json(Path(read_state(docs).get("library", CORE)), "documents.json")
                      if r["format"] in ("pdf", "docx") and "p-rochat-sophie" in [m["entity"] for m in r["mentions"]] and (docs / r["path"]).is_file()]
        if not candidates:
            return fail("no letter about Sophie Rochat at this size")
        record = candidates[0]
        path = docs / record["path"]
        remember(undo, {"action": "delete", "path": str(path), "bytes": path.read_bytes().hex()})
        path.unlink()
        say(f"deleted {record['path']} (a letter about Sophie Rochat)")
        return 0
    if action == "undo":
        if not undo.is_file():
            return fail("nothing to undo")
        for change in reversed(json.loads(undo.read_text(encoding="utf-8"))):
            path = Path(change["path"])
            if change["action"] == "modify":
                path.write_text(change["text"], encoding="utf-8")
            elif change["action"] == "delete":
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(bytes.fromhex(change["bytes"]))
            elif change["action"] == "create" and path.is_file():
                path.unlink()
            say(f"put back {path.name}")
        undo.unlink()
        for renamed in docs.glob("Courrier-Quenneville-ete-2026*.txt"):
            renamed.unlink()
            say(f"removed {renamed.name} (the renamed copy of the spaced letter)")
        return 0
    return fail("lifecycle takes: add, add-spaced, modify, delete or undo")


def names() -> int:
    index = data_dir() / INDEX_FILE
    if not index.is_file():
        return fail(f"no {INDEX_FILE} at {data_dir()}")
    connection = sqlite3.connect(f"file:{index}?mode=ro", uri=True)
    try:
        rows = connection.execute(
            "SELECT type_id, status, canonical_name FROM kb_entities WHERE status IN ('candidate','active') ORDER BY type_id, canonical_name"
        ).fetchall()
    except sqlite3.Error as error:
        return fail(f"the base cannot be read: {error}")
    finally:
        connection.close()
    counts: dict = {}
    for type_id, status, _ in rows:
        counts[(type_id, status)] = counts.get((type_id, status), 0) + 1
    for (type_id, status), number in sorted(counts.items()):
        say(f"{type_id:14} {status:10} {number}")
    fingerprint = hashlib.sha256("\n".join("|".join(r) for r in rows).encode("utf-8")).hexdigest()[:16]
    say(f"entities: {len(rows)}   fingerprint of the list: {fingerprint}")
    asked = [n for n in (NEW_PERSON, "Zenobie Quarante", "Hyacinthe Vaudroz", "Leopold Tschumi") if any(n.lower() in r[2].lower() for r in rows)]
    say(f"names of the test questions found in the base: {asked if asked else 'none'}")
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
                if connection.execute("SELECT 1 FROM sqlite_master WHERE name = 'kb_diagnostics'").fetchone():
                    say(f"emptied kb_diagnostics ({connection.execute('DELETE FROM kb_diagnostics').rowcount} row(s))")
        finally:
            connection.close()
    return 0


def pct(part: int, whole: int) -> str:
    return "-" if whole == 0 else f"{100 * part / whole:.1f} %"


def report() -> int:
    index = data_dir() / INDEX_FILE
    if not index.is_file():
        return fail(f"no {INDEX_FILE} at {data_dir()}")
    connection = sqlite3.connect(f"file:{index}?mode=ro", uri=True)
    try:
        rows = connection.execute(
            "SELECT skipped_reason, entities_found, would_apply, applied, widened, reduction_percent, cited_files, "
            "cited_in_candidates, miss_no_mention, miss_capped, miss_unresolved FROM kb_diagnostics"
        ).fetchall()
    except sqlite3.Error as error:
        return fail(f"kb_diagnostics cannot be read: {error}")
    finally:
        connection.close()
    total = len(rows)
    would = [r for r in rows if r[2]]
    cited = sum(r[6] for r in would)
    held = sum(r[7] for r in would)
    no_mention, capped, unresolved = (sum(r[i] for r in would) for i in (8, 9, 10))
    say(f"Questions recorded            : {total}")
    say(f"  with a name the base knows  : {sum(1 for r in rows if r[1] > 0)}")
    say(f"  the base would reduce       : {len(would)}  ({pct(len(would), total)})")
    say(f"  reduced for real (auto/chip): {sum(1 for r in rows if r[3])}")
    say(f"  widened again               : {sum(1 for r in rows if r[4])}")
    if would:
        say(f"  median reduction            : {statistics.median(r[5] for r in would):.0f} %")
    say(f"Release gate, raw (every cited file)       : {pct(held, cited)}  ({held} of {cited})")
    say(f"  lost: {no_mention} cited files that mention no name of the question (filler or extraction gap), {capped} left out by the cap, {unresolved} files that name an entity the base did not use")
    say(f"Release gate, adjusted (filler excluded)   : {pct(held, held + capped + unresolved)}  (a cited file counts only when it names an entity of the question)")
    reasons: dict = {}
    for r in rows:
        if r[0]:
            reasons[r[0]] = reasons.get(r[0], 0) + 1
    say(f"Why a question was not reduced: {dict(sorted(reasons.items()))}")
    log = data_dir() / TIMING_LOG
    if log.is_file():
        by_mode: dict = {}
        for line in log.read_text(encoding="utf-8").splitlines():
            try:
                entry = json.loads(line)
            except ValueError:
                continue
            mode = (entry.get("targeting") or {}).get("mode", "off")
            if entry.get("path") not in ("documents", "mixed"):
                continue
            by_mode.setdefault(mode, []).append(entry)
        say("")
        say("By mode (questions that searched the documents; milliseconds, median / worst):")
        for mode in ("off", "suggest", "auto"):
            entries = by_mode.get(mode, [])
            if not entries:
                continue
            def stat(key):
                values = [e.get(key) or 0 for e in entries]
                return f"{statistics.median(values):.0f} / {max(values):.0f}"
            lookup = [(e.get("targeting") or {}).get("knowledgeLookupMs") or 0 for e in entries]
            say(f"  {mode:8} {len(entries):3} questions | embedding {stat('embeddingMs')} | search {stat('searchMs')} | "
                f"knowledge {statistics.median(lookup):.0f} / {max(lookup):.0f} | first word {stat('generationFirstTokenMs')} | total {stat('totalMs')} "
                f"| files selected {stat('selectedDocuments')} | chunks scored {stat('chunksConsidered')}")
    return 0


def questions(name: str) -> int:
    library = CORE
    docs = docs_folder()
    if docs is not None:
        library = Path(read_state(docs).get("library", CORE))
    all_questions = load_json(library, "questions.json")
    size = int(read_state(docs).get("size", 200)) if docs else 200
    by_id = {q["id"]: q for q in all_questions}
    lists = {
        "ab": [q["id"] for q in all_questions if q["category"] in ("person", "practitioner", "organization", "identifier", "alias", "multi_entity", "term", "ocr", "long_document", "unknown", "entity_free")
               and q["min_tier"] <= size and q["selection"]["documents"] == "all"][:30],
        "safety": [q["id"] for q in all_questions if q["category"] in ("leak", "small_scope")],
        "follow-up": [q["id"] for q in all_questions if q["category"] == "follow_up"],
        "daily": [q["id"] for q in all_questions if q["min_tier"] <= size and q["category"] not in ("data", "mixed", "leak", "small_scope", "follow_up")],
    }
    if name not in lists:
        return fail("questions takes: " + ", ".join(sorted(lists)))
    say(f"List '{name}' for a library of {size} documents: type the sentences exactly as written.")
    say("")
    for number, qid in enumerate(lists[name], start=1):
        q = by_id[qid]
        selection = q["selection"]["documents"]
        tick = "tous les documents" if selection == "all" else f"SEULEMENT {len(selection['paths'])} documents (voir 'safety')"
        expected = len(q["relevant_readable"])
        facts = f" fact: {q['expect']['contains']}" if q["expect"]["contains"] else ""
        say(f"{number:3}. [{qid}] {q['text']}    (documents cochés: {tick}; fichiers qui contiennent la réponse: {expected}{facts})")
    return 0


COMMANDS = {
    "status": (lambda a: status(), False),
    "backup": (lambda a: backup(), True),
    "restore": (lambda a: restore(), True),
    "generate": (lambda a: generate(), False),
    "prepare": (lambda a: prepare(a[0]) if a else fail("usage: prepare 30 | 100 | 200 | 500 | 1000"), True),
    "swiss-file": (lambda a: swiss_file(), False),
    "extra-files": (lambda a: extra_files(), False),
    "tier": (lambda a: tier(a[0]) if a else fail("usage: tier 30 | 100 | 200 | 500 | 1000"), False),
    "mode": (lambda a: set_mode(a[0]) if a else fail("usage: mode off | suggest | auto"), True),
    "lifecycle": (lambda a: lifecycle(a[0]) if a else fail("usage: lifecycle add | add-spaced | modify | delete | undo"), False),
    "names": (lambda a: names(), False),
    "report": (lambda a: report(), False),
    "clear-log": (lambda a: clear_log(), True),
    "questions": (lambda a: questions(a[0]) if a else fail("usage: questions ab | safety | follow-up | daily"), False),
}


def main(argv: list[str]) -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    arguments = [a for a in argv[1:] if a != "--even-if-running"]
    force = "--even-if-running" in argv
    if not arguments:
        print(__doc__)
        return 2
    name, rest = arguments[0], arguments[1:]
    if name not in COMMANDS:
        return fail(f"unknown command {name!r}; run without arguments for the list")
    action, closes_application = COMMANDS[name]
    try:
        if closes_application and application_running() and not force:
            return fail("close the application first (or add --even-if-running)")
        return action(rest)
    except FileNotFoundError as error:
        return fail(str(error))


if __name__ == "__main__":
    sys.exit(main(sys.argv))
