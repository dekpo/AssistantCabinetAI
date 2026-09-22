import { useEffect, useRef, useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { formatDuration } from "../lib/duration";
import type { GenerationPhase } from "../lib/generation";
import { isNearBottom, prefersReducedMotion, scrollBehaviour } from "../lib/scroll";
import { timestampParts } from "../lib/timestamp";
import type { ChatEntry } from "../state/useChat";
import { ArrowDownGlyph } from "./ArrowDownGlyph";
import { CollapsibleText } from "./CollapsibleText";
import { Markdown } from "./Markdown";

export function MessageList({
  entries,
  phase,
  streamingId,
  onStop,
}: {
  entries: ChatEntry[];
  phase: GenerationPhase;
  streamingId: string | null;
  onStop: () => void;
}) {
  const { t } = useTranslation();
  const messages = useRef<HTMLDivElement>(null);
  const bottom = useRef<HTMLDivElement>(null);
  const turnCount = useRef(0);
  /* Whether a streaming answer may keep pulling the view down. Read from the scroll position as
     she scrolls, never after the text grew, so a delta that adds a line is not mistaken for her
     having scrolled away. */
  const following = useRef(true);
  const [endVisible, setEndVisible] = useState(true);

  const onScroll = () => {
    const root = messages.current;
    if (root === null) {
      return;
    }
    const atEnd = isNearBottom(root.scrollHeight, root.scrollTop, root.clientHeight);
    following.current = atEnd;
    setEndVisible(atEnd);
  };

  useEffect(() => {
    // A new turn is her own action, so it always scrolls. A delta of a streaming answer follows
    // only while she is at the bottom: `entries` is replaced on every delta, so an ungated effect
    // drags the view back down many times a second even though she is reading further up.
    const isNewTurn = entries.length !== turnCount.current;
    turnCount.current = entries.length;
    if (isNewTurn || following.current) {
      bottom.current?.scrollIntoView({ block: "end" });
    }
  }, [entries]);

  if (entries.length === 0) {
    return (
      <div className="messages messages--empty">
        <h2 className="empty__title">{t("chat.emptyTitle")}</h2>
        <p className="empty__body">{t("chat.emptyBody")}</p>
        <p className="empty__body">{t("chat.emptyBodyNetwork")}</p>
      </div>
    );
  }

  return (
    <div className="messages-pane">
      <div className="messages" ref={messages} onScroll={onScroll}>
        {entries.map((entry) => {
          if (entry.role === "user") {
            return (
              <article key={entry.id} className="message message--user">
                <p className="message__timing message__timing--asked">
                  {t("chat.askedAt", timestampParts(entry.createdAt))}
                </p>
                <div className="message__bubble">
                  <CollapsibleText text={entry.content} />
                </div>
              </article>
            );
          }
          // Both read the phase of the one answer being written, so a spinner, a stop and an
          // answer already finished further up can never be confused for one another.
          const streaming = entry.id === streamingId;
          const showPending = streaming && phase === "thinking";
          const showStop = streaming && phase === "writing";
          return (
            <article key={entry.id} className="message message--assistant">
              <p className="message__author">{t("chat.authorAssistant")}</p>
              {/* A div, not a paragraph: markdown emits lists and headings, which cannot legally
                  nest inside a `<p>` and make the browser restructure the DOM silently. */}
              <div className="message__body message__body--markdown">
                {showPending ? (
                  <span className="message__pending">
                    <span className="spinner" aria-hidden="true" />
                    {t("chat.pending")}
                    {/* The second way to stop, where she is already looking. It lives inside the
                        spinner line on purpose: it therefore exists exactly while the spinner
                        does, which is the rule itself rather than a copy of it. */}
                    <button
                      type="button"
                      className="button button--primary message__stop"
                      onClick={onStop}
                    >
                      {t("actions.stop")}
                    </button>
                  </span>
                ) : (
                  <Markdown text={entry.content} />
                )}
              </div>
              {/* Under the text, following its end as it grows, and gone the moment it stops
                  growing. Nothing caps how long an answer may be, so a model that falls into a
                  repetition loop has to be stoppable from where she is reading it. */}
              {showStop ? (
                <p className="message__actions">
                  <button
                    type="button"
                    className="button button--primary message__stop"
                    onClick={onStop}
                  >
                    {t("actions.stop")}
                  </button>
                </p>
              ) : null}
              {entry.sources !== undefined && entry.sources.length > 0 ? (
                <details className="message__sources">
                  <summary className="disclosure">{t("chat.sourcesToggle")}</summary>
                  <ul className="message__sources-list">
                    {entry.sources.map((source, index) => (
                      <li key={source.chunkId} className="message__source">
                        {t("chat.sourceItem", {
                          n: index + 1,
                          path: source.relativePath,
                          page: source.pageNumber,
                        })}
                        {source.origin === "ocr" ? t("chat.sourceOcrMarker") : null}
                      </li>
                    ))}
                  </ul>
                </details>
              ) : null}
              {/* An answer she stopped has no duration to report, so this line takes the place of
                  the one below rather than joining it: what matters is that it is incomplete. */}
              {entry.interrupted === true ? (
                <p className="message__timing">{t("chat.interrupted")}</p>
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
          );
        })}
        <div ref={bottom} />
      </div>
      {endVisible ? null : (
        <button
          type="button"
          className="button messages-pane__to-bottom"
          aria-label={t("chat.scrollToBottom")}
          onClick={() =>
            bottom.current?.scrollIntoView({
              behavior: scrollBehaviour(prefersReducedMotion()),
              block: "end",
            })
          }
        >
          <ArrowDownGlyph />
        </button>
      )}
    </div>
  );
}
