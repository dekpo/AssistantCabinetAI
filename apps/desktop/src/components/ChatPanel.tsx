import { useTranslation } from "../i18n/I18nProvider";
import { useChat } from "../state/useChat";
import { Composer } from "./Composer";
import { ErrorBanner } from "./ErrorBanner";
import { KeyGlyph } from "./KeyGlyph";
import { MessageList } from "./MessageList";

export function ChatPanel({
  onFailure,
  hasWorkFolder,
  modelAlias,
}: {
  onFailure: () => void;
  hasWorkFolder: boolean;
  modelAlias: string;
}) {
  const { t } = useTranslation();
  const { entries, pending, error, send } = useChat(onFailure, hasWorkFolder, modelAlias);

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
      <MessageList entries={entries} pending={pending} />
      {error === null ? null : <ErrorBanner error={error} />}
      <Composer pending={pending} onSend={(question) => void send(question)} hint={hint} />
    </section>
  );
}
