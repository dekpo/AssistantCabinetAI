"""One command per step of the large test. See `docs/test-reports/large-test/README.md`.

    python scripts/large_test/cli.py generate                         the library and the book, into data/large-test/library
    python scripts/large_test/cli.py analyse                          the Analyse of the library on the real stack (once per embedding model)
    python scripts/large_test/cli.py routes                           the road of every question, no answering model
    python scripts/large_test/cli.py run --model ministral-3:3b       the whole book through that model, then the score
    python scripts/large_test/cli.py run --model M --results <folder> --resume    go on after the AI machine went away
    python scripts/large_test/cli.py score --results <folder>         score again a run already made
    python scripts/large_test/cli.py compare <folder> <folder> ...    the models side by side

Settings, all optional: `--gateway` (default http://192.168.1.83:8080 if ACAI_GATEWAY is not set), `--embed-alias`
(assistant-embed), `--library`, `--only Q,Q`, `--limit N`, `--fake` (the tests' fake gateway: plumbing only).
Results go to `data/large-test/<date>-<model>/`; the index of the library to `data/large-test/index/<embed alias>/`.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import platform
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
TAURI = ROOT / "apps" / "desktop" / "src-tauri"
BASE = ROOT / "data" / "large-test"
DEFAULT_GATEWAY = "http://192.168.1.83:8080"


def git(*args: str) -> str:
    try:
        return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return ""


def slug(text: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")


def environment(args, out: Path | None) -> dict:
    env = dict(os.environ)
    env["ACAI_LT_LIBRARY"] = str(args.library)
    env["ACAI_LT_INDEX"] = str(BASE / "index" / slug(args.embed_alias))
    env["ACAI_GATEWAY"] = args.gateway or os.environ.get("ACAI_GATEWAY", DEFAULT_GATEWAY)
    env["ACAI_EMBED_ALIAS"] = args.embed_alias
    if getattr(args, "model", None):
        env["ACAI_MODEL"] = args.model
    if out is not None:
        env["ACAI_LT_OUT"] = str(out)
    if getattr(args, "only", None):
        env["ACAI_LT_ONLY"] = args.only
    if getattr(args, "limit", None):
        env["ACAI_LT_LIMIT"] = str(args.limit)
    if getattr(args, "fake", False):
        env["ACAI_LT_FAKE"] = "1"
    if getattr(args, "resume", False):
        env["ACAI_LT_RESUME"] = "1"
    return env


def cargo(test: str, env: dict) -> None:
    command = ["cargo", "test", "--release", "--test", "large_test", test, "--", "--ignored", "--nocapture"]
    subprocess.run(command, cwd=TAURI, env=env, check=True)


def describe_run(args, out: Path) -> dict:
    manifest = json.loads((args.library / "manifest.json").read_text(encoding="utf-8"))
    questions = (args.library / "questions.json").read_bytes()
    return {
        "date": dt.datetime.now().isoformat(timespec="seconds"), "commit": git("rev-parse", "--short", "HEAD"),
        "dirty": bool(git("status", "--porcelain")), "machine": platform.node(), "system": platform.platform(),
        "model": args.model, "embed_alias": args.embed_alias, "gateway": args.gateway or os.environ.get("ACAI_GATEWAY", DEFAULT_GATEWAY),
        "seed": manifest.get("seed"), "book_sha256": hashlib.sha256(questions).hexdigest(),
        "library": manifest.get("documents"), "workbooks": manifest.get("workbooks"),
        "note": "A model answer is not deterministic: each question is asked once, at the application's own temperature "
                "(0.1 for answers from material); a second run can differ.",
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("step", choices=["generate", "analyse", "routes", "run", "score", "compare"])
    parser.add_argument("folders", nargs="*", type=Path)
    parser.add_argument("--library", type=Path, default=BASE / "library")
    parser.add_argument("--slice", action="store_true", help="generate: the small slice instead of the full library")
    parser.add_argument("--model", default="ministral-3:3b")
    parser.add_argument("--embed-alias", default="assistant-embed")
    parser.add_argument("--gateway", default=None)
    parser.add_argument("--results", type=Path, default=None)
    parser.add_argument("--only", default=None)
    parser.add_argument("--limit", type=int, default=None)
    parser.add_argument("--fake", action="store_true")
    parser.add_argument("--resume", action="store_true", help="run/routes: go on in --results, keeping the answers already given")
    args = parser.parse_args()
    # Cargo runs the tests from the crate's folder: every path it receives must be absolute.
    args.library = args.library.resolve()
    args.results = args.results.resolve() if args.results else None
    scripts = ROOT / "scripts"

    if args.step == "generate":
        command = [sys.executable, str(scripts / "gen-knowledge-sandbox.py"), "--profile", "demo"]
        command += ["--slice"] if args.slice else []
        command += ["--out", str(args.library)]
        started = time.time()
        subprocess.run(command, check=True)
        print(f"generated in {time.time() - started:.0f} s: {args.library}")
    elif args.step == "analyse":
        out = args.results or BASE / "analyse" / slug(args.embed_alias)
        out.mkdir(parents=True, exist_ok=True)
        cargo("analyse", environment(args, out))
    elif args.step in ("routes", "run"):
        out = args.results or BASE / f"{dt.date.today().isoformat()}-{slug(args.model if args.step == 'run' else 'routes')}"
        out.mkdir(parents=True, exist_ok=True)
        if not (args.resume and (out / "run.json").exists()):
            (out / "run.json").write_text(json.dumps(describe_run(args, out), indent=1) + "\n", encoding="utf-8")
        cargo(args.step, environment(args, out))
        score = [sys.executable, str(scripts / "large_test" / "score.py"), "--library", str(args.library), "--results", str(out)]
        subprocess.run(score + (["--routes"] if args.step == "routes" else []), check=True)
        print(f"results: {out}")
    elif args.step == "score":
        if args.results is None:
            parser.error("--results is required")
        subprocess.run([sys.executable, str(scripts / "large_test" / "score.py"), "--library", str(args.library), "--results", str(args.results)], check=True)
    else:
        subprocess.run([sys.executable, str(scripts / "large_test" / "compare.py"), *map(str, args.folders)], check=True)


if __name__ == "__main__":
    main()
