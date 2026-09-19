import { useCallback, useState } from "react";
import { normaliseError, type AppError } from "../lib/errors";
import {
  askWithSources,
  sendChatMessage,
  type ChatTurn,
  type Evidence,
} from "../lib/ipc";

export interface ChatEntry extends ChatTurn {
  id: string;
  /** Present on an assistant answer that came from the local index, so it can cite file and page. */
  sources?: Evidence[];
  /** How long the answer took to write, and which profile wrote it - shown so a slow machine is
   * visible rather than silently endured (`docs/HARDWARE.md`). */
  durationMs?: number;
  modelAlias?: string;
}

export interface ChatState {
  entries: ChatEntry[];
  pending: boolean;
  error: AppError | null;
  send: (question: string) => Promise<void>;
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
  const [entries, setEntries] = useState<ChatEntry[]>([]);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<AppError | null>(null);

  const send = useCallback(
    async (question: string) => {
      const text = question.trim();
      if (text.length === 0 || pending) {
        return;
      }
      const answerId = crypto.randomUUID();
      setError(null);
      setPending(true);
      setEntries((current) => [
        ...current,
        { id: crypto.randomUUID(), role: "user", content: text },
        { id: answerId, role: "assistant", content: "" },
      ]);

      const onDelta = (delta: string) => {
        setEntries((current) =>
          current.map((entry) =>
            entry.id === answerId ? { ...entry, content: entry.content + delta } : entry,
          ),
        );
      };
      const startedAt = performance.now();

      try {
        if (hasWorkFolder) {
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
        setError(normaliseError(raw));
        // Drop the half-written answer: an incomplete summary is worse than none.
        setEntries((current) => current.filter((entry) => entry.id !== answerId));
        onFailure?.();
      } finally {
        setPending(false);
      }
    },
    [entries, pending, onFailure, hasWorkFolder],
  );

  return { entries, pending, error, send };
}
