//! Lot 2 of the long-documents programme: the sections of a document and the section of each passage.
//! Every text is invented.

use std::io::Write;

use assistant_cabinet_ai_lib::chunking::chunk;
use assistant_cabinet_ai_lib::extraction::{ExtractedDocument, ExtractedPage, PageOrigin};
use assistant_cabinet_ai_lib::structure::{
    assign, build, detect, docx_headings, pdf_outline, OutlineEntry, SectionSource, StyledHeading,
};

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
        relative_path: "docs/book".into(),
        pages,
        empty: false,
        used_ocr: false,
        low_confidence: false,
    }
}

/// `count` body lines of 100 characters that cannot be taken for a heading.
fn filler(tag: &str, count: usize) -> String {
    (0..count)
        .map(|i| format!("the ordinary sentence number {i} of the part {tag} goes on with the usual words and ends here."))
        .collect::<Vec<_>>()
        .join("\n")
}

// ------------------------------------------------------------------------------------ sources of titles

#[test]
fn markdown_hash_headings_give_levels_parents_and_anchors() {
    let text = format!(
        "# Title of the book\n{}\n## First part\n{}\n## Second part\n{}\n# Another chapter\n{}",
        filler("a", 3),
        filler("b", 3),
        filler("c", 3),
        filler("d", 3)
    );
    let doc = document(vec![page(1, &text)]);
    let sections = detect("md", &doc, &[], &[]);
    let titles: Vec<(&str, u8, Option<usize>)> = sections
        .iter()
        .map(|s| (s.title.as_str(), s.level, s.parent))
        .collect();
    assert_eq!(
        titles,
        vec![
            ("Title of the book", 1, None),
            ("First part", 2, Some(0)),
            ("Second part", 2, Some(0)),
            ("Another chapter", 1, None)
        ]
    );
    assert!(sections.iter().all(|s| s.source == SectionSource::Markdown));
    assert_eq!(
        (sections[1].anchor_page, sections[1].anchor_paragraph),
        (1, 4)
    );
}

#[test]
fn a_hash_inside_a_code_fence_is_not_a_heading() {
    let text = format!(
        "# Real\n{}\n```\n# not a heading\n```\n## Also real\n{}",
        filler("a", 2),
        filler("b", 2)
    );
    let sections = detect("md", &document(vec![page(1, &text)]), &[], &[]);
    let titles: Vec<&str> = sections.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, vec!["Real", "Also real"]);
}

#[test]
fn numbered_headings_and_chapter_words_are_found_by_heuristics_in_plain_text() {
    let text = format!(
        "Chapitre 1 \u{2014} Hygi\u{e8}ne\n1.1 Principes g\u{e9}n\u{e9}raux\n{}\n1.2 Contr\u{f4}les\n{}\nChapitre 2 \u{2014} D\u{e9}chets\n2.1 Tri\n{}",
        filler("a", 3),
        filler("b", 3),
        filler("c", 3)
    );
    let sections = detect("txt", &document(vec![page(1, &text)]), &[], &[]);
    let found: Vec<(u8, &str)> = sections
        .iter()
        .map(|s| (s.level, s.title.as_str()))
        .collect();
    assert_eq!(
        found,
        vec![
            (1, "Chapitre 1 \u{2014} Hygi\u{e8}ne"),
            (2, "1.1 Principes g\u{e9}n\u{e9}raux"),
            (2, "1.2 Contr\u{f4}les"),
            (1, "Chapitre 2 \u{2014} D\u{e9}chets"),
            (2, "2.1 Tri")
        ]
    );
    assert!(sections
        .iter()
        .all(|s| s.source == SectionSource::Heuristic));
    assert_eq!(sections[2].parent, Some(0));
    assert_eq!(sections[4].parent, Some(3));
}

#[test]
fn a_list_item_a_sentence_and_one_isolated_capital_line_are_not_headings() {
    let text = format!(
        "1. V\u{e9}rifier le niveau des r\u{e9}serves\n2. Consigner la date\n{}\nRepublique Francaise.\n3.5 est le taux retenu pour ce calcul, voir la note de bas de page.\n{}",
        filler("a", 4),
        filler("b", 4)
    );
    assert!(detect("txt", &document(vec![page(1, &text)]), &[], &[]).is_empty());
}

#[test]
fn a_pdf_outline_gives_the_pages_and_the_last_page_of_each_section() {
    let pages: Vec<ExtractedPage> = (1..=30)
        .map(|n| page(n, &filler(&n.to_string(), 4)))
        .collect();
    let outline = vec![
        OutlineEntry {
            level: 1,
            title: "Intro".into(),
            page: 1,
        },
        OutlineEntry {
            level: 2,
            title: "Aims".into(),
            page: 3,
        },
        OutlineEntry {
            level: 1,
            title: "Method".into(),
            page: 12,
        },
        OutlineEntry {
            level: 1,
            title: "Results".into(),
            page: 25,
        },
    ];
    let sections = detect("pdf", &document(pages), &outline, &[]);
    let ranges: Vec<(&str, u32, u32)> = sections
        .iter()
        .map(|s| (s.title.as_str(), s.first_page, s.last_page))
        .collect();
    assert_eq!(
        ranges,
        vec![
            ("Intro", 1, 11),
            ("Aims", 3, 11),
            ("Method", 12, 24),
            ("Results", 25, 30)
        ]
    );
    assert_eq!(sections[1].parent, Some(0));
    assert!(sections.iter().all(|s| s.source == SectionSource::Outline));
}

#[test]
fn word_heading_styles_are_located_in_the_text_in_order() {
    let text = format!(
        "Manual\nFirst chapter\n{}\nSecond chapter\n{}\nA sub part\n{}",
        filler("a", 3),
        filler("b", 3),
        filler("c", 2)
    );
    let styled = vec![
        StyledHeading {
            level: 1,
            title: "First chapter".into(),
        },
        StyledHeading {
            level: 1,
            title: "Second chapter".into(),
        },
        StyledHeading {
            level: 2,
            title: "A sub part".into(),
        },
    ];
    let sections = detect("docx", &document(vec![page(1, &text)]), &[], &styled);
    let found: Vec<(u8, &str, usize)> = sections
        .iter()
        .map(|s| (s.level, s.title.as_str(), s.anchor_paragraph))
        .collect();
    assert_eq!(
        found,
        vec![
            (1, "First chapter", 1),
            (1, "Second chapter", 5),
            (2, "A sub part", 9)
        ]
    );
    assert!(sections.iter().all(|s| s.source == SectionSource::Style));
}

#[test]
fn a_long_document_without_any_heading_is_sliced_by_pages_and_a_short_one_gets_nothing() {
    let pages: Vec<ExtractedPage> = (1..=50)
        .map(|n| page(n, &filler(&n.to_string(), 16)))
        .collect();
    let long = document(pages);
    assert!(assistant_cabinet_ai_lib::chunking::is_long(&long));
    let sections = detect("pdf", &long, &[], &[]);
    let ranges: Vec<(u32, u32)> = sections
        .iter()
        .map(|s| (s.first_page, s.last_page))
        .collect();
    assert_eq!(ranges, vec![(1, 20), (21, 40), (41, 50)]);
    assert!(sections
        .iter()
        .all(|s| s.source == SectionSource::Slice && s.title.is_empty()));

    let short = document(vec![page(1, &filler("x", 3))]);
    assert!(detect("pdf", &short, &[], &[]).is_empty());
}

#[test]
fn one_heading_alone_is_not_a_structure_in_a_short_document() {
    let doc = document(vec![page(1, &format!("# Only title\n{}", filler("a", 3)))]);
    assert!(detect("md", &doc, &[], &[]).is_empty());
}

// ------------------------------------------------------------------------------------ passages

#[test]
fn a_passage_belongs_to_the_section_in_effect_where_it_starts() {
    let mut body = String::new();
    body.push_str("# One\n");
    body.push_str(&filler("a", 14));
    body.push_str("\n# Two\n");
    body.push_str(&filler("b", 30));
    let doc = document(vec![page(1, &body)]);
    let sections = detect("md", &doc, &[], &[]);
    assert_eq!(sections.len(), 2);
    let chunks = chunk(&doc);
    let owners = assign(&sections, &doc, &chunks);
    assert_eq!(owners.len(), chunks.len());
    assert_eq!(
        owners[0].1, 0,
        "the first passage starts at the first heading"
    );
    assert_eq!(
        owners.last().unwrap().1,
        1,
        "the last passage is in the second section"
    );
    // positions never go back
    assert!(owners.windows(2).all(|w| w[0].1 <= w[1].1));
}

#[test]
fn passages_on_a_later_page_follow_the_outline() {
    let pages: Vec<ExtractedPage> = (1..=6)
        .map(|n| page(n, &filler(&n.to_string(), 4)))
        .collect();
    let doc = document(pages);
    let outline = vec![
        OutlineEntry {
            level: 1,
            title: "A".into(),
            page: 1,
        },
        OutlineEntry {
            level: 1,
            title: "B".into(),
            page: 4,
        },
    ];
    let sections = detect("pdf", &doc, &outline, &[]);
    let chunks = chunk(&doc);
    let owners = assign(&sections, &doc, &chunks);
    assert_eq!(owners.len(), chunks.len());
    for (chunk, (_, position)) in chunks.iter().zip(&owners) {
        assert_eq!(
            *position,
            usize::from(chunk.page_number >= 4),
            "{}",
            chunk.chunk_id
        );
    }
}

// ------------------------------------------------------------------------------------ files

fn minimal_pdf(titles: &[(&str, usize)]) -> Vec<u8> {
    // objects: 1 font, then per page (content, page), pages, outlines root, items, catalog
    let page_count = 3usize;
    let mut objects: Vec<Vec<u8>> =
        vec![b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec()];
    for n in 1..=page_count {
        let stream = format!("BT /F1 12 Tf 50 700 Td (page {n}) Tj ET");
        objects.push(
            format!(
                "<< /Length {} >>\nstream\n{stream}\nendstream",
                stream.len()
            )
            .into_bytes(),
        );
        objects.push(
            format!(
                "<< /Type /Page /Parent {} 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 1 0 R >> >> /Contents {} 0 R >>",
                2 + 2 * page_count,
                2 * n
            )
            .into_bytes(),
        );
    }
    let pages_obj = 2 + 2 * page_count;
    let kids: Vec<String> = (1..=page_count)
        .map(|n| format!("{} 0 R", 1 + 2 * n))
        .collect();
    objects.push(
        format!(
            "<< /Type /Pages /Kids [{}] /Count {page_count} >>",
            kids.join(" ")
        )
        .into_bytes(),
    );
    let outlines_obj = pages_obj + 1;
    let first_item = outlines_obj + 1;
    objects.push(
        format!(
            "<< /Type /Outlines /First {first_item} 0 R /Last {} 0 R /Count {} >>",
            first_item + titles.len() - 1,
            titles.len()
        )
        .into_bytes(),
    );
    for (i, (title, page)) in titles.iter().enumerate() {
        let mut body = b"<< /Title (".to_vec();
        body.extend_from_slice(&latin1(title));
        body.extend_from_slice(
            format!(
                ") /Parent {outlines_obj} 0 R /Dest [{} 0 R /XYZ 0 800 null] ",
                1 + 2 * page
            )
            .as_bytes(),
        );
        if i > 0 {
            body.extend_from_slice(format!("/Prev {} 0 R ", first_item + i - 1).as_bytes());
        }
        if i + 1 < titles.len() {
            body.extend_from_slice(format!("/Next {} 0 R ", first_item + i + 1).as_bytes());
        }
        body.extend_from_slice(b">>");
        objects.push(body);
    }
    let catalog = first_item + titles.len();
    objects.push(
        format!("<< /Type /Catalog /Pages {pages_obj} 0 R /Outlines {outlines_obj} 0 R >>")
            .into_bytes(),
    );
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root {catalog} 0 R >>\nstartxref\n{xref}\n%%EOF",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

fn latin1(text: &str) -> Vec<u8> {
    text.chars().map(|c| c as u32 as u8).collect()
}

#[test]
fn the_outline_of_a_real_pdf_is_read_with_its_accents_and_its_duplicate_titles() {
    let bytes = minimal_pdf(&[
        ("Introduction", 1),
        ("D\u{e9}chets", 2),
        ("Introduction", 3),
    ]);
    let entries = pdf_outline(&bytes);
    let found: Vec<(&str, u32)> = entries.iter().map(|e| (e.title.as_str(), e.page)).collect();
    assert_eq!(
        found,
        vec![
            ("Introduction", 1),
            ("D\u{e9}chets", 2),
            ("Introduction", 3)
        ]
    );
    assert!(pdf_outline(b"not a pdf at all").is_empty());
}

#[test]
fn the_heading_styles_of_a_real_docx_are_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("manual.docx");
    let ns = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    let body = [
        ("Title", "The manual"),
        ("Heading1", "First chapter"),
        ("", "Some body text."),
        ("Heading2", "A sub part & more"),
        ("Titre1", "French style"),
    ]
    .iter()
    .map(|(style, text)| {
        let props = if style.is_empty() {
            String::new()
        } else {
            format!("<w:pPr><w:pStyle w:val=\"{style}\"/></w:pPr>")
        };
        format!(
            "<w:p>{props}<w:r><w:t>{}</w:t></w:r></w:p>",
            text.replace('&', "&amp;")
        )
    })
    .collect::<String>();
    let xml = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><w:document xmlns:w=\"{ns}\"><w:body>{body}</w:body></w:document>");
    let file = std::fs::File::create(&path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file(
        "word/document.xml",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(xml.as_bytes()).unwrap();
    zip.finish().unwrap();
    let headings = docx_headings(&path);
    let found: Vec<(u8, &str)> = headings
        .iter()
        .map(|h| (h.level, h.title.as_str()))
        .collect();
    assert_eq!(
        found,
        vec![
            (1, "First chapter"),
            (2, "A sub part & more"),
            (1, "French style")
        ]
    );
}

#[test]
fn build_stores_nothing_for_a_short_unstructured_document_and_everything_for_a_structured_one() {
    let short = document(vec![page(1, "A short letter.\nWith two lines.")]);
    let built = build(
        std::path::Path::new("none.txt"),
        "txt",
        &short,
        &chunk(&short),
    );
    assert!(built.sections.is_empty() && built.chunk_sections.is_empty());

    let text = format!("# One\n{}\n# Two\n{}", filler("a", 6), filler("b", 6));
    let doc = document(vec![page(1, &text)]);
    let chunks = chunk(&doc);
    let built = build(std::path::Path::new("none.md"), "md", &doc, &chunks);
    assert_eq!(built.sections.len(), 2);
    assert_eq!(built.chunk_sections.len(), chunks.len());
}

#[test]
fn an_outline_title_is_found_in_the_page_even_when_the_extraction_garbled_a_character() {
    // The extraction read the long dash of the title as another letter.
    let mut page_two = filler("two", 3);
    page_two.push_str("\nChapitre 4 \u{160} Hygi\u{e8}ne\n");
    page_two.push_str(&filler("two-b", 3));
    let doc = document(vec![page(1, &filler("one", 4)), page(2, &page_two)]);
    let outline = vec![
        OutlineEntry {
            level: 1,
            title: "Intro".into(),
            page: 1,
        },
        OutlineEntry {
            level: 1,
            title: "Chapitre 4 \u{2014} Hygi\u{e8}ne".into(),
            page: 2,
        },
    ];
    let sections = detect("pdf", &doc, &outline, &[]);
    assert_eq!(
        sections[1].anchor_paragraph, 3,
        "found on its line, not assumed at the top of the page"
    );
    assert!(sections[1].located);
}

#[test]
fn an_outline_title_that_is_nowhere_on_the_page_is_a_boundary_but_not_a_line() {
    let doc = document(vec![
        page(
            1,
            &format!(
                "Intro
{}",
                filler("one", 4)
            ),
        ),
        page(2, &filler("two", 4)),
    ]);
    let outline = vec![
        OutlineEntry {
            level: 1,
            title: "Intro".into(),
            page: 1,
        },
        OutlineEntry {
            level: 1,
            title: "A title the page does not show".into(),
            page: 2,
        },
    ];
    let sections = detect("pdf", &doc, &outline, &[]);
    assert_eq!(
        (sections[1].anchor_page, sections[1].anchor_paragraph),
        (2, 0)
    );
    assert!(!sections[1].located, "no line of the page is this heading");
    assert!(sections[0].located);
}
