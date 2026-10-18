import { useEffect } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import type { ChatEntry } from "../state/useChat";
import { readingParts } from "../speech/output/readingParts";
import { leaveEntry } from "../speech/output/readAloud";
import { useReadAloud } from "../speech/output/ReadAloudContext";
import { SpeakerGlyph } from "./SpeakerGlyph";

/**
 * Read this answer aloud, or stop the reading. It is drawn only while the voice and read-aloud are
 * switched on (and nothing at all otherwise, so the action row is exactly what it was with the voice
 * off), it reads only when it is pressed, and it ends the reading when it leaves the screen: a new
 * question, a regenerate and a cleared conversation all hide the row it sits in.
 */
export function ReadAloudButton({ entry }: { entry: ChatEntry }) {
  const { t, locale } = useTranslation();
  const readAloud = useReadAloud();
  const controller = readAloud?.controller;
  const entryId = entry.id;

  useEffect(() => {
    return () => {
      if (controller !== undefined) {
        leaveEntry(controller, entryId);
      }
    };
  }, [controller, entryId]);

  if (readAloud === null || !readAloud.visible) {
    return null;
  }
  const { state } = readAloud;
  const mine = state.entryId === entryId;
  const reading = mine && state.phase !== "idle";
  const problem = state.problem?.entryId === entryId ? state.problem : null;

  return (
    <>
      <button
        type="button"
        className="button button--compact message__read"
        aria-pressed={reading}
        onClick={() => {
          if (reading) {
            readAloud.controller.stop();
          } else {
            void readAloud.controller.start(entryId, readingParts(t, locale, entry), locale);
          }
        }}
      >
        <SpeakerGlyph speaking={mine && state.phase === "speaking"} />
        {t(reading ? "speech.readAloud.stop" : "speech.readAloud.start")}
      </button>
      {problem === null ? null : (
        <span className="message__timing message__speech-note" role="alert">
          {t(problem.key, problem.values)}
        </span>
      )}
    </>
  );
}
