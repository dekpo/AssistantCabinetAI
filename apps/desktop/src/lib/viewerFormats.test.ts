import { describe, expect, it } from "vitest";
import { previewKind, previewModes, viewerRendererOf } from "./viewerFormats";

describe("previewModes", () => {
  it("offers raw then preview for Markdown, whatever the case or a leading dot", () => {
    expect(previewModes("md")).toStrictEqual(["raw", "preview"]);
    expect(previewModes(".MD")).toStrictEqual(["raw", "preview"]);
  });

  it.each(["eml", "srt", "vtt", ".EML", " Srt "])("offers raw then preview for %j", (extension) => {
    expect(previewModes(extension)).toStrictEqual(["raw", "preview"]);
  });

  it.each([
    "txt", "pdf", "docx", "xlsx", "pptx", "odt", "ods", "odp", "rtf", "csv",
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "doc", "xls", "ppt", "tiff", "", "constructor",
  ])("%j has the raw mode only, so no control is shown", (extension) => {
    expect(previewModes(extension)).toStrictEqual(["raw"]);
  });

  it("only offers a preview to a file the text view shows", () => {
    for (const extension of ["md", "txt", "pdf", "csv", "eml"]) {
      if (previewModes(extension).includes("preview")) {
        expect(viewerRendererOf(extension).renderer).toBe("text");
      }
    }
  });
});

describe("previewKind", () => {
  it("draws Markdown as formatting and reads mail and subtitles as plain readable text", () => {
    expect(previewKind("md")).toBe("markdown");
    expect(previewKind(".MD")).toBe("markdown");
    for (const extension of ["eml", "srt", "vtt"]) {
      expect(previewKind(extension)).toBe("readable");
    }
  });

  it.each(["txt", "pdf", "docx", "csv", "png", "doc", "", "constructor"])(
    "%j has no second mode",
    (extension) => {
      expect(previewKind(extension)).toBeNull();
    },
  );

  it("agrees with previewModes for every known extension", () => {
    for (const extension of ["md", "eml", "srt", "vtt", "txt", "pdf", "xlsx", "csv", "svg", "ppt"]) {
      expect(previewModes(extension).length > 1).toBe(previewKind(extension) !== null);
    }
  });
});

describe("viewerRendererOf", () => {
  it.each([
    ["pdf", "pdf"],
    ["docx", "docx"],
    ["xlsx", "xlsx"],
    ["pptx", "pptx"],
    ["odt", "odt"],
    ["ods", "ods"],
    ["odp", "odp"],
    ["rtf", "rtf"],
    ["csv", "csv"],
    ["png", "image"],
    ["jpg", "image"],
    ["jpeg", "image"],
    ["gif", "image"],
    ["bmp", "image"],
    ["webp", "image"],
    ["svg", "image"],
  ])("%s is drawn by the library as %s", (extension, format) => {
    expect(viewerRendererOf(extension)).toStrictEqual({ renderer: "library", format });
  });

  it.each(["txt", "md", "srt", "vtt", "eml"])("%s is shown by the application's text view", (extension) => {
    expect(viewerRendererOf(extension)).toStrictEqual({ renderer: "text" });
  });

  it.each(["doc", "xls", "ppt", "tiff", "tif", "heic", "zip", "exe", "", "pdfx", "constructor"])(
    "%j is unsupported",
    (extension) => {
      expect(viewerRendererOf(extension)).toStrictEqual({ renderer: "unsupported" });
    },
  );

  it("ignores case, spaces and a leading dot", () => {
    expect(viewerRendererOf("PDF")).toStrictEqual({ renderer: "library", format: "pdf" });
    expect(viewerRendererOf(".Docx")).toStrictEqual({ renderer: "library", format: "docx" });
    expect(viewerRendererOf(" SRT ")).toStrictEqual({ renderer: "text" });
  });

  it("never sends a text format to the library, which would join its line breaks", () => {
    for (const extension of ["txt", "md", "srt", "vtt", "eml"]) {
      expect(viewerRendererOf(extension).renderer).toBe("text");
    }
  });
});
