import { useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { copyToClipboard } from "../lib/clipboard";
import { formatConversation, hasCopyableConversation } from "../lib/conversationText";
import { useChat } from "../state/useChat";
import { Composer } from "./Composer";
import { ErrorBanner } from "./ErrorBanner";
import { KeyGlyph } from "./KeyGlyph";
import { MessageList } from "./MessageList";

export function ChatPanel({
  onFailure,
  hasWorkFolder,
  modelAlias,
  aliases,
  onModelAliasChange,
}: {
  onFailure: () => void;
  hasWorkFolder: boolean;
  modelAlias: string;
  aliases: string[];
  onModelAliasChange: (alias: string) => void;
}) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState("");
  const { entries, phase, streamingId, error, send, resend, regenerate, stop } = useChat(
    onFailure,
    hasWorkFolder,
    modelAlias,
  );

  const onStop = () => {
    const stopped = stop();
    if (stopped !== null) {
      setDraft(stopped);
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
      />
      {error === null ? null : <ErrorBanner error={error} />}
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
