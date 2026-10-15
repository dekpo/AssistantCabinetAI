//! Lot E of the retrieval-quality programme: what an answer quotes between guillemets must be in the passages
//! it was written from, and a reference to a passage ([3]) must be to a passage that exists. Neither is
//! rewritten: the anomaly is reported beside the answer and the reader decides.

use assistant_cabinet_ai_lib::quotation::{
    normalise, quotations_in, unknown_passage_numbers, unverified_quotations,
};

const PASSAGE: &str = "Votre franchise annuelle est de 300 francs.\nElle est r\u{e9}duite de moiti\u{e9}\npour les enfants de moins de 18 ans.";

#[test]
fn quotes_spaces_dashes_and_line_breaks_are_one_form() {
    assert_eq!(
        normalise("  L\u{2019}assur\u{e9}\u{a0}a \u{ab} dit \u{bb}\n\u{2013} ceci\t "),
        "L'assur\u{e9} a \" dit \" - ceci"
    );
    assert_eq!(normalise("a\u{ad}b"), "ab", "a soft hyphen is nothing");
}

#[test]
fn a_quotation_is_what_stands_between_guillemets_or_double_quotes_and_has_four_words() {
    let answer = "Le texte dit \u{ab}\u{a0}La franchise annuelle est de 300 francs\u{a0}\u{bb} et \u{ab} oui \u{bb}, puis \"elle est r\u{e9}duite de moiti\u{e9}\".";
    assert_eq!(
        quotations_in(answer, 4),
        vec![
            "La franchise annuelle est de 300 francs".to_string(),
            "elle est r\u{e9}duite de moiti\u{e9}".to_string()
        ],
        "a one-word mark like \u{ab} oui \u{bb} is not a quotation"
    );
}

#[test]
fn a_quotation_that_runs_over_a_line_break_and_a_different_apostrophe_is_still_found() {
    let answer = "Il est \u{e9}crit \u{ab} elle est r\u{e9}duite de moiti\u{e9} pour les enfants de moins de 18 ans \u{bb}.";
    assert!(unverified_quotations(answer, &[PASSAGE], 4).is_empty());
    let curly = "Il est \u{e9}crit \u{ab} votre franchise annuelle est de 300 francs \u{bb}.";
    assert!(unverified_quotations(curly, &[PASSAGE], 4).is_empty());
}

#[test]
fn an_invented_or_altered_quotation_is_reported_as_written() {
    let answer = "Le texte dit \u{ab} la franchise annuelle est de 500 francs \u{bb}.";
    assert_eq!(
        unverified_quotations(answer, &[PASSAGE], 4),
        vec!["la franchise annuelle est de 500 francs".to_string()]
    );
}

#[test]
fn a_quotation_with_an_ellipsis_is_checked_piece_by_piece() {
    let answer = "Il dit \u{ab} Votre franchise annuelle est de 300 francs [\u{2026}] pour les enfants de moins de 18 ans \u{bb}.";
    assert!(unverified_quotations(answer, &[PASSAGE], 4).is_empty());
}

#[test]
fn a_reference_to_a_passage_that_does_not_exist_is_reported() {
    assert_eq!(
        unknown_passage_numbers("Selon [1] et [3], puis [9] et [9].", 4),
        vec![9]
    );
    assert_eq!(unknown_passage_numbers("Voir [0].", 4), vec![0]);
    assert!(unknown_passage_numbers("Voir [2] et [4].", 4).is_empty());
    assert!(unknown_passage_numbers("Aucun renvoi, ni [a] ni [].", 4).is_empty());
}

#[test]
fn the_figure_check_and_the_quotation_check_read_a_broken_line_the_same_way() {
    // the shared normaliser: a figure cut by a page break is whole once the lines are joined
    use assistant_cabinet_ai_lib::number_check::unsupported_numbers;
    let source = "Num\u{e9}ro 756 3052\n5778 43 du patient";
    assert!(unsupported_numbers("Le num\u{e9}ro 756 3052 5778 43", &[source], 3).is_empty());
    assert!(unverified_quotations(
        "\u{ab} Num\u{e9}ro 756 3052 5778 43 du patient \u{bb}",
        &[source],
        4
    )
    .is_empty());
}
