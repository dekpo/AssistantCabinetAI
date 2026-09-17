import type { ThemeChoice } from "./ipc";

const DARK_QUERY = "(prefers-color-scheme: dark)";

export type ResolvedTheme = "light" | "dark";

export function resolveTheme(choice: ThemeChoice, systemPrefersDark: boolean): ResolvedTheme {
  if (choice === "system") {
    return systemPrefersDark ? "dark" : "light";
  }
  return choice;
}

/** Applies the theme and follows the system while `system` is chosen. Returns a cleanup. */
export function applyTheme(choice: ThemeChoice): () => void {
  const media = window.matchMedia(DARK_QUERY);
  const paint = () => {
    document.documentElement.dataset.theme = resolveTheme(choice, media.matches);
  };
  paint();
  if (choice !== "system") {
    return () => undefined;
  }
  media.addEventListener("change", paint);
  return () => media.removeEventListener("change", paint);
}
