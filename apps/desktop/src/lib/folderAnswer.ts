import type { Translator } from "../i18n/translate";
import type { FileRecord, FolderAnswer } from "./ipc";
import { counted } from "./plural";

/**
 * The sentence for an answer the work folder gave by itself.
 *
 * Rust sends facts and machine codes and never prose, so the wording lives here and comes out of
 * the catalogues, in her language (`docs/LANGUAGE-AND-LOCALE.md`). The result is Markdown,
 * because the chat panel already renders Markdown and a list of fifteen file names reads far
 * better as a list than as a sentence.
 *
 * Nothing here invents a file. Every name it prints came from the inventory.
 */
export function formatFolderAnswer(t: Translator, answer: FolderAnswer): string {
  switch (answer.kind) {
    case "file_count":
      return t("folderAnswer.total", { count: files(t, answer.total) });
    case "file_list":
      return withList(
        t,
        t("folderAnswer.listIntro", { count: files(t, answer.files.length) }),
        answer.files,
      );
    case "extension_count":
      return t("folderAnswer.extensionTotal", {
        count: files(t, answer.count),
        extension: extension(answer.extension),
      });
    case "extension_list":
      return withList(
        t,
        t("folderAnswer.extensionIntro", {
          count: files(t, answer.files.length),
          extension: extension(answer.extension),
        }),
        answer.files,
      );
    case "image_list":
      return withList(
        t,
        t("folderAnswer.imageIntro", {
          count: counted(answer.files.length, t("folderAnswer.imageOne"), t("folderAnswer.imageMany")),
        }),
        answer.files,
      );
    case "unreadable_count":
      return t("folderAnswer.unreadableTotal", { count: files(t, answer.count) });
    case "unreadable_list":
      return withList(
        t,
        t("folderAnswer.unreadableIntro", { count: files(t, answer.files.length) }),
        answer.files,
      );
    case "indexed_count":
      return t("folderAnswer.indexedTotal", { count: files(t, answer.count) });
    case "indexed_list":
      return withList(
        t,
        t("folderAnswer.indexedIntro", { count: files(t, answer.files.length) }),
        answer.files,
      );
    case "folder_list":
      return answer.folders.length === 0
        ? t("folderAnswer.noSubfolder")
        : [
            t("folderAnswer.folderIntro", {
              count: counted(
                answer.folders.length,
                t("folderAnswer.folderOne"),
                t("folderAnswer.folderMany"),
              ),
            }),
            "",
            ...answer.folders.map((folder) => `- ${folder}`),
          ].join("\n");
    case "folder_tree":
      // A code block, so the indentation that carries the nesting survives Markdown.
      return [t("folderAnswer.treeIntro", { root: answer.root }), "", "```", ...answer.lines, "```"].join(
        "\n",
      );
    case "file_details":
      return details(t, answer.file);
    case "ambiguous_reference":
      // Never one of them. She chooses.
      return [
        t("folderAnswer.ambiguous", { query: answer.query }),
        "",
        ...answer.candidates.map((file) => `- ${file.relativePath}`),
      ].join("\n");
    case "no_matching_file":
      return t("folderAnswer.noMatch", { query: answer.query });
    case "file_unreadable":
      return t("folderAnswer.fileUnreadable", { name: answer.file.name });
  }
}

function files(t: Translator, count: number): string {
  return counted(count, t("folderAnswer.fileOne"), t("folderAnswer.fileMany"));
}

/** Shown with the dot, the way she writes it herself. */
function extension(value: string): string {
  return value === "" ? value : `.${value}`;
}

/**
 * Each line carries the file's path and what happened to it, in the same words the work folder
 * panel uses. A list of names alone leaves out the most useful fact: whether anything could be
 * read from a file is exactly what decides whether it can be asked about
 * (`docs/WORK-FOLDER-INVENTORY.md`).
 */
function withList(t: Translator, intro: string, records: FileRecord[]): string {
  if (records.length === 0) {
    return intro;
  }
  const lines = records.map(
    (file) => `- ${file.relativePath} — ${t(`folderAnswer.processing.${file.processingStatus}`)}`,
  );
  return [intro, "", ...lines].join("\n");
}

function details(t: Translator, file: FileRecord): string {
  return [
    `**${file.name}**`,
    "",
    `- ${t("folderAnswer.detailPath", { path: file.relativePath })}`,
    `- ${t("folderAnswer.detailExtension", { extension: extension(file.extension) })}`,
    `- ${t("folderAnswer.detailKind", { kind: t(`folderAnswer.kind.${file.kind}`) })}`,
    `- ${t("folderAnswer.detailProcessing", {
      processing: t(`folderAnswer.processing.${file.processingStatus}`),
    })}`,
    `- ${t("folderAnswer.detailExtraction", {
      extraction: t(`folderAnswer.extraction.${file.extractionMethod}`),
    })}`,
  ].join("\n");
}
