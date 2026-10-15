//! Structure of a document: its sections, and which section each passage belongs to (long-documents
//! programme, lot 2; `docs/LONG-DOCUMENTS.md` 5.4).
//!
//! Title sources, in this order, the first that yields anything: the PDF outline (read with `lopdf`,
//! our own walk of the tree), the DOCX heading styles, Markdown `#` headings, heading heuristics
//! (numbered headings, chapter words, short capital lines); and, for a long document with no heading
//! at all, slices of N pages. A short document with fewer than two headings has no structure.
//!
//! A section is anchored at a paragraph (the chunker's own count of lines, `chunking::paragraphs`),
//! and a passage belongs to the section in effect where it starts.

use std::collections::HashMap;
use std::path::Path;

use unicode_normalization::UnicodeNormalization;

use crate::chunking::{is_long, paragraphs, Chunk};
use crate::extraction::ExtractedDocument;

/// Bump when the detection changes: documents stored with an older version are read again by the
/// next Analyse (their vectors are reused, only the structure is rebuilt).
pub const STRUCTURE_VERSION: i64 = 1;

/// Last resort for a long document with no heading: a slice every this many pages ...
const SLICE_PAGES: u32 = 20;
/// ... or, for a document read as a single page, every this many characters.
const SLICE_CHARS: usize = 20_000;
const MAX_HEADING_CHARS: usize = 90;
const MAX_CAPITALS_CHARS: usize = 60;
const MAX_OUTLINE_ENTRIES: usize = 20_000;
const MAX_OUTLINE_DEPTH: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionSource {
    Outline,
    Style,
    Markdown,
    Heuristic,
    Slice,
}

impl SectionSource {
    pub fn code(self) -> &'static str {
        match self {
            SectionSource::Outline => "outline",
            SectionSource::Style => "style",
            SectionSource::Markdown => "markdown",
            SectionSource::Heuristic => "heuristic",
            SectionSource::Slice => "slice",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub position: usize,
    pub parent: Option<usize>,
    pub level: u8,
    /// Empty for a slice: the interface says "pages 21 to 40" from the page range.
    pub title: String,
    pub first_page: u32,
    pub last_page: u32,
    pub source: SectionSource,
    pub anchor_page: u32,
    pub anchor_paragraph: usize,
    /// Whether a line of the text is this heading. False for a slice, and for an outline entry whose
    /// title the page does not show: the section still starts at that page, but no line is cut at.
    pub located: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineEntry {
    pub level: u8,
    pub title: String,
    pub page: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledHeading {
    pub level: u8,
    pub title: String,
}

#[derive(Debug, Clone, Default)]
pub struct Structure {
    pub sections: Vec<Section>,
    /// (chunk id, section position), for the passages that sit inside a section.
    pub chunk_sections: Vec<(String, usize)>,
    /// (chunk id, last page), only for the passages that run over a page break.
    pub chunk_ends: Vec<(String, u32)>,
}

// ---------------------------------------------------------------------------------------------
// Reading the files
// ---------------------------------------------------------------------------------------------

/// The outline of a PDF as (level, title, page). Our own walk, not `Document::get_toc`, which keys the
/// entries by title (a repeated title is lost) and decodes titles as UTF-8 (accents are lost).
pub fn pdf_outline(bytes: &[u8]) -> Vec<OutlineEntry> {
    let Ok(document) = lopdf::Document::load_mem(bytes) else {
        return Vec::new();
    };
    let pages: HashMap<lopdf::ObjectId, u32> = document
        .get_pages()
        .into_iter()
        .map(|(number, id)| (id, number))
        .collect();
    let root = document
        .catalog()
        .ok()
        .and_then(|catalog| catalog.get(b"Outlines").ok())
        .and_then(|object| document.dereference(object).ok())
        .and_then(|(_, object)| object.as_dict().ok());
    let mut out = Vec::new();
    if let Some(root) = root {
        if let Ok(first) = root.get(b"First") {
            walk_outline(&document, &pages, first, 1, &mut out);
        }
    }
    out
}

fn walk_outline(
    document: &lopdf::Document,
    pages: &HashMap<lopdf::ObjectId, u32>,
    first: &lopdf::Object,
    level: usize,
    out: &mut Vec<OutlineEntry>,
) {
    let mut next = Some(first.clone());
    let mut guard = 0;
    while let Some(current) = next.take() {
        guard += 1;
        if guard > MAX_OUTLINE_ENTRIES || out.len() > MAX_OUTLINE_ENTRIES {
            return;
        }
        let Ok((_, resolved)) = document.dereference(&current) else {
            return;
        };
        let Ok(item) = resolved.as_dict() else {
            return;
        };
        let title = item
            .get(b"Title")
            .ok()
            .and_then(|object| document.dereference(object).ok())
            .and_then(|(_, object)| lopdf::decode_text_string(object).ok())
            .map(|title| title.trim().to_string())
            .filter(|title| !title.is_empty());
        if let (Some(title), Some(page)) = (title, destination_page(document, pages, item)) {
            out.push(OutlineEntry {
                level: u8::try_from(level).unwrap_or(u8::MAX),
                title,
                page,
            });
        }
        if level < MAX_OUTLINE_DEPTH {
            if let Ok(child) = item.get(b"First") {
                walk_outline(document, pages, child, level + 1, out);
            }
        }
        next = item.get(b"Next").ok().cloned();
    }
}

fn destination_page(
    document: &lopdf::Document,
    pages: &HashMap<lopdf::ObjectId, u32>,
    item: &lopdf::Dictionary,
) -> Option<u32> {
    let destination = item.get(b"Dest").ok().or_else(|| {
        item.get(b"A")
            .ok()
            .and_then(|action| document.dereference(action).ok())
            .and_then(|(_, action)| action.as_dict().ok())
            .and_then(|action| action.get(b"D").ok())
    })?;
    let (_, destination) = document.dereference(destination).ok()?;
    let id = destination.as_array().ok()?.first()?.as_reference().ok()?;
    pages.get(&id).copied()
}

/// The headings of a DOCX, in order, from the paragraph styles (`Heading1`, `Titre2`, ...) or an
/// explicit outline level. The text is the paragraph's own.
pub fn docx_headings(path: &Path) -> Vec<StyledHeading> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return Vec::new();
    };
    let mut xml = String::new();
    {
        let Ok(mut entry) = archive.by_name("word/document.xml") else {
            return Vec::new();
        };
        if std::io::Read::read_to_string(&mut entry, &mut xml).is_err() {
            return Vec::new();
        }
    }
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(false);
    let mut headings = Vec::new();
    let mut text = String::new();
    let mut level: Option<u8> = None;
    let mut in_text = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(tag)) | Ok(Event::Empty(tag)) => {
                let name = tag.local_name();
                match name.as_ref() {
                    b"t" => in_text = true,
                    b"pStyle" => {
                        if let Some(value) = attribute(&tag, b"val") {
                            if let Some(found) = style_level(&value) {
                                level = Some(found);
                            }
                        }
                    }
                    b"outlineLvl" => {
                        if let (None, Some(value)) = (
                            level,
                            attribute(&tag, b"val").and_then(|v| v.parse::<u8>().ok()),
                        ) {
                            if value < 6 {
                                level = Some(value + 1);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(tag)) => match tag.local_name().as_ref() {
                b"t" => in_text = false,
                b"p" => {
                    let title = text.trim().to_string();
                    if let (Some(found), false) = (level, title.is_empty()) {
                        headings.push(StyledHeading {
                            level: found,
                            title,
                        });
                    }
                    text.clear();
                    level = None;
                }
                _ => {}
            },
            Ok(Event::Text(chunk)) if in_text => {
                if let Ok(decoded) = chunk.decode() {
                    text.push_str(&decoded);
                }
            }
            Ok(Event::GeneralRef(reference)) if in_text => {
                if let Ok(name) = reference.decode() {
                    match name.as_ref() {
                        "amp" => text.push('&'),
                        "lt" => text.push('<'),
                        "gt" => text.push('>'),
                        "quot" => text.push('"'),
                        "apos" => text.push('\''),
                        _ => {}
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
    }
    headings
}

fn attribute(tag: &quick_xml::events::BytesStart<'_>, local: &[u8]) -> Option<String> {
    tag.attributes()
        .flatten()
        .find(|attribute| attribute.key.local_name().as_ref() == local)
        .map(|attribute| String::from_utf8_lossy(&attribute.value).to_string())
}

/// `Heading1`, `heading 2`, `Titre3`, `Titel1`, `Rubrik2` -> the level; anything else (`Title`,
/// `Subtitle`, a body style) -> `None`.
fn style_level(style: &str) -> Option<u8> {
    let lower = style.to_lowercase();
    let digits_at = lower.find(|c: char| c.is_ascii_digit())?;
    let (name, digits) = lower.split_at(digits_at);
    let name = name.trim();
    if !matches!(name, "heading" | "titre" | "titel" | "rubrik") {
        return None;
    }
    digits
        .parse::<u8>()
        .ok()
        .filter(|level| (1..=9).contains(level))
}

// ---------------------------------------------------------------------------------------------
// Detecting the sections
// ---------------------------------------------------------------------------------------------

struct Raw {
    level: u8,
    title: String,
    source: SectionSource,
    page: u32,
    paragraph: usize,
    located: bool,
}

/// The sections of a document from whichever title source applies.
pub fn detect(
    extension: &str,
    document: &ExtractedDocument,
    outline: &[OutlineEntry],
    styled: &[StyledHeading],
) -> Vec<Section> {
    let lines: Vec<(u32, Vec<&str>)> = document
        .pages
        .iter()
        .map(|page| (page.page_number, paragraphs(&page.text)))
        .collect();
    let mut raw = if !outline.is_empty() {
        from_outline(&lines, outline)
    } else if !styled.is_empty() {
        from_styles(&lines, styled)
    } else if extension == "md" {
        from_markdown(&lines)
    } else {
        Vec::new()
    };
    if raw.len() < 2 && extension != "md" {
        let guessed = from_heuristics(&lines);
        if guessed.len() >= 2 {
            raw = guessed;
        }
    }
    let long = is_long(document);
    if raw.len() >= 2 {
        return finish(raw, &lines);
    }
    if long {
        return slices(&lines);
    }
    Vec::new()
}

fn key(text: &str) -> String {
    text.nfd()
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
        .flat_map(char::to_lowercase)
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Whether two keys are the same title up to a slip of the extraction (a garbled dash, a lost accent):
/// equal, or at most two edits apart on titles of some length.
fn same_title(wanted: &str, found: &str) -> bool {
    if wanted == found {
        return true;
    }
    let (a, b): (Vec<char>, Vec<char>) = (wanted.chars().collect(), found.chars().collect());
    if a.len() < 8 || b.len() < 8 || a.len().abs_diff(b.len()) > 2 {
        return false;
    }
    // Levenshtein distance, stopping as soon as it is certain to exceed two.
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut current = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            current.push(
                (previous[j] + cost)
                    .min(previous[j + 1] + 1)
                    .min(current[j] + 1),
            );
        }
        if current.iter().min().copied().unwrap_or(0) > 2 {
            return false;
        }
        previous = current;
    }
    previous[b.len()] <= 2
}

fn from_outline(lines: &[(u32, Vec<&str>)], outline: &[OutlineEntry]) -> Vec<Raw> {
    outline
        .iter()
        .filter(|entry| lines.iter().any(|(page, _)| *page == entry.page))
        .map(|entry| {
            let wanted = key(&entry.title);
            let found =
                lines
                    .iter()
                    .find(|(page, _)| *page == entry.page)
                    .and_then(|(_, page_lines)| {
                        page_lines
                            .iter()
                            .position(|line| same_title(&wanted, &key(line)))
                    });
            Raw {
                level: entry.level,
                title: entry.title.clone(),
                source: SectionSource::Outline,
                page: entry.page,
                paragraph: found.unwrap_or(0),
                located: found.is_some(),
            }
        })
        .collect()
}

fn from_styles(lines: &[(u32, Vec<&str>)], styled: &[StyledHeading]) -> Vec<Raw> {
    let mut out = Vec::new();
    let (mut page_index, mut line_index) = (0usize, 0usize);
    for heading in styled {
        let wanted = key(&heading.title);
        let mut found = None;
        let (mut p, mut l) = (page_index, line_index);
        while p < lines.len() {
            while l < lines[p].1.len() {
                if key(lines[p].1[l]) == wanted {
                    found = Some((p, l));
                    break;
                }
                l += 1;
            }
            if found.is_some() {
                break;
            }
            p += 1;
            l = 0;
        }
        if let Some((p, l)) = found {
            out.push(Raw {
                level: heading.level,
                title: heading.title.clone(),
                source: SectionSource::Style,
                page: lines[p].0,
                paragraph: l,
                located: true,
            });
            page_index = p;
            line_index = l + 1;
        }
    }
    out
}

fn from_markdown(lines: &[(u32, Vec<&str>)]) -> Vec<Raw> {
    let mut out = Vec::new();
    let mut fenced = false;
    for (page, page_lines) in lines {
        for (index, line) in page_lines.iter().enumerate() {
            if line.starts_with("```") || line.starts_with("~~~") {
                fenced = !fenced;
                continue;
            }
            if fenced {
                continue;
            }
            let hashes = line.chars().take_while(|c| *c == '#').count();
            if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
                let title = line[hashes..]
                    .trim()
                    .trim_end_matches('#')
                    .trim()
                    .to_string();
                if !title.is_empty() {
                    out.push(Raw {
                        level: hashes as u8,
                        title,
                        source: SectionSource::Markdown,
                        page: *page,
                        paragraph: index,
                        located: true,
                    });
                }
            }
        }
    }
    out
}

const CHAPTER_WORDS: [&str; 9] = [
    "chapitre", "partie", "section", "annexe", "titre", "livre", "chapter", "part", "appendix",
];

/// `Some(level)` when the line looks like a heading.
fn heading_level(line: &str, next: Option<&str>) -> Option<u8> {
    let chars = line.chars().count();
    if !(3..=MAX_HEADING_CHARS).contains(&chars) {
        return None;
    }
    let last = line.chars().last()?;
    if matches!(last, '.' | ',' | ';') {
        return None;
    }
    // "3.2 Title" and "3.2.1 Title": at least two groups of digits, then a capital letter.
    let mut groups = 0;
    let mut rest = line;
    loop {
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        if digits == 0 || digits > 3 {
            break;
        }
        groups += 1;
        rest = &rest[digits..];
        if rest.starts_with('.') && rest[1..].chars().next().is_some_and(|c| c.is_ascii_digit()) {
            rest = &rest[1..];
        } else {
            break;
        }
    }
    if groups >= 2 {
        let after = rest.trim_start_matches('.');
        if after.starts_with(char::is_whitespace)
            && after
                .trim_start()
                .chars()
                .next()
                .is_some_and(char::is_uppercase)
        {
            return Some(u8::try_from(groups.min(4)).unwrap_or(4));
        }
        return None;
    }
    // "Chapitre 3 - Title", "Annexe B", "Part IV"
    let mut words = line.split_whitespace();
    if let (Some(first), Some(second)) = (words.next(), words.next()) {
        let first = key(first);
        let numbered = second.chars().next().is_some_and(|c| c.is_ascii_digit())
            || (second.chars().count() <= 4 && second.chars().all(|c| c.is_ascii_uppercase()));
        if CHAPTER_WORDS.contains(&first.as_str()) && numbered {
            return Some(1);
        }
    }
    // A short line in capitals followed by a real paragraph.
    let letters: Vec<char> = line.chars().filter(|c| c.is_alphabetic()).collect();
    if chars <= MAX_CAPITALS_CHARS
        && letters.len() >= 4
        && letters.iter().all(|c| c.is_uppercase())
        && !line.chars().any(|c| c.is_ascii_digit())
        && next.is_some_and(|n| n.chars().count() >= 40)
    {
        return Some(1);
    }
    None
}

fn from_heuristics(lines: &[(u32, Vec<&str>)]) -> Vec<Raw> {
    let mut out = Vec::new();
    for (page, page_lines) in lines {
        for (index, line) in page_lines.iter().enumerate() {
            if let Some(level) = heading_level(line, page_lines.get(index + 1).copied()) {
                out.push(Raw {
                    level,
                    title: (*line).to_string(),
                    source: SectionSource::Heuristic,
                    page: *page,
                    paragraph: index,
                    located: true,
                });
            }
        }
    }
    out
}

/// Parents, page ranges and positions.
fn finish(raw: Vec<Raw>, lines: &[(u32, Vec<&str>)]) -> Vec<Section> {
    let last_page = lines.last().map_or(1, |(page, _)| *page);
    let mut sections: Vec<Section> = Vec::with_capacity(raw.len());
    let mut stack: Vec<usize> = Vec::new();
    for (position, item) in raw.iter().enumerate() {
        while stack
            .last()
            .is_some_and(|top| raw[*top].level >= item.level)
        {
            stack.pop();
        }
        sections.push(Section {
            position,
            parent: stack.last().copied(),
            level: item.level,
            title: item.title.clone(),
            first_page: item.page,
            last_page,
            source: item.source,
            anchor_page: item.page,
            anchor_paragraph: item.paragraph,
            located: item.located,
        });
        stack.push(position);
    }
    for position in 0..sections.len() {
        let level = sections[position].level;
        let first = sections[position].first_page;
        if let Some(next) = sections[position + 1..].iter().find(|s| s.level <= level) {
            let end = if next.anchor_paragraph == 0 && next.anchor_page > first {
                next.anchor_page - 1
            } else {
                next.anchor_page
            };
            sections[position].last_page = end.max(first);
        }
    }
    sections
}

fn slices(lines: &[(u32, Vec<&str>)]) -> Vec<Section> {
    let mut sections = Vec::new();
    if lines.len() > 1 {
        let last_page = lines.last().map_or(1, |(page, _)| *page);
        let first_page = lines.first().map_or(1, |(page, _)| *page);
        let mut start = first_page;
        while start <= last_page {
            let end = (start + SLICE_PAGES - 1).min(last_page);
            sections.push(slice(sections.len(), start, end, start, 0));
            start = end + 1;
        }
    } else if let Some((page, page_lines)) = lines.first() {
        let mut since = 0usize;
        let mut first = true;
        for (index, line) in page_lines.iter().enumerate() {
            if first || since >= SLICE_CHARS {
                sections.push(slice(sections.len(), *page, *page, *page, index));
                since = 0;
                first = false;
            }
            since += line.chars().count();
        }
    }
    sections
}

fn slice(
    position: usize,
    first: u32,
    last: u32,
    anchor_page: u32,
    anchor_paragraph: usize,
) -> Section {
    Section {
        position,
        parent: None,
        level: 1,
        title: String::new(),
        first_page: first,
        last_page: last,
        source: SectionSource::Slice,
        anchor_page,
        anchor_paragraph,
        located: false,
    }
}

// ---------------------------------------------------------------------------------------------
// Passages
// ---------------------------------------------------------------------------------------------

/// The section of each passage: the one in effect where the passage starts. A passage before the first
/// heading belongs to none and is left out.
pub fn assign(
    sections: &[Section],
    document: &ExtractedDocument,
    chunks: &[Chunk],
) -> Vec<(String, usize)> {
    let _ = document;
    let mut out = Vec::new();
    let mut page = u32::MAX;
    let mut start = 0usize;
    for chunk in chunks {
        if chunk.page_number != page {
            page = chunk.page_number;
            start = 0;
        }
        let owner = sections
            .iter()
            .rev()
            .find(|s| (s.anchor_page, s.anchor_paragraph) <= (chunk.page_number, start));
        if let Some(section) = owner {
            out.push((chunk.chunk_id.clone(), section.position));
        }
        start += chunk.text.split('\n').count();
    }
    out
}

/// What the file itself says (the PDF outline, the DOCX styles), then the detection.
pub fn sections_for(path: &Path, extension: &str, document: &ExtractedDocument) -> Vec<Section> {
    let outline = if extension == "pdf" {
        std::fs::read(path)
            .map(|bytes| pdf_outline(&bytes))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let styled = if extension == "docx" {
        docx_headings(path)
    } else {
        Vec::new()
    };
    detect(extension, document, &outline, &styled)
}

/// Read what the file itself says (the PDF outline, the DOCX styles), detect, assign.
pub fn build(
    path: &Path,
    extension: &str,
    document: &ExtractedDocument,
    chunks: &[Chunk],
) -> Structure {
    let outline = if extension == "pdf" {
        std::fs::read(path)
            .map(|bytes| pdf_outline(&bytes))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let styled = if extension == "docx" {
        docx_headings(path)
    } else {
        Vec::new()
    };
    let sections = detect(extension, document, &outline, &styled);
    let chunk_sections = assign(&sections, document, chunks);
    Structure {
        sections,
        chunk_sections,
        chunk_ends: Vec::new(),
    }
}
