import { useEffect, useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { copyToClipboard } from "../lib/clipboard";
import { formatConversation, hasCopyableConversation } from "../lib/conversationText";
import { wholeFolderScope } from "../lib/analysisScope";
import type { AnalysisScope } from "../lib/ipc";
import { useChat } from "../state/useChat";
import type { IndexingState } from "../state/useIndexing";
import { Composer } from "./Composer";
import { ErrorBanner } from "./ErrorBanner";
import { KeyGlyph } from "./KeyGlyph";
import { MessageList } from "./MessageList";
import { ScopePicker } from "./ScopePicker";

export function ChatPanel({
  onFailure,
  hasWorkFolder,
  workFolder,
  modelAlias,
  aliases,
  indexing,
  onModelAliasChange,
}: {
  onFailure: () => void;
  hasWorkFolder: boolean;
  /** The chosen folder. Only compared, to notice when it changes. */
  workFolder: string | null;
  modelAlias: string;
  aliases: string[];
  /** The same analysis pass the folder card starts, so the answer that asks for one starts that
   * pass rather than a second one of its own. */
  indexing: IndexingState;
  onModelAliasChange: (alias: string) => void;
}) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState("");
  /* The files this conversation is about. Held here, beside the single in-memory conversation it
     belongs to: nothing persists a session yet, so nothing persists this. */
  const [scope, setScope] = useState<AnalysisScope>(() => wholeFolderScope(Date.now()));
  /* Files chosen in one folder mean nothing in another: their paths would resolve to nothing and
     every question would be refused. A different folder starts from the whole folder again. */
  useEffect(() => setScope(wholeFolderScope(Date.now())), [workFolder]);
  const { entries, phase, streamingId, error, send, resend, regenerate, stop, dismissError } =
    useChat(onFailure, hasWorkFolder, modelAlias, scope);

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
      {hasWorkFolder ? (
        <ScopePicker scope={scope} onChange={setScope} disabled={phase !== "idle"} />
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
