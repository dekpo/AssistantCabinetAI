//! Reading the numbers a model wrote, the way a person writes them, to check them against what the
//! model was given (HAP-1, lot D follow-up; the owner approved the check for document answers on
//! 5 October 2026).
//!
//! Two callers share this: the mixed tier, which compares a model's figures with the engine's
//! (`mixed_answer::verify_numbers`), and a document answer, which compares them with the excerpts it
//! was written from (`unsupported_numbers`). Neither rewrites the answer: a figure that matches
//! nothing is reported beside it, and the reader decides.
//!
//! **A number is read whole.** Some languages group thousands with a space and write the decimal with
//! a comma: "1 450,00" is one amount. Splitting it at the space made the check report "450,00" as
//! contradicting a table value of 1 450, a false correction shown under three correct answers in the
//! lot D replay. A group of exactly three digits after a group of one to three, separated by a space
//! (ordinary, no-break or narrow no-break), belongs to the number.

/// One number found in a text: as written, and the values it can mean.
#[derive(Debug, Clone, PartialEq)]
pub struct Number {
    /// Exactly as it appears, spaces and separators included.
    pub text: String,
    /// Usually one value. Two for "1,500" and "1.500", which are a thousand in one language and one
    /// and a half in the other: both are accepted when comparing, neither is guessed.
    pub values: Vec<f64>,
}

fn is_group_space(ch: char) -> bool {
    ch == ' ' || ch == '\u{a0}' || ch == '\u{202f}'
}

fn is_mark(ch: char) -> bool {
    ch == ',' || ch == '.'
}

/// Every number in `text` with at least `min_digits` digits, or carrying a decimal separator. A
/// bare one- or two-digit number is a page, a day or a count far more often than a claim, so the
/// mixed tier keeps three; a document answer keeps two, which is what catches a price such as 80.
/// A run that starts right after a letter ("A4", "W1") is an identifier, not a number.
pub fn scan(text: &str, min_digits: usize) -> Vec<Number> {
    let chars: Vec<char> = text.chars().collect();
    let mut found = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if !chars[index].is_ascii_digit() {
            index += 1;
            continue;
        }
        if index > 0 && chars[index - 1].is_alphabetic() {
            // An identifier ("A4", "W123"): the whole run of digits is part of it.
            while index < chars.len() && chars[index].is_ascii_digit() {
                index += 1;
            }
            continue;
        }
        let start = index;
        let mut digits = 0usize;
        let mut marks: Vec<char> = Vec::new();
        let mut grouped = false;
        // Digits in the group being read, and in the last group read.
        let mut group = 0usize;
        loop {
            while index < chars.len() && chars[index].is_ascii_digit() {
                digits += 1;
                group += 1;
                index += 1;
            }
            let at_mark = index + 1 < chars.len() && is_mark(chars[index]) && chars[index + 1].is_ascii_digit();
            if at_mark {
                marks.push(chars[index]);
                index += 1;
                group = 0;
                continue;
            }
            // A space-grouped thousand: one to three digits so far, then exactly three more.
            let at_space = index + 3 < chars.len()
                && is_group_space(chars[index])
                && marks.is_empty()
                && group <= 3
                && chars[index + 1..index + 4].iter().all(|ch| ch.is_ascii_digit())
                && chars.get(index + 4).map_or(true, |next| !next.is_ascii_digit());
            if at_space {
                grouped = true;
                index += 1;
                group = 0;
                continue;
            }
            break;
        }
        if digits < min_digits && marks.is_empty() {
            continue;
        }
        let text: String = chars[start..index].iter().collect();
        let compact: String = text.chars().filter(|ch| !is_group_space(*ch)).collect();
        let values = number_values(&compact, &marks, grouped);
        if !values.is_empty() {
            found.push(Number { text, values });
        }
    }
    found
}

fn number_values(compact: &str, marks: &[char], grouped: bool) -> Vec<f64> {
    let parse = |candidate: &str| candidate.parse::<f64>().ok();
    let mut values = Vec::new();
    match marks {
        [] => values.extend(parse(compact)),
        [only] => {
            let mut parts = compact.split(*only);
            let head = parts.next().unwrap_or("");
            let tail = parts.next().unwrap_or("");
            // "1,500" and "1.500": a thousand, or one and a half. Both are kept. A space-grouped
            // integer part ("1 500,00") leaves no doubt: the mark is the decimal.
            if tail.len() == 3 && head.len() <= 3 && !grouped {
                values.extend(parse(&format!("{head}{tail}")));
            }
            values.extend(parse(&format!("{head}.{tail}")));
        }
        many => {
            // "1.450,00" or "1,450.00": the last mark is the decimal, the others group thousands.
            let last = many[many.len() - 1];
            let cut = compact.rfind(last).unwrap_or(0);
            let integer: String = compact[..cut].chars().filter(|ch| !is_mark(*ch)).collect();
            values.extend(parse(&format!("{integer}.{}", &compact[cut + 1..])));
        }
    }
    values
}

/// `text` with its bracketed citation markers ("[1]", "[12]") blanked out: a marker is not a figure
/// the model claims.
fn without_citations(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '[' {
            let mut end = index + 1;
            while end < chars.len() && chars[end].is_ascii_digit() {
                end += 1;
            }
            if end > index + 1 && end < chars.len() && chars[end] == ']' {
                out.push(' ');
                index = end + 1;
                continue;
            }
        }
        out.push(chars[index]);
        index += 1;
    }
    out
}

/// The numbers a source can be read to write. A passage is cut at a line wherever the page broke it, so
/// a figure grouped by spaces can be split across two lines ("1 85 03 69" then "123 456 78"): the source
/// is read as written and once more with each line break taken for a space, and both readings count.
fn scan_source(source: &str) -> Vec<Number> {
    let mut numbers = scan(source, 1);
    if source.contains(['\n', '\r']) {
        let joined = source.replace("\r\n", " ").replace(['\n', '\r'], " ");
        numbers.extend(scan(&joined, 1));
    }
    numbers
}

/// The numbers of `answer` that equal no number written in any of `sources` (the excerpts, and the
/// user's own question). Equal means the same value, however written: "1200" matches "1 200,00".
/// Citation markers are ignored. Each distinct number once, in the order written.
pub fn unsupported_numbers(answer: &str, sources: &[&str], min_digits: usize) -> Vec<String> {
    let known: Vec<Number> = sources.iter().flat_map(|source| scan_source(source)).collect();
    let mut seen: Vec<String> = Vec::new();
    let mut unsupported = Vec::new();
    for number in scan(&without_citations(answer), min_digits) {
        if seen.contains(&number.text) {
            continue;
        }
        seen.push(number.text.clone());
        let matched = known.iter().any(|other| {
            other.text == number.text
                || other
                    .values
                    .iter()
                    .any(|a| number.values.iter().any(|b| (a - b).abs() < 1e-6))
        });
        if !matched {
            unsupported.push(number.text);
        }
    }
    unsupported
}

/// [`unsupported_numbers`] for a document answer: the page numbers of the excerpts are known figures too.
///
/// A correct citation such as "(page 12)" states a number no excerpt contains, and a model that cites its
/// pages draws a false alarm on every one of two digits or more (KBD-49, P-7). A page number excuses only the
/// bare integer written as the page itself: a decimal amount that happens to share its digits ("200.00" on
/// page 200) is still checked, which is the case that caught `200.00` and `286.63`.
pub fn unsupported_numbers_with_pages(
    answer: &str,
    sources: &[&str],
    pages: &[u32],
    min_digits: usize,
) -> Vec<String> {
    let pages: Vec<String> = pages.iter().map(u32::to_string).collect();
    unsupported_numbers(answer, sources, min_digits)
        .into_iter()
        .filter(|number| !pages.contains(number))
        .collect()
}

/// The explicit arithmetic a model wrote in its answer (`10 + 40 = 50`, `3 x 4 = 12`,
/// `100 - 40 = 60`, `10 + 20 + 30 = 60`): two or more numbers joined by `+`, `-`, `x`, `*` or the
/// multiplication sign, then `=` and a result. A number is read whole (space-grouped thousands,
/// decimal comma or point) and may carry a currency sign or code. The result is returned as the
/// model wrote it, without the unit that follows it.
///
/// A model that adds up the rows of a sample states a total the table does not hold (measured on
/// 22 October 2026: `Alpha : 10 + 40 = 50` for a total of 120), so the interface says, beside the
/// answer and never in it, that only the engine is reliable (TABULAR step 03b, decision 39). An
/// expression with no `=`, a date, a citation, an identifier, a range of pages or a telephone
/// number is not arithmetic, and an equals sign with a single number on its left is not either.
/// Each distinct expression once, in the order written.
pub fn arithmetic_expressions(answer: &str) -> Vec<String> {
    use regex::Regex;
    use std::sync::OnceLock;

    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let pattern = PATTERN.get_or_init(|| {
        let number = r"\d(?:[\d \x{a0}\x{202f}.,]*\d)?";
        let unit = r"(?:[ \x{a0}]?(?:\x{20ac}|\$|EUR|CHF))?";
        let operator = r"[ \x{a0}]*(?:[+\-*]|\x{d7}|\x{2212}|\bx\b)[ \x{a0}]*";
        Regex::new(&format!(
            "{number}{unit}(?:{operator}{number}{unit})+[ \\x{{a0}}]*=[ \\x{{a0}}]*{number}"
        ))
        .expect("the arithmetic pattern is valid")
    });
    let chars_before = |end: usize| answer[..end].chars().next_back();
    let mut found: Vec<String> = Vec::new();
    for hit in pattern.find_iter(answer) {
        // A run that starts inside a word, an identifier, a date or a longer number is not the
        // start of an expression.
        if chars_before(hit.start())
            .is_some_and(|before| before.is_alphanumeric() || "-/.,_#".contains(before))
        {
            continue;
        }
        let expression = hit.as_str().trim().to_string();
        if !found.contains(&expression) {
            found.push(expression);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_arithmetic_in_an_answer_is_found() {
        for (text, expected) in [
            ("Alpha : 10 + 40 = 50 EUR", "10 + 40 = 50"),
            (
                "Total 1 250,50 + 30 = 1 280,50 euros",
                "1 250,50 + 30 = 1 280,50",
            ),
            ("We get 3 x 4 = 12 rows", "3 x 4 = 12"),
            ("so 12 \u{d7} 3 = 36", "12 \u{d7} 3 = 36"),
            ("net: 100 - 40 = 60", "100 - 40 = 60"),
            ("10 + 20 + 30 = 60", "10 + 20 + 30 = 60"),
            (
                "10\u{20ac} + 5\u{20ac} = 15\u{20ac}",
                "10\u{20ac} + 5\u{20ac} = 15",
            ),
            ("10+40=50", "10+40=50"),
        ] {
            assert_eq!(
                arithmetic_expressions(text),
                vec![expected.to_string()],
                "{text:?}"
            );
        }
    }

    #[test]
    fn a_repeated_expression_is_reported_once_and_two_are_both_reported() {
        let found = arithmetic_expressions("10 + 40 = 50, and again 10 + 40 = 50, then 2 x 3 = 6.");
        assert_eq!(found, vec!["10 + 40 = 50", "2 x 3 = 6"]);
    }

    #[test]
    fn what_is_not_arithmetic_is_left_alone() {
        for text in [
            "lignes 2 \u{e0} 10",
            "see rows 2 to 10 of the sheet",
            "the invoice of 02/03/2026 is due",
            "dated 2026-03-02 and 02-03-2026",
            "as stated in [1] and [2]",
            "order CMD-2026-003 is open",
            "3 + 4",
            "call 06 12 34 56 78 or +33 6 12 34 56 78",
            "Total = 120",
            "rate = 7.5 percent",
            "a size of A4 x 2",
            "pages 12 - 14",
        ] {
            assert!(arithmetic_expressions(text).is_empty(), "{text:?}");
        }
    }

    #[test]
    fn a_cited_page_number_is_known_and_a_decimal_that_shares_its_digits_is_not() {
        let excerpts = ["Total HT : 326.30 CHF"];
        let answer =
            "The total is 326.30 CHF (page 12). A deposit of 200.00 was also paid (page 200).";

        // Without the pages both page numbers draw a false alarm.
        assert_eq!(
            unsupported_numbers(answer, &excerpts, 2),
            vec!["12", "200.00", "200"]
        );
        // With them, only the figure no excerpt contains is reported.
        assert_eq!(
            unsupported_numbers_with_pages(answer, &excerpts, &[12, 200], 2),
            vec!["200.00"]
        );
        // A page the excerpts did not come from excuses nothing.
        assert_eq!(
            unsupported_numbers_with_pages("See page 14.", &excerpts, &[12], 2),
            vec!["14"]
        );
    }

    fn texts(found: &[Number]) -> Vec<&str> {
        found.iter().map(|number| number.text.as_str()).collect()
    }

    #[test]
    fn a_space_grouped_amount_is_one_number() {
        for (text, expected, value) in [
            ("Total is 1 450,00 EUR.", "1 450,00", 1450.0),
            ("Sum is 2 215 EUR.", "2 215", 2215.0),
            ("Total: 1\u{a0}200,00 EUR", "1\u{a0}200,00", 1200.0),
            ("Total: 12\u{202f}345 EUR", "12\u{202f}345", 12345.0),
            ("1 234 567 EUR", "1 234 567", 1234567.0),
            ("Total: 2 215", "2 215", 2215.0),
        ] {
            let found = scan(text, 3);
            assert_eq!(texts(&found), vec![expected], "{text:?}");
            assert!(found[0].values.contains(&value), "{text:?}: {:?}", found[0].values);
        }
    }

    #[test]
    fn two_numbers_next_to_each_other_stay_two() {
        assert_eq!(texts(&scan("from 6 July to 31 July 2026", 2)), vec!["31", "2026"]);
        assert_eq!(texts(&scan("pages 12 45", 2)), vec!["12", "45"]);
        // Four digits after a space are a year, not a group.
        assert_eq!(texts(&scan("on 5 2026", 1)), vec!["5", "2026"]);
    }

    #[test]
    fn an_ambiguous_separator_keeps_both_readings() {
        let found = scan("1,500 and 1.500", 3);
        assert_eq!(found.len(), 2);
        for number in &found {
            assert!(number.values.contains(&1500.0) && number.values.contains(&1.5), "{number:?}");
        }
    }

    #[test]
    fn a_space_grouped_integer_part_makes_the_mark_a_decimal() {
        let found = scan("1 500,250", 3);
        assert_eq!(found[0].values, vec![1500.25]);
    }

    #[test]
    fn full_european_and_english_formats_read_the_same() {
        assert!(scan("1.450,00", 3)[0].values.contains(&1450.0));
        assert!(scan("1,450.00", 3)[0].values.contains(&1450.0));
    }

    #[test]
    fn a_short_decimal_counts_and_a_short_integer_does_not_at_three_digits() {
        assert_eq!(texts(&scan("The rate is 7.5 percent.", 3)), vec!["7.5"]);
        assert!(scan("See page 3 for 12 items", 3).is_empty());
        assert_eq!(texts(&scan("a price of 80 euros", 2)), vec!["80"]);
    }

    #[test]
    fn an_identifier_is_not_a_number() {
        assert!(scan("a sheet of A4 and client W12", 2).is_empty());
    }

    #[test]
    fn a_date_in_a_cited_file_name_is_supported_once_the_path_is_known() {
        let answer = "See inbox/2026-03-26_report.pdf for the statement.";
        assert!(!unsupported_numbers(answer, &["Some excerpt."], 2).is_empty());
        assert!(unsupported_numbers(answer, &["Some excerpt.", "inbox/2026-03-26_report.pdf"], 2).is_empty());
    }

    #[test]
    fn an_amount_written_two_ways_is_supported() {
        let sources = ["Quote total: 1 200,00 EUR."];
        assert!(unsupported_numbers("The quote is 1200 EUR.", &sources, 2).is_empty());
        assert!(unsupported_numbers("The quote is 1 200,00 EUR.", &sources, 2).is_empty());
    }

    #[test]
    fn a_figure_in_no_source_is_reported_once() {
        let sources = ["Quote total: 1 200,00 EUR."];
        let found = unsupported_numbers("The price is 80 EUR, so 80 EUR.", &sources, 2);
        assert_eq!(found, vec!["80".to_string()]);
    }

    #[test]
    fn a_citation_marker_and_the_users_own_number_are_not_claims() {
        let sources = ["The lease lasts nine years.", "Question: and for 2027?"];
        assert!(unsupported_numbers("According to [12] and [3], for 2027, nine years.", &sources, 2).is_empty());
    }
}
