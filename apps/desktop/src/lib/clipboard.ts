import { writeText } from "@tauri-apps/plugin-clipboard-manager";

/**
 * `navigator.clipboard.writeText` needs a secure context. Windows treats `http://tauri.localhost`
 * as trustworthy; macOS's custom scheme is not guaranteed to be (AGENTS.md: both platforms are
 * mandatory), so copy goes through this plugin on both rather than working on one and silently
 * failing on the other.
 */
export function copyToClipboard(text: string): Promise<void> {
  return writeText(text);
}
