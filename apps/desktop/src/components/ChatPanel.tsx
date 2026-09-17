import { useTranslation } from "../i18n/I18nProvider";
import { useChat } from "../state/useChat";
import { Composer } from "./Composer";
import { ErrorBanner } from "./ErrorBanner";
import { MessageList } from "./MessageList";

export function ChatPanel() {
  const { t } = useTranslation();
  const { entries, pending, error, send } = useChat();

  return (
    <section className="chat">
      <MessageList entries={entries} pending={pending} />
      {error === null ? null : <ErrorBanner error={error} />}
      <Composer pending={pending} onSend={(question) => void send(question)} />
      {/* The notice comes from the catalogue: a model asked to repeat it would sometimes not. */}
      <p className="disclaimer">{t("chat.disclaimer")}</p>
    </section>
  );
}
