import { useCallback, useRef, useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { normaliseError, type AppError } from "../lib/errors";
import {
  canStop,
  phaseAfterFirstText,
  wasStopped,
  type GenerationPhase,
} from "../lib/generation";
import {
  askWithSources,
  cancelChat,
  hasIndexedDocuments,
  sendChatMessage,
  type ChatTurn,
  type Evidence,
} from "../lib/ipc";

export interface ChatEntry extends ChatTurn {
  id: string;
  /** When the turn was written, by the workstation clock. Shown above a question so a long
   * conversation can be read back in order; it never leaves the webview. */
  createdAt: number;
  /** Present on an assistant answer that came from the local index, so it can cite file and page. */
  sources?: Evidence[];
  /** How long the answer took to write, and which profile wrote it - shown so a slow machine is
   * visible rather than silently endured (`docs/HARDWARE.md`). */
  durationMs?: number;
  modelAlias?: string;
}

export interface ChatState {
  entries: ChatEntry[];
  /** "thinking" until the first words of the answer arrive, "writing" afterwards. Both the
   * spinner and the stop affordance read this, so they cannot disagree about what is happening. */
  phase: GenerationPhase;
  error: AppError | null;
  send: (question: string) => Promise<void>;
  /** Stops the question being worked on and hands its text back, so the composer can offer it
   * again: a stop is a question to rephrase far more often than one to forget. Returns null when
   * there is nothing to stop. */
  stop: () => string | null;
}

/**
 * `onFailure` runs when a message could not be answered. The window uses it to re-ask the gateway
 * how it is, so a server that went down is reported by the indicator and not only by this banner.
 *
 * `hasWorkFolder` chooses the path: with a work folder chosen, every question goes through
 * retrieval first and is answered only from what the local index actually returns
 * (`docs/RETRIEVAL.md`). Without one, chat falls back to the plain conversation.
 */
export function useChat(
  onFailure?: () => void,
  hasWorkFolder = false,
  modelAlias?: string,
): ChatState {
  const { t } = useTranslation();
  const [entries, setEntries] = useState<ChatEntry[]>([]);
  const [phase, setPhase] = useState<GenerationPhase>("idle");
  const [error, setError] = useState<AppError | null>(null);
  /** The question in flight, so stopping it can offer it back without reading it out of the list
   * that is about to lose it. */
  const asked = useRef("");
  /** Whether she has asked for the question in flight to stop. Rust decides for the request it is
   * running; this covers the one step that happens before that request exists. */
  const stopping = useRef(false);

  const send = useCallback(
    async (question: string) => {
      const text = question.trim();
      if (text.length === 0 || phase !== "idle") {
        return;
      }
      const questionId = crypto.randomUUID();
      const answerId = crypto.randomUUID();
      const createdAt = Date.now();
      asked.current = text;
      stopping.current = false;
      setError(null);
      setPhase("thinking");
      setEntries((current) => [
        ...current,
        { id: questionId, role: "user", content: text, createdAt },
        { id: answerId, role: "assistant", content: "", createdAt },
      ]);

      const onDelta = (delta: string) => {
        setPhase(phaseAfterFirstText);
        setEntries((current) =>
          current.map((entry) => {
            if (entry.id !== answerId) {
              return entry;
            }
            // Ollama's chat template often opens an answer with a leading space token; strip
            // one only from the very first piece of a fresh answer, never mid-stream, so a
            // legitimate space inside the text is never touched.
            const piece = entry.content.length === 0 ? delta.replace(/^ /, "") : delta;
            return { ...entry, content: entry.content + piece };
          }),
        );
      };
      /* A stop keeps nothing of the turn, question included, so the conversation returns to
         exactly what it was. There is never a half-written answer to decide about: the interface
         offers no stop once the first words are on screen. */
      const discardTurn = () =>
        setEntries((current) =>
          current.filter((entry) => entry.id !== questionId && entry.id !== answerId),
        );
      const startedAt = performance.now();

      try {
        if (hasWorkFolder) {
          // A work folder can be chosen but never analysed yet: retrieval would refuse anyway
          // (`insufficient_evidence`), so say why instantly, from code, rather than spend a
          // round trip on a question the index cannot possibly answer. A failed check itself
          // fails open, so a transient index error surfaces through the real request instead.
          const indexed = await hasIndexedDocuments().catch(() => true);
          // The only moment a stop has nothing to cancel on the other side: this check is its own
          // command, and the request that carries the question has not been made yet.
          if (stopping.current) {
            discardTurn();
            return;
          }
          if (!indexed) {
            setEntries((current) =>
              current.map((entry) =>
                entry.id === answerId ? { ...entry, content: t("chat.notIndexedYet") } : entry,
              ),
            );
            return;
          }
          const { sources } = await askWithSources(text, onDelta);
          const durationMs = performance.now() - startedAt;
          setEntries((current) =>
            current.map((entry) =>
              entry.id === answerId ? { ...entry, sources, durationMs, modelAlias } : entry,
            ),
          );
        } else {
          const turns: ChatTurn[] = [
            ...entries.map(({ role, content }) => ({ role, content })),
            { role: "user", content: text },
          ];
          await sendChatMessage(turns, onDelta);
          const durationMs = performance.now() - startedAt;
          setEntries((current) =>
            current.map((entry) =>
              entry.id === answerId ? { ...entry, durationMs, modelAlias } : entry,
            ),
          );
        }
      } catch (raw: unknown) {
        const failure = normaliseError(raw);
        if (wasStopped(failure.code)) {
          // Her own stop. No banner and no health re-check: nothing went wrong.
          discardTurn();
        } else {
          setError(failure);
          // Drop the half-written answer: an incomplete summary is worse than none.
          setEntries((current) => current.filter((entry) => entry.id !== answerId));
          onFailure?.();
        }
      } finally {
        setPhase("idle");
      }
    },
    [entries, phase, onFailure, hasWorkFolder, modelAlias, t],
  );

  const stop = useCallback(() => {
    if (!canStop(phase)) {
      return null;
    }
    stopping.current = true;
    // Rust ends the run with `chat_cancelled`, and the `catch` above clears the turn. The
    // interface does not decide it is over on its own: that is how the request really stops,
    // rather than being hidden while the practice machine keeps generating.
    void cancelChat();
    return asked.current;
  }, [phase]);

  return { entries, phase, error, send, stop };
}
