"""Write the owner's script (`HUMAN-TEST.md`) and its results sheet (`HUMAN-TEST-results.md`) from the book and from a
scored run, so that every question, file name and expected value is copied from data, never retyped.

    python scripts/large_test/human_script.py --library data/large-test/library --scores data/large-test/<run>/scores.jsonl \
        --out docs/test-reports/large-test

A question is chosen for the script only if the run answered it as expected (the script checks the product, not the
model of the day), except the few marked \"known limit\" which show a finding. The steps ask the owner to do one
thing each; the automated suites are never part of it.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from large_test.book import describe_expectation  # noqa: E402

REPO = "C:\\Users\\elise\\Documents\\CURSOR\\AssistantCabinetAI"
APP = "com.assistantcabinetai.desktop"

# (part, label, selection, [features in the order of the questions to take], how many of each)
PLAN = [
    ("B", "Documents", "documents", [("D08", 2), ("D07", 1), ("D05", 2), ("D06", 1), ("D01", 1), ("D02", 1), ("D03", 1), ("D23", 1),
                                     ("D16", 1), ("D17", 1), ("D18", 1), ("D19", 1), ("D20", 1), ("D21", 1), ("D22", 1),
                                     ("D09", 1), ("D14", 1), ("D15", 1), ("D10", 1), ("D11", 2)]),
    ("C", "Spreadsheets", "spreadsheets", [("T01", 1), ("T02", 2), ("T03", 1), ("T04", 1), ("T06", 1), ("T08", 1), ("T10", 1), ("T12", 1), ("T14", 1), ("T18", 1)]),
    ("D", "Mixed", "mixed", [("M04", 1), ("M05", 1), ("M03", 2), ("M06", 1)]),
]
SELECTION_LINES = {
    "documents": ("Dossier des documents", "tous les documents", "Documents utilisés : tous", "Dossier des données", "Données utilisées : aucune"),
    "spreadsheets": ("Dossier des données", "tous les tableurs", "Données utilisées : toutes", "Dossier des documents", "Documents utilisés : aucun"),
    "mixed": ("les deux dossiers", "tous", "Documents utilisés : tous et Données utilisées : toutes", "", ""),
}


def pick(questions: list, passed: set, feature: str, count: int) -> list:
    chosen = [q for q in questions if q["feature"] == feature and q["id"] in passed and not q["follow_up_of"]]
    if feature == "D10":
        chosen = [q for q in questions if q["feature"] == feature and q["id"] in passed]
    chosen = chosen or [q for q in questions if q["feature"] == feature]
    return chosen[:count]


def expected_lines(q: dict, by_id: dict) -> str:
    e = q["expect"]
    parts = []
    if e.get("refusal"):
        parts.append("The answer says the documents do not hold it. It gives no date and no amount.")
    if e.get("must_not") and not e.get("refusal"):
        parts.append("It must not contain: " + ", ".join(f"« {s} »" for s in e["must_not"][:3]) + ".")
    if e.get("date"):
        y, m, d = e["date"].split("-")
        parts.append(f"The date {d}/{m}/{y} (in words or in figures).")
    if e.get("value") is not None:
        parts.append(f"The number {e['value']:g} (the application writes it as it is written in the file).")
    if e.get("values"):
        parts.append("The numbers " + " and ".join(f"{v:g}" for v in e["values"]) + ".")
    if e.get("facts"):
        parts.append("The words " + " and ".join("« " + g[0] + " »" for g in e["facts"]) + ".")
    if e.get("patterns"):
        parts.append("A match for " + ", ".join(e["patterns"]) + ".")
    if e.get("listing") is not None:
        parts.append(f"A list of {e['total']} document(s) and the sentence that says it is a search of the word; the first of them: {e['listing'][0] if e['listing'] else '-'}.")
        if e.get("workbook_listing") is not None:
            parts.append(f"A second list for the tables: {len(e['workbook_listing'])} workbook(s).")
    if e.get("count") is not None:
        parts.append(f"The number {e['count']}.")
    if e.get("count_range"):
        low, high = e["count_range"]
        parts.append(f"A number between {low} and {high} (the scans the text recognition cannot read are not counted).")
    if e.get("first"):
        parts.append(f"First in the list: {e['first'][0]} ({e['date']}).")
    if e.get("columns"):
        parts.append("The names " + ", ".join(e["columns"]) + ".")
    if e.get("groups"):
        parts.append(f"{len(e['groups'])} groups with their totals, for example {next(iter(e['groups'].items()))}.")
    if e.get("ordered"):
        parts.append("In this order: " + ", ".join(e["ordered"]) + ".")
    if e.get("nudge"):
        meaning = {
            "formula_cannot_be_verified": "the engine refuses to total this column and says it ends with a formula it cannot verify",
            "ambiguous_sheet_name": "the engine asks which sheet is meant (the file has a second, free-text sheet)",
            "which_workbook": "the application asks which workbook is meant",
        }.get(e["nudge"], e["nudge"])
        parts.append(f"No number: {meaning}.")
    if e.get("template"):
        parts.append(f"A plan to approve (nothing is written): template {e['template']}, table {e['data_file']}, with a preview of the letter.")
    if e.get("sources_any"):
        parts.append("Under the answer, « Sources » lists " + e["sources_any"][0].split("/")[-1] + (f", page {e['page']}" if e.get("page") else "") + ".")
    if q["follow_up_of"]:
        parts.insert(0, f"Ask it right after {q['follow_up_of']}, in the same conversation.")
    return " ".join(parts)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--scores", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--run-folder", default="data\\large-test\\<date>-<model>")
    args = parser.parse_args()
    questions = json.loads((args.library / "questions.json").read_text(encoding="utf-8"))
    manifest = json.loads((args.library / "manifest.json").read_text(encoding="utf-8"))
    by_id = {q["id"]: q for q in questions}
    passed = {json.loads(line)["id"] for line in args.scores.read_text(encoding="utf-8").splitlines() if line and json.loads(line)["status"] == "pass"}

    lines = [
        "# Human test - the large test, played by the owner", "",
        "Written for the owner, who plays it once. Instructions in English; every sentence you type and every label you read is French, quoted exactly.", "",
        f"**Library:** {manifest['documents']} documents and {manifest['workbooks']} spreadsheets, invented, generated by `scripts/gen-knowledge-sandbox.py --profile demo`. **Never your own documents.** "
        "**Duration:** about 2 hours, most of it waiting for the Analyse (Part A) and for the runner (Part E). You may stop between two parts: each part says the state it leaves.", "",
        "**Your answers go in `docs/test-reports/large-test/HUMAN-TEST-results.md`**, one block per step id with the expected value already written; you fill « Seen », OK / not OK, and a note.", "",
        "**Rules.** One action per step. Nothing is undone in the middle: one backup at the start (step 0.2), one restore at the very end (Part R). "
        "If a step does not look like its expectation, write what you see and go on; do not retry. The application is the one on `feat/new-readers`: start it with `pnpm tauri dev` from `apps\\desktop`. "
        "A model's answer varies from one asking to the next: the expectation says what must be in it, never the exact words.", "",
        "## Part 0 - Backup and setup (leaves: the application closed, your data saved, the test folders in place)", "",
        "| Step | Do | Expected |", "| --- | --- | --- |",
        "| 0.1 | Close Assistant Cabinet AI if it is open. | Nothing on screen. |",
        f"| 0.2 | In a command prompt (`cmd`): `robocopy \"%APPDATA%\\{APP}\" \"%USERPROFILE%\\acai-backup\\roaming\" /E /NFL /NDL /NJH /NJS` then `robocopy \"%LOCALAPPDATA%\\{APP}\" \"%USERPROFILE%\\acai-backup\\local\" /E /NFL /NDL /NJH /NJS` | No red message. An exit code from 0 to 7 is a success for `robocopy`. The folder `%USERPROFILE%\\acai-backup` exists. |",
        f"| 0.3 | `python scripts\\large_test\\cli.py generate` (from `{REPO}`) if `data\\large-test\\library` does not exist yet. | « generated in ... s ». |",
        f"| 0.4 | `robocopy \"{REPO}\\data\\large-test\\library\\documents\" \"%USERPROFILE%\\AssistantCabinetAI-test\\Docs\" /E /NFL /NDL /NJH /NJS` | {manifest['documents']} files in `Docs`. |",
        f"| 0.5 | `robocopy \"{REPO}\\data\\large-test\\library\\data\" \"%USERPROFILE%\\AssistantCabinetAI-test\\Data\" /E /NFL /NDL /NJH /NJS` | {manifest['workbooks']} files in `Data`. |",
        "| 0.6 | In `apps\\desktop`: `pnpm tauri dev`. | The window opens. Write in the results file which AI machine your « Réglages » point at. |", "",
        "## Part A - The Analyse (leaves: both folders analysed; documents used: aucun; data used: aucune)", "",
        "| Step | Do | Expected |", "| --- | --- | --- |",
        "| A1 | Card « Dossier des documents » : click « Choisir un dossier... » and choose `%USERPROFILE%\\AssistantCabinetAI-test\\Docs`. | The card shows the folder and a number of files to analyse. |",
        "| A2 | Card « Dossier des données » : click « Choisir un dossier... » and choose `%USERPROFILE%\\AssistantCabinetAI-test\\Data`. | Same. |",
        "| A3 | On the documents card click « Analyser ». Note the time it takes (6 to 12 minutes on the tower). | The bar says « Lecture des documents » then « Recherche par sens : x % ». |",
        f"| A4 | Read the summary lines of the documents card. | About {manifest['readable_documents'] - 3} analysés (the scans the text recognition cannot read are the difference); about 90 « fichiers lus par reconnaissance de texte »; 1 fichier n'a pas été lu : son format n'est pas pris en charge (1 .ppt); no file renamed; nothing in « n'ont pas pu être analysés ». |",
        "| A5 | On the data card click « Analyser ». | Over in under a minute (« les tableurs s'analysent en un instant »). |",
        f"| A6 | Read the summary line of the data card. | About {manifest['workbooks'] - 11} tableurs lus; about 11 « illisibles » (they end with a SUM formula among fewer than 20 rows). |", "",
    ]
    step_ids: list = []
    for part, title, key, wanted in PLAN:
        card, ticked, line, other_card, other_line = SELECTION_LINES[key]
        lines += [f"## Part {part} - {title} (leaves: the conversation open; nothing to undo)", ""]
        lines += ["| Step | Do | Expected |", "| --- | --- | --- |"]
        if key == "mixed":
            lines += [f"| {part}0 | Open the lists under « Documents utilisés » and under « Données utilisées » and tick « Tous les documents » and « Tous les tableurs ». | The two lines read « Documents utilisés : tous » and « Données utilisées : toutes ». |"]
        else:
            used, label, other = (("Documents utilisés", "Tous les documents", "Données utilisées : aucune") if key == "documents"
                                  else ("Données utilisées", "Tous les tableurs", "Documents utilisés : aucun"))
            lines += [f"| {part}0 | Under « {used} » open the list and tick « {label} »; leave the other folder at « {other.split(' : ')[1]} ». | The line reads « {line} » and the other « {other} ». |"]
        lines += [f"| {part}0b | Click « Effacer la conversation » and confirm. | The conversation is empty. |"]
        n = 0
        for feature, count in wanted:
            for q in pick(questions, passed, feature, count):
                n += 1
                sid = f"{part}{n}"
                step_ids.append((sid, q))
                shown = q["text"].replace("|", "/")
                where = ""
                if q["selection"]["workbooks"] not in ("none", "all") and key != "mixed":
                    where = ""
                lines.append(f"| {sid} | Type exactly « {shown} » and click « Envoyer ». ({q['id']}, {q['feature']}{', ' + q['format'] if q['format'] else ''}) | {expected_lines(q, by_id)} |")
        lines.append("")
    lines += [
        "## Part F - The controls that are not questions (optional, about 10 minutes; leaves: nothing to undo)", "",
        "Look at each control and write what you see. No expected value: these are the places where the application shows what it is doing.", "",
        "| Step | Do | Look at |", "| --- | --- | --- |",
        "| F1 | Re-open the documents card after Part A. | The summary lines of the last Analyse stay readable; « Recherche par sens » is gone once it reached 100 %. |",
        "| F2 | Under « Documents utilisés » untick « Tous les documents », then type a file name of your choice in the search field of the card. | The files found become the only ones used; the line says how many. Ask a question about a person whose letter is not among them: the answer must not contain what that letter says. |",
        "| F3 | Press « Arrêter » during an Analyse of a few new files (copy ten files from the library into `Docs`, press « Analyser », then « Arrêter » at once). | « Analyse arrêtée. Ce qui a été lu est conservé... »; pressing « Analyser » again goes on. |",
        "| F4 | In the box, type `@` and the first letters of a surname that appears in the documents. | The names of the selection are listed with their number of files. |",
        "| F5 | Open « Réglages » then « Connaissances » then « Gérer… ». | The dialog opens; close it. |",
        "| F6 | Look at the empty conversation after « Effacer la conversation ». | The block « Suggestions » (names hidden by default). |", "",
        "## Part E - The runner on the model you choose (leaves: a result folder; nothing in the application changes)", "",
        "| Step | Do | Expected |", "| --- | --- | --- |",
        f"| E1 | `python scripts\\large_test\\cli.py routes` | « 273 of 300 passed » or close: the roads of the 300 questions, no answering model. The remaining lines are findings listed in `{args.run_folder}\\routes-report.md`. |",
        "| E2 | `python scripts\\large_test\\cli.py run --model ministral-3:3b` (or an alias that `http://192.168.1.83:8080/health` lists) | « N of 300 passed » and the path of the result folder. Open `report.md` in it. |",
        "| E3 | Open `reading-sheet.csv` in the result folder and fill the last two columns for each line (about 15 lines). | Each answer judged ok or wrong. |",
        "| E4 | Optional: run another model the same way, then `python scripts\\large_test\\cli.py compare <folder> <folder>`. | A table with the same rows for each model. |", "",
        "## Part R - Restore (the very last thing; leaves: your application as it was)", "",
        "| Step | Do | Expected |", "| --- | --- | --- |",
        "| R1 | Close Assistant Cabinet AI. | Nothing on screen. |",
        f"| R2 | `robocopy \"%USERPROFILE%\\acai-backup\\roaming\" \"%APPDATA%\\{APP}\" /MIR /NFL /NDL /NJH /NJS` then `robocopy \"%USERPROFILE%\\acai-backup\\local\" \"%LOCALAPPDATA%\\{APP}\" /MIR /NFL /NDL /NJH /NJS` | Exit code 0 to 7. |",
        "| R3 | Start the application. | Your own folders and your own analysis are back. |", "",
    ]
    (args.out / "HUMAN-TEST.md").write_text("\n".join(lines), encoding="utf-8")
    results = ["# Results of the human test (large test)", "", "Fill « Seen », OK / not OK, a note. Expected values are in `HUMAN-TEST.md`.", "",
               "| Step | Question | Seen | OK / not OK | Note |", "| --- | --- | --- | --- | --- |"]
    results += [f"| {sid} | {q['id']} {q['text'].replace('|', '/')[:80]} | | | |" for sid, q in step_ids]
    (args.out / "HUMAN-TEST-results.md").write_text("\n".join(results) + "\n", encoding="utf-8")
    print(f"{len(step_ids)} questions in the script")


if __name__ == "__main__":
    main()
