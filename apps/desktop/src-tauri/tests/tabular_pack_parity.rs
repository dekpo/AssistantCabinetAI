//! The two locale packs of `resources/tabular-questions/` describe the same vocabulary in two
//! languages, so they must have the same shape: a list added to one and forgotten in the other
//! would make a question work in one language and silently not in the other. A third language
//! (German, Italian) is added as a file and checked by this same test (TABULAR plan, decision 19).
//!
//! Words are compared as written. The French pack deliberately holds an accented and an unaccented
//! form of some words, which are one word once folded; a repeated *spelling* is the mistake this
//! test catches.

use serde_json::Value;

const PACKS: [(&str, &str); 2] = [
    (
        "en-US",
        include_str!("../resources/tabular-questions/en-US.json"),
    ),
    (
        "fr-FR",
        include_str!("../resources/tabular-questions/fr-FR.json"),
    ),
];

fn parse(body: &str) -> Value {
    serde_json::from_str(body).expect("a pack is valid JSON")
}

/// Every path to a list, with the list: `structural.sheets`, `weekdays.1`, `table_words`.
fn lists<'a>(prefix: &str, value: &'a Value, out: &mut Vec<(String, &'a Vec<Value>)>) {
    match value {
        Value::Array(items) => out.push((prefix.to_string(), items)),
        Value::Object(map) => {
            for (key, inner) in map {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                lists(&path, inner, out);
            }
        }
        _ => {}
    }
}

/// The shape of a value: its keys at every depth, the lists being leaves.
fn keys(prefix: &str, value: &Value, out: &mut Vec<String>) {
    if let Value::Object(map) = value {
        for (key, inner) in map {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            out.push(path.clone());
            keys(&path, inner, out);
        }
    }
}

#[test]
fn both_packs_have_the_same_keys_at_every_depth() {
    let [(first_tag, first), (second_tag, second)] = PACKS;
    let (mut left, mut right) = (Vec::new(), Vec::new());
    keys("", &parse(first), &mut left);
    keys("", &parse(second), &mut right);
    left.sort();
    right.sort();

    let only_left: Vec<_> = left.iter().filter(|key| !right.contains(key)).collect();
    let only_right: Vec<_> = right.iter().filter(|key| !left.contains(key)).collect();
    assert!(
        only_left.is_empty() && only_right.is_empty(),
        "{first_tag} only: {only_left:?}; {second_tag} only: {only_right:?}"
    );
}

#[test]
fn every_list_holds_words_and_none_is_empty() {
    for (tag, body) in PACKS {
        let pack = parse(body);
        let mut found = Vec::new();
        lists("", &pack, &mut found);
        assert!(found.len() > 20, "{tag}: the lists were not found");
        for (path, items) in found {
            assert!(!items.is_empty(), "{tag}: the list {path} is empty");
            for item in items {
                let word = item
                    .as_str()
                    .unwrap_or_else(|| panic!("{tag}: {path} holds a non-string"));
                assert!(!word.trim().is_empty(), "{tag}: {path} holds a blank word");
                assert_eq!(
                    word,
                    word.trim(),
                    "{tag}: {path}: {word:?} has spaces around it"
                );
            }
        }
    }
}

#[test]
fn no_word_is_written_twice_in_a_list() {
    for (tag, body) in PACKS {
        let pack = parse(body);
        let mut found = Vec::new();
        lists("", &pack, &mut found);
        for (path, items) in found {
            let mut seen: Vec<&str> = Vec::new();
            for item in items {
                let word = item.as_str().unwrap();
                assert!(!seen.contains(&word), "{tag}: {word:?} is twice in {path}");
                seen.push(word);
            }
        }
    }
}

#[test]
fn both_packs_carry_the_table_vocabulary() {
    for (tag, body) in PACKS {
        let pack = parse(body);
        let words = pack["table_words"]
            .as_array()
            .unwrap_or_else(|| panic!("{tag}: no table_words"));
        assert!(
            words.len() >= 10,
            "{tag}: table_words is too short to be useful"
        );
    }
}

#[test]
fn both_packs_carry_the_advisory_lists() {
    for (tag, body) in PACKS {
        let pack = parse(body);
        for list in ["advisory_intents", "advisory_intents_with_a_name"] {
            let words = pack[list]
                .as_array()
                .unwrap_or_else(|| panic!("{tag}: no {list}"));
            assert!(!words.is_empty(), "{tag}: {list} is empty");
        }
        // The general phrases only count beside a name: they are not in the unconditional list.
        let plain: Vec<&str> = pack["advisory_intents"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|word| word.as_str())
            .collect();
        for general in pack["advisory_intents_with_a_name"].as_array().unwrap() {
            let general = general.as_str().unwrap();
            assert!(
                !plain.contains(&general),
                "{tag}: {general:?} is in both advisory lists"
            );
        }
    }
}

#[test]
fn both_packs_carry_the_follow_up_lists() {
    for (tag, body) in PACKS {
        let pack = parse(body);
        for list in [
            "follow_up_markers",
            "follow_up_sort_markers",
            "follow_up_other_workbook",
        ] {
            let words = pack[list]
                .as_array()
                .unwrap_or_else(|| panic!("{tag}: no {list}"));
            assert!(!words.is_empty(), "{tag}: {list} is empty");
        }
        // The shortest connective is the weakest word of the pack: it opens a follow-up only at the very
        // start of a question, and the sort verbs are a list of their own.
        let connectives: Vec<&str> = pack["follow_up_markers"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|word| word.as_str())
            .collect();
        for verb in pack["follow_up_sort_markers"].as_array().unwrap() {
            let verb = verb.as_str().unwrap();
            assert!(
                !connectives.contains(&verb),
                "{tag}: {verb:?} is in both follow-up lists"
            );
        }
    }
}

#[test]
fn both_packs_carry_the_layout_lists() {
    // TABULAR step 13: the words that mark a total row at the foot of a table and the cells that
    // stand for "no value" (a dash, n/a). A pack without them would read a total as data.
    for (tag, body) in PACKS {
        let pack = parse(body);
        for list in ["total_words", "empty_markers"] {
            let words = pack[list]
                .as_array()
                .unwrap_or_else(|| panic!("{tag}: no {list}"));
            assert!(!words.is_empty(), "{tag}: {list} is empty");
        }
        let totals: Vec<&str> = pack["total_words"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|word| word.as_str())
            .collect();
        assert!(
            totals.contains(&"total"),
            "{tag}: the plain word is missing"
        );
    }
}
