"""Measure what a longer conversation memory costs on this server device.

Run it on the server device, against the gateway, when a model is installed or changed:

    uv run python scripts/measure_context.py --url http://127.0.0.1:8080 --alias cabinet-chat

It sends synthetic conversations only - never a document, never real user data - with 0, 1, 2 and
4 remembered exchanges, and prints how long each answer took and how large the prompt was according
to the runtime. Read the result against the window `/health` publishes for the alias, then set
`MODEL_CONTEXT_WINDOWS` (or `DEFAULT_CONTEXT_WINDOW`) in the server device's `.env`
(docs/SELECTION-AND-MEMORY.md, docs/OPERATIONS.md).

A one-off tool for the owner, not a runtime check: nothing in the product calls it.
"""

from __future__ import annotations

import argparse
import sys
import time

import httpx

#: Roughly the size of a real question and of a real short answer in this product, in characters.
QUESTION_CHARS = 300
ANSWER_CHARS = 900
#: Kept small so the measurement is about reading the prompt, not about writing a long answer.
ANSWER_TOKENS = 128

FILLER = (
    "Synthetic administrative text for a timing test. The user received a letter about an "
    "appointment, a request for a certificate and a reminder about a form to return. "
)


def synthetic(label: str, size: int) -> str:
    text = f"{label}. "
    while len(text) < size:
        text += FILLER
    return text[:size]


def conversation(exchanges: int) -> list[dict[str, str]]:
    messages: list[dict[str, str]] = []
    for number in range(1, exchanges + 1):
        messages.append(
            {"role": "user", "content": synthetic(f"Question {number}", QUESTION_CHARS)}
        )
        messages.append(
            {"role": "assistant", "content": synthetic(f"Answer {number}", ANSWER_CHARS)}
        )
    messages.append(
        {"role": "user", "content": "In two sentences, what has this conversation been about?"}
    )
    return messages


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--url", default="http://127.0.0.1:8080", help="the gateway's address")
    parser.add_argument("--alias", default=None, help="the chat alias; the default one if absent")
    parser.add_argument("--locale", default="fr-FR", help="the output locale to request")
    parser.add_argument(
        "--exchanges", default="0,1,2,4", help="remembered exchanges to try, comma-separated"
    )
    parser.add_argument("--timeout", type=float, default=600.0, help="seconds per request")
    arguments = parser.parse_args()

    with httpx.Client(base_url=arguments.url.rstrip("/"), timeout=arguments.timeout) as client:
        health = client.get("/health").json()
        alias = arguments.alias or health["default_model_alias"]
        window = health.get("context_windows", {}).get(alias)
        print(f"alias: {alias}")
        print(f"published context window: {window if window is not None else 'not published'}")
        print(f"answer reserve (MAX_OUTPUT_TOKENS): {health.get('max_output_tokens')}")
        print()
        print(f"{'exchanges':>9}  {'seconds':>8}  {'prompt tokens':>13}  {'answer tokens':>13}")

        for exchanges in [int(item) for item in arguments.exchanges.split(",") if item.strip()]:
            started = time.monotonic()
            response = client.post(
                "/v1/chat/completions",
                json={
                    "model": alias,
                    "messages": conversation(exchanges),
                    "output_locale": arguments.locale,
                    "max_tokens": ANSWER_TOKENS,
                    "stream": False,
                },
            )
            elapsed = time.monotonic() - started
            if response.status_code != 200:
                print(f"{exchanges:>9}  failed: HTTP {response.status_code} {response.text[:200]}")
                continue
            usage = response.json().get("usage", {})
            print(
                f"{exchanges:>9}  {elapsed:>8.1f}  {usage.get('prompt_tokens', '?'):>13}  "
                f"{usage.get('completion_tokens', '?'):>13}"
            )

    print()
    print("Prompt tokens close to the published window mean older exchanges will be dropped;")
    print("seconds that grow sharply mean the window is larger than this device reads comfortably.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
