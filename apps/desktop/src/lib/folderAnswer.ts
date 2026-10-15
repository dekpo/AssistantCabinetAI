import type { Translator } from "../i18n/translate";
import type { DocumentSearchAnswer, FileRecord, FolderAnswer } from "./ipc";
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
    case "file_not_selected":
      return t("folderAnswer.notSelected", { query: answer.query });
    case "nothing_selected":
      return t("folderAnswer.nothingSelected");
    case "document_search":
      return documentSearch(t, answer);
    case "document_search_both":
      return [
        t("folderAnswer.search.sectionDocuments"),
        "",
        documentSearch(t, answer.documents),
        "",
        t("folderAnswer.search.sectionWorkbooks"),
        "",
        documentSearch(t, answer.workbooks),
      ].join("\n");
    case "ranked_file":
      return rankedFile(t, answer);
  }
}

type RankedFile = Extract<FolderAnswer, { kind: "ranked_file" }>;

/**
 * The latest or earliest file, with the ranking it comes from and where each date comes from: the
 * date written in the document, else at the start of the name, else the modification date, which is the
 * date of a copy.
 */
function rankedFile(t: Translator, answer: RankedFile): string {
  if (answer.ranking.length === 0) {
    if (answer.noun === null) {
      return t("folderAnswer.rank.none");
    }
    return t(answer.byType ? "folderAnswer.rank.noneType" : "folderAnswer.rank.noneNoun", {
      noun: answer.noun,
    });
  }
  const order = answer.latest ? "Latest" : "Earliest";
  const count = counted(
    answer.candidates,
    t("folderAnswer.rank.readableOne"),
    t("folderAnswer.rank.readableMany"),
  );
  const lines = [
    answer.noun === null
      ? t(`folderAnswer.rank.intro${order}`, { count })
      : t(`folderAnswer.rank.intro${order}${answer.byType ? "Type" : "Noun"}`, {
          count,
          noun: answer.noun,
        }),
    "",
    ...answer.ranking.map(
      (entry) =>
        `- ${entry.relativePath} — ${entry.date} (${t(`folderAnswer.rank.basis.${entry.basis}`)})`,
    ),
  ];
  if (answer.mixedBasis) {
    lines.push("", t("folderAnswer.rank.mixed"));
  }
  if (answer.unranked > 0) {
    lines.push("", t("folderAnswer.rank.unranked", { count: files(t, answer.unranked) }));
  }
  return lines.join("\n");
}

type DocumentSearch = DocumentSearchAnswer;

/**
 * A set answer: what was searched, how many files hold it, the first ones with their passage, the
 * rest as paths, and what could not be searched. It always says it is a search of the word itself.
 */
function documentSearch(t: Translator, answer: DocumentSearch): string {
  const count = files(t, answer.total);
  const base = answer.identifier
    ? "folderAnswer.search.introIdentifier"
    : answer.quoted
      ? "folderAnswer.search.introPhrase"
      : "folderAnswer.search.intro";
  const intro = answer.domain === "data" ? `${base}Data` : base;
  const lines = [t(intro, { query: answer.query, count })];
  if (answer.entity !== null) {
    lines.push("", t("folderAnswer.search.entity", { query: answer.query, name: answer.entity }));
  }
  if (answer.businessType !== null) {
    lines.push(
      "",
      t("folderAnswer.search.kind", { kind: t(`businessType.${answer.businessType}`) }),
    );
  }
  if (!answer.identifier) {
    lines.push("", t("folderAnswer.search.wordOnly"));
  }
  if (answer.listed.length > 0) {
    lines.push("");
    for (const file of answer.listed) {
      const pages = file.pages ?? [];
      const shown = pages.length > 1;
      const more = (file.matchesTotal ?? 0) > pages.length;
      const place = shown
        ? ` (${t("folderAnswer.search.pages", { pages: pages.join(", ") })}${
            more ? ` ; ${t("folderAnswer.search.matches", { count: file.matchesTotal ?? 0 })}` : ""
          })`
        : file.page === null
          ? ""
          : ` (${t("folderAnswer.search.page", { page: file.page })})`;
      lines.push(`- ${file.relativePath}${place}`);
      if (file.excerpt !== null && file.excerpt !== "") {
        lines.push(`  ${t("folderAnswer.search.excerpt", { text: plain(file.excerpt) })}`);
      }
    }
  }
  if (answer.others.length > 0) {
    lines.push(
      "",
      t("folderAnswer.search.others", { count: files(t, answer.others.length) }),
      "",
      ...answer.others.map((path) => `- ${path}`),
    );
  }
  const { unanalysed, unreadable } = answer.coverage;
  if (unanalysed > 0) {
    lines.push("", t("folderAnswer.search.unanalysed", { count: files(t, unanalysed) }));
  }
  if (unreadable > 0) {
    lines.push("", t("folderAnswer.search.unreadable", { count: files(t, unreadable) }));
  }
  return lines.join("\n");
}

/** A passage of a document is shown as text, never read as Markdown. */
function plain(text: string): string {
  return text.replace(/[\\`*_{}[\]<>#|~]/g, (mark) => `\\${mark}`);
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
