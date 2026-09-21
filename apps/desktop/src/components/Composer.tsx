import { useState, type KeyboardEvent, type ReactNode } from "react";
import { useTranslation } from "../i18n/I18nProvider";

export function Composer({
  pending,
  onSend,
  hint,
}: {
  pending: boolean;
  onSend: (question: string) => void;
  hint: ReactNode;
}) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState("");

  const submit = () => {
    if (draft.trim().length === 0 || pending) {
      return;
    }
    onSend(draft);
    setDraft("");
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      submit();
    }
  };

  return (
    <div className="composer">
      <textarea
        className="composer__input"
        value={draft}
        rows={3}
        placeholder={t("chat.placeholder")}
        aria-label={t("chat.placeholder")}
        onChange={(event) => setDraft(event.target.value)}
        onKeyDown={onKeyDown}
      />
      <div className="composer__footer">
        <div className="composer__footnotes">{hint}</div>
        <button
          type="button"
          className="button button--primary composer__send"
          onClick={submit}
          disabled={pending || draft.trim().length === 0}
        >
          {t("actions.send")}
        </button>
      </div>
    </div>
  );
}
