//! Lot 3 of the long-documents programme: the chunker for documents above the long threshold. It cuts
//! at headings, never cuts a table or a list (and repeats the header of one too big for a chunk), keeps a
//! sentence that runs over a page break together, overlaps by a measured share, and prefixes the text that
//! is embedded and indexed, never the text that is cited. Every text is invented.

use assistant_cabinet_ai_lib::chunking_long::{chunk_long, LongOptions};
use assistant_cabinet_ai_lib::extraction::{ExtractedDocument, ExtractedPage, PageOrigin};
use assistant_cabinet_ai_lib::structure::detect;

fn page(number: u32, text: &str) -> ExtractedPage {
    ExtractedPage {
        page_number: number,
        text: text.to_string(),
        origin: PageOrigin::TextLayer,
        confidence: None,
    }
}

fn document(pages: Vec<ExtractedPage>) -> ExtractedDocument {
    ExtractedDocument {
        relative_path: "docs/guide.md".into(),
        pages,
        empty: false,
        used_ocr: false,
        low_confidence: false,
    }
}

/// `count` ordinary sentences of about 90 characters, each ending with a full stop.
fn body(tag: &str, count: usize) -> String {
    (0..count)
        .map(|n| format!("the ordinary sentence number {n} of the part {tag} goes on with the usual words and ends here."))
        .collect::<Vec<_>>()
        .join("\n")
}

const OPTIONS: LongOptions = LongOptions {
    max_chars: 1_200,
    overlap_percent: 0,
    prefix: false,
};

fn cut(
    doc: &ExtractedDocument,
    options: &LongOptions,
) -> assistant_cabinet_ai_lib::chunking_long::Chunked {
    let sections = detect("md", doc, &[], &[]);
    chunk_long(doc, &sections, options)
}

#[test]
fn a_heading_always_starts_a_passage_and_the_passage_belongs_to_its_section() {
    let text = format!(
        "# One\n{}\n## Two\n{}\n# Three\n{}",
        body("a", 14),
        body("b", 14),
        body("c", 14)
    );
    let doc = document(vec![page(1, &text)]);
    let out = cut(&doc, &OPTIONS);
    for chunk in &out.chunks {
        let lines: Vec<&str> = chunk.text.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            if line.starts_with('#') {
                assert_eq!(index, 0, "a heading inside a passage: {}", chunk.text);
            }
        }
    }
    let heading_two = out
        .chunks
        .iter()
        .find(|c| c.text.starts_with("## Two"))
        .expect("a passage starts at the heading");
    let owner = out
        .owners
        .iter()
        .find(|(id, _)| *id == heading_two.chunk_id)
        .expect("it has an owner");
    assert_eq!(
        owner.1, 1,
        "the passage that starts at '## Two' belongs to it"
    );
    assert_eq!(out.owners.len(), out.chunks.len());
}

#[test]
fn a_table_split_over_a_page_break_stays_in_one_passage_with_its_header() {
    let header = "Site | Delai (jours) | Tarif (CHF)";
    let rows: Vec<String> = (1..=7)
        .map(|n| format!("Site{n} | {} | {}", n * 3, n * 40))
        .collect();
    let page_one = format!("# Tables\n{}\n{header}", body("a", 9));
    let page_two = format!("{}\n{}", rows.join("\n"), body("b", 9));
    let doc = document(vec![page(1, &page_one), page(2, &page_two)]);
    let out = cut(&doc, &OPTIONS);
    let holder = out
        .chunks
        .iter()
        .find(|c| c.text.contains(header))
        .expect("the header is somewhere");
    for row in &rows {
        assert!(
            holder.text.contains(row.as_str()),
            "{row} is not with its header in {}",
            holder.text
        );
    }
    assert_eq!(
        holder.page_number, 1,
        "the passage starts on the page where its first line is"
    );
}

#[test]
fn a_list_stays_whole_with_its_title_even_across_a_page_break() {
    let title = "Liste de controle du poste Kestrel :";
    let items: Vec<String> = (1..=6)
        .map(|n| format!("- verifier l'element numero {n}"))
        .collect();
    let page_one = format!("# Lists\n{}\n{title}", body("a", 10));
    let page_two = format!("{}\n{}", items.join("\n"), body("b", 5));
    let doc = document(vec![page(1, &page_one), page(2, &page_two)]);
    let out = cut(&doc, &OPTIONS);
    let holder = out.chunks.iter().find(|c| c.text.contains(title)).unwrap();
    for item in &items {
        assert!(
            holder.text.contains(item.as_str()),
            "{item} missing in {}",
            holder.text
        );
    }
}

#[test]
fn a_table_too_big_for_a_passage_is_cut_between_rows_and_every_part_repeats_the_header() {
    let header = "Site | Delai (jours) | Tarif (CHF)";
    let rows: Vec<String> = (1..=150)
        .map(|n| format!("Site{n:03} | {} | {}", n % 30, n * 11))
        .collect();
    let text = format!("# Big table\n{header}\n{}", rows.join("\n"));
    let doc = document(vec![page(1, &text)]);
    let out = cut(&doc, &OPTIONS);
    let parts: Vec<_> = out
        .chunks
        .iter()
        .filter(|c| c.text.contains("Site001") || c.text.contains("Site1"))
        .collect();
    assert!(
        parts.len() >= 2,
        "a table of 150 rows does not fit one passage"
    );
    for part in &parts {
        assert!(
            part.text.lines().any(|l| l == header),
            "a part without its header: {}",
            part.text
        );
        assert!(
            part.text.chars().count() <= 2 * OPTIONS.max_chars,
            "a part is too big"
        );
    }
    for row in &rows {
        let holders = parts
            .iter()
            .filter(|p| p.text.lines().any(|l| l == row))
            .count();
        assert_eq!(holders, 1, "{row} must be in exactly one part");
    }
}

#[test]
fn a_sentence_that_runs_over_a_page_break_is_kept_in_one_passage() {
    let end_of_page = format!(
        "# Declarations\n{}\nThe yearly declaration of radioprotection",
        body("a", 9)
    );
    let start_of_page = format!("is signed each year by Odile Pasquereau.\n{}", body("b", 9));
    let doc = document(vec![page(1, &end_of_page), page(2, &start_of_page)]);
    let out = cut(&doc, &OPTIONS);
    let holder = out
        .chunks
        .iter()
        .find(|c| c.text.contains("declaration of radioprotection"))
        .unwrap();
    assert!(holder.text.contains("Odile Pasquereau"), "{}", holder.text);
}

#[test]
fn consecutive_passages_overlap_by_about_the_share_asked_and_never_across_a_heading() {
    let text = format!("# One\n{}\n# Two\n{}", body("a", 40), body("b", 40));
    let doc = document(vec![page(1, &text)]);
    let plain = cut(&doc, &OPTIONS);
    let overlapped = cut(
        &doc,
        &LongOptions {
            overlap_percent: 12,
            ..OPTIONS
        },
    );

    for pair in plain.chunks.windows(2) {
        let (a, b) = (
            pair[0].text.lines().last().unwrap(),
            pair[1].text.lines().next().unwrap(),
        );
        assert_ne!(a, b, "no overlap was asked");
    }
    assert!(overlapped.chunks.len() >= plain.chunks.len());
    let mut shared = 0;
    for pair in overlapped.chunks.windows(2) {
        if pair[1].text.starts_with('#') {
            assert_ne!(
                pair[0].text.lines().last(),
                pair[1].text.lines().next(),
                "nothing carried over a heading"
            );
            continue;
        }
        let last = pair[0].text.lines().last().unwrap();
        if pair[1].text.lines().next() == Some(last) {
            shared += 1;
        }
    }
    assert!(
        shared >= 2,
        "the passages inside a section overlap by their last line"
    );
    for chunk in &overlapped.chunks {
        assert!(
            chunk.text.chars().count() <= 1_200 + 1_200 / 4,
            "{}",
            chunk.text.chars().count()
        );
    }
}

#[test]
fn the_prefix_goes_to_the_indexed_text_only() {
    let text = format!("# Hygiene\n{}\n## Surfaces\n{}", body("a", 6), body("b", 6));
    let doc = document(vec![page(1, &text)]);
    let with = cut(
        &doc,
        &LongOptions {
            prefix: true,
            ..OPTIONS
        },
    );
    let without = cut(&doc, &OPTIONS);
    assert!(without.chunks.iter().all(|c| c.index_text.is_none()));
    let surfaces = with
        .chunks
        .iter()
        .find(|c| c.text.starts_with("## Surfaces"))
        .unwrap();
    let indexed = surfaces.index_text.as_deref().expect("a prefixed text");
    assert!(
        indexed.ends_with(&surfaces.text),
        "the cited text is what follows the prefix"
    );
    assert!(
        indexed.contains("Hygiene > Surfaces"),
        "the section path is in the prefix: {indexed}"
    );
    assert!(
        indexed.contains("guide"),
        "the document title is in the prefix: {indexed}"
    );
    assert!(
        !surfaces.text.contains(" > "),
        "the cited text has no prefix"
    );
}

#[test]
fn ids_are_unique_stable_and_the_page_is_the_page_of_the_first_line() {
    let text = format!("# One\n{}", body("a", 40));
    let doc = document(vec![
        page(1, &text[..text.len() / 2]),
        page(2, &text[text.len() / 2..]),
    ]);
    let first = cut(&doc, &OPTIONS);
    let second = cut(&doc, &OPTIONS);
    let ids: Vec<&str> = first.chunks.iter().map(|c| c.chunk_id.as_str()).collect();
    let again: Vec<&str> = second.chunks.iter().map(|c| c.chunk_id.as_str()).collect();
    assert_eq!(ids, again, "deterministic");
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), ids.len());
    for chunk in &first.chunks {
        assert!(
            chunk
                .chunk_id
                .starts_with(&format!("docs/guide.md#p{}#s", chunk.page_number)),
            "{}",
            chunk.chunk_id
        );
    }
    assert!(
        first.chunks.iter().any(|c| c.page_number == 2),
        "a passage starts on the second page"
    );
}

#[test]
fn an_ordinary_passage_is_never_longer_than_the_size_asked() {
    let text = format!("# One\n{}", body("a", 60));
    let doc = document(vec![page(1, &text)]);
    for max in [800usize, 1_200, 1_600] {
        let out = cut(
            &doc,
            &LongOptions {
                max_chars: max,
                ..OPTIONS
            },
        );
        assert!(out.chunks.len() >= 2);
        for chunk in &out.chunks {
            assert!(
                chunk.text.chars().count() <= max,
                "{max}: {}",
                chunk.text.chars().count()
            );
        }
    }
}

#[test]
fn a_section_boundary_whose_heading_line_was_not_found_never_cuts_a_table() {
    use assistant_cabinet_ai_lib::structure::OutlineEntry;
    let header = "Site | Delai (jours) | Tarif (CHF)";
    let rows: Vec<String> = (1..=7)
        .map(|n| format!("Site{n} | {} | {}", n * 3, n * 40))
        .collect();
    let page_one = format!("{}\n{header}", body("a", 9));
    let page_two = format!("{}\n{}", rows.join("\n"), body("b", 9));
    let doc = document(vec![page(1, &page_one), page(2, &page_two)]);
    let outline = vec![
        OutlineEntry {
            level: 1,
            title: "First".into(),
            page: 1,
        },
        OutlineEntry {
            level: 1,
            title: "A heading the text does not show".into(),
            page: 2,
        },
    ];
    let sections = detect("pdf", &doc, &outline, &[]);
    let out = chunk_long(&doc, &sections, &OPTIONS);
    let holder = out.chunks.iter().find(|c| c.text.contains(header)).unwrap();
    assert!(
        holder.text.contains(rows.last().unwrap().as_str()),
        "{}",
        holder.text
    );
}

// Lot A of the retrieval-quality programme: a passage that runs over a page break says where it ends,
// so a fact after the break is cited on its own page.

#[test]
fn a_table_split_over_a_page_break_reports_the_page_where_it_ends() {
    let header = "Site | Delai (jours) | Tarif (CHF)";
    let rows: Vec<String> = (1..=7)
        .map(|n| format!("Site{n} | {} | {}", n * 3, n * 40))
        .collect();
    let page_one = format!("# Tables\n{}\n{header}", body("a", 9));
    let page_two = format!("{}\n{}", rows.join("\n"), body("b", 9));
    let doc = document(vec![page(1, &page_one), page(2, &page_two)]);
    let out = cut(&doc, &OPTIONS);
    let holder = out
        .chunks
        .iter()
        .find(|c| c.text.contains(header))
        .expect("the header is somewhere");
    assert_eq!(holder.page_number, 1);
    assert!(
        out.ends.contains(&(holder.chunk_id.clone(), 2)),
        "the passage ends on page 2: {:?}",
        out.ends
    );
}

#[test]
fn every_passage_reports_the_last_page_it_reaches_and_only_the_crossing_ones_are_listed() {
    // Three pages of plain sentences, each tagged with its page. A sentence is left open at the end of
    // a page and finished on the next, so a passage holds both: a passage that holds a sentence of
    // page 2 reaches page 2, whatever page it starts on.
    let pages: Vec<ExtractedPage> = ["uno", "dos", "tres"]
        .iter()
        .enumerate()
        .map(|(n, tag)| {
            let mut text = String::new();
            if n > 0 {
                text.push_str(&format!(
                    "and the sentence goes on in the part {tag} until it ends here.
"
                ));
            }
            text.push_str(&body(tag, 8));
            if n < 2 {
                text.push_str(&format!(
                    "
the sentence of the part {tag} runs on without a stop at the end of the page"
                ));
            }
            page(n as u32 + 1, &text)
        })
        .collect();
    let doc = document(pages);
    let out = cut(&doc, &OPTIONS);
    let mut crossing = 0;
    for chunk in &out.chunks {
        let reached = [("part uno ", 1u32), ("part dos ", 2), ("part tres ", 3)]
            .iter()
            .filter(|(tag, _)| chunk.text.contains(tag))
            .map(|(_, page)| *page)
            .max()
            .expect("every passage holds a sentence");
        let listed = out
            .ends
            .iter()
            .find(|(id, _)| *id == chunk.chunk_id)
            .map(|(_, last)| *last);
        assert_eq!(
            listed.unwrap_or(chunk.page_number),
            reached,
            "the last page of {}",
            chunk.chunk_id
        );
        if reached != chunk.page_number {
            crossing += 1;
            assert!(listed.is_some(), "a crossing passage is listed");
        } else {
            assert!(listed.is_none(), "a passage on one page is not listed");
        }
    }
    assert!(
        crossing >= 1,
        "the fixture must have a passage over a break"
    );
}
