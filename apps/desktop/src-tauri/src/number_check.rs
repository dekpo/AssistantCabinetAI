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

/// The numbers of `answer` that equal no number written in any of `sources` (the excerpts, and the
/// user's own question). Equal means the same value, however written: "1200" matches "1 200,00".
/// Citation markers are ignored. Each distinct number once, in the order written.
pub fn unsupported_numbers(answer: &str, sources: &[&str], min_digits: usize) -> Vec<String> {
    let known: Vec<Number> = sources.iter().flat_map(|source| scan(source, 1)).collect();
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

#[cfg(test)]
mod tests {
    use super::*;

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
