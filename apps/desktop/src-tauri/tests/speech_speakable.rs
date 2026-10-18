//! Read-aloud text preparation, phase 7: the rule packs, the scanner that turns numbers, dates,
//! times, units and abbreviations into words, the sentence splitter, the Markdown reader and the
//! `prepare` pipeline that joins them.
//!
//! The unit cases retype the cases of SpeechLab's `normalise.rs` and `sentences.rs`, which are the
//! specification of the rules (their code is not used). Not scanned by the language guard of the
//! Rust sources (`tests/` is not): the sentences are French where the rule pack is.
//!
//! Every name, number and identifier in this file is invented.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use assistant_cabinet_ai_lib::error::AppError;
use assistant_cabinet_ai_lib::knowledge::packs::PackSet;
use assistant_cabinet_ai_lib::speech::speakable::{
    prepare, prepare_for_locale, speak_sentence, NoteCode, Part, PartKind, Pause, Segment,
};
use assistant_cabinet_ai_lib::speech::speakable_numbers::{cardinal, ordinal, year};
use assistant_cabinet_ai_lib::speech::speakable_rules::{embedded_names, parsed, rules_for, Rules};
use assistant_cabinet_ai_lib::speech::speakable_scan::spoken_form;
use assistant_cabinet_ai_lib::speech::speakable_sentences::{split_sentences, MAX_SENTENCE_CHARS};

const PACKS: &[&str] = &["health-fr", "health-ch"];

struct Language {
    rules: &'static Rules,
    packs: PackSet,
}

fn french() -> &'static Language {
    static FRENCH: OnceLock<Language> = OnceLock::new();
    FRENCH.get_or_init(|| Language {
        rules: rules_for("fr-FR").expect("the French pack loads"),
        packs: PackSet::load("fr-FR", PACKS).expect("knowledge packs load"),
    })
}

fn english() -> &'static Language {
    static ENGLISH: OnceLock<Language> = OnceLock::new();
    ENGLISH.get_or_init(|| Language {
        rules: rules_for("en-US").expect("the English pack loads"),
        packs: PackSet::load("en-US", PACKS).expect("knowledge packs load"),
    })
}

fn fr(text: &str) -> String {
    spoken_form(text, french().rules, &french().packs)
}

fn en(text: &str) -> String {
    spoken_form(text, english().rules, &english().packs)
}

/// A whole sentence the way a segment is made: identifiers first, then the scanner.
fn say_fr(text: &str) -> String {
    speak_sentence(text, french().rules, &french().packs)
}

fn sentences_fr(text: &str) -> Vec<String> {
    split_sentences(text, french().rules, &french().packs)
        .into_iter()
        .map(|sentence| sentence.text)
        .collect()
}

fn markdown(text: &str) -> Part {
    Part {
        kind: PartKind::Markdown,
        text: text.to_string(),
    }
}

fn plain(text: &str) -> Part {
    Part {
        kind: PartKind::Plain,
        text: text.to_string(),
    }
}

fn reading_fr(parts: &[Part]) -> Vec<Segment> {
    prepare(parts, french().rules, &french().packs)
}

fn speech(segment: &Segment) -> (&str, Pause) {
    match segment {
        Segment::Speech { text, pause_after } => (text.as_str(), *pause_after),
        Segment::Note { .. } => panic!("a note where speech was expected"),
    }
}

fn texts(segments: &[Segment]) -> Vec<&str> {
    segments
        .iter()
        .filter_map(|segment| match segment {
            Segment::Speech { text, .. } => Some(text.as_str()),
            Segment::Note { .. } => None,
        })
        .collect()
}

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn speakable_sources() -> Vec<(String, String)> {
    let folder = manifest().join("src").join("speech");
    let mut found = Vec::new();
    for entry in fs::read_dir(&folder).expect("folder reads").flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("speakable") && name.ends_with(".rs") && !name.ends_with("tests.rs") {
            found.push((
                name,
                fs::read_to_string(entry.path()).expect("source reads"),
            ));
        }
    }
    found.sort();
    found
}

// -------------------------------------------------------------------------------------------------
// The packs
// -------------------------------------------------------------------------------------------------

#[test]
fn the_compiled_in_rule_packs_parse() {
    for (name, rules) in parsed() {
        assert!(rules.is_ok(), "{name}: {:?}", rules.as_ref().err());
    }
    assert_eq!(embedded_names(), ["fr-FR", "en-US"]);
}

#[test]
fn every_enabled_language_of_the_table_has_its_pack() {
    use assistant_cabinet_ai_lib::speech::languages::{Feature, LANGUAGES};
    for row in LANGUAGES
        .iter()
        .filter(|row| row.supports(Feature::ReadAloud))
    {
        for pack in row.rule_packs {
            assert!(
                embedded_names().contains(pack),
                "{} is enabled for read-aloud but its pack {pack} is not embedded",
                row.id
            );
        }
        assert!(rules_for(row.id).is_ok(), "{}", row.id);
    }
}

#[test]
fn a_locale_without_a_pack_answers_speech_locale_unsupported() {
    for locale in ["de-DE", "de-CH", "it-IT", "ja-JP", "", "fr"] {
        let error = rules_for(locale).expect_err(locale);
        assert_eq!(error.code(), "speech_locale_unsupported", "{locale}");
        let through = prepare_for_locale(&[plain("Bonjour.")], locale, &[]).expect_err(locale);
        assert_eq!(through.code(), "speech_locale_unsupported", "{locale}");
    }
    // Case does not matter, as everywhere in the language table.
    assert!(rules_for("fr-fr").is_ok());
}

#[test]
fn a_pack_ignores_the_keys_it_does_not_know() {
    // Phase 12 adds the dictation commands to the same files.
    let body = fs::read_to_string(manifest().join("resources/speech/rules/en-US.json")).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&body).unwrap();
    value["dictation_commands"] = serde_json::json!({ "new line": "\n" });
    value["a_future_section"] = serde_json::json!([1, 2, 3]);
    let parsed: Rules = serde_json::from_value(value).expect("unknown keys are ignored");
    assert_eq!(parsed.locale, "en-US");
}

#[test]
fn the_rule_packs_hold_no_sentence_for_a_human() {
    // Words, symbols and templates only: no key that looks like an interface message.
    for name in embedded_names() {
        let body =
            fs::read_to_string(manifest().join(format!("resources/speech/rules/{name}.json")))
                .unwrap();
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        let keys: Vec<&String> = value.as_object().unwrap().keys().collect();
        for key in keys {
            assert!(
                !key.contains("message") && !key.contains("error"),
                "{name}: {key}"
            );
        }
    }
}

// -------------------------------------------------------------------------------------------------
// Numbers
// -------------------------------------------------------------------------------------------------

#[test]
fn french_cardinals() {
    let cases: &[(u64, &str)] = &[
        (0, "zéro"),
        (1, "un"),
        (16, "seize"),
        (17, "dix-sept"),
        (21, "vingt et un"),
        (22, "vingt-deux"),
        (31, "trente et un"),
        (61, "soixante et un"),
        (70, "soixante-dix"),
        (71, "soixante et onze"),
        (72, "soixante-douze"),
        (77, "soixante-dix-sept"),
        (80, "quatre-vingts"),
        (81, "quatre-vingt-un"),
        (90, "quatre-vingt-dix"),
        (91, "quatre-vingt-onze"),
        (99, "quatre-vingt-dix-neuf"),
        (100, "cent"),
        (101, "cent un"),
        (180, "cent quatre-vingts"),
        (200, "deux cents"),
        (201, "deux cent un"),
        (280, "deux cent quatre-vingts"),
        (500, "cinq cents"),
        (1000, "mille"),
        (1001, "mille un"),
        (2000, "deux mille"),
        (2045, "deux mille quarante-cinq"),
        (2026, "deux mille vingt-six"),
        (80_000, "quatre-vingt mille"),
        (200_000, "deux cent mille"),
        (1_000_000, "un million"),
        (2_500_000, "deux millions cinq cent mille"),
        (1_998, "mille neuf cent quatre-vingt-dix-huit"),
    ];
    for (n, want) in cases {
        assert_eq!(cardinal(french().rules, *n), *want, "{n}");
    }
}

#[test]
fn english_cardinals_and_years() {
    let rules = english().rules;
    assert_eq!(cardinal(rules, 0), "zero");
    assert_eq!(cardinal(rules, 21), "twenty-one");
    assert_eq!(cardinal(rules, 100), "one hundred");
    assert_eq!(
        cardinal(rules, 1234),
        "one thousand two hundred thirty-four"
    );
    assert_eq!(cardinal(rules, 2_000_005), "two million five");
    assert_eq!(year(rules, 2026), "twenty twenty-six");
    assert_eq!(year(rules, 1998), "nineteen ninety-eight");
    assert_eq!(year(rules, 1905), "nineteen oh five");
    assert_eq!(year(rules, 2005), "two thousand five");
    assert_eq!(year(rules, 2000), "two thousand");
    assert_eq!(year(rules, 2010), "twenty ten");
    assert_eq!(ordinal(rules, 12, false), "twelfth");
    assert_eq!(ordinal(rules, 21, false), "twenty-first");
    assert_eq!(ordinal(rules, 40, false), "fortieth");
}

#[test]
fn ordinals() {
    assert_eq!(fr("le 1er mars"), "le premier mars");
    assert_eq!(fr("la 1re fois"), "la première fois");
    assert_eq!(
        fr("le 2e étage, la 3ème porte, le 5e, le 9e"),
        "le deuxième étage, la troisième porte, le cinquième, le neuvième"
    );
    assert_eq!(
        fr("le 21e, le 12e, le 80e, le 100e"),
        "le vingt et unième, le douzième, le quatre-vingtième, le centième"
    );
    assert_eq!(
        en("the 1st, 2nd, 3rd and 12th"),
        "the first, second, third and twelfth"
    );
    assert_eq!(
        en("the 21st and the 100th"),
        "the twenty-first and the one hundredth"
    );
}

#[test]
fn decimals_thousands_and_negatives() {
    assert_eq!(fr("12,5"), "douze virgule cinq");
    assert_eq!(fr("0,25"), "zéro virgule vingt-cinq");
    assert_eq!(fr("0,05"), "zéro virgule zéro cinq");
    assert_eq!(fr("3.5"), "trois virgule cinq");
    assert_eq!(fr("1 000"), "mille");
    assert_eq!(fr("2 500 patients"), "deux mille cinq cents patients");
    assert_eq!(fr("1.250"), "mille deux cent cinquante");
    assert_eq!(en("12.5"), "twelve point five");
    assert_eq!(en("0.25"), "zero point two five");
    assert_eq!(en("1,250 cases"), "one thousand two hundred fifty cases");
    assert_eq!(fr("-5"), "moins cinq");
    assert_eq!(fr("de 5-10 jours"), "de cinq à dix jours");
    assert_eq!(en("5-10 days"), "five to ten days");
    assert_eq!(fr("007"), "zéro zéro sept");
}

#[test]
fn numbers_have_no_grouping_left_in_what_is_spoken() {
    // "numbers without grouping": a grouped amount is one number, never groups said apart.
    assert_eq!(
        fr("12 345,67"),
        "douze mille trois cent quarante-cinq virgule soixante-sept"
    );
    assert_eq!(
        en("12,345.67"),
        "twelve thousand three hundred forty-five point six seven"
    );
    for spoken in [fr("1 234 567"), en("1,234,567")] {
        assert!(!spoken.chars().any(|c| c.is_ascii_digit()), "{spoken}");
    }
}

#[test]
fn units_follow_the_plural_rule_of_each_language() {
    assert_eq!(
        fr("500 mg trois fois par jour"),
        "cinq cents milligrammes trois fois par jour"
    );
    assert_eq!(fr("1 mg"), "un milligramme");
    assert_eq!(fr("1,5 g"), "un virgule cinq gramme");
    assert_eq!(fr("2,5 g"), "deux virgule cinq grammes");
    assert_eq!(fr("10 ml"), "dix millilitres");
    assert_eq!(fr("75 kg"), "soixante-quinze kilogrammes");
    assert_eq!(fr("37,5 °C"), "trente-sept virgule cinq degrés Celsius");
    assert_eq!(fr("-5 °C"), "moins cinq degrés Celsius");
    assert_eq!(fr("120 mmHg"), "cent vingt millimètres de mercure");
    assert_eq!(fr("72 bpm"), "soixante-douze battements par minute");
    assert_eq!(fr("5 mg/kg"), "cinq milligrammes par kilogramme");
    assert_eq!(fr("10 mg/jour"), "dix milligrammes par jour");
    assert_eq!(fr("2 000 000 g"), "deux millions de grammes");
    assert_eq!(en("500 mg"), "five hundred milligrams");
    assert_eq!(en("1 mg"), "one milligram");
    assert_eq!(en("1.5 g"), "one point five grams");
    assert_eq!(en("98.6 °F"), "ninety-eight point six degrees Fahrenheit");
    // Not units.
    assert_eq!(fr("5 m'a dit"), "cinq m'a dit");
    assert_eq!(fr("3 s'est"), "trois s'est");
}

#[test]
fn percent_and_currency() {
    assert_eq!(fr("50 %"), "cinquante pour cent");
    assert_eq!(fr("12,5%"), "douze virgule cinq pour cent");
    assert_eq!(en("50%"), "fifty percent");
    assert_eq!(fr("50 €"), "cinquante euros");
    assert_eq!(fr("1 €"), "un euro");
    assert_eq!(fr("12,50 €"), "douze euros cinquante");
    assert_eq!(fr("0,50 €"), "cinquante centimes");
    assert_eq!(fr("€50"), "cinquante euros");
    assert_eq!(fr("100 CHF"), "cent francs");
    assert_eq!(fr("1 000 000 €"), "un million d'euros");
    assert_eq!(en("$12.50"), "twelve dollars and fifty cents");
    assert_eq!(en("$1"), "one dollar");
    assert_eq!(en("£5"), "five pounds");
}

#[test]
fn french_times() {
    assert_eq!(fr("à 14 h 30"), "à quatorze heures trente");
    assert_eq!(fr("à 14h30"), "à quatorze heures trente");
    assert_eq!(fr("à 9h00"), "à neuf heures");
    assert_eq!(fr("à 9h05"), "à neuf heures cinq");
    assert_eq!(fr("à 14h"), "à quatorze heures");
    assert_eq!(fr("à 1 h"), "à une heure");
    assert_eq!(fr("à 14:30"), "à quatorze heures trente");
    assert_eq!(fr("rendez-vous à 10.30"), "rendez-vous à dix heures trente");
    assert_eq!(
        fr("entre 10.30 et 11.30"),
        "entre dix heures trente et onze heures trente"
    );
    // A dot after no time word, or before an amount, is a decimal.
    assert_eq!(fr("version 2.15"), "version deux virgule quinze");
    assert_eq!(fr("de 12.50 euros"), "de douze virgule cinquante euros");
    assert_eq!(fr("5 heures"), "cinq heures");
}

#[test]
fn english_times() {
    assert_eq!(en("at 9 am"), "at nine a m");
    assert_eq!(en("at 9:30 pm"), "at nine thirty p m");
    assert_eq!(en("at 14:30"), "at fourteen thirty");
    assert_eq!(en("at 9:00"), "at nine o'clock");
    assert_eq!(en("at 9:05"), "at nine oh five");
    assert_eq!(en("at 10.30"), "at ten thirty");
    assert_eq!(en("version 2.15"), "version two point one five");
    assert_eq!(en("5 p.m."), "five p m.");
    assert_eq!(
        en("the meeting is at 5 p.m."),
        "the meeting is at five p m."
    );
}

#[test]
fn dates() {
    assert_eq!(fr("le 12 mars 2026"), "le douze mars deux mille vingt-six");
    assert_eq!(fr("le 1 mars 2026"), "le premier mars deux mille vingt-six");
    assert_eq!(fr("le 12/03/2026"), "le douze mars deux mille vingt-six");
    assert_eq!(fr("le 1/3/2026"), "le premier mars deux mille vingt-six");
    assert_eq!(fr("le 12.03.2026"), "le douze mars deux mille vingt-six");
    assert_eq!(fr("le 2026-03-12"), "le douze mars deux mille vingt-six");
    assert_eq!(
        fr("le 12 janv. 2026"),
        "le douze janvier deux mille vingt-six"
    );
    assert_eq!(en("March 12th, 2026"), "March twelfth, twenty twenty-six");
    assert_eq!(en("March 12, 2026"), "March twelfth, twenty twenty-six");
    assert_eq!(en("12 March 2026"), "twelfth March twenty twenty-six");
    assert_eq!(fr("le 12/03"), "le douze mars");
    assert_eq!(en("on 03/12/2026"), "on March twelfth, twenty twenty-six");
    assert_eq!(
        en("on 25/12/2026"),
        "on December twenty-fifth, twenty twenty-six"
    );
    assert_eq!(en("March 2026"), "March twenty twenty-six");
}

#[test]
fn fractions_and_multipliers() {
    assert_eq!(fr("1/2 comprimé"), "un demi comprimé");
    assert_eq!(fr("3/4 de litre"), "trois quarts de litre");
    assert_eq!(fr("note 12/20"), "note douze sur vingt");
    assert_eq!(en("1/2 tablet"), "one half tablet");
    assert_eq!(fr("3 x par jour"), "trois fois par jour");
    assert_eq!(fr("2x/jour"), "deux fois par jour");
    assert_eq!(fr("3 x 500 mg"), "trois fois cinq cents milligrammes");
    assert_eq!(en("3x daily"), "three times daily");
}

#[test]
fn feminine_one_before_common_feminine_nouns() {
    assert_eq!(fr("1 fois par jour"), "une fois par jour");
    assert_eq!(fr("21 gélules"), "vingt et une gélules");
    assert_eq!(fr("1 comprimé"), "un comprimé");
    assert_eq!(fr("2 fois"), "deux fois");
}

#[test]
fn abbreviations_and_titles() {
    // Titles come from the knowledge packs (`title_spoken`), the rest from the rule pack.
    assert_eq!(
        fr("M. Martin et Mme Roux voient le Dr Dupont"),
        "monsieur Martin et madame Roux voient le docteur Dupont"
    );
    assert_eq!(
        fr("le Pr. Leroy à St. Gall"),
        "le professeur Leroy à saint Gall"
    );
    assert_eq!(fr("des pommes, etc."), "des pommes, et cetera.");
    assert_eq!(
        fr("des pommes, etc. et des poires"),
        "des pommes, et cetera et des poires"
    );
    assert_eq!(
        fr("facture n° 2045"),
        "facture numéro deux mille quarante-cinq"
    );
    assert_eq!(
        fr("facture N°2045"),
        "facture numéro deux mille quarante-cinq"
    );
    assert_eq!(fr("voir p. 12"), "voir page douze");
    assert_eq!(fr("env. 5 jours"), "environ cinq jours");
    assert_eq!(fr("tél. 021"), "téléphone zéro deux un");
    assert_eq!(fr("c.-à-d. demain"), "c'est-à-dire demain");
    assert_eq!(
        en("Mr. Smith and Dr. Jones, e.g. today"),
        "mister Smith and doctor Jones, for example today"
    );
    assert_eq!(en("Dr Jones"), "doctor Jones");
    // Look-alikes are untouched.
    assert_eq!(fr("la drogue et le prix"), "la drogue et le prix");
    assert_eq!(fr("Pris le matin"), "Pris le matin");
    assert_eq!(fr("Me voici"), "Me voici");
    assert_eq!(fr("Me Dupont plaide"), "maître Dupont plaide");
}

#[test]
fn acronyms_in_the_list_are_spelled_and_others_left_alone() {
    assert_eq!(fr("une IRM demain"), "une i erre emme demain");
    assert_eq!(fr("la TVA"), "la té vé a");
    assert_eq!(fr("la CPAM répond"), "la CPAM répond");
    assert_eq!(en("an MRI today"), "an em ar eye today");
}

#[test]
fn phone_numbers_and_identifiers_by_shape() {
    assert_eq!(
        fr("06 12 34 56 78"),
        "zéro six, douze, trente-quatre, cinquante-six, soixante-dix-huit"
    );
    assert_eq!(
        fr("0612345678"),
        "zéro six, douze, trente-quatre, cinquante-six, soixante-dix-huit"
    );
    assert_eq!(
        fr("079 123 45 67"),
        "zéro sept neuf, un deux trois, quarante-cinq, soixante-sept"
    );
    assert_eq!(
        fr("+41 79 123 45 67"),
        "plus quarante et un, soixante-dix-neuf, un deux trois, quarante-cinq, soixante-sept"
    );
    assert_eq!(
        fr("756.1234.5678.97"),
        "sept cinq six, un deux trois quatre, cinq six sept huit, neuf sept"
    );
    assert_eq!(
        en("call 555 123 4567 now"),
        "call five five five, one two three, four five six seven now"
    );
    // A plain amount with thousands groups is a number, not an identifier.
    assert_eq!(fr("1 000 000"), "un million");
    assert_eq!(
        fr("123 456"),
        "cent vingt-trois mille quatre cent cinquante-six"
    );
}

#[test]
fn the_sentences_the_owner_listened_to() {
    // Coqui css10 speaks no digit without the rewriting: a date, a time, a dose, a percentage, an
    // amount, a telephone number and a health insurance number, on every voice.
    assert_eq!(
        fr("Prendre 500 mg trois fois par jour."),
        "Prendre cinq cents milligrammes trois fois par jour."
    );
    assert_eq!(
        fr("La facture numéro 2045 est payable sous 30 jours."),
        "La facture numéro deux mille quarante-cinq est payable sous trente jours."
    );
    assert_eq!(
        fr("Rendez-vous le 12 mars à 9h00."),
        "Rendez-vous le douze mars à neuf heures."
    );
    assert_eq!(
        fr("Le patient pèse 72,5 kg et mesure 1,80 m."),
        "Le patient pèse soixante-douze virgule cinq kilogrammes et mesure un virgule quatre-vingts mètre."
    );
    assert_eq!(
        fr("La tension est de 120/80 mmHg."),
        "La tension est de cent vingt sur quatre-vingts millimètres de mercure."
    );
    assert_eq!(
        fr("Le taux est de 12,5 % soit 1 250 €."),
        "Le taux est de douze virgule cinq pour cent soit mille deux cent cinquante euros."
    );
    assert_eq!(
        say_fr("Appeler le 06 12 34 56 78."),
        "Appeler le zéro six, douze, trente-quatre, cinquante-six, soixante-dix-huit."
    );
    assert_eq!(
        say_fr("Numéro AVS 756.1234.5678.97."),
        "Numéro a vé esse sept cinq six, un deux trois quatre, cinq six sept huit, neuf sept."
    );
}

#[test]
fn spaces_and_unknown_text_are_kept_sensible() {
    assert_eq!(fr("Bonjour tout le monde"), "Bonjour tout le monde");
    assert_eq!(fr(""), "");
    assert_eq!(fr("  vitamine B12  "), "vitamine B douze");
    assert_eq!(fr("(3)"), "(trois)");
    assert_eq!(fr("3, 4 et 5."), "trois, quatre et cinq.");
}

// -------------------------------------------------------------------------------------------------
// Identifiers of the knowledge packs
// -------------------------------------------------------------------------------------------------

#[test]
fn identifiers_of_the_active_packs_are_read_in_groups() {
    // A Swiss health insurance number, continuous or grouped as written.
    let groups = "sept cinq six, un deux trois quatre, cinq six sept huit, neuf sept";
    assert_eq!(
        say_fr("AVS 756.1234.5678.97"),
        format!("a vé esse {groups}")
    );
    assert_eq!(say_fr("AVS 7561234567897"), format!("a vé esse {groups}"));
    // An IBAN, in groups of four: letters by name, digits one by one.
    assert_eq!(
        say_fr("IBAN CH93 0076 2011 6238 5295 7"),
        "IBAN cé hache neuf trois, zéro zéro sept six, deux zéro un un, six deux trois huit, cinq deux neuf cinq, sept"
    );
    // An invoice number: its letters, then its digits.
    assert_eq!(
        say_fr("Facture FA-2026-0012 payée"),
        "Facture effe a, deux zéro deux six, zéro zéro un deux payée"
    );
    // An e-mail address, with the marks said in words.
    assert_eq!(
        say_fr("Ecrire à jean.dupont@example.org demain"),
        "Ecrire à jean point dupont arobase example point org demain"
    );
    assert_eq!(
        speak_sentence(
            "write to jean.dupont@example.org",
            english().rules,
            &english().packs
        ),
        "write to jean dot dupont at example dot org"
    );
}

// -------------------------------------------------------------------------------------------------
// Sentences
// -------------------------------------------------------------------------------------------------

#[test]
fn plain_sentences() {
    assert_eq!(
        sentences_fr("Bonjour. Comment allez-vous ? Très bien !"),
        ["Bonjour.", "Comment allez-vous ?", "Très bien !"]
    );
    assert_eq!(
        sentences_fr("Sans ponctuation finale"),
        ["Sans ponctuation finale"]
    );
    assert!(sentences_fr("   \n\n  ").is_empty());
    assert!(sentences_fr("").is_empty());
}

#[test]
fn abbreviations_do_not_end_a_sentence() {
    assert_eq!(
        sentences_fr("M. Martin voit le Dr Dupont. Mme Roux arrive."),
        ["M. Martin voit le Dr Dupont.", "Mme Roux arrive."]
    );
    assert_eq!(
        sentences_fr("Voir le Pr. Leroy, St. Gall, env. 5 jours. Fin."),
        ["Voir le Pr. Leroy, St. Gall, env. 5 jours.", "Fin."]
    );
    assert_eq!(
        sentences_fr("Le 12 janv. 2026, il pleut. Fin."),
        ["Le 12 janv. 2026, il pleut.", "Fin."]
    );
    assert_eq!(
        sentences_fr("J. Dupont est là. Oui."),
        ["J. Dupont est là.", "Oui."]
    );
    let english = english();
    let split = |text: &str| -> Vec<String> {
        split_sentences(text, english.rules, &english.packs)
            .into_iter()
            .map(|s| s.text)
            .collect()
    };
    assert_eq!(
        split("Mr. Smith met Dr. Jones vs. Mrs. Lee. Done."),
        ["Mr. Smith met Dr. Jones vs. Mrs. Lee.", "Done."]
    );
    assert_eq!(
        split("Use a tool, e.g. a hammer. Then rest."),
        ["Use a tool, e.g. a hammer.", "Then rest."]
    );
}

#[test]
fn etc_ends_a_sentence_only_before_a_capital() {
    assert_eq!(
        sentences_fr("Des pommes, des poires, etc. Puis il partit."),
        ["Des pommes, des poires, etc.", "Puis il partit."]
    );
    assert_eq!(
        sentences_fr("Des pommes, etc. et des poires."),
        ["Des pommes, etc. et des poires."]
    );
    assert_eq!(sentences_fr("Des pommes etc."), ["Des pommes etc."]);
}

#[test]
fn decimals_times_and_numbers_stay_inside() {
    assert_eq!(
        sentences_fr("Prendre 12,5 mg à 14:30. Puis 9h00 ou 10.30 demain."),
        ["Prendre 12,5 mg à 14:30.", "Puis 9h00 ou 10.30 demain."]
    );
    assert_eq!(
        sentences_fr("La facture n° 2045 est payée. Merci."),
        ["La facture n° 2045 est payée.", "Merci."]
    );
    // A number that ends a sentence does end it.
    assert_eq!(
        sentences_fr("Il y en a 3. Puis 4."),
        ["Il y en a 3.", "Puis 4."]
    );
}

#[test]
fn list_numbers_at_the_start_are_not_sentence_ends() {
    assert_eq!(
        sentences_fr("1. Prendre le matin\n2. Prendre le soir"),
        ["1. Prendre le matin", "2. Prendre le soir"]
    );
}

#[test]
fn ellipses_follow_the_next_word() {
    assert_eq!(
        sentences_fr("Je pense... peut-être. Oui."),
        ["Je pense... peut-être.", "Oui."]
    );
    assert_eq!(
        sentences_fr("Alors... Il partit."),
        ["Alors...", "Il partit."]
    );
    assert_eq!(sentences_fr("Alors… Il partit."), ["Alors…", "Il partit."]);
    assert_eq!(sentences_fr("Vraiment?! Oui."), ["Vraiment?!", "Oui."]);
}

#[test]
fn quotes_and_guillemets_belong_to_the_sentence() {
    assert_eq!(
        sentences_fr("Il dit : « Viens ! » Puis il partit."),
        ["Il dit : « Viens ! »", "Puis il partit."]
    );
    assert_eq!(
        sentences_fr("Il dit : « Viens ! », puis il partit."),
        ["Il dit : « Viens ! », puis il partit."]
    );
    assert_eq!(
        sentences_fr("(Voir ci-dessous.) Suite."),
        ["(Voir ci-dessous.)", "Suite."]
    );
    assert_eq!(
        sentences_fr("« Tu viens ? » demanda-t-il. Oui."),
        ["« Tu viens ? » demanda-t-il.", "Oui."]
    );
}

#[test]
fn line_breaks_and_paragraphs() {
    let sentences = split_sentences(
        "Ordonnance\nAmoxicilline 500 mg\n\nSuite du dossier. Fin.\r\n\r\nDernier",
        french().rules,
        &french().packs,
    );
    let texts: Vec<&str> = sentences.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "Ordonnance",
            "Amoxicilline 500 mg",
            "Suite du dossier.",
            "Fin.",
            "Dernier"
        ]
    );
    let ends: Vec<bool> = sentences.iter().map(|s| s.paragraph_end).collect();
    assert_eq!(ends, [false, true, false, true, true]);
}

#[test]
fn punctuation_only_pieces_are_not_sentences() {
    assert_eq!(
        sentences_fr("Bonjour. ... Merci."),
        ["Bonjour. ...", "Merci."]
    );
    assert_eq!(sentences_fr("- \n Texte."), ["Texte."]);
}

#[test]
fn a_long_sentence_is_cut_at_a_clause_mark_below_the_cap() {
    let clause = "le médecin a examiné le dossier avec soin";
    let long = std::iter::repeat_n(clause, 12)
        .collect::<Vec<_>>()
        .join(", ");
    assert!(long.chars().count() > MAX_SENTENCE_CHARS * 2);
    let pieces = split_sentences(&long, french().rules, &french().packs);
    assert!(pieces.len() >= 3, "{}", pieces.len());
    for (index, piece) in pieces.iter().enumerate() {
        assert!(piece.text.chars().count() <= MAX_SENTENCE_CHARS, "{index}");
        let last = index + 1 == pieces.len();
        assert_eq!(piece.continues, !last, "{index}");
        if !last {
            assert!(piece.text.ends_with(','), "{}", piece.text);
        }
    }
    // Words are never lost by the cut.
    let rejoined: Vec<&str> = pieces
        .iter()
        .flat_map(|p| p.text.split_whitespace())
        .collect();
    assert_eq!(rejoined, long.split_whitespace().collect::<Vec<_>>());
    // And a text with no mark at all is cut at a space.
    let words = std::iter::repeat_n("mot", 200)
        .collect::<Vec<_>>()
        .join(" ");
    let pieces = split_sentences(&words, french().rules, &french().packs);
    assert!(pieces.len() >= 2);
    assert!(pieces
        .iter()
        .all(|p| p.text.chars().count() <= MAX_SENTENCE_CHARS));
}

// -------------------------------------------------------------------------------------------------
// Markdown and the pipeline
// -------------------------------------------------------------------------------------------------

#[test]
fn an_answer_is_stripped_of_its_marks() {
    let answer = "## Résultat\n\nLa facture **FA-2026-0012** du *12 mars 2026* est de 1 250 €.\n\n\
                  - Voir [le document](https://example.org/doc) page 3\n\
                  - Fichier `C:\\Users\\invente\\Docs\\devis-2026_03.pdf` ✅\n\n\
                  > Une citation 🙂 utile.";
    let segments = reading_fr(&[markdown(answer)]);
    let said = texts(&segments);
    assert_eq!(said[0], "Résultat.");
    assert_eq!(
        said[1],
        "La facture effe a, deux zéro deux six, zéro zéro un deux du douze mars deux mille vingt-six est de mille deux cent cinquante euros."
    );
    assert_eq!(said[2], "Voir le document page trois.");
    assert_eq!(said[3], "Fichier devis deux mille vingt-six zéro trois.");
    assert_eq!(said[4], "Une citation utile.");
    let joined = said.join(" ");
    for forbidden in [
        "*", "`", "#", "http", "[", "]", "(", "✅", "🙂", "\\", ".pdf",
    ] {
        assert!(!joined.contains(forbidden), "{forbidden} in {joined}");
    }
}

#[test]
fn pauses_follow_sentences_and_paragraphs() {
    let segments = reading_fr(&[markdown(
        "Première phrase. Deuxième phrase.\n\nNouveau paragraphe.",
    )]);
    let pauses: Vec<Pause> = segments.iter().map(|s| speech(s).1).collect();
    assert_eq!(
        pauses,
        [Pause::Sentence, Pause::Paragraph, Pause::Paragraph]
    );
    // A heading and the end of a part close a paragraph.
    let segments = reading_fr(&[markdown("# Titre\nTexte"), plain("Dernier")]);
    let pauses: Vec<Pause> = segments.iter().map(|s| speech(s).1).collect();
    assert_eq!(
        pauses,
        [Pause::Paragraph, Pause::Paragraph, Pause::Paragraph]
    );
}

#[test]
fn lists_become_sentences_one_per_item() {
    let segments = reading_fr(&[markdown(
        "Pièces à fournir :\n\n1. Une pièce d'identité\n2. Un justificatif\n- une ordonnance\n   de moins de 3 mois",
    )]);
    assert_eq!(
        texts(&segments),
        [
            "Pièces à fournir :",
            "Une pièce d'identité.",
            "Un justificatif.",
            "une ordonnance de moins de trois mois."
        ]
    );
}

#[test]
fn a_small_table_is_read_row_by_row() {
    let table = "| Fournisseur | Montant |\n|---|---|\n| Alpha | 120 € |\n| Beta | 80,50 € |";
    let segments = reading_fr(&[markdown(table)]);
    assert_eq!(
        texts(&segments),
        [
            "Fournisseur: Alpha, Montant: cent vingt euros.",
            "Fournisseur: Beta, Montant: quatre-vingts euros cinquante."
        ]
    );
}

#[test]
fn a_large_table_is_replaced_by_a_note() {
    let mut table = String::from("Avant le tableau.\n\n| A | B |\n|---|---|\n");
    for row in 0..12 {
        table.push_str(&format!("| {row} | x |\n"));
    }
    table.push_str("\nAprès le tableau.");
    let segments = reading_fr(&[markdown(&table)]);
    assert_eq!(segments.len(), 3);
    assert_eq!(speech(&segments[0]).0, "Avant le tableau.");
    assert_eq!(
        segments[1],
        Segment::Note {
            code: NoteCode::TableOmitted,
            count: 12
        }
    );
    assert_eq!(speech(&segments[2]).0, "Après le tableau.");
    // Too many columns is large too.
    let wide = "| 1 | 2 | 3 | 4 | 5 | 6 |\n|---|---|---|---|---|---|\n| a | b | c | d | e | f |";
    let segments = reading_fr(&[markdown(wide)]);
    assert_eq!(
        segments,
        [Segment::Note {
            code: NoteCode::TableOmitted,
            count: 1
        }]
    );
}

#[test]
fn a_block_of_code_is_replaced_by_a_note() {
    let segments = reading_fr(&[markdown(
        "Avant.\n\n```\nlet x = 1;\nlet y = 2;\n```\n\nAprès.",
    )]);
    assert_eq!(
        segments[1],
        Segment::Note {
            code: NoteCode::CodeOmitted,
            count: 2
        }
    );
    assert_eq!(texts(&segments), ["Avant.", "Après."]);
}

#[test]
fn addresses_are_dropped_and_paths_become_the_file_name() {
    let said = |text: &str| texts(&reading_fr(&[plain(text)])).join(" ");
    assert_eq!(
        said("Voir https://example.org/a/b?c=1 et www.example.org pour le détail."),
        "Voir et pour le détail."
    );
    assert_eq!(
        said("Le fichier est dans C:\\Users\\invente\\Documents\\Clients\\dupont-2026.pdf."),
        "Le fichier est dans dupont deux mille vingt-six."
    );
    assert_eq!(
        said("Le fichier est dans /home/invente/clients/devis_12.docx maintenant."),
        "Le fichier est dans devis douze maintenant."
    );
    // A date with slashes is not a path.
    assert_eq!(
        said("Le 12/03/2026 à 9h00."),
        "Le douze mars deux mille vingt-six à neuf heures."
    );
}

#[test]
fn emoji_and_symbols_are_never_spoken() {
    let said = |text: &str| texts(&reading_fr(&[plain(text)])).join(" ");
    assert_eq!(
        said("Bravo 🎉🎉 pour ce résultat ✔ !"),
        "Bravo pour ce résultat !"
    );
    assert_eq!(said("A → B — fin"), "A B, fin.");
}

#[test]
fn parts_are_read_in_order_with_a_pause_between_them() {
    let segments = reading_fr(&[
        markdown("Réponse du jour."),
        plain("Le total est de 30 €."),
        plain("Attention : un nombre n'a pas pu être vérifié."),
    ]);
    assert_eq!(
        texts(&segments),
        [
            "Réponse du jour.",
            "Le total est de trente euros.",
            "Attention : un nombre n'a pas pu être vérifié."
        ]
    );
    assert!(segments.iter().all(|s| speech(s).1 == Pause::Paragraph));
}

#[test]
fn a_part_with_nothing_to_say_gives_nothing() {
    assert!(reading_fr(&[]).is_empty());
    assert!(reading_fr(&[markdown("")]).is_empty());
    assert!(reading_fr(&[markdown("---\n\n***")]).is_empty());
    assert!(reading_fr(&[plain("  \n\n ")]).is_empty());
    assert!(reading_fr(&[markdown("🎉 ✅")]).is_empty());
}

#[test]
fn preparing_for_a_locale_goes_through_the_language_table() {
    let segments = prepare_for_locale(
        &[markdown("Dose : **2 mg**, 3 fois par jour.")],
        "fr-FR",
        &["health-fr"],
    )
    .expect("french prepares");
    assert_eq!(
        texts(&segments),
        ["Dose : deux milligrammes, trois fois par jour."]
    );
    let segments = prepare_for_locale(&[markdown("Dose: **2 mg**, 3 times a day.")], "en-US", &[])
        .expect("english prepares");
    assert_eq!(
        texts(&segments),
        ["Dose: two milligrams, three times a day."]
    );
    // An unknown pack id is the knowledge base's own refusal, passed on.
    let error = prepare_for_locale(&[plain("Bonjour.")], "fr-FR", &["no-such-pack"])
        .expect_err("unknown pack");
    assert!(
        matches!(error, AppError::KnowledgePackInvalid { .. }),
        "{error:?}"
    );
}

#[test]
fn the_segments_serialise_the_way_the_interface_reads_them() {
    let segments = reading_fr(&[markdown("Bonjour.\n\n```\nx\n```")]);
    let value = serde_json::to_value(&segments).unwrap();
    assert_eq!(
        value,
        serde_json::json!([
            { "kind": "speech", "text": "Bonjour.", "pauseAfter": "paragraph" },
            { "kind": "note", "code": "code_omitted", "count": 1 }
        ])
    );
    let parts: Vec<Part> =
        serde_json::from_str(r#"[{"kind":"markdown","text":"a"},{"kind":"plain","text":"b"}]"#)
            .unwrap();
    assert_eq!(parts[0].kind, PartKind::Markdown);
    assert_eq!(parts[1].kind, PartKind::Plain);
}

#[test]
fn preparing_twice_changes_nothing() {
    // Idempotence: what was already words is left as it is.
    let samples = [
        "Prendre 500 mg trois fois par jour. Rendez-vous le 12 mars à 9h00, soit 30 % du total de 1 250 €.",
        "Le Dr Martin rappelle le 06 12 34 56 78 pour la facture n° 2045.",
        "Pièces : une IRM, la TVA et 3 gélules, 2,5 mg/kg.",
    ];
    for sample in samples {
        let once = fr(sample);
        assert!(!once.chars().any(|c| c.is_ascii_digit()), "{once}");
        assert_eq!(fr(&once), once, "{sample}");
    }
    for sample in [
        "Take 5 mg at 9:30 am on March 12th, 2026 for $12.50.",
        "See p. 12, 3x daily.",
    ] {
        let once = en(sample);
        assert!(!once.chars().any(|c| c.is_ascii_digit()), "{once}");
        assert_eq!(en(&once), once, "{sample}");
    }
}

#[test]
fn nothing_the_reading_gives_back_holds_markup_or_a_digit_when_a_number_was_written() {
    let segments = reading_fr(&[markdown(
        "# Synthèse 2026\n\n- Total : **1 250,50 €** (dont 20 % de TVA)\n- Échéance : 31/12/2026 à 17h45\n- Contact : 04 78 12 34 56",
    )]);
    for text in texts(&segments) {
        assert!(!text.chars().any(|c| c.is_ascii_digit()), "{text}");
        assert!(!text.contains(['*', '#', '|', '`']), "{text}");
    }
}

// -------------------------------------------------------------------------------------------------
// What the sources must not do
// -------------------------------------------------------------------------------------------------

#[test]
fn the_speakable_sources_are_ascii_and_english() {
    let sources = speakable_sources();
    assert!(sources.len() >= 6, "{} files", sources.len());
    for (name, text) in sources {
        assert!(text.is_ascii(), "{name} holds a character above ASCII");
    }
}

#[test]
fn speakable_text_is_not_model_facing() {
    // The spoken text is derived by code and never by the model: no gateway, no provider, no
    // prompt, no chat, no embedding anywhere in the files that prepare it.
    for (name, text) in speakable_sources() {
        for token in [
            "gateway",
            "Gateway",
            "AIProvider",
            "ollama",
            "Ollama",
            "openai",
            "prompt",
            ".embed(",
            "crate::chat",
            "crate::conversation",
            "crate::retrieval",
        ] {
            assert!(!text.contains(token), "{name} names {token}");
        }
    }
    let commands = fs::read_to_string(manifest().join("src/speech_commands.rs")).unwrap();
    let body = commands
        .split("pub fn speech_prepare_reading")
        .nth(1)
        .expect("the command exists");
    assert!(!body.contains("gateway") && !body.contains("provider"));
}

#[test]
fn spoken_forms_never_leave_the_selection() {
    // The optional step that says names as the knowledge base spells them is not built: nothing in
    // the preparation opens the index, reads a name or resolves a selection, so no name that is
    // not already in the text can appear in what is spoken.
    for (name, text) in speakable_sources() {
        for token in [
            "open_index",
            "surface_forms",
            "QuerySelection",
            "resolve_selection",
            "rusqlite",
            "index_store",
            "spoken_form_of",
        ] {
            assert!(!text.contains(token), "{name} names {token}");
        }
    }
    let commands = fs::read_to_string(manifest().join("src/speech_commands.rs")).unwrap();
    let body = commands
        .split("pub fn speech_prepare_reading")
        .nth(1)
        .expect("the command exists");
    assert!(!body.contains("scope") && !body.contains("open_index"));
}

#[test]
fn the_speakable_files_name_no_network_client() {
    for (name, text) in speakable_sources() {
        for token in [
            "reqwest",
            "ureq",
            "hyper",
            "isahc",
            "surf::",
            "std::net",
            "TcpStream",
            "UdpSocket",
            "TcpListener",
        ] {
            assert!(!text.contains(token), "{name} names {token}");
        }
    }
}

#[test]
fn no_language_is_named_in_the_logic() {
    // The language table names the languages; the logic reads their packs. A comparison with a
    // language code or a locale literal in these files would be an `if fr / else en` in disguise.
    for (name, text) in speakable_sources() {
        if name == "speakable_rules.rs" {
            continue;
        }
        for token in [
            "\"fr\"",
            "\"en\"",
            "\"fr-FR\"",
            "\"en-US\"",
            "Lang::Fr",
            "Lang::En",
            "Language::",
        ] {
            assert!(!text.contains(token), "{name} names {token}");
        }
    }
}

#[test]
fn the_command_is_registered_and_has_no_setup_hook_behind_it() {
    let lib = fs::read_to_string(manifest().join("src/lib.rs")).unwrap();
    assert!(lib.contains("speech_commands::speech_prepare_reading"));
    assert!(!lib.contains(".setup("), "a setup hook would run at launch");
    let commands = fs::read_to_string(manifest().join("src/speech_commands.rs")).unwrap();
    assert!(commands.contains("pub fn speech_prepare_reading"));
    assert!(commands.contains("reading_allowed"));
    // A command that takes plain arguments over a function: no engine and no worker in its body.
    let body = commands
        .split("pub fn speech_prepare_reading")
        .nth(1)
        .unwrap();
    assert!(!body.contains("SpeechState") && !body.contains("worker"));
    assert!(Path::new(&manifest().join("resources/speech/rules/fr-FR.json")).exists());
}
