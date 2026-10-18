//! The rule packs of read-aloud: the words a language needs to turn written text into speakable
//! text (`resources/speech/rules/<locale>.json`).
//!
//! A pack holds data and no sentences for a human: number words, units, months, abbreviations,
//! the acronyms to spell, the exceptions of the sentence splitter and how identifiers are read.
//! The logic that uses them (`speakable_numbers`, `speakable_scan`, `speakable_sentences`) names no
//! language. Packs are embedded with `include_str!` and parsed once, the way the knowledge packs
//! are. A locale reaches its pack through the language table (`languages::LanguageRow::rule_packs`),
//! so adding a language is a row of that table, a data file and a line of [`REGISTRY`]: no change
//! of logic, but still a source edit.
//!
//! The loader ignores the keys it does not know (no `deny_unknown_fields`): a later phase adds the
//! dictation commands to the same files.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use super::languages::{self, Feature};
use crate::error::AppError;

/// The embedded packs, by the name the language table gives them.
const REGISTRY: &[(&str, &str)] = &[
    (
        "fr-FR",
        include_str!("../../resources/speech/rules/fr-FR.json"),
    ),
    (
        "en-US",
        include_str!("../../resources/speech/rules/en-US.json"),
    ),
];

/// The pack of a locale, parsed once. A locale that is not in the table, is only planned, or has no
/// read-aloud capability answers `speech_locale_unsupported`, and so does a row whose pack is not
/// embedded.
pub fn rules_for(locale: &str) -> Result<&'static Rules, AppError> {
    let row = languages::resolve(locale, Feature::ReadAloud)?;
    let unsupported = || AppError::SpeechLocaleUnsupported {
        locale: locale.trim().to_string(),
    };
    let pack_name = row.rule_packs.first().ok_or_else(unsupported)?;
    parsed()
        .iter()
        .find(|(name, _)| name == pack_name)
        .and_then(|(_, rules)| rules.as_ref().ok())
        .ok_or_else(unsupported)
}

/// Every embedded pack with the outcome of its parse; a test proves that none failed.
pub fn parsed() -> &'static [(&'static str, Result<Rules, String>)] {
    static PARSED: OnceLock<Vec<(&'static str, Result<Rules, String>)>> = OnceLock::new();
    PARSED.get_or_init(|| {
        REGISTRY
            .iter()
            .map(|(name, body)| {
                let rules = serde_json::from_str::<Rules>(body)
                    .map_err(|error| error.to_string())
                    .and_then(|rules| rules.validate().map(|()| rules));
                (*name, rules)
            })
            .collect()
    })
}

/// The pack names that are embedded, in registry order.
pub fn embedded_names() -> Vec<&'static str> {
    REGISTRY.iter().map(|(name, _)| *name).collect()
}

/// How a day is said: a cardinal ("twelve" in French) or an ordinal ("twelfth" in English).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DayStyle {
    #[default]
    Cardinal,
    Ordinal,
}

/// The order of the day and the month in a numeric date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DateOrder {
    #[default]
    Dmy,
    Mdy,
}

/// How a year is said: as one number, or in two pairs of digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum YearStyle {
    #[default]
    Cardinal,
    Pairs,
}

/// What an abbreviation needs after it to count as one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Need {
    #[default]
    Nothing,
    /// A capitalised word follows after one blank (a name).
    Name,
    /// A digit follows after at most one blank.
    Digit,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HundredRule {
    pub word: String,
    /// The plural form, used when nothing follows and the multiplier is above one.
    #[serde(default)]
    pub plural: Option<String>,
    /// "one hundred" is "cent" alone when true.
    #[serde(default)]
    pub one_omitted: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScaleRule {
    pub value: u64,
    pub singular: String,
    #[serde(default)]
    pub plural: Option<String>,
    #[serde(default)]
    pub one_omitted: bool,
    /// Whether the number in front of the scale takes its plural form ("quatre-vingts millions").
    #[serde(default)]
    pub multiplier_plural: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Numbers {
    /// Exactly the words of 0 to 99.
    pub below_hundred: Vec<String>,
    /// Words that replace an entry of `below_hundred` when nothing follows and the plural applies.
    #[serde(default)]
    pub final_forms: BTreeMap<String, String>,
    pub hundred: HundredRule,
    /// Largest first or in any order; sorted when used.
    pub scales: Vec<ScaleRule>,
    /// Characters that separate groups of three digits ("1 000", "1.000", "1,000"). A blank
    /// stands for any space.
    #[serde(default)]
    pub group_separators: Vec<String>,
    /// Characters that separate the decimals.
    #[serde(default)]
    pub decimal_separators: Vec<String>,
    /// The word said for the decimal mark.
    pub decimal_word: String,
    /// Decimals up to this many digits are said as a number after their leading zeros; more
    /// digits, or zero here, are said one by one.
    #[serde(default)]
    pub decimals_as_number_up_to: usize,
    /// From this value the plural of a unit applies. 0 and 1 are singular in some languages.
    #[serde(default)]
    pub singular_values: Vec<u64>,
    /// Whether a fraction (1.5) makes the unit plural.
    #[serde(default)]
    pub fraction_makes_plural: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EndingRule {
    pub ends_with: String,
    pub strip: usize,
    pub add: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FormPair {
    pub masculine: String,
    #[serde(default)]
    pub feminine: Option<String>,
}

/// One written ending of an ordinal ("er", "th") and which numbers it can follow.
#[derive(Debug, Clone, Deserialize)]
pub struct OrdinalSuffix {
    pub suffix: String,
    #[serde(default)]
    pub min: u64,
    #[serde(default)]
    pub only: Option<u64>,
    #[serde(default)]
    pub feminine: bool,
    #[serde(default)]
    pub plural: bool,
    /// A fixed word for this suffix ("second", "seconde").
    #[serde(default)]
    pub word: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Ordinals {
    /// The last word of the cardinal, replaced as a whole ("cinq" -> "cinquieme").
    #[serde(default)]
    pub irregular_last: BTreeMap<String, String>,
    #[serde(default)]
    pub endings: Vec<EndingRule>,
    /// Added to the last word of the cardinal when nothing else applies.
    pub default_add: String,
    /// Whole ordinals by number ("1" -> premier), masculine and feminine.
    #[serde(default)]
    pub exact: BTreeMap<String, FormPair>,
    #[serde(default)]
    pub plural_add: String,
    #[serde(default)]
    pub suffixes: Vec<OrdinalSuffix>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YearRule {
    #[serde(default)]
    pub style: YearStyle,
    /// The word said for a zero in the second pair ("nineteen oh five").
    #[serde(default)]
    pub oh: String,
    #[serde(default)]
    pub hundred: String,
    /// Years from the same thousand, 2001 to 2009 for example, said as plain numbers.
    #[serde(default)]
    pub plain_from: u64,
    #[serde(default)]
    pub plain_to: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Words {
    pub minus: String,
    pub plus: String,
    pub and: String,
    pub percent: String,
    pub times: String,
    pub per: String,
    pub range_to: String,
    pub over: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DateRules {
    #[serde(default)]
    pub order: DateOrder,
    /// A numeric date whose first number is above twelve is read day first whatever the order.
    #[serde(default)]
    pub swap_when_first_over_twelve: bool,
    /// Placeholders: `{day}`, `{month}`, `{year}`.
    pub template: String,
    #[serde(default)]
    pub day_style: DayStyle,
    /// The day 1 in cardinal style ("premier").
    #[serde(default)]
    pub first_day_word: Option<String>,
    #[serde(default)]
    pub month_capitalised: bool,
    /// A day or a year can follow a month name ("March 12", "March 2026").
    #[serde(default)]
    pub day_after_month: bool,
    pub year: YearRule,
    /// Words after which "12/03" is a day and a month and not a fraction.
    #[serde(default)]
    pub day_month_after: Vec<String>,
    /// Placeholders: `{day}`, `{month}`.
    #[serde(default)]
    pub day_month_template: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HourForms {
    #[serde(default)]
    pub zero: Option<String>,
    #[serde(default)]
    pub one: Option<String>,
    /// Placeholder `{hours}`.
    pub other: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AmPm {
    pub written: Vec<String>,
    pub spoken: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TimeRules {
    /// The letter between hours and minutes in "14h30", when the language writes it so.
    #[serde(default)]
    pub hour_marker: Option<String>,
    pub hour_forms: HourForms,
    pub on_the_hour: String,
    pub on_the_hour_ampm: String,
    pub with_minutes: String,
    pub with_minutes_ampm: String,
    pub with_minutes_below_ten: String,
    pub with_minutes_below_ten_ampm: String,
    #[serde(default)]
    pub ampm: Vec<AmPm>,
    /// Words after which "10.30" is a time and not a decimal.
    #[serde(default)]
    pub dot_after: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UnitRule {
    pub symbols: Vec<String>,
    pub singular: String,
    pub plural: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PerWord {
    pub written: Vec<String>,
    pub spoken: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CurrencyRule {
    pub symbols: Vec<String>,
    pub singular: String,
    pub plural: String,
    pub minor_singular: String,
    pub minor_plural: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CurrencyStyle {
    /// Said between the major and the minor amount ("and"), or empty.
    #[serde(default)]
    pub minor_joiner: String,
    /// Whether the minor unit is named when there is a major amount too.
    #[serde(default)]
    pub name_minor_with_major: bool,
    /// Words that mean a currency after an amount, to tell "10.30 euros" from a time.
    #[serde(default)]
    pub words: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DegreeRules {
    pub singular: String,
    pub plural: String,
    #[serde(default)]
    pub scales: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Glue {
    /// From this value, when the amount is a whole number of millions, a glue word follows.
    pub from: u64,
    pub before_vowel: String,
    pub otherwise: String,
    pub vowels: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Abbreviation {
    pub written: String,
    pub spoken: String,
    #[serde(default)]
    pub need: Need,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Feminine {
    pub one: String,
    pub one_feminine: String,
    pub nouns: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SentenceRules {
    /// Lower case words, without the dot, after which a dot never ends a sentence.
    #[serde(default)]
    pub never_end: Vec<String>,
    /// Words after which a dot ends the sentence only when a capital letter follows.
    #[serde(default)]
    pub ends_before_capital: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PhoneRules {
    /// A lone group of ten digits is read in pairs.
    #[serde(default)]
    pub ten_digits_in_pairs: bool,
}

/// How one identifier is read: its digits in groups, one by one inside each group.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct IdentifierReading {
    /// Group lengths, used when the text carries no separator.
    #[serde(default)]
    pub chunks: Vec<usize>,
    /// Group length when `chunks` is empty or too short.
    #[serde(default)]
    pub size: usize,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct IdentifierRules {
    /// By scheme id of the knowledge packs, then by kind. A scheme with no entry here is left to
    /// the number rules.
    #[serde(default)]
    pub readings: BTreeMap<String, IdentifierReading>,
    /// Said for a character of an e-mail address ("@", ".").
    #[serde(default)]
    pub email_symbols: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rules {
    pub locale: String,
    pub numbers: Numbers,
    pub ordinals: Ordinals,
    pub words: Words,
    /// Month names in lower case, January first.
    pub months: Vec<String>,
    pub dates: DateRules,
    pub times: TimeRules,
    pub units: Vec<UnitRule>,
    #[serde(default)]
    pub per_words: Vec<PerWord>,
    pub currencies: Vec<CurrencyRule>,
    pub currency_style: CurrencyStyle,
    pub degrees: DegreeRules,
    /// "1/2" -> "one half".
    #[serde(default)]
    pub fractions: BTreeMap<String, String>,
    #[serde(default)]
    pub glue: Option<Glue>,
    #[serde(default)]
    pub abbreviations: Vec<Abbreviation>,
    #[serde(default)]
    pub feminine: Option<Feminine>,
    #[serde(default)]
    pub sentences: SentenceRules,
    #[serde(default)]
    pub phones: PhoneRules,
    /// Acronyms said letter by letter.
    #[serde(default)]
    pub spell_acronyms: Vec<String>,
    /// The name of each letter, lower case key.
    #[serde(default)]
    pub letters: BTreeMap<String, String>,
    #[serde(default)]
    pub identifiers: IdentifierRules,
}

impl Rules {
    /// Structural checks that a hand-edited pack could break.
    fn validate(&self) -> Result<(), String> {
        if self.numbers.below_hundred.len() != 100 {
            return Err("numbers.below_hundred must hold exactly 100 words".into());
        }
        if self.months.len() != 12 {
            return Err("months must hold exactly 12 names".into());
        }
        if self.numbers.scales.is_empty() {
            return Err("numbers.scales is empty".into());
        }
        Ok(())
    }
}
