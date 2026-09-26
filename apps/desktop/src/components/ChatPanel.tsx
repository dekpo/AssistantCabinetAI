import { useEffect, useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { copyToClipboard } from "../lib/clipboard";
import { formatConversation, hasCopyableConversation } from "../lib/conversationText";
import type { AnalysisScope } from "../lib/ipc";
import { useChat } from "../state/useChat";
import type { IndexingState } from "../state/useIndexing";
import { Composer } from "./Composer";
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
  /** The files this conversation is about, chosen in the folder card beside it. */
  scope: AnalysisScope;
  /** Whether a question is being answered, so the folder card can lock the choice meanwhile. */
  onBusyChange: (busy: boolean) => void;
  modelAlias: string;
  aliases: string[];
  /** The same analysis pass the folder card starts, so the answer that asks for one starts that
   * pass rather than a second one of its own. */
  indexing: IndexingState;
  onModelAliasChange: (alias: string) => void;
}) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState("");
  const { entries, phase, streamingId, error, send, resend, regenerate, stop, dismissError } =
    useChat(onFailure, hasWorkFolder, modelAlias, scope);
  /* The sidebar's picker is locked while a question is being answered, and only this panel knows
     when that is. */
  const busy = phase !== "idle";
  useEffect(() => onBusyChange(busy), [busy, onBusyChange]);

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
    const text = formatConversation(entries, streamingId, (role) =>
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
      {hasCopyableConversation(entries, streamingId) ? (
        <p className="message__actions chat__copy-all">
          <button type="button" className="button button--compact" onClick={copyAll}>
            {t("actions.copyConversation")}
          </button>
        </p>
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
      />
    </section>
  );
}
