import { useEffect, useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { copyToClipboard } from "../lib/clipboard";
import { answerForCopy } from "../lib/annotations";
import { formatConversation, hasCopyableConversation } from "../lib/conversationText";
import type { CombinedScope } from "../lib/ipc";
import { useChat } from "../state/useChat";
import type { IndexingState } from "../state/useIndexing";
import { Composer } from "./Composer";
import { ConfirmDialog } from "./ConfirmDialog";
import { ErrorBanner } from "./ErrorBanner";
import { KeyGlyph } from "./KeyGlyph";
import { MessageList } from "./MessageList";

export function ChatPanel({
  onFailure,
  hasWorkFolder,
  scope,
  onBusyChange,
  modelAlias,
  aliases,
  indexing,
  onModelAliasChange,
}: {
  onFailure: () => void;
  hasWorkFolder: boolean;
  /** The files this conversation is about, chosen in the two folder cards beside it. */
  scope: CombinedScope;
  /** Whether a question is being answered, so the folder card can lock the choice meanwhile. */
  onBusyChange: (busy: boolean) => void;
  modelAlias: string;
  aliases: string[];
  /** The same analysis pass the folder card starts, so the answer that asks for one starts that
   * pass rather than a second one of its own. */
  indexing: IndexingState;
  onModelAliasChange: (alias: string) => void;
}) {
  const { t, locale } = useTranslation();
  const [draft, setDraft] = useState("");
  const {
    entries,
    phase,
    streamingId,
    error,
    send,
    resend,
    regenerate,
    stop,
    dismissError,
    clear,
  } = useChat(onFailure, hasWorkFolder, modelAlias, scope);
  /* The sidebar's picker is locked while a question is being answered, and only this panel knows
     when that is. */
  const busy = phase !== "idle";
  useEffect(() => onBusyChange(busy), [busy, onBusyChange]);
  /* Confirmed before anything is thrown away - the one guarantee this temporary control has to
     keep, since there is nowhere to recover a cleared conversation from yet
     (`docs/ROADMAP.md`, saved conversations). */
  const [confirmingClear, setConfirmingClear] = useState(false);

  const onStop = () => {
    const stopped = stop();
    if (stopped !== null) {
      setDraft(stopped);
    }
  };

  /* What she asked for by clicking "analyse" under an unanswered question: the pass, and then the
     answer she wanted in the first place. A pass that failed stops here - its error is already on
     screen in the folder card, and asking again would only produce the same refusal. */
  const analyseThenAsk = async (answerEntryId: string) => {
    if (await indexing.run()) {
      await regenerate(answerEntryId);
    }
  };

  const copyAll = () => {
    // Everything she sees except the lists of sources: the text as shown, the engine's own figure
    // and the notes under each answer (`lib/annotations.ts`).
    const shown = entries.map((entry) =>
      entry.role === "assistant" ? { ...entry, content: answerForCopy(t, locale, entry) } : entry,
    );
    const text = formatConversation(shown, streamingId, (role) =>
      t(role === "user" ? "chat.authorUser" : "chat.authorAssistant"),
    );
    void copyToClipboard(text);
  };

  const hint = (
    <>
      <p className="composer__hint">
        <span>{t("chat.keyboardHintSend")}</span>
        <KeyGlyph name="enter" />
        <span>,</span>
        <span>{t("chat.keyboardHintBreak")}</span>
        <KeyGlyph name="shift" />
        <KeyGlyph name="enter" />
      </p>
      <p className="disclaimer">{t("chat.disclaimer")}</p>
    </>
  );

  return (
    <section className="chat">
      <MessageList
        entries={entries}
        phase={phase}
        streamingId={streamingId}
        onStop={onStop}
        onResend={(questionEntryId, text) => void resend(questionEntryId, text)}
        onRegenerate={(answerEntryId) => void regenerate(answerEntryId)}
        onAnalyse={(answerEntryId) => void analyseThenAsk(answerEntryId)}
        analysing={indexing.running}
      />
      {error === null ? null : <ErrorBanner error={error} onDismiss={dismissError} />}
      {entries.length === 0 ? null : (
        <p className="message__actions chat__conversation-actions">
          {/* Temporary, until conversations can be saved and listed (`docs/ROADMAP.md`): today the
              only way to a clean slate is restarting the application, and this replaces that.
              Left, opposite "Copier la conversation", because the two act on the whole panel but
              pull in opposite directions - one keeps the conversation, the other throws it away. */}
          <button
            type="button"
            className="button button--compact"
            disabled={busy}
            onClick={() => setConfirmingClear(true)}
          >
            {t("actions.clearConversation")}
          </button>
          {hasCopyableConversation(entries, streamingId) ? (
            <button type="button" className="button button--compact" onClick={copyAll}>
              {t("actions.copyConversation")}
            </button>
          ) : null}
        </p>
      )}
      {confirmingClear ? (
        <ConfirmDialog
          title={t("chat.clearTitle")}
          body={t("chat.clearBody")}
          confirmLabel={t("actions.clearConversation")}
          destructive
          onConfirm={() => {
            clear();
            setConfirmingClear(false);
          }}
          onCancel={() => setConfirmingClear(false)}
        />
      ) : null}
      <Composer
        draft={draft}
        onDraftChange={setDraft}
        phase={phase}
        onSend={(question) => void send(question)}
        onStop={onStop}
        hint={hint}
        modelAlias={modelAlias}
        aliases={aliases}
        onModelAliasChange={onModelAliasChange}
        needsFolder={!hasWorkFolder}
      />
    </section>
  );
}
