import type { Translator } from "../../i18n/translate";
import { cautionLines, computedBlock } from "../../lib/annotations";
import { explanationBlock } from "../../lib/calculationFollowUp";
import { localiseInternalNames } from "../../lib/displayNames";
import type { ChatEntry } from "../../state/useChat";
import type { ReadingPart } from "../ipc";

/**
 * What is read aloud of one answer, as the parts `speech_prepare_reading` takes: the text as it is
 * shown, the figure the engine computed (when there is one), and the caution notes that say the
 * answer is incomplete or unchecked. Never the timings, the model, the sources or the coverage: they
 * are references, not the answer, and the first things a listener would not want to hear.
 *
 * Nothing is stripped, numbered or translated here. Rust removes the Markdown, the links and the
 * emoji and writes the numbers as words, by code, never by the model.
 */

/** A line that carries Markdown structure: a table row, a list item, a heading, a quote, emphasis or
 * code. Text without any is sent as plain, so a `_` in a file name is not read as emphasis. */
const MARKDOWN_SHAPES = /^\s*\|.+\|\s*$|^\s*(?:#{1,6}\s|>|[-*+]\s|\d+[.)]\s)|\*\*|__|`/m;

export function holdsMarkdown(text: string): boolean {
  return MARKDOWN_SHAPES.test(text);
}

export function readingParts(t: Translator, locale: string, entry: ChatEntry): ReadingPart[] {
  const parts: ReadingPart[] = [];
  const shown = localiseInternalNames(entry.content, t);
  if (shown.trim().length > 0) {
    parts.push({ kind: "markdown", text: shown });
  }
  const computed = computedBlock(t, locale, entry);
  if (computed !== null) {
    parts.push({ kind: holdsMarkdown(computed) ? "markdown" : "plain", text: computed });
  }
  // The explanation of the calculation is read after the figure it explains.
  const explanation = explanationBlock(t, entry);
  if (explanation !== null) {
    parts.push({ kind: "markdown", text: explanation });
  }
  for (const line of cautionLines(t, locale, entry)) {
    parts.push({ kind: "plain", text: line });
  }
  return parts;
}
