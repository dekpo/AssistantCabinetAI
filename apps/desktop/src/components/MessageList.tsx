import { useEffect, useRef } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { formatDuration } from "../lib/duration";
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
            {entry.content.length === 0 && pending ? (
              <span className="message__pending">
                <span className="spinner" aria-hidden="true" />
                {t("chat.pending")}
              </span>
            ) : (
              entry.content
            )}
          </p>
          {entry.sources !== undefined && entry.sources.length > 0 ? (
            <ul className="message__sources">
              {entry.sources.map((source) => (
                <li key={source.chunkId} className="message__source">
                  {t("chat.sourceLabel", {
                    path: source.relativePath,
                    page: source.pageNumber,
                  })}
                  {source.origin === "ocr" ? t("chat.sourceOcrMarker") : null}
                </li>
              ))}
            </ul>
          ) : null}
          {entry.durationMs !== undefined && entry.modelAlias !== undefined ? (
            <p className="message__timing">
              {t("chat.generatedBy", {
                model: entry.modelAlias,
                duration: formatDuration(entry.durationMs),
              })}
            </p>
          ) : null}
        </article>
      ))}
      <div ref={bottom} />
    </div>
  );
}
