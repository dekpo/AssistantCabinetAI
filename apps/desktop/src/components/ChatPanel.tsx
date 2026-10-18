import { useEffect, useRef, useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { copyToClipboard } from "../lib/clipboard";
import { answerForCopy } from "../lib/annotations";
import { combinedHasSelection } from "../lib/analysisScope";
import { formatConversation, hasCopyableConversation } from "../lib/conversationText";
import { chipIds, withChip, withoutChip, type Chip } from "../lib/entityPicker";
import type { CombinedScope, OutsideSelection, SpeechSettings } from "../lib/ipc";
import { useDictation } from "../speech/useDictation";
import { useRepairSuggestions } from "../speech/useRepairSuggestions";
import { useChat } from "../state/useChat";
import type { IndexingState } from "../state/useIndexing";
import { Composer } from "./Composer";
import { ConfirmDialog } from "./ConfirmDialog";
import { EntityCardDialog } from "./EntityCardDialog";
import { ErrorBanner } from "./ErrorBanner";
import { KeyGlyph } from "./KeyGlyph";
import { MessageList } from "./MessageList";
import { SuggestionChips } from "./SuggestionChips";

export function ChatPanel({
  onFailure,
  hasWorkFolder,
  scope,
  onBusyChange,
  modelAlias,
  aliases,
  indexing,
  onModelAliasChange,
  showDiagnostics,
  copySources,
  onAddToSelection,
  revealNames,
  onRevealNamesChange,
  suggestionsRefresh,
  suggestionsEnabled,
  requestedQuestion,
  onRequestedQuestionHandled,
  speech,
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
  /** Shows the developer panel under the last answer. Off in normal use. */
  showDiagnostics: boolean;
  /** Whether the copy buttons also copy the sources of each answer (the `copySources` setting). */
  copySources: boolean;
  /** Adds the files of a name that is only in unselected files to the selection: her own click,
   * made in the name picker. */
  onAddToSelection: (outside: OutsideSelection) => void;
  /** Whether the user asked, for this session, to see suggestions that name somebody. */
  revealNames: boolean;
  onRevealNamesChange: (reveal: boolean) => void;
  /** Changes whenever the knowledge base may have changed, so the suggestions are looked for again. */
  suggestionsRefresh: unknown;
  /** False while an analysis is writing to the base. */
  suggestionsEnabled: boolean;
  /** A question chosen outside the conversation (a suggestion under an analysis summary). It is
   * sent as soon as nothing is being answered, then reported as handled. */
  requestedQuestion: { text: string; nonce: number } | null;
  onRequestedQuestionHandled: () => void;
  /** The voice settings: whether the question box offers a microphone, and for how long. */
  speech: SpeechSettings;
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
    retarget,
    stop,
    dismissError,
    clear,
  } = useChat(onFailure, hasWorkFolder, modelAlias, scope);
  /* The microphone of the question box. It lives here because the draft, sending, stopping and
     clearing do: each of them ends a dictation first. */
  const dictation = useDictation({
    speech,
    draft,
    setDraft,
    chatBusy: phase !== "idle",
    needsFolder: !hasWorkFolder,
    analysing: indexing.running,
  });
  /* After a dictation: the words that may have meant a name of the selection. They only suggest;
     nothing changes until a name is pressed. */
  const repair = useRepairSuggestions({
    source: dictation?.heard ?? null,
    draft,
    setDraft,
    scope,
    revealNames,
    onRevealNamesChange,
    enabled: dictation !== undefined,
  });
  /* The names chosen for the next question. They are ids with the words shown; sending clears them,
     and a stop that gives the question back gives them back too. */
  const [chips, setChips] = useState<Chip[]>([]);
  const sentChips = useRef<Chip[]>([]);
  /* The sheet of a name, opened from a chip or from the list. */
  const [card, setCard] = useState<{ entityId: number; display: string } | null>(null);
  /* A name counted its files in one selection; in another the count means nothing, so a change of
     selection starts the chips over. */
  useEffect(() => {
    setChips([]);
    setCard(null);
  }, [scope]);
  /* A suggestion chosen under an analysis summary asks its question here, once, as soon as nothing
     is being answered. */
  useEffect(() => {
    if (requestedQuestion !== null && phase === "idle" && hasWorkFolder) {
      void send(requestedQuestion.text);
      onRequestedQuestionHandled();
    }
    // `send` is rebuilt on every render; what matters is the request and the phase.
  }, [requestedQuestion, phase, hasWorkFolder]);
  /* The sidebar's picker is locked while a question is being answered, and only this panel knows
     when that is. */
  const busy = phase !== "idle";
  useEffect(() => onBusyChange(busy), [busy, onBusyChange]);
  /* Confirmed before anything is thrown away - the one guarantee this temporary control has to
     keep, since there is nowhere to recover a cleared conversation from yet
     (`docs/ROADMAP.md`, saved conversations). */
  const [confirmingClear, setConfirmingClear] = useState(false);

  const onStop = () => {
    dictation?.cancel();
    const stopped = stop();
    if (stopped !== null) {
      setDraft(stopped);
      setChips(sentChips.current);
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
    // Everything she sees: the text as shown, the engine's own figure and the notes under each
    // answer (`lib/annotations.ts`), and the lists of sources when the `copySources` setting is on.
    const shown = entries.map((entry) =>
      entry.role === "assistant"
        ? { ...entry, content: answerForCopy(t, locale, entry, copySources) }
        : entry,
    );
    const text = formatConversation(shown, streamingId, (role) =>
      t(role === "user" ? "chat.authorUser" : "chat.authorAssistant"),
    );
    void copyToClipboard(text);
  };

  const hint = (
    <p className="composer__hint">
      <span>{t("chat.keyboardHintSend")}</span>
      <KeyGlyph name="enter" />
      <span>,</span>
      <span>{t("chat.keyboardHintBreak")}</span>
      <KeyGlyph name="shift" />
      <KeyGlyph name="enter" />
    </p>
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
        onRetarget={(answerEntryId, choice) => void retarget(answerEntryId, choice)}
        showDiagnostics={showDiagnostics}
        copySources={copySources}
        onAnalyse={(answerEntryId) => void analyseThenAsk(answerEntryId)}
        analysing={indexing.running}
        emptyExtra={
          hasWorkFolder ? (
            <SuggestionChips
              scope={scope}
              revealNames={revealNames}
              onRevealNamesChange={onRevealNamesChange}
              refresh={suggestionsRefresh}
              enabled={suggestionsEnabled}
              disabled={phase !== "idle"}
              onAsk={(question) => void send(question)}
            />
          ) : null
        }
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
            dictation?.cancel();
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
        onSend={(question) => {
          dictation?.cancel();
          sentChips.current = chips;
          void send(question, chips.length > 0 ? { entityIds: chipIds(chips) } : undefined);
          setChips([]);
        }}
        onStop={onStop}
        hint={hint}
        modelAlias={modelAlias}
        aliases={aliases}
        onModelAliasChange={onModelAliasChange}
        needsFolder={!hasWorkFolder}
        chips={chips}
        onAddChip={(chip) => setChips((current) => withChip(current, chip))}
        onRemoveChip={(entityId) => setChips((current) => withoutChip(current, entityId))}
        scope={scope}
        hasSelection={combinedHasSelection(scope)}
        onAddToSelection={onAddToSelection}
        onOpenCard={setCard}
        dictation={dictation}
        repair={repair}
      />
      {card === null ? null : (
        <EntityCardDialog
          entityId={card.entityId}
          display={card.display}
          scope={scope}
          onClose={() => setCard(null)}
        />
      )}
    </section>
  );
}
