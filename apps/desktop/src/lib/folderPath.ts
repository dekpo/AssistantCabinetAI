/**
 * A sidebar box has room for a folder name, not a whole path, and the home directory it sits
 * under is the part that tells her the least. The separator comes from the path itself rather
 * than from the platform the UI is running on, since Rust already returns it in the native form
 * (`dunce::simplified`) and the same interface can show a path reported by either OS. Generic
 * over any folder path, so both the Documents Folder and the Data Folder cards share it.
 */
export function abbreviateFolderPath(path: string): string {
  const separator = path.includes("\\") ? "\\" : "/";
  const segments = path.split(/[\\/]+/).filter((segment) => segment.length > 0);
  const last = segments[segments.length - 1];
  return last === undefined ? path : `…${separator}${last}`;
}
