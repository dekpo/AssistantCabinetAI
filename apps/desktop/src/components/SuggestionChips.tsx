import { useTranslation } from "../i18n/I18nProvider";
import { suggestionView, type SuggestionView } from "../lib/suggestions";
import type { CombinedScope } from "../lib/ipc";
import { useSuggestions } from "../state/useSuggestions";

/**
 * Questions worth asking, as buttons: in the empty conversation and under an analysis summary.
 *
 * Every one was proved answerable on the selection before it got here (`knowledge::suggest`), so a
 * click sends the sentence as it stands. Names are left out unless the setting allows them; the
 * action that shows them is the user's own click, lasts for the session, and can be undone.
 * Nothing is drawn while there is nothing to suggest.
 */
/** The buttons themselves, drawn from a view: nothing here looks anything up. */
export function SuggestionList({
  view,
  disabled,
  compact,
  onAsk,
  onRevealNamesChange,
}: {
  view: SuggestionView;
  /** A question is being answered. */
  disabled: boolean;
  compact: boolean;
  onAsk: (question: string) => void;
  onRevealNamesChange: (reveal: boolean) => void;
}) {
  const { t } = useTranslation();
  if (view.items.length === 0 && !view.canRevealNames) {
    return null;
  }
  return (
    <section className={compact ? "suggestions suggestions--compact" : "suggestions"}>
      <h3 className="suggestions__title">{t("suggestions.title")}</h3>
      {compact ? null : <p className="suggestions__intro">{t("suggestions.intro")}</p>}
      {view.items.length > 0 ? (
        <ul className="suggestions__list">
          {view.items.map((item) => (
            <li key={item.key}>
              <button
                type="button"
                className="suggestions__item"
                disabled={disabled}
                onClick={() => onAsk(item.text)}
              >
                {item.text}
              </button>
            </li>
          ))}
        </ul>
      ) : null}
      {view.canRevealNames ? (
        <p className="suggestions__names">
          <span>{t("suggestions.namesHidden")}</span>
          <button
            type="button"
            className="button button--compact"
            onClick={() => onRevealNamesChange(true)}
          >
            {t("suggestions.showNames")}
          </button>
        </p>
      ) : null}
      {view.canHideNames ? (
        <p className="suggestions__names">
          <button
            type="button"
            className="button button--compact"
            onClick={() => onRevealNamesChange(false)}
          >
            {t("suggestions.hideNames")}
          </button>
        </p>
      ) : null}
    </section>
  );
}

export function SuggestionChips({
  scope,
  revealNames,
  onRevealNamesChange,
  refresh,
  enabled,
  disabled,
  onAsk,
  compact = false,
}: {
  scope: CombinedScope;
  /** Whether the user asked, for this session, to see suggestions that name somebody. */
  revealNames: boolean;
  onRevealNamesChange: (reveal: boolean) => void;
  /** Changes whenever the base may have changed, so the questions are looked for again. */
  refresh: unknown;
  /** False while something is writing to the base. */
  enabled: boolean;
  /** A question is being answered. */
  disabled: boolean;
  onAsk: (question: string) => void;
  compact?: boolean;
}) {
  const { t, locale } = useTranslation();
  const set = useSuggestions(scope, revealNames, refresh, enabled);
  return (
    <SuggestionList
      view={suggestionView(t, locale, set, revealNames)}
      disabled={disabled}
      compact={compact}
      onAsk={onAsk}
      onRevealNamesChange={onRevealNamesChange}
    />
  );
}
