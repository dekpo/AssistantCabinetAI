import { useEffect, useRef } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import type { ChatEntry } from "../state/useChat";

export function MessageList({ entries, pending }: { entries: ChatEntry[]; pending: boolean }) {
  const { t } = useTranslation();
  const bottom = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottom.current?.scrollIntoView({ block: "end" });
  }, [entries]);

  if (entries.length === 0) {
    return (
      <div className="messages messages--empty">
        <h2 className="empty__title">{t("chat.emptyTitle")}</h2>
        <p className="empty__body">{t("chat.emptyBody")}</p>
      </div>
    );
  }

  return (
    <div className="messages">
      {entries.map((entry) => (
        <article key={entry.id} className={`message message--${entry.role}`}>
          <p className="message__author">
            {entry.role === "user" ? t("chat.authorUser") : t("chat.authorAssistant")}
          </p>
          <p className="message__body">
            {entry.content.length === 0 && pending ? t("chat.pending") : entry.content}
          </p>
        </article>
      ))}
      <div ref={bottom} />
    </div>
  );
}
