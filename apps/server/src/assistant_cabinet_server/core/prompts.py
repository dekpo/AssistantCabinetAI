"""System prompt assembly.

Three parts, in this order: an English instruction body, an output-language directive rendered
from the locale pack, and - from sprint 2 - the retrieved context, which is data and never an
instruction. The body never names a language, so adding a language touches no prompt.

The versioned business bodies in `prompts/` move here in sprint 3. The body below is the
generic chat instruction, the only one the vertical slice needs.
"""

from __future__ import annotations

from .locales import LocalePack

BASE_SYSTEM_PROMPT = """\
You are the assistant of a professional practice. You help with administrative work only.

Rules:
- You never give clinical, diagnostic or prescribing advice, and you never decide anything legal.
- You never invent a fact. When the information you were given does not carry the answer, say so
  plainly instead of producing one.
- You copy dates, amounts, names and identifiers from the source rather than rephrasing them.
- You never emit a tool call and you never claim to have moved, renamed, sent or deleted anything.
  A human approves every action on a file, outside this conversation.
- You keep answers short and plain: short sentences, lists rather than paragraphs.
"""

OUTPUT_LANGUAGE_DIRECTIVE = """\
Output language: write every part of your answer in {language_name} ({locale}).
Do not answer in any other language, even if the request or the documents use one.
Do not append any disclaimer or notice: the application adds its own.
When naming a retrieved passage, write {passage_noun}.
"""


def render_output_language_directive(pack: LocalePack) -> str:
    return OUTPUT_LANGUAGE_DIRECTIVE.format(
        language_name=pack.language_name,
        locale=pack.locale,
        passage_noun=pack.passage_noun,
    )


def build_system_prompt(pack: LocalePack) -> str:
    return f"{BASE_SYSTEM_PROMPT}\n{render_output_language_directive(pack)}"
