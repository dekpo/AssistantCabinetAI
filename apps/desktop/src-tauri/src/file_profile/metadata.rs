//! Document properties read as evidence: the PDF Info dictionary and XMP packet, the DOCX and XLSX
//! `docProps` parts, the front matter of a Markdown or text file, the capture date of a JPEG.
//!
//! `docs/LEXICAL-RETRIEVAL-DESIGN.md` 4.1. Every value comes out as a [`MetadataField`] with its
//! **source** and a **confidence**, or it does not come out. Properties are often the software's or the
//! scanner's name rather than the author's, so:
//!
//! - a value that is generic (`Microsoft Word`, a scanner model, `admin`, the user's own name; the list
//!   is data, `vocabulary.rs`) is dropped and counted in `ignored`, never filled;
//! - a title that is only a file name is dropped;
//! - a date that cannot be right (before 1990, in the future) is dropped;
//! - the producer and creator-tool strings are not read at all: they describe a program, never a document.
//!
//! Confidences are below a name found in a title or a header block (0.85 and more) on purpose: a property
//! is a hint the user can search by, not proof of who wrote the document. Nothing here reads more than the
//! head of a text file, the head of a JPEG or the property parts of an archive; a PDF is parsed once.

use std::io::Read;
use std::path::Path;

use chrono::{Datelike, NaiveDate, Utc};

use super::vocabulary::{words_key, ProfileVocabulary};

/// What a property is about. The strings are machine codes (`as_str`), stored as they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MetadataKind {
    Title,
    Author,
    LastModifiedBy,
    Subject,
    Keywords,
    Company,
    Created,
    Modified,
}

impl MetadataKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Author => "author",
            Self::LastModifiedBy => "last_modified_by",
            Self::Subject => "subject",
            Self::Keywords => "keywords",
            Self::Company => "company",
            Self::Created => "created",
            Self::Modified => "modified",
        }
    }

    /// The kinds whose value is a calendar date, stored as `YYYY-MM-DD`.
    pub fn is_date(self) -> bool {
        matches!(self, Self::Created | Self::Modified)
    }
}

/// Where a value was read. Machine codes, stored as they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetadataSource {
    PdfInfo,
    PdfXmp,
    DocxCore,
    DocxApp,
    XlsxCore,
    XlsxApp,
    FrontMatter,
    Exif,
}

impl MetadataSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PdfInfo => "pdf_info",
            Self::PdfXmp => "pdf_xmp",
            Self::DocxCore => "docx_core",
            Self::DocxApp => "docx_app",
            Self::XlsxCore => "xlsx_core",
            Self::XlsxApp => "xlsx_app",
            Self::FrontMatter => "front_matter",
            Self::Exif => "exif",
        }
    }
}

/// One property, filled.
#[derive(Debug, Clone, PartialEq)]
pub struct MetadataField {
    pub kind: MetadataKind,
    /// The value as the file wrote it, trimmed; a date is `YYYY-MM-DD`.
    pub value: String,
    pub source: MetadataSource,
    pub confidence: f32,
}

/// Why a value that was present produced no field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IgnoredReason {
    Generic,
    FileNameTitle,
    ImplausibleDate,
}

impl IgnoredReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::FileNameTitle => "file_name_title",
            Self::ImplausibleDate => "implausible_date",
        }
    }
}

/// A property that was there and was not kept. It carries no text: the value was judged worthless
/// or wrong, and nothing is logged as text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IgnoredValue {
    pub kind: MetadataKind,
    pub source: MetadataSource,
    pub reason: IgnoredReason,
}

/// Everything read from one file's properties.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MetadataReading {
    pub fields: Vec<MetadataField>,
    pub ignored: Vec<IgnoredValue>,
    /// True when the file has a metadata container that could not be parsed. Distinct from a file with
    /// no properties: the profile records the reason instead of treating both as empty.
    pub unreadable: bool,
}

impl MetadataReading {
    pub fn first(&self, kind: MetadataKind) -> Option<&MetadataField> {
        self.fields.iter().find(|field| field.kind == kind)
    }

    pub fn values(&self, kind: MetadataKind) -> Vec<&str> {
        self.fields
            .iter()
            .filter(|field| field.kind == kind)
            .map(|field| field.value.as_str())
            .collect()
    }
}

/// The confidence of each source and kind. One table, so a later calibration changes one place.
fn confidence(source: MetadataSource, kind: MetadataKind) -> f32 {
    use MetadataKind as K;
    use MetadataSource as S;
    match (source, kind) {
        // The author of a Markdown file wrote the front matter on purpose.
        (S::FrontMatter, K::Created) => 0.70,
        (S::FrontMatter, _) => 0.80,
        (_, K::Title | K::Author | K::Subject) => 0.60,
        (_, K::Keywords) => 0.55,
        (_, K::Company) => 0.45,
        (_, K::LastModifiedBy) => 0.40,
        (S::Exif, K::Created) => 0.30,
        (_, K::Created) => 0.50,
        (_, K::Modified) => 0.30,
    }
}

/// The oldest date a property may carry: an earlier one is a default (`1601`, `1970`, `1980`).
const OLDEST_PLAUSIBLE_YEAR: i32 = 1990;

/// Read the properties of the file at `path`, whose extension is `extension` (lowercase, no dot).
/// Never panics and never fails: a file that cannot be read gives an empty reading with `unreadable`.
pub fn read_metadata(
    path: &Path,
    extension: &str,
    vocabulary: &ProfileVocabulary,
) -> MetadataReading {
    let mut raw = RawReading::default();
    match extension {
        "docx" | "xlsx" | "xlsm" => match read_ooxml(path, extension, &mut raw) {
            Ok(()) => {}
            Err(()) => raw.unreadable = true,
        },
        "pdf" => match std::fs::read(path) {
            Ok(bytes) => read_pdf(&bytes, &mut raw),
            Err(_) => raw.unreadable = true,
        },
        "md" | "txt" => {
            if let Some(head) = read_head(path, TEXT_HEAD_BYTES) {
                read_front_matter(&String::from_utf8_lossy(&head), &mut raw);
            }
        }
        "jpg" | "jpeg" => {
            if let Some(head) = read_head(path, JPEG_HEAD_BYTES) {
                read_exif(&head, &mut raw);
            }
        }
        _ => {}
    }
    finish(raw, vocabulary)
}

/// [`read_metadata`] over bytes already in memory (what the tests and a caller that holds the file use).
pub fn read_metadata_bytes(
    bytes: &[u8],
    extension: &str,
    vocabulary: &ProfileVocabulary,
) -> MetadataReading {
    let mut raw = RawReading::default();
    match extension {
        "docx" | "xlsx" | "xlsm" => {
            let cursor = std::io::Cursor::new(bytes);
            if read_ooxml_archive(cursor, extension, &mut raw).is_err() {
                raw.unreadable = true;
            }
        }
        "pdf" => read_pdf(bytes, &mut raw),
        "md" | "txt" => {
            let head = &bytes[..bytes.len().min(TEXT_HEAD_BYTES)];
            read_front_matter(&String::from_utf8_lossy(head), &mut raw);
        }
        "jpg" | "jpeg" => read_exif(&bytes[..bytes.len().min(JPEG_HEAD_BYTES)], &mut raw),
        _ => {}
    }
    finish(raw, vocabulary)
}

const TEXT_HEAD_BYTES: usize = 8 * 1024;
const JPEG_HEAD_BYTES: usize = 128 * 1024;

/// A value as read, before the judgement of `finish`.
#[derive(Default)]
struct RawReading {
    values: Vec<(MetadataKind, MetadataSource, String)>,
    unreadable: bool,
}

impl RawReading {
    fn push(&mut self, kind: MetadataKind, source: MetadataSource, value: &str) {
        let value = value.trim();
        if !value.is_empty() {
            self.values.push((kind, source, value.to_string()));
        }
    }
}

fn read_head(path: &Path, limit: usize) -> Option<Vec<u8>> {
    let file = std::fs::File::open(path).ok()?;
    let mut head = Vec::new();
    file.take(limit as u64).read_to_end(&mut head).ok()?;
    Some(head)
}

/// Judge each raw value: drop the generic, the file-name titles and the impossible dates, fold a
/// date to `YYYY-MM-DD`, drop a value read twice, and attach the confidence.
fn finish(raw: RawReading, vocabulary: &ProfileVocabulary) -> MetadataReading {
    let today = Utc::now().date_naive();
    let mut reading = MetadataReading {
        unreadable: raw.unreadable,
        ..MetadataReading::default()
    };
    for (kind, source, value) in raw.values {
        let value = if kind.is_date() {
            match iso_date(&value) {
                Some(date) if plausible(date, today) => date.format("%Y-%m-%d").to_string(),
                _ => {
                    reading.ignored.push(IgnoredValue {
                        kind,
                        source,
                        reason: IgnoredReason::ImplausibleDate,
                    });
                    continue;
                }
            }
        } else {
            value
        };
        if !kind.is_date() {
            if vocabulary.is_generic(&value) {
                reading.ignored.push(IgnoredValue {
                    kind,
                    source,
                    reason: IgnoredReason::Generic,
                });
                continue;
            }
            if kind == MetadataKind::Title && vocabulary.looks_like_file_name(&value) {
                reading.ignored.push(IgnoredValue {
                    kind,
                    source,
                    reason: IgnoredReason::FileNameTitle,
                });
                continue;
            }
        }
        reading.fields.push(MetadataField {
            kind,
            value,
            source,
            confidence: confidence(source, kind),
        });
    }
    // The same value read from two places is one fact: keep the most confident reading of it.
    reading.fields.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut seen: Vec<(MetadataKind, String)> = Vec::new();
    reading.fields.retain(|field| {
        let key = (field.kind, words_key(&field.value));
        if seen.contains(&key) {
            false
        } else {
            seen.push(key);
            true
        }
    });
    reading.fields.sort_by_key(|field| field.kind);
    reading
}

fn plausible(date: NaiveDate, today: NaiveDate) -> bool {
    date.year() >= OLDEST_PLAUSIBLE_YEAR && date <= today + chrono::Duration::days(1)
}

/// `D:20261001093000+02'00'`, `2026-10-01T09:30:00Z`, `2026:10:01 09:30:00`, `20261001`, `2026-10-01`
/// all read as 1 October 2026. The time and the zone are dropped: only the day is a fact worth keeping.
fn iso_date(text: &str) -> Option<NaiveDate> {
    let text = text.trim();
    let text = text.strip_prefix("D:").unwrap_or(text);
    let bytes = text.as_bytes();
    let digits = |range: std::ops::Range<usize>| {
        bytes
            .get(range)
            .is_some_and(|part| part.iter().all(u8::is_ascii_digit))
    };
    let (year, month, day) = if digits(0..4)
        && bytes.get(4).is_some_and(|b| !b.is_ascii_digit())
        && digits(5..7)
        && bytes.get(7).is_some_and(|b| !b.is_ascii_digit())
        && digits(8..10)
    {
        (&text[0..4], &text[5..7], &text[8..10])
    } else if digits(0..8) {
        (&text[0..4], &text[4..6], &text[6..8])
    } else {
        return None;
    };
    NaiveDate::from_ymd_opt(year.parse().ok()?, month.parse().ok()?, day.parse().ok()?)
}

// ---------------------------------------------------------------------------------------------
// OOXML: DOCX and XLSX
// ---------------------------------------------------------------------------------------------

fn read_ooxml(path: &Path, extension: &str, raw: &mut RawReading) -> Result<(), ()> {
    let file = std::fs::File::open(path).map_err(|_| ())?;
    read_ooxml_archive(file, extension, raw)
}

fn read_ooxml_archive<R: Read + std::io::Seek>(
    reader: R,
    extension: &str,
    raw: &mut RawReading,
) -> Result<(), ()> {
    let mut archive = zip::ZipArchive::new(reader).map_err(|_| ())?;
    let (core_source, app_source) = if extension == "docx" {
        (MetadataSource::DocxCore, MetadataSource::DocxApp)
    } else {
        (MetadataSource::XlsxCore, MetadataSource::XlsxApp)
    };
    // A package without these parts simply has no properties: that is not an error.
    if let Some(xml) = read_part(&mut archive, "docProps/core.xml") {
        read_core_properties(&xml, core_source, raw)?;
    }
    if let Some(xml) = read_part(&mut archive, "docProps/app.xml") {
        read_app_properties(&xml, app_source, raw)?;
    }
    Ok(())
}

/// A property part is a few hundred bytes; one that is large is not a property part.
const MAX_PART_BYTES: u64 = 1024 * 1024;

fn read_part<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
) -> Option<String> {
    let entry = archive.by_name(name).ok()?;
    let mut xml = String::new();
    entry.take(MAX_PART_BYTES).read_to_string(&mut xml).ok()?;
    Some(xml)
}

/// The text of each element whose local name is one of `names`, in document order, with the name it
/// matched. Namespaces are ignored (`dc:title` and `title` are the same): the packages of every word
/// processor spell the prefixes differently.
fn elements_text(xml: &str, names: &[&str]) -> Result<Vec<(String, String)>, ()> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut found = Vec::new();
    let mut current: Option<String> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(tag)) => {
                let local = String::from_utf8_lossy(tag.local_name().as_ref()).to_string();
                if names.contains(&local.as_str()) {
                    current = Some(local);
                }
            }
            Ok(Event::Text(text)) => {
                if let Some(name) = &current {
                    let decoded = text.decode().map_err(|_| ())?;
                    found.push((name.clone(), decoded.to_string()));
                }
            }
            Ok(Event::End(tag)) => {
                let local = String::from_utf8_lossy(tag.local_name().as_ref()).to_string();
                if current.as_deref() == Some(local.as_str()) {
                    current = None;
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return Err(()),
        }
    }
    Ok(found)
}

fn read_core_properties(xml: &str, source: MetadataSource, raw: &mut RawReading) -> Result<(), ()> {
    let names = [
        "title",
        "creator",
        "lastModifiedBy",
        "subject",
        "keywords",
        "created",
        "modified",
    ];
    for (name, value) in elements_text(xml, &names)? {
        let kind = match name.as_str() {
            "title" => MetadataKind::Title,
            "creator" => MetadataKind::Author,
            "lastModifiedBy" => MetadataKind::LastModifiedBy,
            "subject" => MetadataKind::Subject,
            "keywords" => MetadataKind::Keywords,
            "created" => MetadataKind::Created,
            _ => MetadataKind::Modified,
        };
        raw.push(kind, source, &value);
    }
    Ok(())
}

fn read_app_properties(xml: &str, source: MetadataSource, raw: &mut RawReading) -> Result<(), ()> {
    for (_, value) in elements_text(xml, &["Company"])? {
        raw.push(MetadataKind::Company, source, &value);
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// PDF: Info dictionary and XMP packet
// ---------------------------------------------------------------------------------------------

fn read_pdf(bytes: &[u8], raw: &mut RawReading) {
    let Ok(document) = lopdf::Document::load_mem(bytes) else {
        raw.unreadable = true;
        return;
    };

    if let Some(info) = document
        .trailer
        .get(b"Info")
        .ok()
        .and_then(|object| document.dereference(object).ok())
        .and_then(|(_, object)| object.as_dict().ok())
    {
        for (key, kind) in [
            (&b"Title"[..], MetadataKind::Title),
            (b"Author", MetadataKind::Author),
            (b"Subject", MetadataKind::Subject),
            (b"Keywords", MetadataKind::Keywords),
            (b"CreationDate", MetadataKind::Created),
            (b"ModDate", MetadataKind::Modified),
        ] {
            if let Some(text) = info
                .get(key)
                .ok()
                .and_then(|object| document.dereference(object).ok())
                .and_then(|(_, object)| lopdf::decode_text_string(object).ok())
            {
                raw.push(kind, MetadataSource::PdfInfo, &text);
            }
        }
    }

    let xmp = document
        .catalog()
        .ok()
        .and_then(|catalog| catalog.get(b"Metadata").ok())
        .and_then(|object| document.dereference(object).ok())
        .and_then(|(_, object)| object.as_stream().ok())
        .and_then(|stream| stream.decompressed_content().ok());
    if let Some(xmp) = xmp {
        read_xmp(&String::from_utf8_lossy(&xmp), raw);
    }
}

/// The XMP packet: `dc:title`, `dc:creator`, `dc:description`, `dc:subject`, `pdf:Keywords`,
/// `xmp:CreateDate`, `xmp:ModifyDate`, as elements or as attributes of `rdf:Description`. The
/// packet's own producer fields are not read.
fn read_xmp(xml: &str, raw: &mut RawReading) {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut property: Option<MetadataKind> = None;
    let kind_of = |local: &str| match local {
        "title" => Some(MetadataKind::Title),
        "creator" => Some(MetadataKind::Author),
        "description" => Some(MetadataKind::Subject),
        "subject" | "Keywords" => Some(MetadataKind::Keywords),
        "CreateDate" => Some(MetadataKind::Created),
        "ModifyDate" => Some(MetadataKind::Modified),
        _ => None,
    };
    loop {
        match reader.read_event() {
            Ok(Event::Start(tag)) => {
                let local = String::from_utf8_lossy(tag.local_name().as_ref()).to_string();
                if local == "Description" {
                    for attribute in tag.attributes().flatten() {
                        let name = String::from_utf8_lossy(attribute.key.local_name().as_ref())
                            .to_string();
                        // `dc:description` is an element, never an attribute that matters here.
                        if let (Some(kind), Ok(value)) = (
                            kind_of(&name),
                            attribute.normalized_value(quick_xml::XmlVersion::Implicit1_0),
                        ) {
                            if name != "description" {
                                raw.push(kind, MetadataSource::PdfXmp, &value);
                            }
                        }
                    }
                } else if let Some(kind) = kind_of(&local) {
                    property = Some(kind);
                }
            }
            Ok(Event::Text(text)) => {
                if let (Some(kind), Ok(value)) = (property, text.decode()) {
                    raw.push(kind, MetadataSource::PdfXmp, &value);
                }
            }
            Ok(Event::End(tag)) => {
                let local = String::from_utf8_lossy(tag.local_name().as_ref()).to_string();
                if kind_of(&local).is_some() {
                    property = None;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Markdown and text front matter
// ---------------------------------------------------------------------------------------------

/// `---`, then `key: value` lines, then `---` (or `...`), at the very top of the file. Anything else at the
/// top, or a block that never closes, is not front matter and gives nothing.
fn read_front_matter(head: &str, raw: &mut RawReading) {
    let head = head.strip_prefix('\u{feff}').unwrap_or(head);
    let mut lines = head.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return;
    }
    let mut entries: Vec<(String, String)> = Vec::new();
    let mut closed = false;
    for line in lines {
        let line = line.trim_end();
        if line == "---" || line == "..." {
            closed = true;
            break;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        entries.push((key.trim().to_ascii_lowercase(), unquote(value)));
    }
    if !closed {
        return;
    }
    for (key, value) in entries {
        let kind = match key.as_str() {
            "title" | "titre" => MetadataKind::Title,
            "author" | "auteur" => MetadataKind::Author,
            "subject" | "sujet" => MetadataKind::Subject,
            "keywords" | "tags" | "mots-cles" => MetadataKind::Keywords,
            "date" | "created" => MetadataKind::Created,
            _ => continue,
        };
        raw.push(kind, MetadataSource::FrontMatter, &value);
    }
}

/// The text without a front-matter block at its top (the same rule as `read_front_matter`: a block that
/// never closes is not front matter and is left in place). The properties are read from the block; the
/// header and the body are read from what follows.
pub fn strip_front_matter(text: &str) -> &str {
    let body = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut consumed = 0usize;
    let mut lines = body.split_inclusive('\n');
    match lines.next() {
        Some(first) if first.trim_end() == "---" => consumed += first.len(),
        _ => return text,
    }
    for line in lines {
        consumed += line.len();
        let trimmed = line.trim_end();
        if trimmed == "---" || trimmed == "..." {
            return &body[consumed..];
        }
    }
    text
}

/// A scalar without its quotes, and a `[a, b]` list as `a, b`.
fn unquote(value: &str) -> String {
    let value = value.trim();
    let value = value
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .unwrap_or(value);
    let value = value.trim();
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|v| v.strip_suffix(quote))
        {
            return inner.to_string();
        }
    }
    value.to_string()
}

// ---------------------------------------------------------------------------------------------
// JPEG: the capture date, as a low-priority source
// ---------------------------------------------------------------------------------------------

/// The EXIF `DateTimeOriginal` (or, failing it, `DateTime`) of a JPEG. Nothing else of the EXIF block is
/// read: a camera's model, a GPS position and a thumbnail are not facts about a document.
fn read_exif(head: &[u8], raw: &mut RawReading) {
    if head.len() < 4 || head[0] != 0xFF || head[1] != 0xD8 {
        return;
    }
    let mut position = 2;
    while position + 4 <= head.len() {
        if head[position] != 0xFF {
            return;
        }
        let marker = head[position + 1];
        if marker == 0xDA || marker == 0xD9 {
            return;
        }
        let length = u16::from_be_bytes([head[position + 2], head[position + 3]]) as usize;
        let segment_start = position + 4;
        let segment_end = (position + 2 + length).min(head.len());
        if marker == 0xE1
            && segment_end > segment_start + 6
            && &head[segment_start..segment_start + 6] == b"Exif\0\0"
        {
            if let Some(date) = exif_date(&head[segment_start + 6..segment_end]) {
                raw.push(MetadataKind::Created, MetadataSource::Exif, &date);
            }
            return;
        }
        position = position + 2 + length;
    }
}

fn exif_date(tiff: &[u8]) -> Option<String> {
    let big_endian = match tiff.get(..2)? {
        b"MM" => true,
        b"II" => false,
        _ => return None,
    };
    let u16_at = |offset: usize| -> Option<u16> {
        let bytes: [u8; 2] = tiff.get(offset..offset + 2)?.try_into().ok()?;
        Some(if big_endian {
            u16::from_be_bytes(bytes)
        } else {
            u16::from_le_bytes(bytes)
        })
    };
    let u32_at = |offset: usize| -> Option<u32> {
        let bytes: [u8; 4] = tiff.get(offset..offset + 4)?.try_into().ok()?;
        Some(if big_endian {
            u32::from_be_bytes(bytes)
        } else {
            u32::from_le_bytes(bytes)
        })
    };
    if u16_at(2)? != 42 {
        return None;
    }
    // The value of an ASCII tag of 20 bytes is stored at an offset.
    let ascii_at = |entry: usize| -> Option<String> {
        let count = u32_at(entry + 4)? as usize;
        let offset = u32_at(entry + 8)? as usize;
        let bytes = tiff.get(offset..offset.checked_add(count)?)?;
        Some(
            String::from_utf8_lossy(bytes)
                .trim_end_matches('\0')
                .to_string(),
        )
    };
    let entries = |ifd: usize| -> Option<Vec<(u16, usize)>> {
        let count = u16_at(ifd)? as usize;
        (0..count.min(256))
            .map(|index| {
                let entry = ifd + 2 + index * 12;
                Some((u16_at(entry)?, entry))
            })
            .collect()
    };
    let ifd0 = u32_at(4)? as usize;
    let ifd0_entries = entries(ifd0)?;
    if let Some((_, pointer)) = ifd0_entries.iter().find(|(tag, _)| *tag == 0x8769) {
        let exif_ifd = u32_at(pointer + 8)? as usize;
        if let Some((_, entry)) = entries(exif_ifd)?
            .into_iter()
            .find(|(tag, _)| *tag == 0x9003)
        {
            return ascii_at(entry);
        }
    }
    ifd0_entries
        .iter()
        .find(|(tag, _)| *tag == 0x0132)
        .and_then(|(_, entry)| ascii_at(*entry))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn vocabulary() -> ProfileVocabulary {
        ProfileVocabulary::load(&["health-fr"]).expect("loads")
    }

    const CORE_OPEN: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\">";

    fn core(inner: &str) -> String {
        format!("{CORE_OPEN}{inner}</cp:coreProperties>")
    }

    fn app(company: &str) -> String {
        format!("<?xml version=\"1.0\"?><Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\"><Application>Microsoft Office Word</Application><Company>{company}</Company></Properties>")
    }

    /// A package with the given property parts, the way a word processor or a spreadsheet writes them.
    fn package(main_part: &str, core_xml: Option<String>, app_xml: Option<String>) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options: zip::write::FileOptions<()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let mut parts = vec![(main_part.to_string(), "<x/>".to_string())];
        if let Some(xml) = core_xml {
            parts.push(("docProps/core.xml".to_string(), xml));
        }
        if let Some(xml) = app_xml {
            parts.push(("docProps/app.xml".to_string(), xml));
        }
        for (name, body) in parts {
            zip.start_file(name, options).expect("starts a part");
            zip.write_all(body.as_bytes()).expect("writes a part");
        }
        zip.finish().expect("finishes").into_inner()
    }

    fn docx(core_xml: Option<String>, app_xml: Option<String>) -> Vec<u8> {
        package("word/document.xml", core_xml, app_xml)
    }

    fn pdf(info: &[(&str, &str)], xmp: Option<&str>) -> Vec<u8> {
        use lopdf::{dictionary, Document, Object, Stream};
        let mut document = Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let page_id = document.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page_id.into()],
                "Count" => 1,
            }),
        );
        let mut catalog = dictionary! { "Type" => "Catalog", "Pages" => pages_id };
        if let Some(xmp) = xmp {
            let stream = Stream::new(
                dictionary! { "Type" => "Metadata", "Subtype" => "XML" },
                xmp.as_bytes().to_vec(),
            );
            let id = document.add_object(stream);
            catalog.set("Metadata", id);
        }
        let catalog_id = document.add_object(catalog);
        document.trailer.set("Root", catalog_id);
        if !info.is_empty() {
            let mut dictionary = lopdf::Dictionary::new();
            for (key, value) in info {
                dictionary.set(*key, lopdf::text_string(value));
            }
            let id = document.add_object(dictionary);
            document.trailer.set("Info", id);
        }
        let mut bytes = Vec::new();
        document.save_to(&mut bytes).expect("saves");
        bytes
    }

    fn xmp(inner: &str) -> String {
        format!("<?xpacket begin=\"x\"?><x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\" xmlns:pdf=\"http://ns.adobe.com/pdf/1.3/\"><rdf:Description>{inner}</rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end=\"w\"?>")
    }

    fn value(reading: &MetadataReading, kind: MetadataKind) -> Option<String> {
        reading.first(kind).map(|field| field.value.clone())
    }

    // ---- DOCX ------------------------------------------------------------------------------

    #[test]
    fn a_docx_with_filled_properties_gives_each_field_with_its_source() {
        let bytes = docx(
            Some(core("<dc:title>Annual review</dc:title><dc:creator>Zorglub Metadatus</dc:creator><cp:lastModifiedBy>Ylva Reviewer</cp:lastModifiedBy><dc:subject>Budget</dc:subject><cp:keywords>budget, audit</cp:keywords><dcterms:created xsi:type=\"dcterms:W3CDTF\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">2026-10-01T09:30:00Z</dcterms:created><dcterms:modified xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">2026-10-03T08:00:00Z</dcterms:modified>")),
            Some(app("Orbital Clinic")),
        );
        let reading = read_metadata_bytes(&bytes, "docx", &vocabulary());

        assert_eq!(
            value(&reading, MetadataKind::Title).as_deref(),
            Some("Annual review")
        );
        assert_eq!(
            value(&reading, MetadataKind::Author).as_deref(),
            Some("Zorglub Metadatus")
        );
        assert_eq!(
            value(&reading, MetadataKind::LastModifiedBy).as_deref(),
            Some("Ylva Reviewer")
        );
        assert_eq!(
            value(&reading, MetadataKind::Subject).as_deref(),
            Some("Budget")
        );
        assert_eq!(
            value(&reading, MetadataKind::Keywords).as_deref(),
            Some("budget, audit")
        );
        assert_eq!(
            value(&reading, MetadataKind::Company).as_deref(),
            Some("Orbital Clinic")
        );
        assert_eq!(
            value(&reading, MetadataKind::Created).as_deref(),
            Some("2026-10-01")
        );
        assert_eq!(
            value(&reading, MetadataKind::Modified).as_deref(),
            Some("2026-10-03")
        );
        let author = reading.first(MetadataKind::Author).expect("author");
        assert_eq!(author.source, MetadataSource::DocxCore);
        assert!(
            author.confidence > 0.0 && author.confidence < 0.85,
            "a property is below a header or a title"
        );
        assert_eq!(
            reading
                .first(MetadataKind::Company)
                .expect("company")
                .source,
            MetadataSource::DocxApp
        );
        assert!(reading.ignored.is_empty());
        assert!(!reading.unreadable);
    }

    #[test]
    fn a_docx_with_empty_properties_gives_nothing_and_is_not_unreadable() {
        let bytes = docx(
            Some(core(
                "<dc:title></dc:title><dc:creator>   </dc:creator><cp:lastModifiedBy/>",
            )),
            Some(app("")),
        );
        let reading = read_metadata_bytes(&bytes, "docx", &vocabulary());
        assert!(reading.fields.is_empty(), "{:?}", reading.fields);
        assert!(reading.ignored.is_empty());
        assert!(!reading.unreadable);

        let no_parts = read_metadata_bytes(&docx(None, None), "docx", &vocabulary());
        assert!(no_parts.fields.is_empty() && !no_parts.unreadable);
    }

    #[test]
    fn a_docx_with_misleading_properties_fills_nothing_wrong() {
        let bytes = docx(
            Some(core("<dc:title>courrier-martin.docx</dc:title><dc:creator>Microsoft Office User</dc:creator><cp:lastModifiedBy>admin</cp:lastModifiedBy><dc:subject>Document1</dc:subject><dcterms:created>1601-01-01T00:00:00Z</dcterms:created><dcterms:modified>2099-01-01T00:00:00Z</dcterms:modified>")),
            Some(app("Microsoft")),
        );
        let reading = read_metadata_bytes(&bytes, "docx", &vocabulary());
        assert!(
            reading.fields.is_empty(),
            "nothing misleading may be filled: {:?}",
            reading.fields
        );
        let reasons: Vec<&str> = reading
            .ignored
            .iter()
            .map(|ignored| ignored.reason.as_str())
            .collect();
        assert_eq!(
            reasons.iter().filter(|r| **r == "generic").count(),
            4,
            "{reasons:?}"
        );
        assert!(reasons.contains(&"file_name_title"), "{reasons:?}");
        assert_eq!(
            reasons.iter().filter(|r| **r == "implausible_date").count(),
            2,
            "{reasons:?}"
        );
    }

    #[test]
    fn the_users_own_name_in_a_property_is_not_an_author() {
        let bytes = docx(
            Some(core("<dc:creator>Zorglub Metadatus</dc:creator>")),
            None,
        );
        let plain = read_metadata_bytes(&bytes, "docx", &vocabulary());
        assert!(plain.first(MetadataKind::Author).is_some());
        let own = vocabulary().with_user_names(&["Zorglub Metadatus".to_string()]);
        let reading = read_metadata_bytes(&bytes, "docx", &own);
        assert!(reading.first(MetadataKind::Author).is_none());
        assert_eq!(reading.ignored.len(), 1);
    }

    #[test]
    fn a_damaged_package_is_unreadable_not_empty() {
        let reading = read_metadata_bytes(b"this is not a zip", "docx", &vocabulary());
        assert!(reading.fields.is_empty());
        assert!(reading.unreadable);
        let broken_xml = docx(Some(core("<dc:title>Open")), None);
        assert!(read_metadata_bytes(&broken_xml, "docx", &vocabulary()).unreadable);
    }

    // ---- XLSX ------------------------------------------------------------------------------

    #[test]
    fn a_workbook_package_gives_its_properties_with_a_workbook_source() {
        let bytes = package(
            "xl/workbook.xml",
            Some(core("<dc:title>Stock count</dc:title><dc:creator>Ylva Reviewer</dc:creator><dcterms:created>2026-09-30T10:00:00Z</dcterms:created>")),
            Some(app("Orbital Clinic")),
        );
        for extension in ["xlsx", "xlsm"] {
            let reading = read_metadata_bytes(&bytes, extension, &vocabulary());
            assert_eq!(
                value(&reading, MetadataKind::Title).as_deref(),
                Some("Stock count")
            );
            assert_eq!(
                value(&reading, MetadataKind::Created).as_deref(),
                Some("2026-09-30")
            );
            assert_eq!(
                reading.first(MetadataKind::Author).expect("author").source,
                MetadataSource::XlsxCore
            );
            assert_eq!(
                reading
                    .first(MetadataKind::Company)
                    .expect("company")
                    .source,
                MetadataSource::XlsxApp
            );
        }
    }

    #[test]
    fn a_csv_or_an_old_binary_workbook_has_no_properties_to_read() {
        for extension in ["csv", "xls"] {
            let reading = read_metadata_bytes(b"a;b\n1;2\n", extension, &vocabulary());
            assert!(reading.fields.is_empty() && !reading.unreadable);
        }
    }

    // ---- PDF -------------------------------------------------------------------------------

    #[test]
    fn a_pdf_info_dictionary_gives_its_fields_and_reads_utf16_titles() {
        let bytes = pdf(
            &[
                ("Title", "Caf\u{e9} accounts \u{2014} October"),
                ("Author", "Zorglub Metadatus"),
                ("Subject", "Accounts"),
                ("Keywords", "accounts, october"),
                ("CreationDate", "D:20261001093000+02'00'"),
                ("ModDate", "D:20261002101500Z"),
                ("Producer", "Microsoft Word"),
                ("Creator", "Acrobat PDFMaker"),
            ],
            None,
        );
        let reading = read_metadata_bytes(&bytes, "pdf", &vocabulary());
        assert_eq!(
            value(&reading, MetadataKind::Title).as_deref(),
            Some("Caf\u{e9} accounts \u{2014} October")
        );
        assert_eq!(
            value(&reading, MetadataKind::Author).as_deref(),
            Some("Zorglub Metadatus")
        );
        assert_eq!(
            value(&reading, MetadataKind::Created).as_deref(),
            Some("2026-10-01")
        );
        assert_eq!(
            value(&reading, MetadataKind::Modified).as_deref(),
            Some("2026-10-02")
        );
        assert_eq!(
            reading.first(MetadataKind::Author).expect("author").source,
            MetadataSource::PdfInfo
        );
        // The producer and the creator tool describe programs: never read.
        assert_eq!(reading.fields.len(), 6, "{:?}", reading.fields);
        assert!(reading.ignored.is_empty());
    }

    #[test]
    fn a_pdf_xmp_packet_is_read_and_a_value_found_twice_is_one_fact() {
        let packet = xmp("<dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">Annual review</rdf:li></rdf:Alt></dc:title><dc:creator><rdf:Seq><rdf:li>Ylva Reviewer</rdf:li></rdf:Seq></dc:creator><xmp:CreateDate>2026-10-01T09:30:00+02:00</xmp:CreateDate><pdf:Producer>Microsoft Word</pdf:Producer>");
        let bytes = pdf(
            &[("Author", "Ylva Reviewer"), ("Title", "Annual review")],
            Some(&packet),
        );
        let reading = read_metadata_bytes(&bytes, "pdf", &vocabulary());
        assert_eq!(
            reading.values(MetadataKind::Title),
            vec!["Annual review"],
            "read twice, kept once"
        );
        assert_eq!(reading.values(MetadataKind::Author), vec!["Ylva Reviewer"]);
        assert_eq!(
            value(&reading, MetadataKind::Created).as_deref(),
            Some("2026-10-01")
        );
        assert_eq!(
            reading
                .first(MetadataKind::Created)
                .expect("created")
                .source,
            MetadataSource::PdfXmp
        );

        let only_xmp = pdf(&[], Some(&packet));
        let reading = read_metadata_bytes(&only_xmp, "pdf", &vocabulary());
        assert_eq!(
            reading.first(MetadataKind::Author).expect("author").source,
            MetadataSource::PdfXmp
        );
    }

    #[test]
    fn a_pdf_with_misleading_properties_fills_nothing_wrong() {
        let bytes = pdf(
            &[
                ("Title", "Microsoft Word - courrier-martin.docx"),
                ("Author", "PDFCreator"),
                ("Subject", "Canon iR-ADV C3530"),
                ("CreationDate", "D:19700101000000Z"),
            ],
            None,
        );
        let reading = read_metadata_bytes(&bytes, "pdf", &vocabulary());
        assert!(reading.fields.is_empty(), "{:?}", reading.fields);
        assert_eq!(reading.ignored.len(), 4);
    }

    #[test]
    fn a_pdf_without_properties_is_empty_and_garbage_is_unreadable() {
        let reading = read_metadata_bytes(&pdf(&[], None), "pdf", &vocabulary());
        assert!(reading.fields.is_empty() && !reading.unreadable);
        let garbage = read_metadata_bytes(b"%PDF-1.4 not really", "pdf", &vocabulary());
        assert!(garbage.fields.is_empty() && garbage.unreadable);
    }

    // ---- Markdown and text -----------------------------------------------------------------

    #[test]
    fn markdown_front_matter_gives_its_fields_with_the_highest_property_confidence() {
        let text = "---\ntitle: \"Meeting minutes\"\nauthor: Ylva Reviewer\ndate: 2026-10-01\ntags: [budget, audit]\nlayout: post\n---\n# Heading\nBody.\n";
        let reading = read_metadata_bytes(text.as_bytes(), "md", &vocabulary());
        assert_eq!(
            value(&reading, MetadataKind::Title).as_deref(),
            Some("Meeting minutes")
        );
        assert_eq!(
            value(&reading, MetadataKind::Author).as_deref(),
            Some("Ylva Reviewer")
        );
        assert_eq!(
            value(&reading, MetadataKind::Created).as_deref(),
            Some("2026-10-01")
        );
        assert_eq!(
            value(&reading, MetadataKind::Keywords).as_deref(),
            Some("budget, audit")
        );
        assert_eq!(reading.fields.len(), 4, "an unknown key is not a field");
        assert_eq!(
            reading.first(MetadataKind::Title).expect("title").source,
            MetadataSource::FrontMatter
        );

        let txt = read_metadata_bytes(text.as_bytes(), "txt", &vocabulary());
        assert_eq!(txt.fields.len(), 4, "a text file has the same front matter");
    }

    #[test]
    fn the_front_matter_block_is_cut_off_the_text_only_when_it_closes() {
        assert_eq!(
            strip_front_matter("---\ntitle: x\n---\n# Heading\n"),
            "# Heading\n"
        );
        assert_eq!(strip_front_matter("\u{feff}---\na: b\n...\nBody"), "Body");
        let open = "---\ntitle: never closed\nBody text\n";
        assert_eq!(strip_front_matter(open), open);
        assert_eq!(strip_front_matter("plain text\n"), "plain text\n");
        assert_eq!(strip_front_matter(""), "");
    }

    #[test]
    fn text_that_only_looks_like_front_matter_gives_nothing() {
        let vocabulary = vocabulary();
        for text in [
            "# Heading\n---\ntitle: Not at the top\n---\n",
            "---\ntitle: Never closed\nauthor: Nobody\n\nBody text goes on.\n",
            "Dear colleague,\ntitle: not a block\n",
            "---\n---\nBody.\n",
            "",
        ] {
            let reading = read_metadata_bytes(text.as_bytes(), "md", &vocabulary);
            assert!(
                reading.fields.is_empty(),
                "{text:?} gave {:?}",
                reading.fields
            );
        }
        let generic = read_metadata_bytes(b"---\nauthor: admin\n---\nBody.\n", "md", &vocabulary);
        assert!(generic.fields.is_empty() && generic.ignored.len() == 1);
    }

    // ---- JPEG ------------------------------------------------------------------------------

    /// A JPEG header with an EXIF block holding one date, in the given byte order.
    fn jpeg_with_date(date: &str, in_exif_ifd: bool, big_endian: bool) -> Vec<u8> {
        let put16 = |value: u16| {
            if big_endian {
                value.to_be_bytes()
            } else {
                value.to_le_bytes()
            }
        };
        let put32 = |value: u32| {
            if big_endian {
                value.to_be_bytes()
            } else {
                value.to_le_bytes()
            }
        };
        let mut tiff: Vec<u8> = Vec::new();
        tiff.extend(if big_endian { b"MM" } else { b"II" });
        tiff.extend(put16(42));
        tiff.extend(put32(8));
        let mut data = date.as_bytes().to_vec();
        data.push(0);
        if in_exif_ifd {
            // IFD0: one entry pointing to the Exif IFD at offset 26; Exif IFD: one date entry at 44.
            tiff.extend(put16(1));
            tiff.extend(put16(0x8769));
            tiff.extend(put16(4));
            tiff.extend(put32(1));
            tiff.extend(put32(26));
            tiff.extend(put32(0));
            tiff.extend(put16(1));
            tiff.extend(put16(0x9003));
            tiff.extend(put16(2));
            tiff.extend(put32(data.len() as u32));
            tiff.extend(put32(44));
            tiff.extend(put32(0));
        } else {
            tiff.extend(put16(1));
            tiff.extend(put16(0x0132));
            tiff.extend(put16(2));
            tiff.extend(put32(data.len() as u32));
            tiff.extend(put32(26));
            tiff.extend(put32(0));
        }
        tiff.extend(&data);
        let mut segment = b"Exif\0\0".to_vec();
        segment.extend(&tiff);
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE1];
        jpeg.extend(((segment.len() + 2) as u16).to_be_bytes());
        jpeg.extend(&segment);
        jpeg.extend([0xFF, 0xDA, 0, 2]);
        jpeg
    }

    #[test]
    fn a_jpeg_capture_date_is_a_low_confidence_created_date() {
        for (in_exif_ifd, big_endian) in
            [(true, true), (true, false), (false, true), (false, false)]
        {
            let bytes = jpeg_with_date("2026:10:01 09:30:00", in_exif_ifd, big_endian);
            let reading = read_metadata_bytes(&bytes, "jpg", &vocabulary());
            let created = reading.first(MetadataKind::Created).unwrap_or_else(|| {
                panic!("no date for exif_ifd={in_exif_ifd} big_endian={big_endian}")
            });
            assert_eq!(created.value, "2026-10-01");
            assert_eq!(created.source, MetadataSource::Exif);
            assert!(created.confidence <= 0.30);
        }
    }

    #[test]
    fn a_jpeg_without_a_usable_date_gives_nothing() {
        let vocabulary = vocabulary();
        assert!(
            read_metadata_bytes(&[0xFF, 0xD8, 0xFF, 0xD9], "jpeg", &vocabulary)
                .fields
                .is_empty()
        );
        assert!(read_metadata_bytes(b"not a jpeg", "jpg", &vocabulary)
            .fields
            .is_empty());
        let zero = jpeg_with_date("0000:00:00 00:00:00", true, true);
        let reading = read_metadata_bytes(&zero, "jpg", &vocabulary);
        assert!(reading.fields.is_empty());
        assert_eq!(
            reading.ignored.len(),
            1,
            "a placeholder date is recorded as ignored"
        );
        // PNG carries no property this reader trusts.
        assert!(read_metadata_bytes(
            &jpeg_with_date("2026:10:01 09:30:00", true, true),
            "png",
            &vocabulary
        )
        .fields
        .is_empty());
    }

    // ---- From a path -----------------------------------------------------------------------

    #[test]
    fn reading_from_a_path_is_the_same_as_reading_the_bytes() {
        let directory = tempfile::tempdir().expect("temporary folder");
        let vocabulary = vocabulary();
        let cases: Vec<(&str, Vec<u8>)> = vec![
            (
                "a.docx",
                docx(Some(core("<dc:creator>Ylva Reviewer</dc:creator>")), None),
            ),
            ("b.pdf", pdf(&[("Author", "Ylva Reviewer")], None)),
            ("c.md", b"---\nauthor: Ylva Reviewer\n---\nBody\n".to_vec()),
            ("d.jpg", jpeg_with_date("2026:10:01 09:30:00", true, false)),
        ];
        for (name, bytes) in cases {
            let path = directory.path().join(name);
            std::fs::write(&path, &bytes).expect("writes");
            let extension = name.rsplit('.').next().expect("extension");
            assert_eq!(
                read_metadata(&path, extension, &vocabulary),
                read_metadata_bytes(&bytes, extension, &vocabulary),
                "{name}"
            );
        }
        let missing = read_metadata(&directory.path().join("missing.pdf"), "pdf", &vocabulary);
        assert!(missing.unreadable && missing.fields.is_empty());
    }
}
