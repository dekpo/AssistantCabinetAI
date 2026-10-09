"""A permanent diagnostic, not part of the product - kept alongside `measure_context.py`,
documented in `docs/OPERATIONS.md` ("Diagnosing the tabular hidden interpreter against a real
model") for any agent or owner who needs it later, not only the one who wrote it. Sends the exact
instruction + schema message `tabular::query_plan::build_schema_message` builds to a real model,
non-streaming, and prints the raw reply - to see whether a tabular question that nudges
unexpectedly, or computes a number that looks wrong, is the model writing a bad plan or Rust
misreading a good one. Never run against a real workbook; the schema here is a small fictional
fixture of its own, independent of `fixtures/`.

    uv run python scripts/probe_query_plan.py --url http://127.0.0.1:8080 --alias gemma2:2b
    uv run python scripts/probe_query_plan.py --url http://127.0.0.1:8080 --alias gemma2:2b
        --question q2-beta-fr

When this finds a real model writing a reply `tabular::query_plan` does not handle well, the
established pattern (`src-tauri/src/tabular/query_plan.rs`, every test named
`a_real_<model>_reply_...`) is: fix the gap, then add the captured reply verbatim as its own
permanent regression test - not only the fix on its own.
"""

from __future__ import annotations

import argparse
import json

import httpx

# Copied verbatim from apps/desktop/src-tauri/src/tabular/query_plan.rs's QUERY_PLAN_INSTRUCTION.
QUERY_PLAN_INSTRUCTION = (
    "You are given the structure of a data table - its sheets, and for each column its name, "
    "type and whether it holds formulas - never a value, a row or a file path. Read the question "
    "and reply with exactly one JSON object describing how to answer it, matching this shape and "
    "nothing else: {\"sheet\": string or null, \"filters\": [{\"column\": string, \"op\": one of "
    "\"eq\", \"in\", \"gt\", \"lt\", \"between\", \"contains\", \"weekday\", \"month\", \"year\", "
    "\"date_range\", \"value\": the comparison value}], \"group_by\": string or null, "
    "\"aggregate\": {\"op\": one of \"count\", \"sum\", \"mean\", \"median\", \"min\", \"max\", "
    "\"distinct\", \"column\": string or null} or null, \"sort\": {\"column\": string, "
    "\"descending\": boolean} or null, \"limit\": number or null, \"unsupported\": boolean}. "
    "Every sheet and column name must be copied exactly from what you were given. A filter's "
    "value must be a value you believe the data genuinely holds - never a guess at one. Set "
    "\"unsupported\" to true and leave every other field at its default when the question cannot "
    "be expressed this way. Reply with the JSON object alone: no explanation, no code fence. "
    "Leave \"filters\" empty unless the question names a specific condition to filter rows by - "
    "never repeat \"aggregate\"'s own op there."
)

# The exact schema `tabular::query_plan::build_schema` would build for factures-test.csv.
SCHEMA = [
    {
        "name": "factures-test",
        "rowCount": 9,
        "columns": [
            {"name": "fournisseur", "type": "categorical", "unit": None, "hasFormulas": False},
            {"name": "montant", "type": "numeric", "unit": None, "hasFormulas": False},
        ],
    }
]

QUESTIONS = {
    "q1-total-en": "What's the grand total we've taken in altogether?",
    "q1-total-fr": (
        "Quel est le total general que nous avons encaisse, toutes societes confondues ?"
    ),
    "q2-beta-en": "How much have we taken in from Beta specifically?",
    "q2-beta-fr": "Quelle est la somme encaissee avec Beta uniquement ?",
    "q5-group-en": "Which trading partner brought in the most money?",
    "q5-group-fr": "Quel partenaire commercial nous a rapporte le plus d'argent ?",
}


def message(question: str) -> str:
    schema_json = json.dumps(SCHEMA, separators=(",", ":"))
    return f"{QUERY_PLAN_INSTRUCTION}\n\nSchema:\n{schema_json}\n\nQuestion: {question}"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", default="http://127.0.0.1:8080")
    parser.add_argument("--alias", required=True)
    parser.add_argument(
        "--question",
        choices=sorted(QUESTIONS),
        default=None,
        help="one key, or every one if omitted",
    )
    arguments = parser.parse_args()

    keys = [arguments.question] if arguments.question else list(QUESTIONS)
    with httpx.Client(base_url=arguments.url, timeout=120.0) as client:
        for key in keys:
            content = message(QUESTIONS[key])
            response = client.post(
                "/v1/chat/completions",
                json={
                    "model": arguments.alias,
                    "messages": [{"role": "user", "content": content}],
                    "stream": False,
                },
            )
            print(f"=== {key} ({arguments.alias}) ===")
            if response.status_code != 200:
                print(f"HTTP {response.status_code}: {response.text[:500]}")
                print()
                continue
            body = response.json()
            reply = body["choices"][0]["message"]["content"]
            usage = body.get("usage", {})
            print(f"completion_tokens={usage.get('completion_tokens')}")
            print(reply)
            print()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
