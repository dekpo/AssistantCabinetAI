import { useEffect, useRef } from "react";
import { useTranslation } from "../i18n/I18nProvider";

/**
 * One question, two answers, nothing else.
 *
 * Built rather than borrowed from the operating system on purpose. A native confirmation is
 * written and laid out by the platform: its buttons carry the system's words in the system's
 * language, which is not necessarily hers, and that breaks the one rule the product does not bend
 * — everything a human reads is in her language (`docs/LANGUAGE-AND-LOCALE.md`). It would also
 * look like Windows on Windows and like macOS on macOS, in the middle of a window that looks like
 * neither.
 *
 * Small and centred rather than the full-window `.dialog` that settings uses: this is a question
 * about one button, not a place to work. It is deliberately generic — the file actions of sprint 3
 * need exactly this shape, a plan and a human approving it (`AGENTS.md`).
 */
export function ConfirmDialog({
  title,
  body,
  confirmLabel,
  destructive = false,
  busy = false,
  onConfirm,
  onCancel,
}: {
  title: string;
  body: string;
  confirmLabel: string;
  /** Marks the confirming button as the one that throws something away. */
  destructive?: boolean;
  busy?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const cancelRef = useRef<HTMLButtonElement>(null);

  /* Focus lands on the safe answer, not the destructive one: a stray Enter left over from the
     click that opened this must not be the thing that empties the index. */
  useEffect(() => {
    cancelRef.current?.focus();
  }, []);

  /* Escape is how every other dismissible surface in this window closes, and a confirmation the
     keyboard cannot refuse is a trap. Bound on the document because the backdrop is not always
     what holds focus. */
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) {
        onCancel();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [busy, onCancel]);

  return (
    <div className="confirm__backdrop" onMouseDown={() => (busy ? undefined : onCancel())}>
      {/* The click that chose an answer must not also count as a click on the backdrop. */}
      <div
        className="confirm"
        role="alertdialog"
        aria-modal="true"
        aria-label={title}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <h2 className="confirm__title">{title}</h2>
        <p className="confirm__body">{body}</p>
        <div className="confirm__actions">
          <button
            type="button"
            ref={cancelRef}
            className="button button--compact"
            disabled={busy}
            onClick={onCancel}
          >
            {t("actions.cancel")}
          </button>
          <button
            type="button"
            className={
              destructive
                ? "button button--compact button--destructive"
                : "button button--compact button--primary"
            }
            disabled={busy}
            onClick={onConfirm}
          >
            {busy ? (
              <span className="message__pending">
                <span className="spinner" aria-hidden="true" />
                {confirmLabel}
              </span>
            ) : (
              confirmLabel
            )}
          </button>
        </div>
      </div>
    </div>
  );
}
