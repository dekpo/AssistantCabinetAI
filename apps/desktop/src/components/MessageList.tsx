import { useEffect, useRef, useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { copyToClipboard } from "../lib/clipboard";
import { coverageLine } from "../lib/coverage";
import { formatDuration } from "../lib/duration";
import { counted } from "../lib/plural";
import { groupSources } from "../lib/sourceGroups";
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
  onResend,
  onRegenerate,
  onAnalyse,
  analysing,
}: {
  entries: ChatEntry[];
  phase: GenerationPhase;
  streamingId: string | null;
  onStop: () => void;
  /** Edit-and-repost a past question (docs/CHAT-UX-ASSESSMENT.md item 3). */
  onResend: (questionEntryId: string, text: string) => void;
  /** Regenerate a past answer (item 4). */
  onRegenerate: (answerEntryId: string) => void;
  /** Analyse the folder, then ask this question again. Offered under the one answer that says the
   * documents have not been read yet, because that is the only thing that answer needs. */
  onAnalyse: (answerEntryId: string) => void;
  /** Whether the shared analysis pass is running, wherever it was started from. */
  analysing: boolean;
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
  /* The one question being edited in place, or null. Editing is exclusive - only one turn's
     textarea exists at a time - so this is a single id rather than a set. */
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editDraft, setEditDraft] = useState("");
  /* Copy and edit change nothing about the conversation itself, so a generating turn does not
     block them; but resend and regenerate start a new request, which the existing "one question
     at a time" rule already enforces in `useChat` - here they are simply hidden while busy, since
     an answer already being written cannot be edited or regenerated. */
  const idle = phase === "idle";

  const startEdit = (entry: ChatEntry) => {
    setEditingId(entry.id);
    setEditDraft(entry.content);
  };
  const cancelEdit = () => {
    setEditingId(null);
    setEditDraft("");
  };
  const submitEdit = (questionEntryId: string) => {
    const text = editDraft.trim();
    if (text.length === 0) {
      return;
    }
    onResend(questionEntryId, text);
    cancelEdit();
  };

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
            const editing = editingId === entry.id;
            return (
              <article
                key={entry.id}
                className={editing ? "message message--user message--editing" : "message message--user"}
              >
                <p className="message__timing message__timing--asked">
                  {t("chat.askedAt", timestampParts(entry.createdAt))}
                </p>
                {editing ? (
                  <div className="message__edit">
                    <textarea
                      className="message__edit-input"
                      value={editDraft}
                      rows={3}
                      autoFocus
                      onChange={(event) => setEditDraft(event.target.value)}
                    />
                    <p className="message__actions">
                      <button type="button" className="button" onClick={cancelEdit}>
                        {t("actions.cancel")}
                      </button>
                      <button
                        type="button"
                        className="button button--primary"
                        onClick={() => submitEdit(entry.id)}
                        disabled={editDraft.trim().length === 0}
                      >
                        {t("actions.send")}
                      </button>
                    </p>
                  </div>
                ) : (
                  <>
                    <div className="message__bubble">
                      <CollapsibleText text={entry.content} />
                    </div>
                    {idle ? (
                      <p className="message__actions message__actions--turn">
                        <button
                          type="button"
                          className="button button--compact"
                          onClick={() => void copyToClipboard(entry.content)}
                        >
                          {t("actions.copy")}
                        </button>
                        <button
                          type="button"
                          className="button button--compact"
                          onClick={() => startEdit(entry)}
                        >
                          {t("actions.edit")}
                        </button>
                      </p>
                    ) : null}
                  </>
                )}
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
                      className="button button--compact message__stop"
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
                  {/* Discreet on purpose, styled like the copy/edit/regenerate row rather than
                      `button--primary`: a filled accent button reads as "the thing to do next",
                      which is wrong for a stop that most answers never need. */}
                  <button
                    type="button"
                    className="button button--compact message__stop"
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
                    {groupSources(entry.sources).map((group) => (
                      <li key={group.relativePath} className="message__source">
                        {t(group.pages.length > 1 ? "chat.sourceItemPages" : "chat.sourceItemPage", {
                          path: group.relativePath,
                          pages: group.pages.join(", "),
                        })}
                        {group.ocr ? t("chat.sourceOcrMarker") : null}
                      </li>
                    ))}
                  </ul>
                </details>
              ) : null}
              {/* How much of the folder this answer rests on, for a question that asked about
                  every document. Written beside the answer rather than inside it, so the model
                  cannot leave it out (`docs/WORK-FOLDER-INVENTORY.md`). */}
              {entry.coverage !== undefined ? (
                <p className="message__timing">{coverageLine(t, entry.coverage)}</p>
              ) : null}
              {/* Documents the answer could not have used. Retrieval refuses when it has too
                  little evidence; this is the other half, for when it had plenty and the file she
                  had in mind was simply not among it. */}
              {entry.unanalysedFiles === undefined ? null : (
                <p className="message__timing">
                  {t("chat.unanalysed", {
                    documents: counted(
                      entry.unanalysedFiles,
                      t("chat.unanalysedOne"),
                      t("chat.unanalysedMany"),
                    ),
                  })}
                </p>
              )}
              {/* Files she chose for this conversation that are gone or have changed. The answer was
                  written without them, and the line says so. */}
              {entry.scopeOutdated === undefined ? null : (
                <p className="message__timing">
                  {t("chat.scopeOutdated", {
                    files: counted(
                      entry.scopeOutdated.length,
                      t("chat.scopeFileOne"),
                      t("chat.scopeFileMany"),
                    ),
                  })}
                </p>
              )}
              {/* An answer she stopped has no duration to report, so this line takes the place of
                  the one below rather than joining it: what matters is that it is incomplete. */}
              {entry.interrupted === undefined ? null : (
                <p className="message__timing">
                  {t(entry.interrupted === "timedOut" ? "chat.timedOut" : "chat.interrupted")}
                </p>
              )}
              {entry.durationMs !== undefined && entry.modelAlias !== undefined ? (
                <p className="message__timing">
                  {t("chat.generatedBy", {
                    model: entry.modelAlias,
                    duration: formatDuration(entry.durationMs),
                  })}
                </p>
              ) : null}
              {/* Where the "generated by" line would be, for an answer no model wrote. It takes
                  that slot rather than sitting beside it: an answer has one provenance. */}
              {entry.deterministic === true ? (
                <p className="message__timing">{t("chat.deterministic")}</p>
              ) : null}
              {/* An answer that says "analyse your documents first" is not text to keep or to
                  write again - it is a step she has not taken. Copy and regenerate would both
                  produce the same sentence, so the turn offers the step itself, and asks the
                  question again once the pass has run. */}
              {idle && entry.needsIndexing === true ? (
                <p className="message__actions message__actions--turn">
                  <button
                    type="button"
                    className="button button--compact"
                    disabled={analysing}
                    onClick={() => onAnalyse(entry.id)}
                  >
                    {analysing ? (
                      <span className="message__pending">
                        <span className="spinner" aria-hidden="true" />
                        {t("actions.analyzing")}
                      </span>
                    ) : (
                      t("actions.analyze")
                    )}
                  </button>
                </p>
              ) : idle && entry.content.length > 0 ? (
                <p className="message__actions message__actions--turn">
                  <button
                    type="button"
                    className="button button--compact"
                    onClick={() => void copyToClipboard(entry.content)}
                  >
                    {t("actions.copy")}
                  </button>
                  {/* Regenerating drops this answer and reruns retrieval, embedding and the model
                      from the question that produced it, so a new answer never keeps an old
                      answer's sources - a citation from the previous run would be worse than
                      none.

                      On an answer the work folder computed, rerunning would return the same
                      answer byte for byte, so the same control means "ask the model instead" and
                      says so. One button, two honest meanings, rather than a second button. */}
                  <button
                    type="button"
                    className="button button--compact"
                    onClick={() => onRegenerate(entry.id)}
                  >
                    {t(
                      entry.deterministic === true
                        ? "actions.regenerateWithModel"
                        : "actions.regenerate",
                    )}
                  </button>
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
