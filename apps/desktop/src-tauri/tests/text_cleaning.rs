//! Lot 1 of the long-documents programme: the clean-text pass for documents above the long threshold.
//! Every text is invented.

use assistant_cabinet_ai_lib::chunking::{is_long, LONG_DOCUMENT_CHARS};
use assistant_cabinet_ai_lib::cleaning::clean;
use assistant_cabinet_ai_lib::extraction::{ExtractedDocument, ExtractedPage, PageOrigin};

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
        relative_path: "docs/book.pdf".into(),
        pages,
        empty: false,
        used_ocr: false,
        low_confidence: false,
    }
}

/// A body line that differs on every page, so nothing in it can be mistaken for a repeated line.
fn body(number: u32) -> String {
    let mut lines = Vec::new();
    for line in 0..12 {
        lines.push(format!(
            "Paragraph {number} line {line}: the registry number {} is archived under code {}-{} by the team.",
            number * 31 + line,
            number,
            line * 7
        ));
    }
    lines.join("\n")
}

/// A document above the threshold, 60 pages, with `wrap` around each page's body.
fn long_document(wrap: impl Fn(u32, String) -> String) -> ExtractedDocument {
    let pages: Vec<ExtractedPage> = (1..=60).map(|n| page(n, &wrap(n, body(n)))).collect();
    let doc = document(pages);
    assert!(is_long(&doc), "the fixture must be above the threshold");
    doc
}

fn texts(doc: &ExtractedDocument) -> Vec<String> {
    doc.pages.iter().map(|p| p.text.clone()).collect()
}

#[test]
fn the_threshold_is_counted_in_characters() {
    assert_eq!(LONG_DOCUMENT_CHARS, 60_000);
    assert!(!is_long(&document(vec![page(1, &"x".repeat(60_000))])));
    assert!(is_long(&document(vec![page(1, &"x".repeat(60_001))])));
    assert!(is_long(&document(vec![
        page(1, &"x".repeat(30_000)),
        page(2, &"x".repeat(30_001))
    ])));
}

#[test]
fn a_short_document_is_returned_untouched_even_if_it_looks_dirty() {
    let pages: Vec<ExtractedPage> = (1..=6)
        .map(|n| {
            page(
                n,
                &format!("Running header\nbody of page {n} with a cut wor-\nd here\nPage {n} / 6"),
            )
        })
        .collect();
    let doc = document(pages);
    assert!(!is_long(&doc));
    let before = texts(&doc);
    assert_eq!(texts(&clean(doc)), before);
}

#[test]
fn a_header_and_a_numbered_footer_repeated_on_every_page_are_removed() {
    let doc =
        long_document(|n, b| format!("Quality guide - version 3.2 - internal\n{b}\nPage {n} / 60"));
    let cleaned = clean(doc);
    for (index, p) in cleaned.pages.iter().enumerate() {
        let n = index as u32 + 1;
        assert!(!p.text.contains("Quality guide"), "page {n}: {}", p.text);
        assert!(!p.text.contains("Page "), "page {n}");
        assert_eq!(p.text, body(n), "the body is untouched on page {n}");
    }
}

#[test]
fn a_line_that_is_on_a_few_pages_only_is_kept() {
    let doc = long_document(|n, b| {
        if n == 5 || n == 9 {
            format!("Annex B\n{b}")
        } else {
            b
        }
    });
    let cleaned = clean(doc);
    assert!(cleaned.pages[4].text.starts_with("Annex B"));
    assert!(cleaned.pages[8].text.starts_with("Annex B"));
}

#[test]
fn a_word_cut_at_the_end_of_a_line_is_joined() {
    let doc = long_document(|n, b| {
        if n == 7 {
            format!("{b}\nThe yearly declaration of pharmaco-\nvigilance is signed each year by Odile Pasquereau.")
        } else {
            b
        }
    });
    let text = clean(doc).pages[6].text.clone();
    assert!(
        text.contains(
            "The yearly declaration of pharmacovigilance\nis signed each year by Odile Pasquereau."
        ),
        "{text}"
    );
    assert!(!text.contains("pharmaco-"));
}

#[test]
fn a_word_cut_across_a_page_break_is_joined_on_the_page_where_it_starts() {
    let doc = long_document(|n, b| match n {
        10 => format!("{b}\nThe yearly declaration of radio-"),
        11 => format!("protection is signed by Armand Barbaroux.\n{b}"),
        _ => b,
    });
    let cleaned = clean(doc);
    assert!(
        cleaned.pages[9]
            .text
            .ends_with("The yearly declaration of radioprotection"),
        "{}",
        cleaned.pages[9].text
    );
    assert!(
        cleaned.pages[10]
            .text
            .starts_with("is signed by Armand Barbaroux.\n"),
        "{}",
        cleaned.pages[10].text
    );
}

#[test]
fn a_real_hyphen_is_kept() {
    let doc = long_document(|n, b| {
        if n == 3 {
            format!("{b}\nIt can peut-\netre be done.\nA name such as Jean-\nPierre stays.\nThe number 12-\n34 stays.")
        } else {
            b
        }
    });
    let text = clean(doc).pages[2].text.clone();
    assert!(text.contains("peut-\netre"), "{text}");
    assert!(text.contains("Jean-\nPierre"), "{text}");
    assert!(text.contains("12-\n34"), "{text}");
}

#[test]
fn ligatures_unusual_spaces_and_decomposed_accents_are_normalised() {
    let doc = long_document(|n, b| {
        if n == 2 {
            format!("{b}\nThe o\u{FB03}ce \u{FB01}le costs 12\u{202F}345\u{00A0}CHF, caf\u{0065}\u{0301} done.")
        } else {
            b
        }
    });
    let text = clean(doc).pages[1].text.clone();
    assert!(
        text.contains("The office file costs 12 345 CHF, caf\u{00E9} done."),
        "{text}"
    );
}

#[test]
fn lines_with_no_letter_or_digit_are_dropped() {
    let doc = long_document(|n, b| {
        if n == 4 {
            format!("{b}\n-----------\n. . . . .\nreal line")
        } else {
            b
        }
    });
    let text = clean(doc).pages[3].text.clone();
    assert!(!text.contains("-----"));
    assert!(!text.contains(". . ."));
    assert!(text.ends_with("real line"));
}
