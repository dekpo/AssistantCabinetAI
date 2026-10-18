import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * The window's close request, the way a note that is not saved is protected from the cross of the
 * window. `hold()` is asked each time: when it answers `true` the close is stopped and the caller
 * shows its question; when it answers `false` the window closes as it always did. Outside the
 * desktop shell there is no window to ask, and nothing is registered. Closing the window after
 * the user's answer goes through `closeWindow` (`core:window:allow-destroy`).
 *
 * Quitting from the macOS menu or with Cmd+Q does not raise this request.
 */
export function guardWindowClose(hold: () => boolean): () => void {
  let unlisten: (() => void) | null = null;
  let stopped = false;
  try {
    void getCurrentWindow()
      .onCloseRequested((event) => {
        if (hold()) {
          event.preventDefault();
        }
      })
      .then((stop) => {
        if (stopped) {
          stop();
        } else {
          unlisten = stop;
        }
      })
      .catch(() => undefined);
  } catch {
    // No window outside the desktop shell.
  }
  return () => {
    stopped = true;
    unlisten?.();
  };
}

/** Closes the window for good, without raising the close request again. */
export async function closeWindow(): Promise<void> {
  try {
    await getCurrentWindow().destroy();
  } catch {
    // Nothing to close outside the desktop shell.
  }
}
