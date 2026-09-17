import { useCallback, useState } from "react";
import { normaliseError, type AppError } from "../lib/errors";
import { sendChatMessage, type ChatTurn } from "../lib/ipc";

export interface ChatEntry extends ChatTurn {
  id: string;
}

export interface ChatState {
  entries: ChatEntry[];
  pending: boolean;
  error: AppError | null;
  send: (question: string) => Promise<void>;
}

export function useChat(): ChatState {
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
      const turns: ChatTurn[] = [
        ...entries.map(({ role, content }) => ({ role, content })),
        { role: "user", content: text },
      ];
      setError(null);
      setPending(true);
      setEntries((current) => [
        ...current,
        { id: crypto.randomUUID(), role: "user", content: text },
        { id: answerId, role: "assistant", content: "" },
      ]);

      try {
        await sendChatMessage(turns, (delta) => {
          setEntries((current) =>
            current.map((entry) =>
              entry.id === answerId ? { ...entry, content: entry.content + delta } : entry,
            ),
          );
        });
      } catch (raw: unknown) {
        setError(normaliseError(raw));
        // Drop the half-written answer: an incomplete summary is worse than none.
        setEntries((current) => current.filter((entry) => entry.id !== answerId));
      } finally {
        setPending(false);
      }
    },
    [entries, pending],
  );

  return { entries, pending, error, send };
}
