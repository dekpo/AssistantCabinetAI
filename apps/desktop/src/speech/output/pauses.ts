import type { Translator } from "../../i18n/translate";
import { counted } from "../../lib/plural";
import type { ReadingItem, ReadingNoteCode, ReadingPause } from "../ipc";
import type { SpeakSegment } from "./engine";

/**
 * How long the voice rests after each kind of item. The numbers are this side's to choose
 * (Rust only says which kind of rest follows): a `clause` item is a long sentence cut at a comma, so
 * the rest is a breath; a `sentence` and a `paragraph` are longer.
 */
export const PAUSE_MS: Record<ReadingPause, number> = {
  clause: 120,
  sentence: 380,
  paragraph: 760,
};

/** The rest after a note that stands where a table or a block of code was left out. */
export const NOTE_PAUSE_MS = PAUSE_MS.paragraph;

/** Every note code Rust may answer. A test holds this list against both catalogues. */
export const NOTE_CODES: readonly ReadingNoteCode[] = ["table_omitted", "code_omitted"];

/** The catalogue key of the sentence for a note code. */
export function noteKey(code: ReadingNoteCode): string {
  return `speech.readAloudNote.${code}`;
}

/** The sentence that says a table or a block of code was left out, with its size in the right plural.
 * The size is the number of rows of the table, or of lines of the code. */
export function noteSentence(t: Translator, code: ReadingNoteCode, count: number): string {
  return code === "table_omitted"
    ? t(noteKey(code), {
        rows: counted(count, t("speech.readAloudNote.rowOne"), t("speech.readAloudNote.rowMany")),
      })
    : t(noteKey(code), {
        lines: counted(count, t("speech.readAloudNote.lineOne"), t("speech.readAloudNote.lineMany")),
      });
}

export interface SpokenReading {
  segments: SpeakSegment[];
  /** False when no item carries anything to say: playback must not start, and the user is told. */
  hasSpeech: boolean;
}

/**
 * The segments to speak, in the order Rust gave them. A note is spoken where it stands, so a
 * listener who does not see the screen hears that a table or a block of code was left out. Notes
 * alone are not a reading: `hasSpeech` needs at least one `speech` item with text.
 */
export function spokenReading(
  items: ReadingItem[],
  noteText: (code: ReadingNoteCode, count: number) => string,
): SpokenReading {
  const segments: SpeakSegment[] = [];
  let hasSpeech = false;
  for (const item of items) {
    if (item.kind === "speech") {
      const text = item.text.trim();
      if (text.length > 0) {
        hasSpeech = true;
        segments.push({ text, pauseAfterMs: PAUSE_MS[item.pauseAfter] });
      }
    } else {
      const text = noteText(item.code, item.count).trim();
      if (text.length > 0) {
        segments.push({ text, pauseAfterMs: NOTE_PAUSE_MS });
      }
    }
  }
  return { segments, hasSpeech };
}
