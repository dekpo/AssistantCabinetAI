//! How the knowledge base compares names: the normal forms of `knowledge::normalize` and the
//! phonetic keys of `knowledge::phonetic`.
//!
//! These are pure functions, so the tests are tables. Accented names are written as the people
//! who own them write them; this file is not scanned by the language guard of the Rust sources
//! (tests/ is not), and a test about accents that avoided accents would prove nothing.
//!
//! The phonetic expectations live in `tests/fixtures/knowledge/phonetic-fr.json`, read at run
//! time, so the owner can add a name and re-run without touching Rust:
//!
//! ```text
//! cargo test --test knowledge_names -- --nocapture
//! ```
//!
//! Every name here is invented or too common to designate anyone.

use std::collections::BTreeMap;
use std::path::Path;

use assistant_cabinet_ai_lib::knowledge::normalize::{
    filename_name_candidates, fold, normalize_identifier, normalize_name, normalize_name_with,
    sound_form, TitleSet, WeakTitles,
};
use assistant_cabinet_ai_lib::knowledge::phonetic::{
    bounded_damerau_levenshtein, encoder_for_locale, EnglishPhonetic, FrenchPhonetic,
    PhoneticEncoder,
};
use serde_json::Value;

// ----------------------------------------------------------------------------- fixtures

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("knowledge")
        .join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{name} is not valid JSON: {error}"))
}

fn words(value: &Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .unwrap_or_else(|| panic!("`{key}` is missing"))
        .iter()
        .map(|word| word.as_str().expect("a string").to_string())
        .collect()
}

fn french_words() -> TitleSet {
    let file = fixture("name-words-fr.json");
    TitleSet::new()
        .with_titles(words(&file, "titles"))
        .with_weak_titles(words(&file, "weak_titles"))
        .with_particles(words(&file, "particles"))
        .with_stop_words(words(&file, "stop_words"))
}

fn key_of(encoder: &dyn PhoneticEncoder, name: &str, set: &TitleSet) -> String {
    encoder.encode_name(&normalize_name(name, set))
}

// ----------------------------------------------------------------------------- normalisation

#[test]
fn every_spelling_of_a_name_has_one_order_free_key() {
    let set = french_words();
    let expected = normalize_name("Jean Dupont", &set).token_set_key;
    assert_eq!(expected, "dupont jean");

    for spelling in [
        "Jean Dupont",
        "jean dupont",
        "JEAN DUPONT",
        "Jean  Dupont",
        "  Jean\tDupont\n",
        "Jean-Dupont",
        "Jean_Dupont",
        "Dupont, Jean",
        "DUPONT Jean",
        "Dupont/Jean",
        "Dr Jean Dupont",
        "Dr. Jean Dupont",
        "Docteur Jean Dupont",
        "Mme Jean Dupont",
    ] {
        assert_eq!(
            normalize_name(spelling, &set).token_set_key,
            expected,
            "{spelling:?}"
        );
    }
}

#[test]
fn the_display_form_is_never_touched() {
    let set = french_words();
    for text in ["Jean  Dupont ", "DUPONT, Jean", "d’Aubigné", "Œuvre"] {
        assert_eq!(normalize_name(text, &set).display, text);
    }
}

#[test]
fn accents_and_their_two_encodings_are_one_name() {
    let set = french_words();
    let composed = normalize_name("René Lefèvre", &set);
    let decomposed = normalize_name("Rene\u{301} Lefe\u{300}vre", &set);
    let plain = normalize_name("Rene Lefevre", &set);

    assert_eq!(composed.token_set_key, "lefevre rene");
    assert_eq!(composed.token_set_key, decomposed.token_set_key);
    assert_eq!(composed.token_set_key, plain.token_set_key);
    assert_eq!(composed.sounds_like, decomposed.sounds_like);
    assert_eq!(fold("Ésaïe"), fold("E\u{301}sai\u{308}e"));
}

#[test]
fn ligatures_become_two_letters() {
    let set = french_words();
    assert_eq!(normalize_name("Cœur", &set).joined, "coeur");
    assert_eq!(normalize_name("Œuvre Æther", &set).joined, "oeuvre aether");
}

#[test]
fn every_kind_of_apostrophe_is_a_separator_and_the_elision_is_kept_apart() {
    let set = french_words();
    for apostrophe in ["'", "’", "‘", "`", "´", "ʼ"] {
        let name = normalize_name(&format!("d{apostrophe}Aubigné"), &set);
        assert_eq!(name.joined, "d aubigne", "{apostrophe:?}");
        assert_eq!(name.token_set_key, "aubigne");
        assert_eq!(name.compact, "daubigne");
    }

    let hospital = normalize_name("l'Hôpital", &set);
    assert_eq!(hospital.tokens, ["l", "hopital"]);
    assert_eq!(hospital.joined, "l hopital");
    assert_eq!(hospital.without_elision, "hopital");
    assert!(
        hospital.initials.is_empty(),
        "an elided article is not an initial"
    );
    assert_eq!(hospital.sounds_like, ["hopital"]);

    assert_eq!(
        normalize_name("O'Brien", &set).compact,
        normalize_name("OBrien", &set).compact
    );
}

#[test]
fn hyphens_underscores_dots_and_slashes_separate_and_digits_stay() {
    let set = french_words();
    assert_eq!(
        normalize_name("Jean-Pierre", &set).tokens,
        ["jean", "pierre"]
    );
    assert_eq!(
        normalize_name("jean_pierre.dupont/x", &set).tokens,
        ["jean", "pierre", "dupont", "x"]
    );
    assert_eq!(
        normalize_name("Studio 54 (Paris)", &set).tokens,
        ["studio", "54", "paris"]
    );
    assert!(normalize_name("... --- ???", &set).tokens.is_empty());
}

#[test]
fn titles_are_removed_and_remembered() {
    let set = french_words();
    for title in [
        "Dr",
        "Dr.",
        "DR",
        "Docteur",
        "docteur",
        "Pr",
        "Professeur",
        "Mme",
        "Madame",
        "Mlle",
        "Mr",
        "Monsieur",
    ] {
        let name = normalize_name(&format!("{title} Martin"), &set);
        assert_eq!(name.joined, "martin", "{title}");
        assert_eq!(name.titles.len(), 1, "{title}");
    }
    let two = normalize_name("Professeur Docteur Martin", &set);
    assert_eq!(two.titles, ["professeur", "docteur"]);
    assert_eq!(two.joined, "martin");
}

#[test]
fn me_and_m_are_titles_only_where_a_title_can_stand() {
    let set = french_words();

    // A title: capital letter, first, followed by a capitalised word.
    assert_eq!(normalize_name("Me Martin", &set).titles, ["me"]);
    assert_eq!(normalize_name("M. Dupont", &set).titles, ["m"]);
    assert_eq!(normalize_name("M Dupont", &set).joined, "dupont");

    // Not a title: a pronoun, a middle initial, a trailing initial.
    for text in [
        "me martin",
        "Appelle me Martin",
        "Jean M Dupont",
        "Dupont Jean M",
        "ME",
    ] {
        let name = normalize_name(text, &set);
        assert!(name.titles.is_empty(), "{text}: {:?}", name.titles);
    }
    assert_eq!(normalize_name("Jean M Dupont", &set).initials, ["m"]);

    // The extractor, which sees the sentence, can overrule.
    assert_eq!(
        normalize_name_with("me martin", &set, WeakTitles::Always).titles,
        ["me"]
    );
    assert!(normalize_name_with("Me Martin", &set, WeakTitles::Never)
        .titles
        .is_empty());
}

#[test]
fn particles_stay_in_the_tokens_and_leave_the_key() {
    let set = french_words();
    let spaced = normalize_name("Pierre du Pont", &set);
    let joined = normalize_name("Pierre Dupont", &set);

    assert_eq!(spaced.tokens, ["pierre", "du", "pont"]);
    assert_eq!(spaced.token_set_key, "pierre pont");
    assert_eq!(spaced.compact, "pierredupont");
    // "du pont" and "dupont" share a compact form but are a possible match, never an exact one.
    assert_eq!(spaced.compact, joined.compact);
    assert_ne!(spaced.token_set_key, joined.token_set_key);

    let van = normalize_name("Ludwig van Beethoven", &set);
    assert_eq!(van.token_set_key, "beethoven ludwig");
    assert_eq!(normalize_name("Le Gall", &set).token_set_key, "gall");
}

#[test]
fn initials_are_flagged_and_never_part_of_the_key() {
    let set = french_words();
    let abbreviated = normalize_name("J. Dupont", &set);
    assert_eq!(abbreviated.initials, ["j"]);
    assert_eq!(abbreviated.token_set_key, "dupont");
    assert_eq!(abbreviated.tokens, ["j", "dupont"]);
    assert_eq!(abbreviated.sounds_like.len(), 2);

    let two = normalize_name("J.-P. Dupont", &set);
    assert_eq!(two.initials, ["j", "p"]);
    assert_eq!(two.token_set_key, "dupont");

    // A digit alone is not an initial.
    assert!(normalize_name("Lot 7", &set).initials.is_empty());
}

#[test]
fn an_empty_title_set_still_normalises() {
    let bare = TitleSet::new();
    let name = normalize_name("Dr Jean Dupont", &bare);
    assert_eq!(name.titles, Vec::<String>::new());
    assert_eq!(name.token_set_key, "dr dupont jean");
}

// ----------------------------------------------------------------------------- identifiers

#[test]
fn identifiers_lose_their_formatting_and_nothing_else() {
    let iban = "FR76 3000 4028 3798 7654 3210 943";
    let expected = "FR7630004028379876543210943";
    for spelling in [
        iban,
        "fr7630004028379876543210943",
        "FR76-3000-4028-3798-7654-3210-943",
        "fr76.3000.4028.3798.7654.3210.943",
        " FR76\u{a0}3000 4028 3798 7654 3210 943 ",
    ] {
        assert_eq!(normalize_identifier(spelling), expected, "{spelling:?}");
    }

    assert_eq!(normalize_identifier("FA-2026-0042"), "FA20260042");
    assert_eq!(normalize_identifier("fa 2026/0042"), "FA20260042");
    assert_eq!(normalize_identifier("+33 6 12.34.56.78"), "33612345678");
    assert_eq!(normalize_identifier("06 12 34 56 78"), "0612345678");
    assert_eq!(normalize_identifier("Réf-É1"), "REFE1");
    assert_eq!(normalize_identifier(""), "");
    // Different numbers stay different.
    assert_ne!(
        normalize_identifier("FA-2026-0042"),
        normalize_identifier("FA-2026-0043")
    );
}

// ----------------------------------------------------------------------------- file names

#[test]
fn a_sanitised_file_name_yields_ordered_name_candidates() {
    let set = french_words();

    assert_eq!(
        filename_name_candidates("Dupont_Jean_2026-03", &set),
        [["dupont", "jean"], ["jean", "dupont"]]
    );
    assert_eq!(
        filename_name_candidates("Jean-Dupont", &set),
        [["jean", "dupont"], ["dupont", "jean"]]
    );
    // A single word is a single candidate; three words keep their order.
    assert_eq!(filename_name_candidates("Dupont_2026", &set), [["dupont"]]);
    assert_eq!(
        filename_name_candidates("Jean_Pierre_Dupont", &set),
        [["jean", "pierre", "dupont"]]
    );
    // Stop-words, titles and anything with a digit end a run.
    assert_eq!(
        filename_name_candidates("Facture_Dr_Martin_2026-03-12_scan", &set),
        [["martin"]]
    );
    assert_eq!(
        filename_name_candidates("Martin_Facture_Durand", &set),
        [["martin"], ["durand"]]
    );
    assert!(filename_name_candidates("2026-03-12", &set).is_empty());
    assert!(filename_name_candidates("Compte_rendu_mars", &set).is_empty());
    assert!(filename_name_candidates("", &set).is_empty());
}

// ----------------------------------------------------------------------------- phonetics

fn contract() -> Value {
    fixture("phonetic-fr.json")
}

fn names_in(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("a list of names")
        .iter()
        .map(|name| name.as_str().expect("a string").to_string())
        .collect()
}

/// The rows of one contract file that the encoder does not respect.
fn broken_rows(file: &str, encoder: &dyn PhoneticEncoder, set: &TitleSet) -> Vec<String> {
    let contract = fixture(file);
    let mut broken: Vec<String> = Vec::new();

    for group in contract["collide"].as_array().expect("collide") {
        let names = names_in(group);
        let keys: Vec<String> = names.iter().map(|n| key_of(encoder, n, set)).collect();
        if keys.iter().any(|key| key != &keys[0]) {
            let shown: Vec<String> = names
                .iter()
                .zip(&keys)
                .map(|(name, key)| format!("{name} = {key}"))
                .collect();
            broken.push(format!("should collide: {}", shown.join(", ")));
        }
    }
    for pair in contract["differ"].as_array().expect("differ") {
        let names = names_in(pair);
        let (a, b) = (
            key_of(encoder, &names[0], set),
            key_of(encoder, &names[1], set),
        );
        if a == b {
            broken.push(format!(
                "should differ: {} and {} are both {a}",
                names[0], names[1]
            ));
        }
    }
    // A pair that no rule can separate must leave its list once a rule does, or the list goes stale.
    for list in ["known_shared", "false_friends"] {
        for pair in contract[list].as_array().into_iter().flatten() {
            let names = names_in(pair);
            let (a, b) = (
                key_of(encoder, &names[0], set),
                key_of(encoder, &names[1], set),
            );
            if a != b {
                broken.push(format!(
                    "{list} is stale: {} ({a}) and {} ({b}) no longer share a key",
                    names[0], names[1]
                ));
            }
        }
    }
    broken
}

#[test]
fn the_phonetic_contract_holds() {
    let broken = broken_rows("phonetic-fr.json", &FrenchPhonetic, &french_words());
    assert!(
        broken.is_empty(),
        "phonetic-fr.json is not respected:\n{}",
        broken.join("\n")
    );
}

#[test]
fn the_english_phonetic_contract_holds() {
    let broken = broken_rows("phonetic-en.json", &EnglishPhonetic, &TitleSet::new());
    assert!(
        broken.is_empty(),
        "phonetic-en.json is not respected:\n{}",
        broken.join("\n")
    );
}

#[test]
fn a_phonetic_key_is_ascii_and_never_empty_for_a_name() {
    let set = french_words();
    let contract = contract();
    for name in names_in(&contract["observe"]) {
        let key = key_of(&FrenchPhonetic, &name, &set);
        assert!(!key.is_empty(), "{name}");
        assert!(key.is_ascii(), "{name}: {key}");
    }
    for group in contract["collide"].as_array().unwrap() {
        for name in names_in(group) {
            assert!(key_of(&FrenchPhonetic, &name, &set).is_ascii(), "{name}");
        }
    }
    let english = fixture("phonetic-en.json");
    for name in names_in(&english["observe"]) {
        let key = key_of(&EnglishPhonetic, &name, &TitleSet::new());
        assert!(!key.is_empty(), "{name}");
        assert!(key.is_ascii(), "{name}: {key}");
    }
}

#[test]
fn a_name_made_of_a_particle_a_title_or_a_long_elision_still_has_a_key() {
    let set = french_words();
    // Found on the Insee list: 75 surnames had an empty key.
    for name in [
        "Maitre",
        "Le",
        "Le Du",
        "Le Maitre",
        "Floc'h",
        "Le Floc'H",
        "Du",
    ] {
        assert!(!key_of(&FrenchPhonetic, name, &set).is_empty(), "{name}");
    }
    // A one-letter elision is still dropped; a longer word before an apostrophe is a name.
    assert_eq!(
        key_of(&FrenchPhonetic, "l'Hopital", &set),
        key_of(&FrenchPhonetic, "Hopital", &set)
    );
    assert_eq!(
        key_of(&FrenchPhonetic, "Floc'h", &set),
        key_of(&FrenchPhonetic, "Floc", &set)
    );
}

#[test]
fn the_endings_that_tell_two_people_apart_stay_sounded() {
    let set = french_words();
    let key = |name: &str| key_of(&FrenchPhonetic, name, &set);
    // A final mute "e" after a vowel and "l" marks the feminine form.
    for (masculine, feminine) in [
        ("Michel", "Michèle"),
        ("Paul", "Paule"),
        ("Pascal", "Pascale"),
        ("Daniel", "Danièle"),
        ("Raphaël", "Raphaële"),
        ("Noël", "Noële"),
        ("Frédéric", "Frédérique"),
    ] {
        assert_ne!(key(masculine), key(feminine), "{masculine} / {feminine}");
    }
    // The same feminine name written with one l or two is one name.
    assert_eq!(key("Nicole"), key("Nicolle"));
    assert_eq!(key("Odile"), key("Odille"));
    // A sounded final s, d or m.
    for (silent, sounded) in [
        ("Ana", "Anas"),
        ("Elia", "Elias"),
        ("Gilda", "Gildas"),
        ("Anna", "Anass"),
        ("Amy", "Hamid"),
        ("Karin", "Karim"),
        ("Marian", "Mariam"),
        ("Ilan", "Ilham"),
    ] {
        assert_ne!(key(silent), key(sounded), "{silent} / {sounded}");
    }
    // ... and the ones that stay silent.
    assert_eq!(key("Thomas"), key("Thoma"));
    assert_eq!(key("Lucas"), key("Luca"));
    assert_eq!(key("Schmitt"), key("Schmidt"));
    assert_eq!(key("Gay"), key("Gai"));
    assert_eq!(key("Leroy"), key("Leroi"));
}

#[test]
fn the_english_key_is_ascii_never_empty_and_drops_silent_letters() {
    let english = TitleSet::new();
    for name in [
        "John", "Joan", "O'Brien", "McLean", "Hugh", "Knight", "Wright", "Xavier",
    ] {
        let key = key_of(&EnglishPhonetic, name, &english);
        assert!(!key.is_empty() && key.is_ascii(), "{name}: {key}");
    }
    assert_eq!(
        key_of(&EnglishPhonetic, "Knight", &english),
        key_of(&EnglishPhonetic, "Night", &english)
    );
    assert_eq!(
        key_of(&EnglishPhonetic, "Wright", &english),
        key_of(&EnglishPhonetic, "Right", &english)
    );
}

#[test]
fn a_cedilla_and_its_absence_meet_on_the_name_they_come_from() {
    let set = french_words();
    // Typed with or without the cedilla, as a document and as a sanitised file name.
    for (written, bare) in [("François", "Francois"), ("Françoise", "Francoise")] {
        assert_eq!(
            key_of(&FrenchPhonetic, written, &set),
            key_of(&FrenchPhonetic, bare, &set)
        );
    }
    // A cedilla in either Unicode form.
    assert_eq!(
        key_of(&FrenchPhonetic, "Fran\u{e7}ois", &set),
        key_of(&FrenchPhonetic, "Franc\u{327}ois", &set)
    );
}

#[test]
fn a_whole_name_is_encoded_word_by_word_without_titles_or_particles() {
    let set = french_words();
    let dupont = FrenchPhonetic.encode_token("dupont");
    let jean = FrenchPhonetic.encode_token("jean");

    assert_eq!(
        key_of(&FrenchPhonetic, "Jean Dupont", &set),
        format!("{jean}-{dupont}")
    );
    assert_eq!(
        key_of(&FrenchPhonetic, "Dr Jean Dupont", &set),
        format!("{jean}-{dupont}")
    );
    assert_eq!(
        key_of(&FrenchPhonetic, "Jean du Dupont", &set),
        format!("{jean}-{dupont}")
    );
    assert_eq!(key_of(&FrenchPhonetic, "Dupont", &set), dupont);
    assert_eq!(key_of(&FrenchPhonetic, "", &set), "");
}

#[test]
fn the_english_key_is_selected_by_the_locale() {
    let set = TitleSet::new();
    let encoder = EnglishPhonetic;
    assert_eq!(
        key_of(&encoder, "Smith", &set),
        key_of(&encoder, "Smyth", &set)
    );
    assert_ne!(
        key_of(&encoder, "Smith", &set),
        key_of(&encoder, "Jones", &set)
    );
    assert_eq!(encoder.signature(), "en-rules:1");
    assert_eq!(encoder_for_locale("en-US").signature(), encoder.signature());
    assert_eq!(
        encoder_for_locale("fr-FR").signature(),
        FrenchPhonetic.signature()
    );
}

/// Not an assertion: prints which names the encoder treats as the same sound, so the owner can
/// read the table and edit `phonetic-fr.json`. Run it with `-- --nocapture`.
#[test]
fn print_the_phonetic_collision_table() {
    let set = french_words();
    let encoder = FrenchPhonetic;
    let contract = contract();

    let mut every_name: Vec<String> = Vec::new();
    for list in ["collide", "differ"] {
        for group in contract[list].as_array().unwrap() {
            every_name.extend(names_in(group));
        }
    }
    every_name.extend(names_in(&contract["observe"]));
    every_name.sort();
    every_name.dedup();

    let mut by_key: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for name in &every_name {
        by_key
            .entry(key_of(&encoder, name, &set))
            .or_default()
            .push(name.clone());
    }

    println!();
    println!(
        "French phonetic key ({}), {} names, {} keys",
        encoder.signature(),
        every_name.len(),
        by_key.len()
    );
    println!("{:<18} names that sound alike", "key");
    println!("{:-<18} {:-<40}", "", "");
    for (key, names) in by_key.iter().filter(|(_, names)| names.len() > 1) {
        println!("{key:<18} {}", names.join(" = "));
    }
    println!();
    println!("{:<18} names with a key of their own", "key");
    println!("{:-<18} {:-<40}", "", "");
    for (key, names) in by_key.iter().filter(|(_, names)| names.len() == 1) {
        println!("{key:<18} {}", names[0]);
    }
    println!();

    assert!(!by_key.is_empty());
}

// ----------------------------------------------------------------------------- distance

#[test]
fn the_edit_distance_counts_one_per_edit_and_stops_at_the_bound() {
    let d = |a: &str, b: &str, max: usize| bounded_damerau_levenshtein(a, b, max);

    assert_eq!(d("dupont", "dupont", 0), Some(0));
    assert_eq!(d("dupont", "dupond", 1), Some(1)); // substitution
    assert_eq!(d("dupont", "dupon", 1), Some(1)); // deletion
    assert_eq!(d("dupont", "duponts", 1), Some(1)); // insertion
    assert_eq!(d("dupont", "dupnot", 1), Some(1)); // transposition of two neighbours
    assert_eq!(d("dupont", "dumond", 1), None);
    assert_eq!(d("dupont", "dumond", 2), Some(2));
    assert_eq!(d("", "", 0), Some(0));
    assert_eq!(d("", "ab", 2), Some(2));
    assert_eq!(d("ab", "", 1), None);
    assert_eq!(d("kitten", "sitting", 3), Some(3));
    assert_eq!(d("kitten", "sitting", 2), None);
    // Characters, not bytes.
    assert_eq!(d("é", "e", 1), Some(1));
    assert_eq!(d("héctor", "hector", 1), Some(1));
    // A length gap above the bound is rejected without looking at the letters.
    assert_eq!(d("a", "abcdefghij", 3), None);
}

// ----------------------------------------------------------------------------- properties

/// A small deterministic generator, so the property tests need no dependency and fail the same
/// way twice.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn pick<'a>(&mut self, items: &'a [&'a str]) -> &'a str {
        items[(self.next() as usize) % items.len()]
    }
}

fn random_text(rng: &mut Lcg) -> String {
    const PIECES: &[&str] = &[
        "Jean", "dupont", "DUPOND", "René", "Lefèvre", "d'", "l’", "Aubigné", "Dr", "Dr.", "Me",
        "M.", "J.", "du", "de", "la", "Le", "van", "Œuvre", "ç", "Çà", "e\u{301}", "-", "_", ".",
        "/", ",", " ", "  ", "42", "O'Brien", "x", "Z", "ß", "İ", "\t", "(", ")", "&",
    ];
    let count = 1 + rng.next() % 7;
    (0..count)
        .map(|_| rng.pick(PIECES))
        .collect::<Vec<_>>()
        .join(if rng.next() % 2 == 0 { " " } else { "" })
}

#[test]
fn normalising_is_idempotent_and_encoding_is_deterministic() {
    let set = french_words();
    let mut rng = Lcg(2026);

    for _ in 0..2_000 {
        let text = random_text(&mut rng);

        // `fold` of a folded text is itself.
        assert_eq!(fold(&fold(&text)), fold(&text), "{text:?}");

        // Normalising the joined form again gives the same joined form and the same key.
        let once = normalize_name(&text, &set);
        let twice = normalize_name(&once.joined, &set);
        assert_eq!(once.joined, twice.joined, "{text:?}");
        assert_eq!(once.compact, twice.compact, "{text:?}");

        // Encoding is deterministic and its alphabet is plain ASCII.
        let key = FrenchPhonetic.encode_name(&once);
        assert_eq!(
            key,
            FrenchPhonetic.encode_name(&normalize_name(&text, &set)),
            "{text:?}"
        );
        assert!(key.is_ascii(), "{text:?}: {key}");

        // The sound form is a fixed point as well.
        assert_eq!(
            sound_form(&sound_form(&text)),
            sound_form(&text),
            "{text:?}"
        );
    }
}

#[test]
fn the_edit_distance_is_symmetric_and_bounded() {
    let mut rng = Lcg(7);
    for _ in 0..500 {
        let a = sound_form(&random_text(&mut rng));
        let b = sound_form(&random_text(&mut rng));
        for max in 0..4 {
            let forward = bounded_damerau_levenshtein(&a, &b, max);
            assert_eq!(
                forward,
                bounded_damerau_levenshtein(&b, &a, max),
                "{a:?} {b:?} {max}"
            );
            if let Some(distance) = forward {
                assert!(distance <= max);
            }
        }
        assert_eq!(bounded_damerau_levenshtein(&a, &a, 0), Some(0));
    }
}
