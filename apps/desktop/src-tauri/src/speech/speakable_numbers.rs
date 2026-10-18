//! Numbers as words, from the data of a rule pack. No language is named here: the words, the
//! plural rules and the shape of the ordinals and of the years all come from [`Rules`].

use super::speakable_rules::{Rules, YearStyle};

/// Largest number said as a word; longer digit strings are read one digit at a time.
pub const MAX_DIGITS: usize = 12;

/// The words of a number below one hundred. `plural_final` selects the form a number takes when
/// nothing follows it and the plural applies ("quatre-vingts").
fn below_hundred(rules: &Rules, n: u64, plural_final: bool) -> String {
    let numbers = &rules.numbers;
    if plural_final {
        if let Some(form) = numbers.final_forms.get(&n.to_string()) {
            return form.clone();
        }
    }
    numbers.below_hundred[n as usize % 100].clone()
}

fn below_thousand(rules: &Rules, n: u64, plural_final: bool) -> String {
    let hundreds = n / 100;
    let rest = n % 100;
    if hundreds == 0 {
        return below_hundred(rules, rest, plural_final);
    }
    let rule = &rules.numbers.hundred;
    let mut spoken = if hundreds == 1 && rule.one_omitted {
        rule.word.clone()
    } else {
        format!(
            "{} {}",
            rules.numbers.below_hundred[hundreds as usize], rule.word
        )
    };
    if rest == 0 {
        if hundreds > 1 && plural_final {
            if let Some(plural) = &rule.plural {
                spoken = format!(
                    "{} {}",
                    rules.numbers.below_hundred[hundreds as usize], plural
                );
            }
        }
    } else {
        spoken.push(' ');
        spoken.push_str(&below_hundred(rules, rest, plural_final));
    }
    spoken
}

/// A whole number below one trillion as words. `plural_final` is true for a plain quantity and
/// false when the number becomes the base of an ordinal.
pub fn cardinal_with(rules: &Rules, n: u64, plural_final: bool) -> String {
    if n == 0 {
        return rules.numbers.below_hundred[0].clone();
    }
    let mut scales: Vec<_> = rules.numbers.scales.iter().collect();
    scales.sort_by_key(|scale| std::cmp::Reverse(scale.value));
    let mut rest = n;
    let mut parts: Vec<String> = Vec::new();
    for scale in scales {
        if scale.value == 0 {
            continue;
        }
        let quotient = rest / scale.value;
        rest %= scale.value;
        if quotient == 0 {
            continue;
        }
        let word = if quotient > 1 {
            scale.plural.as_ref().unwrap_or(&scale.singular)
        } else {
            &scale.singular
        };
        if quotient == 1 && scale.one_omitted {
            parts.push(word.clone());
        } else {
            let multiplier = below_thousand(rules, quotient % 1000, scale.multiplier_plural);
            parts.push(format!("{multiplier} {word}"));
        }
    }
    if rest > 0 {
        parts.push(below_thousand(rules, rest, plural_final));
    }
    parts.join(" ")
}

pub fn cardinal(rules: &Rules, n: u64) -> String {
    cardinal_with(rules, n, true)
}

/// The spoken word of one digit.
pub fn digit_word(rules: &Rules, digit: char) -> &str {
    let index = digit.to_digit(10).unwrap_or(0) as usize;
    &rules.numbers.below_hundred[index]
}

pub fn digits_one_by_one(rules: &Rules, digits: &str) -> String {
    digits
        .chars()
        .filter(char::is_ascii_digit)
        .map(|digit| digit_word(rules, digit))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Splits a number name after its last word, where a word ends at a space or a hyphen.
fn split_last_word(spoken: &str) -> (&str, &str) {
    match spoken.rfind([' ', '-']) {
        Some(position) => (&spoken[..=position], &spoken[position + 1..]),
        None => ("", spoken),
    }
}

/// The ordinal word of `n` ("first", "twenty-first").
pub fn ordinal(rules: &Rules, n: u64, feminine: bool) -> String {
    let ordinals = &rules.ordinals;
    if let Some(exact) = ordinals.exact.get(&n.to_string()) {
        return match (&exact.feminine, feminine) {
            (Some(form), true) => form.clone(),
            _ => exact.masculine.clone(),
        };
    }
    let base = cardinal_with(rules, n, false);
    let (head, last) = split_last_word(&base);
    let word = if let Some(form) = ordinals.irregular_last.get(last) {
        form.clone()
    } else if let Some(ending) = ordinals
        .endings
        .iter()
        .find(|ending| last.ends_with(&ending.ends_with))
    {
        let keep = last.len().saturating_sub(ending.strip);
        format!("{}{}", &last[..keep], ending.add)
    } else {
        format!("{last}{}", ordinals.default_add)
    };
    format!("{head}{word}")
}

/// A year as words, in the style of the language.
pub fn year(rules: &Rules, y: u64) -> String {
    let style = &rules.dates.year;
    if style.style == YearStyle::Cardinal {
        return cardinal(rules, y);
    }
    let plain = y % 1000 == 0 || (style.plain_from..=style.plain_to).contains(&y);
    if plain {
        return cardinal(rules, y);
    }
    let (high, low) = (y / 100, y % 100);
    let high_words = below_hundred(rules, high % 100, false);
    match low {
        0 => format!("{high_words} {}", style.hundred),
        1..=9 => format!(
            "{high_words} {} {}",
            style.oh,
            below_hundred(rules, low, false)
        ),
        _ => format!("{high_words} {}", below_hundred(rules, low, false)),
    }
}

/// Puts a feminine form on a trailing "one", when the language has one.
pub fn feminine_one(rules: &Rules, spoken: String) -> String {
    let Some(rule) = &rules.feminine else {
        return spoken;
    };
    let (head, last) = split_last_word(&spoken);
    if last == rule.one {
        format!("{head}{}", rule.one_feminine)
    } else {
        spoken
    }
}
