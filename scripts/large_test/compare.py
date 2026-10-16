"""Compare the results of several models on the same book.

    python scripts/large_test/compare.py data/large-test/2026-10-16-ministral data/large-test/2026-10-16-gemma --out compare.md

Each folder holds the `summary.json` that `score.py` wrote. The rows are the same for every model: the blocks, the
features, the formats, the refusals, the leaks, the times.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def cell(entry) -> str:
    return "-" if entry is None else f"{entry['passed']}/{entry['total']}"


def render(summaries: list, names: list) -> str:
    lines = ["# Models compared on the same library and the same book", ""]
    header = "| | " + " | ".join(names) + " |"
    rule = "| --- | " + " | ".join("---" for _ in names) + " |"

    def table(title, key):
        keys = sorted({k for s in summaries for k in s[key]})
        out = ["", f"## {title}", "", header, rule]
        for k in keys:
            label = k if key != "features" else f"{k} {next((s['features'][k]['name'] for s in summaries if k in s['features']), '')}"
            out.append(f"| {label} | " + " | ".join(cell(s[key].get(k)) for s in summaries) + " |")
        return out

    lines += ["| | " + " | ".join(names) + " |", rule]
    lines += [f"| passed | " + " | ".join(f"{s['passed']}/{s['questions']}" for s in summaries) + " |"]
    lines += [f"| model | " + " | ".join(str(s.get("model", "?")) for s in summaries) + " |"]
    lines += [f"| embedding | " + " | ".join(str(s.get("embed_alias", "?")) for s in summaries) + " |"]
    lines += [f"| refusals correct | " + " | ".join(f"{s['refusals']['correct']}/{s['refusals']['total']}" for s in summaries) + " |"]
    lines += [f"| leaks | " + " | ".join(str(s["leaks"]) for s in summaries) + " |"]
    lines += [f"| wrong files in a listing | " + " | ".join(str(s["wrong_files"]) for s in summaries) + " |"]
    lines += [f"| median time per question | " + " | ".join(ms(s["median_ms"]) for s in summaries) + " |"]
    lines += [f"| 95th percentile | " + " | ".join(ms(s["p95_ms"]) for s in summaries) + " |"]
    lines += [f"| model's part, median | " + " | ".join(ms(s["median_generate_ms"]) for s in summaries) + " |"]
    lines += table("Blocks", "blocks") + table("Features", "features") + table("Formats", "formats")
    lines += ["", "Answers by the engines alone (no model) do not depend on the model: the same figure is expected in every column.", ""]
    lines += ["| | " + " | ".join(names) + " |", rule, "| engine-only passed | " + " | ".join(f"{s['model_free']['passed']}/{s['model_free']['total']}" for s in summaries) + " |"]
    return "\n".join(lines) + "\n"


def ms(value) -> str:
    return "-" if value is None else f"{value / 1000:.1f} s"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("folders", nargs="+", type=Path)
    parser.add_argument("--out", type=Path, default=None)
    args = parser.parse_args()
    summaries = [json.loads((folder / "summary.json").read_text(encoding="utf-8")) for folder in args.folders]
    names = [folder.name for folder in args.folders]
    text = render(summaries, names)
    if args.out:
        args.out.write_text(text, encoding="utf-8")
    print(text)


if __name__ == "__main__":
    main()
