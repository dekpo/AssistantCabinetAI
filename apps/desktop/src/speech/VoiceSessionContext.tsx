import { createContext, useContext, useState, useSyncExternalStore, type ReactNode } from "react";
import { createVoiceSession, type VoiceOwner, type VoiceSession } from "./voiceSession";

const VoiceSessionContext = createContext<VoiceSession | null>(null);

/** Mounted once, in `App`. Renders nothing of its own. */
export function VoiceSessionProvider({ children }: { children: ReactNode }) {
  const [session] = useState(createVoiceSession);
  return <VoiceSessionContext.Provider value={session}>{children}</VoiceSessionContext.Provider>;
}

/** The one session of the window, and who holds it right now (re-rendered when that changes). */
export function useVoiceSession(): { session: VoiceSession; holder: VoiceOwner | null } {
  const session = useContext(VoiceSessionContext);
  if (session === null) {
    throw new Error("useVoiceSession must be used inside VoiceSessionProvider");
  }
  const holder = useSyncExternalStore(session.subscribe, session.holder);
  return { session, holder };
}
