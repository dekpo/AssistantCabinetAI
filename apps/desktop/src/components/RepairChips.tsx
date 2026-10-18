import { useTranslation } from "../i18n/I18nProvider";
import { pluralise } from "../lib/plural";
import type { RepairBridge } from "../speech/useRepairSuggestions";

/**
 * Under a text box, after a dictation: the words that may have meant a name of the files in use,
 * each with the name or names it may have meant. Nothing is replaced until she presses the name;
 * Ignore removes the suggestion; the text is never touched otherwise. While names are hidden, the
 * names are not here at all: one line says how many suggestions wait, with the button that shows
 * names (the same switch the suggestions under a conversation use). It draws nothing when there is
 * nothing to say, and it holds no state of the box it sits under, so a note editor can mount it too.
 */
export function RepairChips({ repair }: { repair: RepairBridge }) {
  const { t } = useTranslation();
  const { chips, hiddenCount } = repair;
  if (chips.length === 0 && hiddenCount === 0) {
    return null;
  }
  return (
    <div className="repair" role="group" aria-label={t("speech.repair.label")}>
      {hiddenCount > 0 ? (
        <p className="repair__hidden">
          <span>
            {t(pluralise(hiddenCount, "speech.repair.hiddenOne", "speech.repair.hiddenMany"), {
              count: hiddenCount,
            })}
          </span>
          <button type="button" className="button button--compact" onClick={repair.reveal}>
            {t("speech.repair.show")}
          </button>
        </p>
      ) : null}
      {chips.length === 0 ? null : (
        <ul className="repair__list">
          {chips.map((chip) => {
            const [first] = chip.candidates;
            return (
              <li className="repair__item" key={chip.id}>
                {chip.action === "suggest" && first !== undefined ? (
                  <>
                    <span className="repair__text">
                      {t("speech.repair.suggest", {
                        heard: chip.heard,
                        display: first.display,
                      })}
                    </span>
                    <button
                      type="button"
                      className="button button--compact"
                      aria-label={t("speech.repair.acceptLabel", {
                        heard: chip.heard,
                        display: first.display,
                      })}
                      onClick={() => repair.accept(chip.id, first.entityId)}
                    >
                      {t("speech.repair.accept")}
                    </button>
                  </>
                ) : (
                  <>
                    <span className="repair__text">
                      {t("speech.repair.ask", { heard: chip.heard })}
                    </span>
                    {chip.candidates.map((candidate) => (
                      <button
                        type="button"
                        className="button button--compact"
                        key={candidate.entityId}
                        aria-label={t("speech.repair.acceptLabel", {
                          heard: chip.heard,
                          display: candidate.display,
                        })}
                        onClick={() => repair.accept(chip.id, candidate.entityId)}
                      >
                        {candidate.display}
                      </button>
                    ))}
                  </>
                )}
                <button
                  type="button"
                  className="button button--compact"
                  aria-label={t("speech.repair.ignoreLabel", {
                    heard: chip.heard,
                  })}
                  onClick={() => repair.ignore(chip.id)}
                >
                  {t("speech.repair.ignore")}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
