//! The scanner that rewrites one sentence into words: numbers, decimals, percentages, currencies,
//! units, ordinals, dates, times, fractions, telephone numbers, abbreviations, titles and acronyms.
//!
//! It is a hand-written scanner over the characters of the sentence, with no dependency. Every
//! word, symbol list and rule of a language comes from the [`Rules`] of its pack; the titles come
//! from the knowledge packs (`title_spoken`). Nothing here names a language, so a language with the
//! same grammar family is a data file. The text on screen is never changed: this runs on a copy.
//!
//! Rewriting numbers is always on, for every voice: a character-based voice says nothing for a digit.

use super::speakable_numbers::{
    cardinal, digit_word, digits_one_by_one, feminine_one, ordinal, year, MAX_DIGITS,
};
use super::speakable_rules::{DateOrder, DayStyle, Need, Rules, UnitRule};
use crate::knowledge::packs::PackSet;

/// Rewrites `text` (one sentence) into words.
pub fn spoken_form(text: &str, rules: &Rules, titles: &PackSet) -> String {
    let mut scanner = Scanner::new(text, rules, titles);
    scanner.run();
    collapse_spaces(&scanner.out)
}

fn collapse_spaces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_space = false;
    for ch in text.chars() {
        let is_space = ch == ' ';
        if !(is_space && last_space) {
            out.push(ch);
        }
        last_space = is_space;
    }
    out.trim().to_string()
}

fn is_blank(c: char) -> bool {
    c == ' ' || c == '\u{a0}' || c == '\u{202f}'
}

/// A number read from the text: its integer digits (without group separators), its decimals and
/// where it ends.
struct Num {
    int: String,
    frac: Option<String>,
    end: usize,
}

impl Num {
    fn value(&self) -> Option<u64> {
        (self.int.len() <= MAX_DIGITS)
            .then(|| self.int.parse().ok())
            .flatten()
    }

    fn has_fraction(&self) -> bool {
        self.frac
            .as_deref()
            .map(|f| f.chars().any(|c| c != '0'))
            .unwrap_or(false)
    }

    fn leading_zero(&self) -> bool {
        self.int.len() > 1 && self.int.starts_with('0')
    }
}

struct Scanner<'a> {
    rules: &'a Rules,
    titles: &'a PackSet,
    c: Vec<char>,
    out: String,
    i: usize,
    /// The last word written, in lower case; cleared after a number. It tells "at 10.30" from a
    /// decimal and a day-month date from a fraction.
    last_word: String,
    /// 1 right after a month name (a day or a year may follow), 2 after that day (a year may follow).
    month_stage: u8,
    /// The previous piece was a spoken number: a letter right after it needs a space.
    just_spoke: bool,
}

impl<'a> Scanner<'a> {
    fn new(text: &str, rules: &'a Rules, titles: &'a PackSet) -> Self {
        let c = text
            .chars()
            .map(|c| if is_blank(c) { ' ' } else { c })
            .collect();
        Self {
            rules,
            titles,
            c,
            out: String::new(),
            i: 0,
            last_word: String::new(),
            month_stage: 0,
            just_spoke: false,
        }
    }

    fn n(&self) -> usize {
        self.c.len()
    }

    fn at(&self, p: usize) -> Option<char> {
        self.c.get(p).copied()
    }

    fn digits_len(&self, p: usize) -> usize {
        let mut q = p;
        while q < self.n() && self.c[q].is_ascii_digit() {
            q += 1;
        }
        q - p
    }

    fn digits(&self, p: usize, len: usize) -> String {
        self.c[p..p + len].iter().collect()
    }

    fn word_end(&self, p: usize) -> usize {
        let mut q = p;
        while q < self.n() && self.c[q].is_alphabetic() {
            q += 1;
        }
        q
    }

    fn boundary_after(&self, p: usize) -> bool {
        match self.at(p) {
            None => true,
            Some(ch) => !(ch.is_alphanumeric() || ch == '\'' || ch == '\u{2019}'),
        }
    }

    /// True when the digits at `p` are not followed by another digit, a letter, or a separator and
    /// a digit.
    fn boundary_after_digits(&self, p: usize) -> bool {
        match self.at(p) {
            None => true,
            Some(c) if c.is_ascii_digit() || c.is_alphabetic() => false,
            Some(',') | Some('.') | Some(':') => self.digits_len(p + 1) == 0,
            _ => true,
        }
    }

    fn rest_is_blank(&self, p: usize) -> bool {
        self.c[p.min(self.n())..].iter().all(|c| c.is_whitespace())
    }

    fn skip_one_blank(&self, p: usize) -> usize {
        if self.at(p) == Some(' ') {
            p + 1
        } else {
            p
        }
    }

    fn matches_at(&self, p: usize, literal: &str) -> bool {
        let literal: Vec<char> = literal.chars().collect();
        p + literal.len() <= self.n() && self.c[p..p + literal.len()] == literal[..]
    }

    fn matches_ci_at(&self, p: usize, literal: &str) -> bool {
        let literal: Vec<char> = literal.chars().collect();
        p + literal.len() <= self.n()
            && self.c[p..p + literal.len()]
                .iter()
                .zip(literal.iter())
                .all(|(a, b)| a.to_lowercase().eq(b.to_lowercase()))
    }

    // ---- output helpers ------------------------------------------------------------------------

    /// Appends spoken words, with a space before them when they would stick to a letter or digit.
    fn speak(&mut self, spoken: &str) {
        if let Some(last) = self.out.chars().last() {
            if last.is_alphanumeric() {
                self.out.push(' ');
            }
        }
        self.out.push_str(spoken);
        self.just_spoke = true;
        self.last_word.clear();
    }

    fn copy(&mut self, ch: char) {
        if self.just_spoke && ch.is_alphanumeric() {
            self.out.push(' ');
        }
        self.just_spoke = false;
        self.out.push(ch);
    }

    fn cardinal(&self, n: u64) -> String {
        cardinal(self.rules, n)
    }

    // ---- data lookups --------------------------------------------------------------------------

    fn is_group_separator(&self, ch: char) -> bool {
        self.rules.numbers.group_separators.iter().any(|s| {
            s.chars()
                .next()
                .is_some_and(|c| c == ch || (c == ' ' && is_blank(ch)))
        })
    }

    fn is_decimal_separator(&self, ch: char) -> bool {
        self.rules
            .numbers
            .decimal_separators
            .iter()
            .any(|s| s.starts_with(ch))
    }

    fn month_index(&self, word: &str) -> Option<usize> {
        let lower = word.to_lowercase();
        self.rules.months.iter().position(|month| *month == lower)
    }

    fn month_name(&self, month: usize) -> String {
        let name = &self.rules.months[month - 1];
        if self.rules.dates.month_capitalised {
            let mut chars = name.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        } else {
            name.clone()
        }
    }

    fn find_unit(&self, symbol: &str) -> Option<&'a UnitRule> {
        self.rules
            .units
            .iter()
            .find(|unit| unit.symbols.iter().any(|s| s == symbol))
    }

    fn per_word(&self, written: &str) -> Option<&'a str> {
        let lower = written.to_lowercase();
        self.rules
            .per_words
            .iter()
            .find(|entry| entry.written.contains(&lower))
            .map(|entry| entry.spoken.as_str())
    }

    fn day_words(&self, day: u64) -> String {
        let dates = &self.rules.dates;
        match dates.day_style {
            DayStyle::Ordinal => ordinal(self.rules, day, false),
            DayStyle::Cardinal => match (&dates.first_day_word, day) {
                (Some(word), 1) => word.clone(),
                _ => self.cardinal(day),
            },
        }
    }

    fn plural(&self, num: &Num) -> bool {
        let numbers = &self.rules.numbers;
        match num.value() {
            None => true,
            Some(value) => {
                if num.has_fraction() && numbers.fraction_makes_plural {
                    return true;
                }
                !numbers.singular_values.contains(&value)
            }
        }
    }

    // ---- main loop -----------------------------------------------------------------------------

    fn run(&mut self) {
        while self.i < self.n() {
            let ch = self.c[self.i];
            let prev = if self.i > 0 {
                Some(self.c[self.i - 1])
            } else {
                None
            };
            let prev_alnum = prev.map(|p| p.is_alphanumeric()).unwrap_or(false);
            let next_is_digit = self
                .at(self.i + 1)
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false);

            if ch.is_ascii_digit() {
                if prev_alnum {
                    let len = self.digits_len(self.i);
                    let word = self.read_cardinal_digits(&self.digits(self.i, len));
                    self.speak(&word);
                    self.i += len;
                } else {
                    self.handle_digit(self.i, false);
                }
            } else if (ch == '-' || ch == '\u{2212}')
                && next_is_digit
                && prev.map(|p| p.is_whitespace() || p == '(').unwrap_or(true)
            {
                self.handle_digit(self.i + 1, true);
            } else if ch == '+' && next_is_digit && !prev_alnum {
                if let Some((spoken, end)) = self.try_phone(self.i) {
                    self.speak(&spoken);
                    self.i = end;
                } else {
                    let plus = self.rules.words.plus.clone();
                    self.speak(&plus);
                    self.i += 1;
                }
            } else if self.is_currency_sign(ch) && self.currency_prefix(self.i) {
                // handled
            } else if ch == '&'
                && prev.map(|p| p.is_whitespace()).unwrap_or(true)
                && self
                    .at(self.i + 1)
                    .map(|c| c.is_whitespace())
                    .unwrap_or(true)
            {
                let and = self.rules.words.and.clone();
                self.speak(&and);
                self.i += 1;
            } else if ch == '/' && self.try_per_word() {
                // handled
            } else if ch.is_alphabetic() && !prev_alnum {
                self.handle_word();
            } else {
                if !(ch == ',' || ch.is_whitespace()) {
                    self.month_stage = 0;
                }
                self.copy(ch);
                self.i += 1;
            }
        }
    }

    // ---- words ---------------------------------------------------------------------------------

    /// A title in front of a name ("Dr Martin"), said as the knowledge packs say it. With its dot a
    /// title needs nothing after it; without, a capitalised name must follow after one blank.
    fn try_title(&self, p: usize) -> Option<(String, usize, bool)> {
        let e = self.word_end(p);
        if e == p || !self.c[p].is_uppercase() {
            return None;
        }
        let token: String = self.c[p..e].iter().collect();
        let spoken = self.titles.title_spoken(&token)?.to_string();
        let dotted = self.at(e) == Some('.');
        if dotted {
            return Some((spoken, e + 1, true));
        }
        if !self.boundary_after(e) {
            return None;
        }
        let q = self.skip_one_blank(e);
        (q > e && self.at(q).map(|c| c.is_uppercase()).unwrap_or(false))
            .then_some((spoken, e, false))
    }

    fn handle_word(&mut self) {
        let p = self.i;

        if let Some((spoken, end, dotted)) = self.try_title(p) {
            let sentence_end = dotted && self.rest_is_blank(end);
            self.speak(&spoken);
            if sentence_end {
                self.out.push('.');
            }
            self.last_word = spoken.to_lowercase();
            self.i = end;
            return;
        }

        // The abbreviation table, longest written form first.
        let rules = self.rules;
        let mut best: Option<(usize, &str, usize)> = None; // (form length, spoken, end)
        for abbreviation in &rules.abbreviations {
            let form = abbreviation.written.as_str();
            let form_len = form.chars().count();
            let first = match form.chars().next() {
                Some(first) => first,
                None => continue,
            };
            let exact = self.matches_at(p, form);
            let capitalised = first.is_lowercase() && {
                let mut chars: Vec<char> = form.chars().collect();
                chars[0] = first.to_uppercase().next().unwrap_or(first);
                self.matches_at(p, &chars.iter().collect::<String>())
            };
            if !(exact || capitalised) {
                continue;
            }
            let end = p + form_len;
            // The written form must end the word ("Dr" is not the start of "Drogue").
            let ends_alnum = form
                .chars()
                .last()
                .map(|c| c.is_alphanumeric())
                .unwrap_or(false);
            if ends_alnum && !self.boundary_after(end) {
                continue;
            }
            let ok = match abbreviation.need {
                Need::Nothing => true,
                Need::Name => {
                    let q = self.skip_one_blank(end);
                    q > end && self.at(q).map(|c| c.is_uppercase()).unwrap_or(false)
                }
                Need::Digit => {
                    let q = self.skip_one_blank(end);
                    self.at(q).map(|c| c.is_ascii_digit()).unwrap_or(false)
                }
            };
            if ok && best.map(|b| form_len > b.0).unwrap_or(true) {
                best = Some((form_len, abbreviation.spoken.as_str(), end));
            }
        }
        if let Some((_, spoken, end)) = best {
            let dotted = self.c[end - 1] == '.';
            let sentence_end = dotted && self.rest_is_blank(end);
            self.speak(spoken);
            if sentence_end {
                self.out.push('.');
            }
            self.last_word = spoken.to_lowercase();
            self.month_stage = self.stage_after_word(spoken);
            self.i = end;
            return;
        }

        // A plain word: letters, with inner apostrophes and hyphens.
        let mut q = p;
        while q < self.n() {
            let ch = self.c[q];
            let inner = (ch == '\'' || ch == '\u{2019}' || ch == '-')
                && self.at(q + 1).map(|c| c.is_alphabetic()).unwrap_or(false);
            if ch.is_alphabetic() || inner {
                q += 1;
            } else {
                break;
            }
        }
        let word: String = self.c[p..q].iter().collect();
        if rules.spell_acronyms.contains(&word) {
            let spelled = self.spell(&word);
            self.speak(&spelled);
            self.last_word = word.to_lowercase();
            self.i = q;
            return;
        }
        for ch in word.chars() {
            self.copy(ch);
        }
        self.last_word = word.to_lowercase();
        self.month_stage = self.stage_after_word(&word);
        self.i = q;
    }

    fn stage_after_word(&self, word: &str) -> u8 {
        if self.rules.dates.day_after_month && self.month_index(word).is_some() {
            1
        } else {
            0
        }
    }

    /// Letter by letter, each with the name the pack gives it.
    fn spell(&self, word: &str) -> String {
        word.chars()
            .map(|ch| {
                let key = ch.to_lowercase().to_string();
                self.rules
                    .letters
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(|| ch.to_string())
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// "/day" after a count: "3 times/day".
    fn try_per_word(&mut self) -> bool {
        let p = self.i + 1;
        let e = self.word_end(p);
        if e == p || !self.boundary_after(e) {
            return false;
        }
        let written: String = self.c[p..e].iter().collect();
        let Some(spoken) = self.per_word(&written) else {
            return false;
        };
        if self.out.ends_with(|c: char| !c.is_whitespace()) {
            self.out.push(' ');
        }
        let phrase = format!("{} {spoken}", self.rules.words.per);
        self.out.push_str(&phrase);
        self.just_spoke = true;
        self.i = e;
        true
    }

    // ---- digits --------------------------------------------------------------------------------

    /// Reads whatever starts with a digit at `p` (after the sign when `negative`) and moves `self.i`.
    fn handle_digit(&mut self, p: usize, negative: bool) {
        let stage = self.month_stage;
        if !negative {
            if let Some((spoken, end)) = self.try_numeric_date(p) {
                return self.finish(&spoken, end, 0);
            }
            if let Some((spoken, end)) = self.try_phone(p) {
                return self.finish(&spoken, end, 0);
            }
            if let Some((spoken, end)) = self.try_time(p) {
                return self.finish(&spoken, end, 0);
            }
            if let Some((spoken, end)) = self.try_fraction(p) {
                return self.finish(&spoken, end, 0);
            }
            if let Some((spoken, end)) = self.try_ordinal(p) {
                // A day after a month name lets a year follow.
                let next = if stage > 0 { 2 } else { 0 };
                return self.finish(&spoken, end, next);
            }
        }
        let (mut spoken, end) = self.number_with_tail(p);
        if negative {
            spoken = format!("{} {spoken}", self.rules.words.minus);
        }
        // The date logic inside `number_with_tail` moves the stage itself; otherwise the date
        // context ends here.
        let next = if self.month_stage == stage {
            0
        } else {
            self.month_stage
        };
        self.finish(&spoken, end, next);
    }

    fn finish(&mut self, spoken: &str, end: usize, month_stage: u8) {
        self.speak(spoken);
        self.i = end;
        self.month_stage = month_stage;
    }

    fn read_cardinal_digits(&self, digits: &str) -> String {
        if digits.len() > MAX_DIGITS || (digits.len() > 1 && digits.starts_with('0')) {
            return digits_one_by_one(self.rules, digits);
        }
        self.cardinal(digits.parse().unwrap_or(0))
    }

    // ---- numbers -------------------------------------------------------------------------------

    fn parse_number(&self, p: usize) -> Option<Num> {
        let first_len = self.digits_len(p);
        if first_len == 0 {
            return None;
        }
        let mut int = self.digits(p, first_len);
        let mut e = p + first_len;
        // Groups of exactly three digits after a first group of one to three, never "0".
        let grouping = first_len <= 3 && !int.starts_with('0');
        while let Some(sep) = self.at(e).filter(|_| grouping) {
            if self.is_group_separator(sep) && self.digits_len(e + 1) == 3 {
                int.push_str(&self.digits(e + 1, 3));
                e += 4;
            } else {
                break;
            }
        }
        let mut frac = None;
        if let Some(sep) = self.at(e) {
            if self.is_decimal_separator(sep) {
                let frac_len = self.digits_len(e + 1);
                if frac_len > 0 {
                    frac = Some(self.digits(e + 1, frac_len));
                    e += 1 + frac_len;
                }
            }
        }
        Some(Num { int, frac, end: e })
    }

    fn quantity_words(&self, num: &Num, feminine: bool) -> String {
        let mut spoken = match num.value() {
            Some(value) if !num.leading_zero() => {
                let words = self.cardinal(value);
                if feminine && num.frac.is_none() {
                    feminine_one(self.rules, words)
                } else {
                    words
                }
            }
            _ => digits_one_by_one(self.rules, &num.int),
        };
        if let Some(frac) = &num.frac {
            spoken.push(' ');
            spoken.push_str(&self.rules.numbers.decimal_word);
            spoken.push(' ');
            spoken.push_str(&self.fraction_digits(frac));
        }
        spoken
    }

    fn fraction_digits(&self, frac: &str) -> String {
        let limit = self.rules.numbers.decimals_as_number_up_to;
        if limit == 0 || frac.len() > limit {
            return digits_one_by_one(self.rules, frac);
        }
        // Leading zeros are said one by one, the rest as a number ("0,05" -> zero five).
        let zeros = frac.chars().take_while(|c| *c == '0').count();
        let rest = &frac[zeros..];
        let mut parts: Vec<String> = (0..zeros)
            .map(|_| digit_word(self.rules, '0').to_string())
            .collect();
        if !rest.is_empty() {
            parts.push(self.cardinal(rest.parse().unwrap_or(0)));
        }
        parts.join(" ")
    }

    /// A number with whatever follows it: percent, degrees, currency, unit, multiplier, range, day.
    fn number_with_tail(&mut self, p: usize) -> (String, usize) {
        let Some(first) = self.parse_number(p) else {
            return (String::new(), p + 1);
        };
        // A range with no spaces, "5-10".
        if matches!(self.at(first.end), Some('-') | Some('\u{2013}'))
            && self.digits_len(first.end + 1) > 0
        {
            if let Some(second) = self.parse_number(first.end + 1) {
                let a = self.quantity_words(&first, false);
                let (b, end) = self.tail(&second);
                return (format!("{a} {} {b}", self.rules.words.range_to), end);
            }
        }
        self.tail(&first)
    }

    fn tail(&mut self, num: &Num) -> (String, usize) {
        let rules = self.rules;
        let end = num.end;
        let q = self.skip_one_blank(end);

        if self.at(q) == Some('%') {
            let spoken = format!(
                "{} {}",
                self.quantity_words(num, false),
                rules.words.percent
            );
            return (spoken, q + 1);
        }
        if matches!(self.at(q), Some('\u{b0}') | Some('\u{ba}')) {
            let r = self.skip_one_blank(q + 1);
            let scale = self
                .at(r)
                .filter(|_| self.boundary_after(r + 1))
                .and_then(|c| rules.degrees.scales.get(&c.to_string()));
            let (suffix, stop) = match scale {
                Some(name) => (format!(" {name}"), r + 1),
                None => (String::new(), q + 1),
            };
            let word = if self.plural(num) {
                &rules.degrees.plural
            } else {
                &rules.degrees.singular
            };
            return (
                format!("{} {word}{suffix}", self.quantity_words(num, false)),
                stop,
            );
        }
        if let Some((symbol, stop)) = self.currency_at(q) {
            if let Some(spoken) = self.currency_words(num, &symbol) {
                return (spoken, stop);
            }
        }
        // A multiplier, "3 x per day".
        if matches!(self.at(q), Some('x') | Some('\u{d7}'))
            && !self.at(q + 1).map(|c| c.is_alphabetic()).unwrap_or(false)
        {
            let spoken = format!("{} {}", self.quantity_words(num, false), rules.words.times);
            return (spoken, q + 1);
        }
        if let Some(found) = self.unit_at(num, q) {
            return found;
        }
        // A day before a month name.
        if let Some(day) = num
            .value()
            .filter(|d| (1..=31).contains(d) && num.frac.is_none())
        {
            let r = self.skip_one_blank(end);
            if r > end {
                let word_end = self.word_end(r);
                if word_end > r {
                    let word: String = self.c[r..word_end].iter().collect();
                    if self.month_index(&word).is_some() {
                        return (self.day_words(day), end);
                    }
                }
            }
        }
        // A day or a year after a month name.
        if rules.dates.day_after_month && self.month_stage > 0 && num.frac.is_none() {
            if let Some(value) = num.value() {
                if self.month_stage == 1 && (1..=31).contains(&value) && num.int.len() <= 2 {
                    self.month_stage = 2;
                    return (self.day_words(value), end);
                }
                if num.int.len() == 4 && (1100..=2099).contains(&value) {
                    self.month_stage = 0;
                    return (year(rules, value), end);
                }
            }
        }
        let feminine = rules.feminine.is_some() && self.next_word_is_feminine(end);
        (self.quantity_words(num, feminine), end)
    }

    fn next_word_is_feminine(&self, end: usize) -> bool {
        let Some(rule) = &self.rules.feminine else {
            return false;
        };
        let r = self.skip_one_blank(end);
        let e = self.word_end(r);
        if e == r {
            return false;
        }
        let word: String = self.c[r..e].iter().collect::<String>().to_lowercase();
        rule.nouns.contains(&word)
    }

    fn unit_at(&self, num: &Num, q: usize) -> Option<(String, usize)> {
        let e = self.word_end(q);
        if e == q {
            return None;
        }
        let symbol: String = self.c[q..e].iter().collect();
        let unit = self.find_unit(&symbol)?;
        // "5 m'a dit" and "3 s'y" are not units; a rate keeps going after the slash.
        if !self.boundary_after(e) && self.at(e) != Some('/') {
            return None;
        }
        let plural = self.plural(num);
        let mut name = if plural {
            unit.plural.clone()
        } else {
            unit.singular.clone()
        };
        let mut end = e;
        // Rates, "mg/kg" and "mg/day".
        if self.at(e) == Some('/') {
            let e2 = self.word_end(e + 1);
            if e2 > e + 1 && self.boundary_after(e2) {
                let second: String = self.c[e + 1..e2].iter().collect();
                let per = &self.rules.words.per;
                if let Some(unit2) = self.find_unit(&second) {
                    name = format!("{name} {per} {}", unit2.singular);
                    end = e2;
                } else if let Some(spoken) = self.per_word(&second) {
                    name = format!("{name} {per} {spoken}");
                    end = e2;
                }
            }
        }
        let quantity = self.quantity_words(num, false);
        let glue = self.round_million_glue(num, &name);
        Some((format!("{quantity} {glue}{name}"), end))
    }

    /// "deux millions de grammes": the glue word after a whole number of millions, when the
    /// language has one. Empty otherwise.
    fn round_million_glue(&self, num: &Num, name: &str) -> String {
        let Some(glue) = &self.rules.glue else {
            return String::new();
        };
        let round = num.frac.is_none()
            && num
                .value()
                .map(|v| v >= glue.from && v % glue.from == 0)
                .unwrap_or(false);
        if !round {
            return String::new();
        }
        if name.starts_with(|c: char| glue.vowels.contains(c)) {
            glue.before_vowel.clone()
        } else {
            glue.otherwise.clone()
        }
    }

    // ---- currency ------------------------------------------------------------------------------

    /// A sign that is a single non-letter character of one of the currencies.
    fn is_currency_sign(&self, ch: char) -> bool {
        !ch.is_alphabetic()
            && self.rules.currencies.iter().any(|currency| {
                currency
                    .symbols
                    .iter()
                    .any(|s| s.chars().count() == 1 && s.starts_with(ch))
            })
    }

    /// The currency symbol or code that starts at `q`, and where it ends.
    fn currency_at(&self, q: usize) -> Option<(String, usize)> {
        let ch = self.at(q)?;
        for currency in &self.rules.currencies {
            for symbol in &currency.symbols {
                let length = symbol.chars().count();
                if length == 1 && !ch.is_alphabetic() && symbol.starts_with(ch) {
                    return Some((symbol.clone(), q + 1));
                }
                if length > 1 && self.matches_at(q, symbol) && self.boundary_after(q + length) {
                    return Some((symbol.clone(), q + length));
                }
            }
        }
        None
    }

    fn currency_prefix(&mut self, p: usize) -> bool {
        let symbol = self.c[p].to_string();
        let q = self.skip_one_blank(p + 1);
        if self.digits_len(q) == 0 {
            return false;
        }
        let Some(num) = self.parse_number(q) else {
            return false;
        };
        match self.currency_words(&num, &symbol) {
            Some(spoken) => {
                self.speak(&spoken);
                self.i = num.end;
                true
            }
            None => false,
        }
    }

    fn currency_words(&self, num: &Num, symbol: &str) -> Option<String> {
        let rules = self.rules;
        let currency = rules
            .currencies
            .iter()
            .find(|c| c.symbols.iter().any(|s| s == symbol))?;
        let value = num.value()?;
        let minor: u64 = match &num.frac {
            None => 0,
            Some(f) if f.len() <= 2 => format!("{f:0<2}").parse().ok()?,
            Some(_) => return None,
        };
        let style = &rules.currency_style;
        let singular_major = rules.numbers.singular_values.contains(&value);
        let mut spoken = String::new();
        if value > 0 || minor == 0 {
            let name = if singular_major {
                &currency.singular
            } else {
                &currency.plural
            };
            let glue = self.round_million_glue(num, name);
            spoken = format!("{} {glue}{name}", self.cardinal(value));
        }
        if minor > 0 {
            if !spoken.is_empty() {
                spoken.push(' ');
                if !style.minor_joiner.is_empty() {
                    spoken.push_str(&style.minor_joiner);
                    spoken.push(' ');
                }
            }
            spoken.push_str(&self.cardinal(minor));
            if value == 0 || style.name_minor_with_major {
                let word = if minor == 1 {
                    &currency.minor_singular
                } else {
                    &currency.minor_plural
                };
                spoken.push(' ');
                spoken.push_str(word);
            }
        }
        Some(spoken)
    }

    // ---- ordinals ------------------------------------------------------------------------------

    fn try_ordinal(&self, p: usize) -> Option<(String, usize)> {
        let len = self.digits_len(p);
        if len == 0 || len > 9 {
            return None;
        }
        let n: u64 = self.digits(p, len).parse().ok()?;
        let e = p + len;
        let mut suffix_end = e;
        while suffix_end < self.n() && self.c[suffix_end].is_alphabetic() {
            suffix_end += 1;
        }
        if suffix_end == e || !self.boundary_after(suffix_end) {
            return None;
        }
        let suffix: String = self.c[e..suffix_end]
            .iter()
            .collect::<String>()
            .to_lowercase();
        let entry = self
            .rules
            .ordinals
            .suffixes
            .iter()
            .find(|entry| entry.suffix == suffix)?;
        if n < entry.min || entry.only.is_some_and(|only| only != n) {
            return None;
        }
        let mut spoken = match &entry.word {
            Some(word) => word.clone(),
            None => ordinal(self.rules, n, entry.feminine),
        };
        if entry.plural {
            spoken.push_str(&self.rules.ordinals.plural_add);
        }
        Some((spoken, suffix_end))
    }

    // ---- fractions -----------------------------------------------------------------------------

    fn try_fraction(&self, p: usize) -> Option<(String, usize)> {
        let a = self.digits_len(p);
        if a == 0 || a > 3 || self.at(p + a) != Some('/') {
            return None;
        }
        let b = self.digits_len(p + a + 1);
        if b == 0
            || b > 3
            || !self.boundary_after(p + a + 1 + b)
            || self.at(p + a + 1 + b) == Some('/')
        {
            return None;
        }
        let x: u64 = self.digits(p, a).parse().ok()?;
        let y: u64 = self.digits(p + a + 1, b).parse().ok()?;
        let end = p + a + 1 + b;
        let rules = self.rules;
        // "12/03" after a date word is a day and a month, not a fraction.
        if (1..=31).contains(&x)
            && (1..=12).contains(&y)
            && rules.dates.day_month_after.contains(&self.last_word)
        {
            let spoken = rules
                .dates
                .day_month_template
                .replace("{day}", &self.day_words(x))
                .replace("{month}", &self.month_name(y as usize));
            return Some((spoken, end));
        }
        let key = format!("{x}/{y}");
        let known = rules.fractions.get(&key);
        // A ratio followed by a unit, "120/80 mmHg".
        if known.is_none() {
            let q = self.skip_one_blank(end);
            let denominator = Num {
                int: y.to_string(),
                frac: None,
                end,
            };
            if let Some((with_unit, stop)) = self.unit_at(&denominator, q) {
                let spoken = format!("{} {} {with_unit}", self.cardinal(x), rules.words.over);
                return Some((spoken, stop));
            }
        }
        let spoken = match known {
            Some(words) => words.clone(),
            None => format!(
                "{} {} {}",
                self.cardinal(x),
                rules.words.over,
                self.cardinal(y)
            ),
        };
        Some((spoken, end))
    }

    // ---- dates ---------------------------------------------------------------------------------

    /// 12/03/2026, 12.03.2026, 12-03-2026 and 2026-03-12.
    fn try_numeric_date(&self, p: usize) -> Option<(String, usize)> {
        let l1 = self.digits_len(p);
        if !(l1 == 1 || l1 == 2 || l1 == 4) {
            return None;
        }
        let sep = self.at(p + l1)?;
        if !matches!(sep, '/' | '.' | '-') {
            return None;
        }
        let l2 = self.digits_len(p + l1 + 1);
        if !(l2 == 1 || l2 == 2) || self.at(p + l1 + 1 + l2) != Some(sep) {
            return None;
        }
        let l3 = self.digits_len(p + l1 + l2 + 2);
        let end = p + l1 + l2 + 2 + l3;
        let v1: u64 = self.digits(p, l1).parse().ok()?;
        let v2: u64 = self.digits(p + l1 + 1, l2).parse().ok()?;
        let dates = &self.rules.dates;
        let (day, month, year_value) = if l1 == 4 && (l3 == 1 || l3 == 2) && sep == '-' {
            (
                self.digits(p + l1 + l2 + 2, l3).parse::<u64>().ok()?,
                v2,
                v1,
            )
        } else if l3 == 4 && l1 <= 2 {
            let y: u64 = self.digits(p + l1 + l2 + 2, 4).parse().ok()?;
            match dates.order {
                DateOrder::Dmy => (v1, v2, y),
                DateOrder::Mdy => {
                    if dates.swap_when_first_over_twelve && v1 > 12 {
                        (v1, v2, y)
                    } else {
                        (v2, v1, y)
                    }
                }
            }
        } else {
            return None;
        };
        if !(1..=31).contains(&day) || !(1..=12).contains(&month) {
            return None;
        }
        // Not the start of a longer group of numbers.
        if matches!(self.at(end), Some(c) if c.is_ascii_digit()
            || (matches!(c, '/' | '.' | '-') && self.digits_len(end + 1) > 0))
        {
            return None;
        }
        let spoken = dates
            .template
            .replace("{day}", &self.day_words(day))
            .replace("{month}", &self.month_name(month as usize))
            .replace("{year}", &year(self.rules, year_value));
        Some((spoken, end))
    }

    // ---- times ---------------------------------------------------------------------------------

    /// "am" or "p.m." after a time. A final dot that also ends the sentence is kept.
    fn ampm_at(&self, q: usize) -> Option<(String, usize)> {
        let r = self.skip_one_blank(q);
        for entry in &self.rules.times.ampm {
            for written in &entry.written {
                if !self.matches_ci_at(r, written) {
                    continue;
                }
                let length = written.chars().count();
                if written.ends_with('.') {
                    let dot = if self.rest_is_blank(r + length) {
                        "."
                    } else {
                        ""
                    };
                    return Some((format!("{}{dot}", entry.spoken), r + length));
                }
                if self.boundary_after(r + length) {
                    return Some((entry.spoken.clone(), r + length));
                }
            }
        }
        None
    }

    fn time_words(&self, hours: u64, minutes: u64, ampm: Option<&str>) -> String {
        let times = &self.rules.times;
        let forms = &times.hour_forms;
        let hour = match (hours, &forms.zero, &forms.one) {
            (0, Some(zero), _) => zero.clone(),
            (1, _, Some(one)) => one.clone(),
            _ => forms.other.replace("{hours}", &self.cardinal(hours)),
        };
        let template = match (minutes, ampm.is_some()) {
            (0, false) => &times.on_the_hour,
            (0, true) => &times.on_the_hour_ampm,
            (1..=9, false) => &times.with_minutes_below_ten,
            (1..=9, true) => &times.with_minutes_below_ten_ampm,
            (_, false) => &times.with_minutes,
            (_, true) => &times.with_minutes_ampm,
        };
        template
            .replace("{hour}", &hour)
            .replace("{minutes}", &self.cardinal(minutes))
            .replace("{ampm}", ampm.unwrap_or(""))
    }

    fn follows_with_unit_or_money(&self, q: usize) -> bool {
        let r = self.skip_one_blank(q);
        if self.currency_at(r).is_some() || matches!(self.at(r), Some('%') | Some('\u{b0}')) {
            return true;
        }
        let e = self.word_end(r);
        if e == r {
            return false;
        }
        let word: String = self.c[r..e].iter().collect();
        self.find_unit(&word).is_some()
            || self
                .rules
                .currency_style
                .words
                .iter()
                .any(|w| *w == word.to_lowercase())
    }

    fn try_time(&self, p: usize) -> Option<(String, usize)> {
        let times = &self.rules.times;
        let hour_len = self.digits_len(p);
        if hour_len == 0 || hour_len > 2 {
            return None;
        }
        let hours: u64 = self.digits(p, hour_len).parse().ok()?;
        let after_hours = p + hour_len;

        // Written with a marker between hours and minutes, "14h30" and "14 h 30".
        if let Some(marker) = &times.hour_marker {
            let q = self.skip_one_blank(after_hours);
            if self.matches_ci_at(q, marker) && hours <= 24 {
                let r = q + marker.chars().count();
                let r2 = self.skip_one_blank(r);
                if self.digits_len(r2) == 2 && self.boundary_after_digits(r2 + 2) {
                    let minutes: u64 = self.digits(r2, 2).parse().ok()?;
                    if minutes <= 59 {
                        return Some((self.time_words(hours, minutes, None), r2 + 2));
                    }
                }
                if !self.at(r).map(|c| c.is_alphanumeric()).unwrap_or(false) {
                    return Some((self.time_words(hours, 0, None), r));
                }
                return None;
            }
        }
        // An hour and am or pm, "9 am".
        if !times.ampm.is_empty() {
            if let Some((ampm, stop)) = self.ampm_at(after_hours) {
                if (1..=12).contains(&hours) {
                    return Some((self.time_words(hours, 0, Some(&ampm)), stop));
                }
            }
        }
        // Hours and minutes, "14:30" always, "10.30" only after a time word.
        let sep = self.at(after_hours)?;
        if (sep == ':' || sep == '.')
            && self.digits_len(after_hours + 1) == 2
            && self.boundary_after_digits(after_hours + 3)
            && hours <= 23
            && self.at(after_hours + 3) != Some(':')
        {
            let minutes: u64 = self.digits(after_hours + 1, 2).parse().ok()?;
            if minutes > 59 {
                return None;
            }
            if let Some((ampm, stop)) = self.ampm_at(after_hours + 3) {
                return Some((self.time_words(hours, minutes, Some(&ampm)), stop));
            }
            let after_time_word = times.dot_after.contains(&self.last_word);
            if sep == ':' || (after_time_word && !self.follows_with_unit_or_money(after_hours + 3))
            {
                return Some((self.time_words(hours, minutes, None), after_hours + 3));
            }
        }
        None
    }

    // ---- telephone numbers and identifiers -----------------------------------------------------

    fn try_phone(&self, p: usize) -> Option<(String, usize)> {
        let plus = self.at(p) == Some('+');
        let mut q = if plus { p + 1 } else { p };
        let mut groups: Vec<(usize, usize)> = Vec::new(); // (start, length)
        loop {
            let len = self.digits_len(q);
            if len == 0 {
                break;
            }
            groups.push((q, len));
            q += len;
            match self.at(q) {
                Some(sep)
                    if (is_blank(sep) || sep == '.' || sep == '-')
                        && self.digits_len(q + 1) > 0 =>
                {
                    q += 1
                }
                _ => break,
            }
        }
        if groups.is_empty() {
            return None;
        }
        if self.at(q).map(|c| c.is_alphabetic()).unwrap_or(false) {
            return None;
        }
        let total: usize = groups.iter().map(|g| g.1).sum();
        let first_zero = self.at(groups[0].0) == Some('0');
        let n_groups = groups.len();
        let rest_pairs = n_groups >= 3 && groups[1..].iter().all(|g| g.1 == 2) && groups[0].1 <= 3;
        let all_threes = n_groups >= 2
            && groups.iter().skip(1).all(|g| g.1 == 3)
            && groups[0].1 <= 3
            && !first_zero;
        // Several groups and at least nine digits that are not plain thousands groups: an identifier.
        let grouped_id = n_groups >= 3 && total >= 9 && !all_threes;
        let class_phone = (plus && total >= 7)
            || (first_zero && total >= 9)
            || (rest_pairs && total >= 8 && !all_threes);
        let class_id = !class_phone && (grouped_id || (n_groups == 1 && total >= 8));
        if !class_phone && !class_id {
            return None;
        }
        let in_pairs =
            class_phone && self.rules.phones.ten_digits_in_pairs && n_groups == 1 && total == 10;
        let mut parts: Vec<String> = Vec::new();
        for (start, len) in groups.iter() {
            let digits = self.digits(*start, *len);
            let spoken = if in_pairs {
                digits
                    .as_bytes()
                    .chunks(2)
                    .map(|pair| self.pair_words(std::str::from_utf8(pair).unwrap_or("0")))
                    .collect::<Vec<_>>()
                    .join(", ")
            } else if class_phone && *len == 2 {
                self.pair_words(&digits)
            } else {
                digits_one_by_one(self.rules, &digits)
            };
            parts.push(spoken);
        }
        let mut spoken = parts.join(", ");
        if plus {
            spoken = format!("{} {spoken}", self.rules.words.plus);
        }
        Some((spoken, q))
    }

    /// Two digits read as a number; a leading zero is read one by one.
    fn pair_words(&self, digits: &str) -> String {
        if digits.starts_with('0') {
            digits_one_by_one(self.rules, digits)
        } else {
            self.cardinal(digits.parse().unwrap_or(0))
        }
    }
}
