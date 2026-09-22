import type { KeyboardEvent, ReactNode } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { canDiscard, type GenerationPhase } from "../lib/generation";

/**
 * The draft is held by the chat panel rather than here, because stopping a question offers its
 * text back: the text she was about to have answered belongs to the conversation, not to the box.
 */
export function Composer({
  draft,
  onDraftChange,
  phase,
  onSend,
  onStop,
  hint,
  modelAlias,
  aliases,
  onModelAliasChange,
}: {
  draft: string;
  onDraftChange: (draft: string) => void;
  phase: GenerationPhase;
  onSend: (question: string) => void;
  onStop: () => void;
  hint: ReactNode;
  modelAlias: string;
  aliases: string[];
  onModelAliasChange: (alias: string) => void;
}) {
  const { t } = useTranslation();

  const submit = () => {
    if (draft.trim().length === 0 || phase !== "idle") {
      return;
    }
    onSend(draft);
    onDraftChange("");
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      submit();
    }
  };

  /* One button in one place, so the eye does not have to move and the keyboard does not lose it:
     it offers the stop only while the answer has not started, and goes back to sending - disabled,
     since the box is empty again - the moment the first words arrive. Stopping an answer that is
     already being written belongs under that answer, not here: it is a different act, because it
     keeps the text. */
  const stopping = canDiscard(phase);

  return (
    <div className="composer">
      <textarea
        className="composer__input"
        value={draft}
        rows={3}
        placeholder={t("chat.placeholder")}
        aria-label={t("chat.placeholder")}
        onChange={(event) => onDraftChange(event.target.value)}
        onKeyDown={onKeyDown}
      />
      <div className="composer__footer">
        <div className="composer__footnotes">{hint}</div>
        <div className="composer__actions">
          {aliases.length === 0 ? null : (
            <select
              className="field__control composer__model-select"
              aria-label={t("chat.modelLabel")}
              value={modelAlias}
              onChange={(event) => onModelAliasChange(event.target.value)}
            >
              {aliases.map((alias) => (
                <option key={alias} value={alias}>
                  {alias}
                </option>
              ))}
            </select>
          )}
          <button
            type="button"
            className="button button--primary composer__send"
            onClick={stopping ? onStop : submit}
            disabled={!stopping && (phase !== "idle" || draft.trim().length === 0)}
          >
            {t(stopping ? "actions.stop" : "actions.send")}
          </button>
        </div>
      </div>
    </div>
  );
}
