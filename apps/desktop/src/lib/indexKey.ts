import type { FinderDomain } from "./ipc";

/*
 * Which file the index means, as the client sees it. The twin of `file_key.rs` (`docs/DECISIONS.md`, "root
 * and family"), pure and tested, with no call and no React.
 *
 * Two words. The **root** is the physical folder a file sits in, `documents` or `data`: the explorer lists
 * files by root and path. The **family** is how the file is read, by its extension: the document pipeline
 * (`documents`) or the tabular one (`data`, the family the interface calls Tableaux). The index and the
 * scopes speak **keys**: a file in the root of its own family keeps its path; a file of the other family
 * found in a root is stored under `/<root>/<path>`, so it never collides with a file of the same path in
 * the other folder.
 */

/** The physical folder a file sits in. */
export type IndexRoot = FinderDomain;

/** The family that reads a file; the same two words as the finder's domains. */
export type IndexFamily = FinderDomain;

/** The root that holds a family's own files. */
export function homeRoot(family: IndexFamily): IndexRoot {
  return family;
}

/** The key of a file of `family` that sits in `root` at `path` (a path inside that root, forward slashes). */
export function indexKey(root: IndexRoot, family: IndexFamily, path: string): string {
  return root === homeRoot(family) ? path : `/${root}/${path}`;
}

/** Whether the key is a file of the other folder's. */
export function isForeignKey(key: string): boolean {
  return key.startsWith("/");
}

/** The path a key of `family` stands for inside its root, as a person reads it: the key of a file of the
 * other folder (`/data/Test/a.docx`) loses its prefix. A key that parses to nothing is shown as it is. */
export function displayPathOfKey(family: IndexFamily, key: string): string {
  return parseIndexKey(family, key)?.path ?? key;
}

/** The root and the path inside it that a key of `family` stands for; null for a key that starts with a
 * slash and names no root or no path, which is never produced and is ignored. */
export function parseIndexKey(
  family: IndexFamily,
  key: string,
): { root: IndexRoot; path: string } | null {
  if (!key.startsWith("/")) {
    return { root: homeRoot(family), path: key };
  }
  const rest = key.slice(1);
  const cut = rest.indexOf("/");
  if (cut < 0) {
    return null;
  }
  const name = rest.slice(0, cut);
  const path = rest.slice(cut + 1);
  if ((name !== "documents" && name !== "data") || path === "" || name === homeRoot(family)) {
    return null;
  }
  return { root: name, path };
}
