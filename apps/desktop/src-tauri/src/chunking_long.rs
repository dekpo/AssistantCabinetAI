//! The chunker for long documents (long-documents programme, lot 3; `docs/LONG-DOCUMENTS.md` 5.5).
//!
//! Used above the long threshold only (`chunking::is_long`); below it `chunking::chunk` runs, byte for
//! byte as before (golden test). It works on the document as one stream of lines, so a passage may run
//! over a page break; its page is the page of its first line.
//!
//! - **A heading starts a passage.** Nothing is carried over a heading.
//! - **A table or a list is never cut.** A run of rows (`|`, tabs, or columns of spaces) is a table, a run
//!   of bullets or numbered items, with the line that ends with `:` before it, is a list. One that does not
//!   fit a passage goes in whole up to twice the size; a bigger one is cut between rows and every part
//!   repeats its header row (its title).
//! - **A sentence is not cut when it can be avoided**: a full passage is closed at the last line that ends a
//!   sentence, if that keeps at least 60 % of it.
//! - **Overlap**: a share of the previous passage's last lines opens the next one, inside a section and
//!   never into or out of a table or a list.
//! - **Prefix**: the document's title and the section path go to `index_text`, the text that is embedded
//!   and indexed; `text`, what is cited, never has it.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::RwLock;

use crate::chunking::{paragraphs, Chunk};
use crate::extraction::{ExtractedDocument, PageOrigin};
use crate::structure::{Section, SectionSource};

/// The chunker that cut a document: 1 below the long threshold (`chunking::chunk`), 2 above it.
pub const CHUNKER_SHORT: i64 = 1;
pub const CHUNKER_LONG: i64 = 2;

/// A passage closed at a sentence end keeps at least this share of the window.
const MIN_SENTENCE_CUT_SHARE: f64 = 0.6;
/// An atomic block (table, list) goes in whole up to this many times the size.
const ATOMIC_FACTOR: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LongOptions {
    pub max_chars: usize,
    pub overlap_percent: usize,
    pub prefix: bool,
}

impl Default for LongOptions {
    /// The owner's choice (14 October 2026), made on the measured grid of `docs/LONG-DOCUMENTS.md`
    /// 8 quater: 1200 characters, no overlap, no prefix. The overlap did not raise any channel and
    /// cost Analyse time; 800 and 1600 characters were not better; the prefix lowered the meaning-based
    /// channels (mixed search 0.96 -> 0.86, vector search 0.90 -> 0.74), so it stays behind its switch.
    fn default() -> Self {
        Self {
            max_chars: 1_200,
            overlap_percent: 0,
            prefix: false,
        }
    }
}

static OPTIONS: RwLock<Option<LongOptions>> = RwLock::new(None);

/// The options the Analyse uses for a long document.
pub fn long_options() -> LongOptions {
    OPTIONS
        .read()
        .ok()
        .and_then(|guard| *guard)
        .unwrap_or_default()
}

/// **Measurement only.** The laboratory sets the options to compare sizes and overlaps; the product never
/// calls this. `None` restores the defaults.
pub fn set_long_options_for_measurement(options: Option<LongOptions>) {
    if let Ok(mut guard) = OPTIONS.write() {
        *guard = options;
    }
}

#[derive(Debug, Clone, Default)]
pub struct Chunked {
    pub chunks: Vec<Chunk>,
    /// (chunk id, section position) for the passages that start inside a section.
    pub owners: Vec<(String, usize)>,
    /// (chunk id, last page) for the passages that run over a page break; the others end on the page
    /// they start.
    pub ends: Vec<(String, u32)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Heading,
    Row,
    Item,
    Plain,
}

enum Block {
    Heading(usize),
    Plain(usize),
    Table(Range<usize>),
    List(Range<usize>, bool),
}

struct Line<'a> {
    text: &'a str,
    page: u32,
    para: usize,
    page_index: usize,
}

fn is_row(text: &str) -> bool {
    if text.matches('|').count() >= 2 || text.contains('\t') {
        return true;
    }
    // Columns of spaces: three cells at least, each separated from the next by a run of three spaces.
    let mut gap = 0;
    let mut boundaries = 0;
    let mut seen = false;
    for c in text.trim().chars() {
        if c == ' ' {
            gap += 1;
        } else {
            if gap >= 3 && seen {
                boundaries += 1;
            }
            gap = 0;
            seen = true;
        }
    }
    boundaries >= 2
}

/// How many tables a passage holds: runs of two rows or more.
pub fn count_tables(text: &str) -> usize {
    let mut tables = 0;
    let mut run = 0;
    for line in text.lines() {
        if is_row(line.trim()) {
            run += 1;
            if run == 2 {
                tables += 1;
            }
        } else {
            run = 0;
        }
    }
    tables
}

fn is_item(text: &str) -> bool {
    let t = text.trim_start();
    if t.starts_with("- ") || t.starts_with("* ") || t.starts_with('\u{2022}') {
        return true;
    }
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    digits > 0
        && digits <= 3
        && matches!(t[digits..].chars().next(), Some('.') | Some(')'))
        && t[digits + 1..].starts_with(' ')
}

fn ends_sentence(text: &str) -> bool {
    matches!(
        text.trim_end().chars().last(),
        Some('.' | '!' | '?' | ':' | ';')
    )
}

fn title_of(relative_path: &str) -> String {
    let name = relative_path.rsplit('/').next().unwrap_or(relative_path);
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    let stem = match stem.split_once('_') {
        Some((head, rest))
            if head.len() == 10 && head.chars().filter(char::is_ascii_digit).count() == 8 =>
        {
            rest
        }
        _ => stem,
    };
    stem.replace(['-', '_'], " ")
}

struct Cutter<'a> {
    lines: Vec<Line<'a>>,
    kinds: Vec<Kind>,
    /// The table or list a line belongs to, if any: a passage is never closed inside one.
    atomic: Vec<Option<usize>>,
    document: &'a ExtractedDocument,
    sections: &'a [Section],
    options: LongOptions,
    title: String,
    ordinals: HashMap<u32, u32>,
    out: Chunked,
}

#[derive(Default)]
struct Cur {
    lead: Vec<usize>,
    own: Vec<usize>,
    chars: usize,
    /// The last thing added is a table or a list: nothing is carried out of it.
    atomic_tail: bool,
}

pub fn chunk_long(
    document: &ExtractedDocument,
    sections: &[Section],
    options: &LongOptions,
) -> Chunked {
    let mut lines: Vec<Line> = Vec::new();
    for (page_index, page) in document.pages.iter().enumerate() {
        for (para, text) in paragraphs(&page.text).into_iter().enumerate() {
            lines.push(Line {
                text,
                page: page.page_number,
                para,
                page_index,
            });
        }
    }
    let headings: HashMap<(u32, usize), usize> = sections
        .iter()
        .filter(|s| s.source != SectionSource::Slice && !s.title.is_empty() && s.located)
        .map(|s| ((s.anchor_page, s.anchor_paragraph), s.position))
        .collect();
    let kinds: Vec<Kind> = lines
        .iter()
        .map(|line| {
            if headings.contains_key(&(line.page, line.para)) {
                Kind::Heading
            } else if is_row(line.text) {
                Kind::Row
            } else if is_item(line.text) {
                Kind::Item
            } else {
                Kind::Plain
            }
        })
        .collect();
    let mut cutter = Cutter {
        lines,
        kinds,
        atomic: Vec::new(),
        document,
        sections,
        options: *options,
        title: title_of(&document.relative_path),
        ordinals: HashMap::new(),
        out: Chunked::default(),
    };
    let blocks = cutter.blocks();
    cutter.atomic = vec![None; cutter.lines.len()];
    for (id, block) in blocks.iter().enumerate() {
        if let Block::Table(range) | Block::List(range, _) = block {
            for k in range.clone() {
                cutter.atomic[k] = Some(id);
            }
        }
    }
    cutter.pack(&blocks);
    cutter.out
}

impl<'a> Cutter<'a> {
    fn len(&self, index: usize) -> usize {
        self.lines[index].text.chars().count()
    }

    fn blocks(&self) -> Vec<Block> {
        let n = self.lines.len();
        let mut blocks: Vec<Block> = Vec::new();
        let mut i = 0;
        while i < n {
            match self.kinds[i] {
                Kind::Heading => {
                    blocks.push(Block::Heading(i));
                    i += 1;
                }
                Kind::Plain => {
                    blocks.push(Block::Plain(i));
                    i += 1;
                }
                kind @ (Kind::Row | Kind::Item) => {
                    let mut j = i;
                    while j < n && self.kinds[j] == kind {
                        j += 1;
                    }
                    if j - i < 2 {
                        for k in i..j {
                            blocks.push(Block::Plain(k));
                        }
                    } else if kind == Kind::Row {
                        blocks.push(Block::Table(i..j));
                    } else {
                        let titled = matches!(blocks.last(), Some(Block::Plain(p)) if *p + 1 == i && ends_sentence(self.lines[*p].text) && self.lines[*p].text.trim_end().ends_with(':'));
                        if titled {
                            if let Some(Block::Plain(p)) = blocks.pop() {
                                blocks.push(Block::List(p..j, true));
                            }
                        } else {
                            blocks.push(Block::List(i..j, false));
                        }
                    }
                    i = j;
                }
            }
        }
        blocks
    }

    fn pack(&mut self, blocks: &[Block]) {
        let max = self.options.max_chars;
        let hard = max * ATOMIC_FACTOR;
        let mut cur = Cur::default();
        for block in blocks {
            match block {
                Block::Heading(i) => {
                    if self.has_body(&cur) {
                        self.emit(&mut cur, false);
                    }
                    self.push_own(&mut cur, *i);
                }
                Block::Plain(i) => {
                    let l = self.len(*i);
                    if !cur.own.is_empty() && cur.chars + 1 + l > max && self.has_body(&cur) {
                        self.split_and_emit(&mut cur);
                    }
                    self.push_own(&mut cur, *i);
                }
                Block::Table(range) | Block::List(range, _) => {
                    let titled = matches!(block, Block::List(_, true));
                    let size: usize = range.clone().map(|k| self.len(k) + 1).sum();
                    if size <= hard {
                        if self.has_body(&cur) && cur.chars + 1 + size > max {
                            self.emit(&mut cur, false);
                        }
                        // the overlap of the previous passage does not lead into a table or a list
                        if !cur.lead.is_empty() && cur.own.is_empty() {
                            cur.lead.clear();
                            cur.chars = 0;
                        }
                        for k in range.clone() {
                            self.push_own(&mut cur, k);
                        }
                        cur.atomic_tail = true;
                        if cur.chars > max {
                            self.emit(&mut cur, false);
                        }
                    } else {
                        if self.has_body(&cur) {
                            self.emit(&mut cur, false);
                        }
                        cur.lead.clear();
                        cur.chars = cur.own.iter().map(|k| self.len(*k) + 1).sum();
                        let header = range.start;
                        let first_row = if titled || self.kinds[header] == Kind::Row {
                            header + 1
                        } else {
                            header
                        };
                        let repeated = titled || self.kinds[header] == Kind::Row;
                        let mut first_part = true;
                        let mut k = first_row;
                        while k < range.end {
                            if first_part {
                                if repeated {
                                    self.push_own(&mut cur, header);
                                }
                            } else if repeated {
                                cur.lead.push(header);
                                cur.chars += self.len(header) + 1;
                            }
                            let mut rows = 0;
                            while k < range.end && (rows == 0 || cur.chars + 1 + self.len(k) <= max)
                            {
                                self.push_own(&mut cur, k);
                                k += 1;
                                rows += 1;
                            }
                            cur.atomic_tail = true;
                            self.emit(&mut cur, false);
                            first_part = false;
                        }
                    }
                }
            }
        }
        self.emit(&mut cur, false);
    }

    fn has_body(&self, cur: &Cur) -> bool {
        cur.own.iter().any(|i| self.kinds[*i] != Kind::Heading)
    }

    fn push_own(&self, cur: &mut Cur, index: usize) {
        cur.chars += self.len(index) + usize::from(cur.chars > 0);
        cur.own.push(index);
        if self.kinds[index] == Kind::Plain || self.kinds[index] == Kind::Heading {
            cur.atomic_tail = false;
        }
    }

    /// Close a full passage at the last sentence end that keeps most of it; what follows opens the next.
    fn split_and_emit(&mut self, cur: &mut Cur) {
        let total: usize = cur.own.iter().map(|k| self.len(*k) + 1).sum();
        let mut kept = 0usize;
        let mut cut = None;
        for (position, k) in cur.own.iter().enumerate() {
            kept += self.len(*k) + 1;
            let next = cur.own.get(position + 1).copied();
            let inside_block = self.atomic[*k].is_some()
                && next.is_some_and(|n| self.atomic[n] == self.atomic[*k]);
            if position + 1 < cur.own.len()
                && !inside_block
                && ends_sentence(self.lines[*k].text)
                && kept as f64 >= MIN_SENTENCE_CUT_SHARE * total as f64
            {
                cut = Some(position + 1);
            }
        }
        match cut {
            Some(at) => {
                let pending: Vec<usize> = cur.own.split_off(at);
                self.emit(cur, true);
                for k in pending {
                    self.push_own(cur, k);
                }
            }
            None => self.emit(cur, true),
        }
    }

    fn section_at(&self, line: &Line) -> Option<usize> {
        self.sections
            .iter()
            .rev()
            .find(|s| (s.anchor_page, s.anchor_paragraph) <= (line.page, line.para))
            .map(|s| s.position)
    }

    fn prefix_for(&self, owner: Option<usize>) -> String {
        let mut path: Vec<&str> = Vec::new();
        let mut at = owner;
        while let Some(position) = at {
            let Some(section) = self.sections.iter().find(|s| s.position == position) else {
                break;
            };
            if !section.title.is_empty() {
                path.push(section.title.as_str());
            }
            at = section.parent;
        }
        path.reverse();
        // Headings are Markdown or plain; the path names the section, not its markup.
        let path: Vec<String> = path
            .iter()
            .map(|t| t.trim_start_matches('#').trim().to_string())
            .collect();
        if path.is_empty() {
            self.title.clone()
        } else {
            format!("{}\n{}", self.title, path.join(" > "))
        }
    }

    fn emit(&mut self, cur: &mut Cur, carry: bool) {
        if cur.own.is_empty() {
            return;
        }
        let first = &self.lines[cur.own[0]];
        let owner = self.section_at(first);
        let page = first.page;
        let origin_page = &self.document.pages[first.page_index];
        let ordinal = {
            let counter = self.ordinals.entry(page).or_insert(0);
            *counter += 1;
            *counter
        };
        let text = cur
            .lead
            .iter()
            .chain(cur.own.iter())
            .map(|i| self.lines[*i].text)
            .collect::<Vec<_>>()
            .join("\n");
        let chunk_id = format!("{}#p{}#s{}", self.document.relative_path, page, ordinal);
        let index_text = self
            .options
            .prefix
            .then(|| format!("{}\n{}", self.prefix_for(owner), text));
        let (origin, confidence): (PageOrigin, Option<f32>) =
            (origin_page.origin, origin_page.confidence);
        if let Some(position) = owner {
            self.out.owners.push((chunk_id.clone(), position));
        }
        // The page of the passage's last line: past the page it starts on when a table, a list or a
        // sentence runs over a break.
        if let Some(last) = cur.own.last().map(|i| self.lines[*i].page) {
            if last > page {
                self.out.ends.push((chunk_id.clone(), last));
            }
        }
        self.out.chunks.push(Chunk {
            chunk_id,
            relative_path: self.document.relative_path.clone(),
            page_number: page,
            section: ordinal,
            text,
            index_text,
            origin,
            confidence,
        });

        // The lines that open the next passage.
        let mut lead: Vec<usize> = Vec::new();
        let budget = self.options.max_chars * self.options.overlap_percent / 100;
        if carry && budget > 0 && !cur.atomic_tail {
            let mut total = 0;
            for k in cur.own.iter().rev() {
                if self.kinds[*k] != Kind::Plain {
                    break;
                }
                let l = self.len(*k) + 1;
                if total + l > budget && !(lead.is_empty() && l <= 2 * budget) {
                    break;
                }
                total += l;
                lead.push(*k);
            }
            lead.reverse();
        }
        cur.chars = lead.iter().map(|k| self.len(*k) + 1).sum();
        cur.lead = lead;
        cur.own.clear();
        cur.atomic_tail = false;
    }
}
