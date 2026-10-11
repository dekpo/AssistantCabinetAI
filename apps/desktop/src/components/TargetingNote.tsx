import { useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import {
  targetingNote,
  targetingOffers,
  type TargetingChoice,
} from "../lib/targeting";
import type { TargetingReport } from "../lib/ipc";

/**
 * Under an answer: what the knowledge base did to the search, and what it asks back. Small, in the
 * family of the coverage and "without documents" lines, and silent when it did nothing.
 *
 * Everything shown is written by `lib/targeting.ts` from machine data; this only draws it and
 * forwards a click as a choice. A click re-asks the question (`onChoose`), it never edits it.
 */
export function TargetingNote({
  report,
  chosenIds,
  idle,
  onChoose,
}: {
  report: TargetingReport | undefined;
  /** Names already picked on this answer, so they are not offered again. */
  chosenIds: readonly number[];
  /** Nothing can be re-asked while an answer is being written. */
  idle: boolean;
  onChoose: (choice: TargetingChoice) => void;
}) {
  const { t } = useTranslation();
  const [listing, setListing] = useState(false);
  const note = targetingNote(t, report);
  const offers = targetingOffers(t, report, chosenIds);

  if (note === null && offers.length === 0) {
    return null;
  }

  return (
    <div className="targeting">
      {note === null
        ? null
        : note.lines.map((line) => (
            <p className="message__timing targeting__line" key={line.kind}>
              <span>{line.text}</span>
              {idle && line.action !== null ? (
                <button
                  type="button"
                  className="button button--compact"
                  onClick={() => onChoose({ mode: "off" })}
                >
                  {line.action.label}
                </button>
              ) : null}
              {line.kind !== "workbook" && note.files.length > 0 ? (
                <button
                  type="button"
                  className="button button--compact"
                  aria-expanded={listing}
                  onClick={() => setListing(!listing)}
                >
                  {t(listing ? "chat.targeting.hideFiles" : "chat.targeting.showFiles")}
                </button>
              ) : null}
            </p>
          ))}
      {note !== null && listing ? (
        <ul className="message__sources-list targeting__files">
          {note.files.map((file) => (
            <li className="message__source" key={file}>
              {file}
            </li>
          ))}
        </ul>
      ) : null}
      {/* The question was answered anyway, on the whole selection. These ask which name was meant,
          and the answer to that narrows the search at once. Nothing is offered while she waits. */}
      {idle
        ? offers.map((offer, index) => (
            <div className="targeting__offer" key={`${offer.kind}-${index}`}>
              <p className="message__timing">{offer.text}</p>
              <p className="message__actions">
                {offer.options.map((option) => (
                  <button
                    key={option.entityId}
                    type="button"
                    className="button button--compact"
                    onClick={() => onChoose({ entityIds: [option.entityId] })}
                  >
                    {option.label}
                  </button>
                ))}
              </p>
            </div>
          ))
        : null}
    </div>
  );
}
