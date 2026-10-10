//! The lexicon packs, read as the application reads them: every bundled pack in every locale, the
//! rules that keep them words-and-patterns only, and a printed summary the owner can read.
//!
//! ```text
//! cargo test --test knowledge_packs -- --nocapture
//! ```
//!
//! prints one line per pack and locale (titles, particles, stop-words, identifier schemes,
//! columns, relations, roles, detection terms) and the identifier patterns run on a few invented
//! strings. Not scanned by the language guard of the Rust sources (tests/ is not): the sample
//! strings are French where the pack is.
//!
//! Every identifier, name and number in this file is invented.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use assistant_cabinet_ai_lib::error::AppError;
use assistant_cabinet_ai_lib::knowledge::packs::{
    available_pack_ids, bundled_packs, canonical_pack_id, is_domain_country_id, IdentifierStrength,
    PackSet, BASE_PACK_ID,
};
use assistant_cabinet_ai_lib::knowledge::RoleId;
use serde_json::Value;

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

fn files_under(directory: &Path, extension: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(directory).expect("readable directory") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            found.extend(files_under(&path, extension));
        } else if path.extension().is_some_and(|e| e == extension) {
            found.push(path);
        }
    }
    found.sort();
    found
}

fn locales() -> Vec<&'static str> {
    vec!["fr-FR", "en-US"]
}

// ----------------------------------------------------------------------------- loading

#[test]
fn every_pack_loads_in_every_locale_alone_and_together() {
    let all = available_pack_ids();
    assert_eq!(all, ["accounting-fr", "health-ch", "health-fr", "legal-fr"]);
    for locale in locales() {
        PackSet::load(locale, &[]).unwrap_or_else(|error| panic!("base {locale}: {error:?}"));
        for id in &all {
            PackSet::load(locale, &[id]).unwrap_or_else(|error| panic!("{id} {locale}: {error:?}"));
        }
        // Every combination, in both orders: a header or a scheme two packs disagree about would
        // only show when they are active together.
        PackSet::load(locale, &all).unwrap_or_else(|error| panic!("all packs {locale}: {error:?}"));
        let reversed: Vec<&str> = all.iter().rev().copied().collect();
        PackSet::load(locale, &reversed)
            .unwrap_or_else(|error| panic!("all packs reversed {locale}: {error:?}"));
    }
}

#[test]
fn every_pack_exists_in_both_locales_and_the_files_match_the_registry() {
    let root = manifest().join("resources").join("knowledge");
    let on_disk: BTreeSet<(String, String)> = files_under(&root, "json")
        .iter()
        .map(|path| {
            let locale = path.file_stem().unwrap().to_string_lossy().to_string();
            let pack = path
                .parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_string_lossy();
            (pack.to_string(), locale)
        })
        .collect();
    let bundled: BTreeSet<(String, String)> = bundled_packs()
        .into_iter()
        .map(|(id, locale)| (id.to_string(), locale.to_string()))
        .collect();
    assert_eq!(
        on_disk, bundled,
        "a file under resources/knowledge that the loader does not embed, or the reverse"
    );
    for id in available_pack_ids().into_iter().chain([BASE_PACK_ID]) {
        for locale in locales() {
            assert!(
                bundled.contains(&(id.to_string(), locale.to_string())),
                "{id} has no {locale} file"
            );
        }
    }
}

#[test]
fn a_pack_keeps_the_same_shape_in_both_languages() {
    for id in available_pack_ids().into_iter().chain([BASE_PACK_ID]) {
        let active: Vec<&str> = if id == BASE_PACK_ID { vec![] } else { vec![id] };
        let french = PackSet::load("fr-FR", &active).unwrap();
        let english = PackSet::load("en-US", &active).unwrap();
        let roles = |set: &PackSet| -> Vec<String> {
            set.roles()
                .map(|label| label.id.as_str().to_string())
                .collect()
        };
        let schemes = |set: &PackSet| -> Vec<(String, bool)> {
            set.identifier_schemes()
                .iter()
                .map(|scheme| (scheme.kind.clone(), scheme.personal_key))
                .collect()
        };
        assert_eq!(roles(&french), roles(&english), "{id}: roles differ");
        assert_eq!(
            french.identifier_schemes().len(),
            english.identifier_schemes().len(),
            "{id}: one language has more identifier schemes"
        );
        assert_eq!(
            schemes(&french)
                .iter()
                .filter(|(_, personal)| *personal)
                .count(),
            schemes(&english)
                .iter()
                .filter(|(_, personal)| *personal)
                .count(),
            "{id}: personal keys differ"
        );
    }
}

// ----------------------------------------------------------------------------- words only

/// Every string a list of a pack holds, with the JSON path, so the failure names the entry.
fn list_entries(pack: &Value, file: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let mut take = |path: String, value: &Value| {
        if let Some(text) = value.as_str() {
            entries.push((format!("{file} {path}"), text.to_string()));
        }
    };
    for field in ["weak_titles", "particles", "stop_words", "detection_terms"] {
        for (index, value) in pack[field].as_array().into_iter().flatten().enumerate() {
            take(format!("{field}[{index}]"), value);
        }
    }
    for (type_id, words) in pack["titles"].as_object().into_iter().flatten() {
        for (index, value) in words.as_array().into_iter().flatten().enumerate() {
            take(format!("titles.{type_id}[{index}]"), value);
        }
    }
    for side in ["prefix", "suffix"] {
        for (index, value) in pack["org_markers"][side]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            take(format!("org_markers.{side}[{index}]"), value);
        }
    }
    for (index, column) in pack["columns"].as_array().into_iter().flatten().enumerate() {
        for (position, value) in column["headers"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            take(format!("columns[{index}].headers[{position}]"), value);
        }
    }
    for (title, spoken) in pack["title_spoken"].as_object().into_iter().flatten() {
        take(format!("title_spoken.{title}"), spoken);
    }
    entries
}

#[test]
fn a_pack_holds_words_and_phrases_never_a_sentence() {
    let root = manifest().join("resources").join("knowledge");
    let mut checked = 0;
    for path in files_under(&root, "json") {
        let pack: Value = serde_json::from_str(&read(&path)).expect("valid JSON");
        let file = path.strip_prefix(&root).unwrap().display().to_string();
        for (place, text) in list_entries(&pack, &file) {
            let words = text.split_whitespace().count();
            assert!(
                (1..=4).contains(&words),
                "{place}: {words} words is a sentence, not a word: {text:?}"
            );
            assert!(
                !text.contains(['?', '!', ';', ':']) && !text.trim_end().ends_with('.'),
                "{place}: punctuation of a sentence: {text:?}"
            );
            checked += 1;
        }
        // Nothing but the known fields: a stray "description" or "message" would be prose.
        let known: BTreeSet<&str> = [
            "schema",
            "id",
            "locale",
            "titles",
            "weak_titles",
            "particles",
            "title_spoken",
            "stop_words",
            "org_markers",
            "identifier_schemes",
            "columns",
            "relations",
            "roles",
            "detection_terms",
        ]
        .into_iter()
        .collect();
        for key in pack.as_object().unwrap().keys() {
            assert!(known.contains(key.as_str()), "{file}: unknown field {key}");
        }
    }
    assert!(checked > 300, "only {checked} entries checked");
}

#[test]
fn every_role_label_key_exists_in_both_catalogues() {
    let catalogue = |locale: &str| -> Value {
        serde_json::from_str(&read(
            &manifest()
                .join("..")
                .join("src")
                .join("locales")
                .join(format!("{locale}.json")),
        ))
        .expect("valid catalogue")
    };
    let catalogues: Vec<(String, Value)> = locales()
        .iter()
        .map(|locale| (locale.to_string(), catalogue(locale)))
        .collect();
    let all = available_pack_ids();
    for locale in locales() {
        let packs = PackSet::load(locale, &all).unwrap();
        for label in packs.roles() {
            for (name, value) in &catalogues {
                let mut node = value;
                for segment in label.label_key.split('.') {
                    node = &node[segment];
                }
                assert!(
                    node.as_str().is_some_and(|text| !text.trim().is_empty()),
                    "{}: {} (role {}) has no sentence in {name}",
                    label.pack,
                    label.label_key,
                    label.id.as_str()
                );
            }
        }
    }
}

// ----------------------------------------------------------------------------- content

#[test]
fn the_base_pack_knows_the_neutral_roles_and_no_profession() {
    for locale in locales() {
        let base = PackSet::load(locale, &[]).unwrap();
        let roles: BTreeSet<&str> = base.roles().map(|label| label.id.as_str()).collect();
        for neutral in [
            "client",
            "provider",
            "counterparty",
            "sender",
            "recipient",
            "author",
            "supplier",
        ] {
            assert!(
                roles.contains(neutral),
                "{locale}: base lacks role {neutral}"
            );
        }
        // The base keeps its label keys neutral: no domain segment.
        for label in base.roles() {
            assert_eq!(label.pack, BASE_PACK_ID);
            assert_eq!(
                label.label_key,
                format!("knowledge.roles.{}", label.id.as_str())
            );
        }
        for rule in base.column_rules() {
            assert_eq!(rule.pack, BASE_PACK_ID);
        }
        // Nothing the base maps depends on a profession's vocabulary.
        for profession in [
            "patient",
            "ordonnance",
            "avocat",
            "lawyer",
            "dossier medical",
        ] {
            assert!(
                base.column_rule_for(profession).is_none(),
                "{locale}: the base maps {profession}"
            );
        }
    }
}

#[test]
fn a_domain_pack_only_adds_and_never_changes_a_type() {
    for locale in locales() {
        let base = PackSet::load(locale, &[]).unwrap();
        for id in available_pack_ids() {
            let with = PackSet::load(locale, &[id]).unwrap();
            // Everything the base maps is still mapped the same way.
            for rule in base.column_rules() {
                for header in &rule.headers {
                    let text = header.join(" ");
                    let after = with
                        .column_rule_for(&text)
                        .unwrap_or_else(|| panic!("{id} {locale}: {text} is no longer mapped"));
                    assert_eq!(after.entity_type, rule.entity_type, "{id} {locale}: {text}");
                }
            }
            assert!(
                with.column_rules().len() >= base.column_rules().len(),
                "{id} {locale} removed rules"
            );
            assert!(
                with.identifier_schemes().len() >= base.identifier_schemes().len(),
                "{id} {locale} removed schemes"
            );
            assert!(
                with.detection_terms(id).len() >= 10,
                "{id} {locale} has too few detection terms for lot 10"
            );
            // Roles a pack relabels already exist in the base: it adds a label, not a meaning.
            let base_roles: BTreeSet<&str> = base.roles().map(|l| l.id.as_str()).collect();
            for label in with.roles().filter(|label| label.pack == id) {
                assert!(
                    base_roles.contains(label.id.as_str()),
                    "{id}: new role {:?}",
                    label.id
                );
            }
        }
    }
}

#[test]
fn a_domain_label_replaces_the_base_one_only_while_its_pack_is_active() {
    let client = RoleId::new("client").unwrap();
    let base = PackSet::load("fr-FR", &[]).unwrap();
    let health = PackSet::load("fr-FR", &["health-fr"]).unwrap();
    let legal = PackSet::load("fr-FR", &["legal-fr"]).unwrap();
    assert_eq!(base.role_label_key(&client), Some("knowledge.roles.client"));
    assert_eq!(
        health.role_label_key(&client),
        Some("knowledge.roles.health.client")
    );
    // Legal does not relabel the client: it keeps the base label.
    assert_eq!(
        legal.role_label_key(&client),
        Some("knowledge.roles.client")
    );
}

#[test]
fn the_titles_of_the_packs_feed_the_name_normaliser() {
    use assistant_cabinet_ai_lib::knowledge::normalize::normalize_name;

    let french = PackSet::load("fr-FR", &[]).unwrap().title_set();
    let name = normalize_name("Docteur H\u{e9}l\u{e8}ne de la Fontaine", &french);
    assert_eq!(name.titles, ["docteur"]);
    assert_eq!(name.token_set_key, "fontaine helene");
    // "Me" is a title only before a capitalised name; "me" the pronoun is not.
    assert_eq!(normalize_name("Me Dupont", &french).titles, ["me"]);
    assert!(normalize_name("me Dupont", &french).titles.is_empty());

    let english = PackSet::load("en-US", &[]).unwrap().title_set();
    assert_eq!(normalize_name("Mrs. Jane Smith", &english).titles, ["mrs"]);
    // The French base does not know an English title and the reverse.
    assert!(normalize_name("Mrs Smith", &french).titles.is_empty());
}

#[test]
fn spoken_forms_are_words() {
    let french = PackSet::load("fr-FR", &[]).unwrap();
    assert_eq!(french.title_spoken("Dr."), Some("docteur"));
    assert_eq!(french.title_spoken("MME"), Some("madame"));
    assert_eq!(french.title_spoken("unknown"), None);
    let english = PackSet::load("en-US", &[]).unwrap();
    assert_eq!(english.title_spoken("Mr"), Some("mister"));
}

#[test]
fn stop_words_are_matched_through_accents_and_case() {
    let french = PackSet::load("fr-FR", &[]).unwrap();
    assert!(french.is_stop_word("F\u{e9}vrier"));
    assert!(french.is_stop_word("FEVRIER"));
    assert!(french.is_stop_word("N/A"));
    assert!(!french.is_stop_word("Dupont"));
}

// ----------------------------------------------------------------------------- identifiers

/// An invented string each scheme must find, and what it must reduce to.
fn samples() -> Vec<(
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
)> {
    // (locale, packs, scheme id, text, reduced value)
    vec![
        (
            "fr-FR",
            "",
            "email",
            "ecrire a jean.dupont@example.org demain",
            "JEANDUPONTEXAMPLEORG",
        ),
        (
            "fr-FR",
            "",
            "phone",
            "appeler le 06 12 34 56 78 ce soir",
            "0612345678",
        ),
        (
            "fr-FR",
            "",
            "phone",
            "ligne +33 1 23 45 67 89",
            "33123456789",
        ),
        (
            "fr-FR",
            "",
            "iban",
            "IBAN FR76 3000 6000 0112 3456 7890 189",
            "FR7630006000011234567890189",
        ),
        (
            "fr-FR",
            "",
            "invoice_number",
            "facture FA-2026-0042 du mois",
            "FA20260042",
        ),
        (
            "fr-FR",
            "",
            "order_number",
            "bon CMD-123456 recu",
            "CMD123456",
        ),
        (
            "fr-FR",
            "health-fr",
            "nir",
            "NIR 1 85 03 69 123 456 78",
            "185036912345678",
        ),
        (
            "fr-FR",
            "legal-fr",
            "case_number",
            "dossier RG 24/01234 audience",
            "RG2401234",
        ),
        (
            "fr-FR",
            "accounting-fr",
            "siret",
            "SIRET 123 456 789 00012",
            "12345678900012",
        ),
        (
            "fr-FR",
            "accounting-fr",
            "vat_number",
            "TVA FR 12 345678901",
            "FR12345678901",
        ),
        (
            "fr-FR",
            "health-ch",
            "avs",
            "Numero AVS 756.1234.5678.97 de la patiente",
            "7561234567897",
        ),
        (
            "fr-FR",
            "health-ch",
            "avs",
            "AVS 756 1234 5678 97",
            "7561234567897",
        ),
        (
            "fr-FR",
            "health-ch",
            "ch_iban",
            "IBAN CH93 0076 2011 6238 5295 7",
            "CH9300762011623852957",
        ),
        (
            "fr-FR",
            "health-ch",
            "ch_phone",
            "appeler le 021 123 45 67 ou le +41 79 123 45 67",
            "0211234567",
        ),
        (
            "en-US",
            "health-ch",
            "avs",
            "AHV number 756.1234.5678.97 on file",
            "7561234567897",
        ),
        (
            "en-US",
            "",
            "email",
            "write to jane.smith@example.org",
            "JANESMITHEXAMPLEORG",
        ),
        (
            "en-US",
            "",
            "phone",
            "call (555) 123-4567 today",
            "5551234567",
        ),
        (
            "en-US",
            "health-fr",
            "ssn",
            "ssn 123-45-6789 on file",
            "123456789",
        ),
        (
            "en-US",
            "legal-fr",
            "case_number",
            "see 24-cv-01234",
            "24CV01234",
        ),
        (
            "en-US",
            "accounting-fr",
            "ein",
            "EIN 12-3456789",
            "123456789",
        ),
    ]
}

#[test]
fn every_scheme_finds_an_invented_sample_and_reduces_it() {
    for (locale, pack, scheme, text, expected) in samples() {
        let active: Vec<&str> = if pack.is_empty() { vec![] } else { vec![pack] };
        let packs = PackSet::load(locale, &active).unwrap();
        let scheme_ref = packs
            .identifier_scheme(scheme)
            .unwrap_or_else(|| panic!("{locale} {pack}: no scheme {scheme}"));
        let found = scheme_ref.scan(text);
        assert!(
            !found.is_empty(),
            "{locale} {scheme}: nothing found in {text:?}"
        );
        let expected = expected.replace(' ', "");
        assert!(
            found.iter().any(|m| m.normalized == expected),
            "{locale} {scheme}: expected {expected}, got {:?}",
            found.iter().map(|m| &m.normalized).collect::<Vec<_>>()
        );
    }
}

#[test]
fn a_social_security_number_split_by_a_line_break_is_still_one_number() {
    // Defect KBD-05: a figure broken across two lines must be read whole. The pattern uses \s, which
    // spans a newline.
    let packs = PackSet::load("fr-FR", &["health-fr"]).unwrap();
    let nir = packs.identifier_scheme("nir").unwrap();
    let found = nir.scan("NIR 1 85 03 69\n123 456 78 ouvert");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].normalized, "185036912345678");
}

#[test]
fn every_optional_pack_is_named_domain_dash_country() {
    // The owner's rule of 11 October 2026: modules never mix countries. A new country is a new
    // pack (`health-eu`, `legal-ch`...), and its name says which.
    for id in available_pack_ids() {
        assert!(is_domain_country_id(id), "{id} is not <domain>-<country>");
    }
    assert!(!is_domain_country_id("health"));
    assert!(!is_domain_country_id("health-xx"));
    assert_eq!(canonical_pack_id("health"), "health-fr");
    assert_eq!(canonical_pack_id("legal"), "legal-fr");
    assert_eq!(canonical_pack_id("accounting"), "accounting-fr");
    assert_eq!(canonical_pack_id("health-ch"), "health-ch");
    assert_eq!(canonical_pack_id("nonsense"), "nonsense");
}

#[test]
fn each_health_module_is_complete_alone_and_both_can_be_on_together() {
    for locale in locales() {
        let france = PackSet::load(locale, &["health-fr"]).unwrap();
        let switzerland = PackSet::load(locale, &["health-ch"]).unwrap();
        let both = PackSet::load(locale, &["health-fr", "health-ch"]).unwrap();
        let ids = |set: &PackSet| -> BTreeSet<String> {
            set.identifier_schemes()
                .iter()
                .map(|s| s.id.clone())
                .collect()
        };
        // Each country brings its own number and not the other's...
        assert!(ids(&switzerland).contains("avs"), "{locale}");
        assert!(!ids(&france).contains("avs"), "{locale}");
        assert!(
            !ids(&switzerland).contains("nir") && !ids(&switzerland).contains("ssn"),
            "{locale}"
        );
        // ...each is usable alone...
        for (name, set) in [("health-fr", &france), ("health-ch", &switzerland)] {
            assert!(set.column_rule_for("patient").is_some(), "{locale} {name}");
            assert!(set.roles().count() >= 2, "{locale} {name}");
        }
        // ...and both together read both numbers, as for a cross-border worker.
        let together = ids(&both);
        assert!(together.contains("avs"), "{locale}");
        assert!(
            together.contains("nir") || together.contains("ssn"),
            "{locale}"
        );
    }
}

#[test]
fn the_type_vocabulary_of_every_pack_is_read_whatever_modules_are_on() {
    // A module never excludes or transforms an entity type (owner, 11 October 2026): the words that
    // make an organisation an organisation come from every shipped pack, active or not.
    let prefixes = |set: &PackSet| -> Vec<String> {
        set.org_markers()
            .prefix
            .iter()
            .map(|words| words.join(" "))
            .collect()
    };
    for active in [
        vec![],
        vec!["health-ch"],
        vec!["health-fr"],
        vec!["legal-fr"],
    ] {
        let set = PackSet::load_with_type_vocabulary("fr-FR", &active).unwrap();
        assert!(prefixes(&set).contains(&"cpam".to_string()), "{active:?}");
        assert!(
            prefixes(&set).contains(&"caisse maladie".to_string()),
            "{active:?}"
        );
        // What only a module reads is still switched by it.
        let schemes: Vec<_> = set
            .identifier_schemes()
            .iter()
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(
            schemes.contains(&"nir"),
            active.contains(&"health-fr"),
            "{active:?}"
        );
        assert_eq!(
            schemes.contains(&"avs"),
            active.contains(&"health-ch"),
            "{active:?}"
        );
        assert!(
            set.loaded()
                .iter()
                .all(|(id, _)| id == "base" || active.contains(&id.as_str())),
            "an inactive pack is not listed as loaded: {:?}",
            set.loaded()
        );
    }
    // The plain loader stays exactly what was asked for.
    let plain = PackSet::load("fr-FR", &["health-ch"]).unwrap();
    assert!(!prefixes(&plain).contains(&"cpam".to_string()));
}

#[test]
fn the_schemes_do_not_fire_on_ordinary_text() {
    for locale in locales() {
        let packs = PackSet::load(locale, &available_pack_ids()).unwrap();
        for scheme in packs.identifier_schemes() {
            for text in [
                "Jean Dupont habite a Lyon.",
                "the quick brown fox",
                "rendez-vous en 2026",
                "",
            ] {
                assert!(
                    scheme.scan(text).is_empty(),
                    "{locale} {}: matched {text:?}",
                    scheme.id
                );
            }
        }
    }
}

#[test]
fn identifier_schemes_declare_how_far_a_collision_can_be_trusted() {
    let packs = PackSet::load("fr-FR", &available_pack_ids()).unwrap();
    // A phone number is shared by a household; an invoice number is not.
    assert_eq!(
        packs.identifier_scheme("phone").unwrap().strength,
        IdentifierStrength::Possible
    );
    assert_eq!(
        packs.identifier_scheme("invoice_number").unwrap().strength,
        IdentifierStrength::Exact
    );
    assert!(packs.identifier_scheme("email").unwrap().personal_key);
    assert!(
        !packs
            .identifier_scheme("invoice_number")
            .unwrap()
            .personal_key
    );
}

// ----------------------------------------------------------------------------- failures

#[test]
fn an_unknown_pack_is_an_error_with_a_code_a_pack_and_a_path() {
    match PackSet::load("fr-FR", &["astrology"]) {
        Err(error @ AppError::KnowledgePackInvalid { .. }) => {
            assert_eq!(error.code(), "knowledge_pack_invalid");
            assert_eq!(error.data()["pack"], "astrology");
            assert!(error.data()["path"].is_string());
        }
        other => panic!("unexpected result {other:?}"),
    }
}

// ----------------------------------------------------------------------------- the I10 guard

/// Everything before the first `#[cfg(test)]` line: the code that ships.
fn shipped_part(source: &str) -> &str {
    source.split("#[cfg(test)]").next().unwrap_or(source)
}

#[test]
fn the_knowledge_sources_hold_no_non_ascii_letter_outside_their_tests() {
    let root = manifest().join("src").join("knowledge");
    let files = files_under(&root, "rs");
    assert!(files.len() >= 10, "{} knowledge sources found", files.len());
    for path in files {
        let source = read(&path);
        let offending: Vec<char> = shipped_part(&source)
            .chars()
            .filter(|character| !character.is_ascii())
            .collect();
        assert!(
            offending.is_empty(),
            "{} holds non-ASCII characters outside its tests: {offending:?}",
            path.display()
        );
    }
}

// ----------------------------------------------------------------------------- the printed summary

#[test]
fn print_the_summary_of_every_pack() {
    println!();
    println!(
        "{:<12} {:<6} {:>6} {:>6} {:>6} {:>7} {:>7} {:>6} {:>5} {:>6}",
        "pack", "locale", "titles", "weak", "parts", "stopw", "schemes", "cols", "rels", "roles"
    );
    for locale in locales() {
        for id in std::iter::once(BASE_PACK_ID).chain(available_pack_ids()) {
            let root = manifest().join("resources").join("knowledge");
            let path = if id == BASE_PACK_ID {
                root.join("base").join(format!("{locale}.json"))
            } else {
                root.join("packs").join(id).join(format!("{locale}.json"))
            };
            let pack: Value = serde_json::from_str(&read(&path)).unwrap();
            let count = |value: &Value| value.as_array().map_or(0, Vec::len);
            println!(
                "{:<12} {:<6} {:>6} {:>6} {:>6} {:>7} {:>7} {:>6} {:>5} {:>6}   terms {}",
                id,
                locale,
                count(&pack["titles"]["person"]),
                count(&pack["weak_titles"]),
                count(&pack["particles"]),
                count(&pack["stop_words"]),
                count(&pack["identifier_schemes"]),
                count(&pack["columns"]),
                count(&pack["relations"]),
                count(&pack["roles"]),
                count(&pack["detection_terms"]),
            );
        }
        let all = PackSet::load(locale, &available_pack_ids()).unwrap();
        println!(
            "{:<12} {:<6} merged: {} schemes, {} column rules, {} relations, {} roles, hash {}",
            "(all)",
            locale,
            all.identifier_schemes().len(),
            all.column_rules().len(),
            all.relations().len(),
            all.roles().count(),
            &all.hash()[..16]
        );
    }
    // For orientation only (a debug build, nothing asserted): a pass loads the packs once.
    let started = std::time::Instant::now();
    let runs = 20;
    for _ in 0..runs {
        PackSet::load("fr-FR", &available_pack_ids()).unwrap();
    }
    println!(
        "loading and validating every pack (fr-FR): {:.1} ms each, debug build",
        started.elapsed().as_secs_f64() * 1000.0 / f64::from(runs)
    );
    println!();
}
