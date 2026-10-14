//! The words the file profile reads with, as data.
//!
//! ```text
//! resources/profile/base/<locale>.json              always loaded
//! resources/profile/<pack-id>/<locale>.json         one file per knowledge pack that has something to add
//! ```
//!
//! The ids are the knowledge packs' ids (`health-fr`, `health-ch`, ...), so a module for another
//! country is another pack and never a flag inside one. This loader is separate from
//! `knowledge::packs` on purpose: the profile's vocabulary must be able to grow without changing the
//! pack hash that tells the knowledge base its words changed, so a new profile word never makes an
//! Analyse re-read every document for the knowledge base.
//!
//! **What a list does.** `generic_values` are the strings that software, scanners and shared
//! accounts write into a document's properties: they say nothing about the document, so a property
//! that matches one produces no field (`metadata.rs`). Every shipped pack's list is always read,
//! whichever packs are active, because a list can only suppress a value: reading a country's
//! scanner brands never harms a user of another country.
//!
//! Loading is strict: an unknown field, an unsupported schema or an id the file does not match is
//! `knowledge_pack_invalid` (the same machine code, so no new sentence is needed). A test loads every bundled file, so a typo fails the build
//! rather than a user's analysis.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::error::AppError;
use crate::knowledge::normalize::fold;

/// The only vocabulary layout this build reads.
pub const VOCABULARY_SCHEMA: u32 = 1;

struct Bundled {
    id: &'static str,
    locale: &'static str,
    body: &'static str,
}

const BUNDLED: &[Bundled] = &[
    Bundled {
        id: "base",
        locale: "en-US",
        body: include_str!("../../resources/profile/base/en-US.json"),
    },
    Bundled {
        id: "base",
        locale: "fr-FR",
        body: include_str!("../../resources/profile/base/fr-FR.json"),
    },
    Bundled {
        id: "health-fr",
        locale: "en-US",
        body: include_str!("../../resources/profile/health-fr/en-US.json"),
    },
    Bundled {
        id: "health-fr",
        locale: "fr-FR",
        body: include_str!("../../resources/profile/health-fr/fr-FR.json"),
    },
    Bundled {
        id: "health-ch",
        locale: "en-US",
        body: include_str!("../../resources/profile/health-ch/en-US.json"),
    },
    Bundled {
        id: "health-ch",
        locale: "fr-FR",
        body: include_str!("../../resources/profile/health-ch/fr-FR.json"),
    },
];

/// Every `(pack id, locale)` this build ships a profile vocabulary for. What the test that loads
/// everything iterates over.
pub fn bundled_vocabularies() -> Vec<(&'static str, &'static str)> {
    let mut all: Vec<(&'static str, &'static str)> =
        BUNDLED.iter().map(|file| (file.id, file.locale)).collect();
    all.sort_unstable();
    all
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVocabulary {
    schema: u32,
    id: String,
    locale: String,
    #[serde(default)]
    generic_values: RawGenericValues,
    #[serde(default)]
    file_name_extensions: Vec<String>,
    #[serde(default)]
    months: Vec<Vec<String>>,
    #[serde(default)]
    header_labels: RawHeaderLabels,
    #[serde(default)]
    document_types: Vec<RawDocumentType>,
    #[serde(default)]
    identifier_layouts: BTreeMap<String, Vec<Vec<usize>>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHeaderLabels {
    #[serde(default)]
    subject_line: Vec<String>,
    #[serde(default)]
    date: Vec<String>,
    #[serde(default)]
    date_markers: Vec<String>,
    #[serde(default)]
    references: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    roles: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDocumentType {
    code: String,
    scope: String,
    #[serde(default)]
    generic: bool,
    #[serde(default)]
    name: Vec<String>,
    #[serde(default)]
    title: Vec<String>,
    #[serde(default)]
    header: Vec<String>,
    #[serde(default)]
    body: Vec<String>,
    #[serde(default)]
    sheet: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGenericValues {
    #[serde(default)]
    exact: Vec<String>,
    #[serde(default)]
    prefix: Vec<String>,
}

/// The roles a document gives an entity (design 4.2): a closed set, so a typo in a pack is refused.
pub const ROLES: &[&str] = &["author", "sender", "recipient", "subject", "issuer"];

/// A phrase as the folded words it is compared on: `Note d'honoraires` is `["note", "d", "honoraires"]`.
pub type Phrase = Vec<String>;

/// Which kind of file a business type is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeScope {
    Documents,
    Data,
}

/// One business type as data: a code, and the phrases that are evidence for it in each place a phrase
/// can be found. `generic` marks the catch-all (a letter): a specific type with evidence always beats it.
#[derive(Debug, Clone)]
pub struct DocumentTypeRule {
    pub code: String,
    pub scope: TypeScope,
    pub generic: bool,
    pub name: Vec<Phrase>,
    pub title: Vec<Phrase>,
    pub header: Vec<Phrase>,
    pub body: Vec<Phrase>,
    pub sheet: Vec<Phrase>,
}

/// The labels a header block is read with. Every list is phrases; a role lists the labels that give it.
#[derive(Debug, Clone, Default)]
pub struct HeaderLabels {
    pub subject_line: Vec<Phrase>,
    pub date: Vec<Phrase>,
    /// Words that introduce a date inside a sentence (`du`, `on`): a date after one of them is
    /// the date the document says it was written, not a date it merely mentions.
    pub date_markers: Vec<Phrase>,
    /// Reference kind code (`case`, `invoice`, ...) to the labels that introduce it.
    pub references: BTreeMap<String, Vec<Phrase>>,
    /// Role code (one of [`ROLES`]) to the labels that give it.
    pub roles: BTreeMap<String, Vec<Phrase>>,
}

/// The base and the shipped packs' profile words, merged.
#[derive(Debug, Clone, Default)]
pub struct ProfileVocabulary {
    generic_exact: HashSet<String>,
    generic_prefix: Vec<String>,
    file_name_extensions: HashSet<String>,
    /// Folded month name or abbreviation to its number, 1 to 12, from every language shipped: a
    /// document is written in its own language, whatever the interface speaks.
    months: HashMap<String, u32>,
    labels: HeaderLabels,
    types: BTreeMap<String, DocumentTypeRule>,
    /// Identifier scheme id to the ways its compact form is written in groups (`[3, 4, 4, 2]` for the
    /// Swiss AVS number). Kept here rather than on the knowledge packs' schemes so that adding a layout
    /// never changes the packs' fingerprint and makes every document be read again for names.
    layouts: BTreeMap<String, Vec<Vec<usize>>>,
    hash: String,
}

impl ProfileVocabulary {
    /// Load the base and every shipped pack, in every shipped language.
    ///
    /// The **language is not selected**: a document is written in its own language whatever the
    /// interface speaks, so the French and the English words are always read together (a French
    /// practice with an English interface still files `facture`). What a pack is selected by is
    /// `active_pack_ids`: the business types and header labels of a pack are read only when it is
    /// active, so a legal user never gets `prescription`. The lists that can only suppress a value
    /// (generic values, months, file-name extensions) are read for every pack.
    pub fn load(active_pack_ids: &[&str]) -> Result<Self, AppError> {
        let mut vocabulary = Self::default();
        let mut hasher = Sha256::new();
        hasher.update(format!("schema:{VOCABULARY_SCHEMA}\n"));
        // `base` first, then the packs in id order, each language in locale order: the merge order is
        // part of the hash.
        let mut files: Vec<&Bundled> = BUNDLED.iter().collect();
        files.sort_by_key(|file| (file.id != "base", file.id, file.locale));
        for file in files {
            let raw: RawVocabulary =
                serde_json::from_str(file.body).map_err(|_| invalid(file.id, "file"))?;
            if raw.schema != VOCABULARY_SCHEMA {
                return Err(invalid(file.id, "schema"));
            }
            if raw.id != file.id || raw.locale != file.locale {
                return Err(invalid(file.id, "id"));
            }
            hasher.update(format!("{}:{}\n{}\n", file.id, file.locale, file.body));
            for value in raw.generic_values.exact {
                let key = words_key(&value);
                if key.is_empty() {
                    return Err(invalid(file.id, "generic_values.exact"));
                }
                vocabulary.generic_exact.insert(key);
            }
            for value in raw.generic_values.prefix {
                let key = words_key(&value);
                if key.is_empty() {
                    return Err(invalid(file.id, "generic_values.prefix"));
                }
                vocabulary.generic_prefix.push(key);
            }
            vocabulary
                .file_name_extensions
                .extend(raw.file_name_extensions.iter().map(|ext| fold(ext)));
            if !raw.months.is_empty() {
                if raw.months.len() != 12 {
                    return Err(invalid(file.id, "months"));
                }
                for (index, names) in raw.months.iter().enumerate() {
                    for name in names {
                        let key = words_key(name);
                        if key.is_empty() {
                            return Err(invalid(file.id, "months"));
                        }
                        vocabulary.months.insert(key, index as u32 + 1);
                    }
                }
            }
            for (scheme, groups) in raw.identifier_layouts {
                for layout in &groups {
                    if layout.is_empty()
                        || layout.iter().any(|size| *size == 0 || *size > 8)
                        || layout.iter().sum::<usize>() > 64
                    {
                        return Err(invalid(file.id, "identifier_layouts"));
                    }
                }
                vocabulary.layouts.entry(scheme).or_default().extend(groups);
            }
            let active = file.id == "base" || active_pack_ids.contains(&file.id);
            if !active {
                continue;
            }
            vocabulary.add_labels(file.id, raw.header_labels)?;
            for rule in raw.document_types {
                vocabulary.add_type(file.id, rule)?;
            }
        }
        let digest = hasher.finalize();
        vocabulary.hash = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        Ok(vocabulary)
    }

    fn add_labels(&mut self, pack: &str, raw: RawHeaderLabels) -> Result<(), AppError> {
        let labels = &mut self.labels;
        labels.subject_line.extend(phrases(
            pack,
            "header_labels.subject_line",
            &raw.subject_line,
        )?);
        labels
            .date
            .extend(phrases(pack, "header_labels.date", &raw.date)?);
        labels.date_markers.extend(phrases(
            pack,
            "header_labels.date_markers",
            &raw.date_markers,
        )?);
        for (kind, list) in raw.references {
            let found = phrases(pack, "header_labels.references", &list)?;
            labels.references.entry(kind).or_default().extend(found);
        }
        for (role, list) in raw.roles {
            if !ROLES.contains(&role.as_str()) {
                return Err(invalid(pack, "header_labels.roles"));
            }
            let found = phrases(pack, "header_labels.roles", &list)?;
            labels.roles.entry(role).or_default().extend(found);
        }
        Ok(())
    }

    fn add_type(&mut self, pack: &str, raw: RawDocumentType) -> Result<(), AppError> {
        let scope = match raw.scope.as_str() {
            "documents" => TypeScope::Documents,
            "data" => TypeScope::Data,
            _ => return Err(invalid(pack, "document_types.scope")),
        };
        if raw.code.is_empty()
            || raw.code == "other"
            || !raw.code.chars().all(|c| c.is_ascii_lowercase() || c == '_')
        {
            return Err(invalid(pack, "document_types.code"));
        }
        let rule = self
            .types
            .entry(raw.code.clone())
            .or_insert_with(|| DocumentTypeRule {
                code: raw.code.clone(),
                scope,
                generic: false,
                name: Vec::new(),
                title: Vec::new(),
                header: Vec::new(),
                body: Vec::new(),
                sheet: Vec::new(),
            });
        if rule.scope != scope {
            return Err(invalid(pack, "document_types.scope"));
        }
        rule.generic |= raw.generic;
        rule.name
            .extend(phrases(pack, "document_types.name", &raw.name)?);
        rule.title
            .extend(phrases(pack, "document_types.title", &raw.title)?);
        rule.header
            .extend(phrases(pack, "document_types.header", &raw.header)?);
        rule.body
            .extend(phrases(pack, "document_types.body", &raw.body)?);
        rule.sheet
            .extend(phrases(pack, "document_types.sheet", &raw.sheet)?);
        Ok(())
    }

    /// The grouped writings of an identifier scheme's compact form, longest first (empty when the scheme
    /// is written in one piece, an e-mail address for instance).
    pub fn identifier_layouts(&self, scheme_id: &str) -> &[Vec<usize>] {
        self.layouts.get(scheme_id).map_or(&[], Vec::as_slice)
    }

    /// The number (1 to 12) of a month written as `word` (a name or an abbreviation, any case).
    pub fn month_number(&self, word: &str) -> Option<u32> {
        self.months.get(&words_key(word)).copied()
    }

    pub fn labels(&self) -> &HeaderLabels {
        &self.labels
    }

    /// The business types of the base and the active packs, in code order.
    pub fn document_types(&self) -> impl Iterator<Item = &DocumentTypeRule> {
        self.types.values()
    }

    /// The same vocabulary, plus names that must never be taken from a property: the user's own name
    /// and the practice's (a word processor stamps the licence holder on every document).
    ///
    /// The names are part of the vocabulary's hash: a profile built before a name was added is built again by
    /// the next Analyse, and none is built again while the names stay the same.
    pub fn with_user_names(mut self, names: &[String]) -> Self {
        let mut added: Vec<String> = Vec::new();
        for name in names {
            let key = words_key(name);
            if !key.is_empty() {
                self.generic_exact.insert(key.clone());
                added.push(key);
            }
        }
        if !added.is_empty() {
            added.sort();
            added.dedup();
            let mut hasher = Sha256::new();
            hasher.update(self.hash.as_bytes());
            hasher.update(format!("\nuser_names:{}\n", added.join("|")).as_bytes());
            self.hash = hasher
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
        }
        self
    }

    /// Whether `value` is a string a program, a scanner or a shared account writes, so that it says
    /// nothing about the document. Compared on folded words: case, accents and punctuation do not
    /// matter, and an `exact` entry must be the whole value while a `prefix` entry must be its
    /// leading words (`Canon iR-ADV C3530` starts with `canon ir`).
    pub fn is_generic(&self, value: &str) -> bool {
        let key = words_key(value);
        if key.is_empty() {
            return true;
        }
        if self.generic_exact.contains(&key) {
            return true;
        }
        self.generic_prefix.iter().any(|prefix| {
            key == *prefix
                || (key.starts_with(prefix.as_str())
                    && (key.as_bytes()[prefix.len()] == b' ' || prefix_ends_in_digit_run(prefix)))
        })
    }

    /// Whether `title` is only a file name (`courrier.docx`, `Scan 12.pdf`): a word processor fills the
    /// title with the name the document was saved under.
    pub fn looks_like_file_name(&self, title: &str) -> bool {
        let trimmed = title.trim();
        match trimmed.rsplit_once('.') {
            Some((stem, extension)) => {
                !stem.trim().is_empty()
                    && !extension.contains(char::is_whitespace)
                    && self.file_name_extensions.contains(&fold(extension))
            }
            None => false,
        }
    }

    /// The same vocabulary, plus the user's own business types (`user_types.rs`). Their codes start with
    /// `user_`, so none meets a shipped one. Their phrases are part of the hash: a change builds the profiles
    /// again at the next Analyse, and none is built again while the types stay the same.
    pub fn with_user_types(mut self, rules: Vec<DocumentTypeRule>) -> Self {
        if rules.is_empty() {
            return self;
        }
        let mut hasher = Sha256::new();
        hasher.update(self.hash.as_bytes());
        for rule in rules {
            hasher.update(
                format!(
                    "\nuser_type:{}:{:?}:{:?}:{:?}:{:?}:{:?}\n",
                    rule.code, rule.name, rule.title, rule.header, rule.body, rule.sheet
                )
                .as_bytes(),
            );
            self.types.insert(rule.code.clone(), rule);
        }
        self.hash = hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        self
    }

    /// A digest of every word read, so a later step can tell that the vocabulary changed.
    pub fn hash(&self) -> &str {
        &self.hash
    }
}

/// Each entry as folded words; an empty entry is refused.
fn phrases(pack: &str, path: &str, entries: &[String]) -> Result<Vec<Phrase>, AppError> {
    entries
        .iter()
        .map(|entry| {
            let words: Phrase = words_key(entry)
                .split(' ')
                .filter(|word| !word.is_empty())
                .map(str::to_string)
                .collect();
            if words.is_empty() {
                Err(invalid(pack, path))
            } else {
                Ok(words)
            }
        })
        .collect()
}

/// Whether `phrase` appears in `words`, as consecutive words.
pub fn contains_phrase(words: &[String], phrase: &[String]) -> bool {
    !phrase.is_empty() && words.windows(phrase.len()).any(|window| window == phrase)
}

/// `scan0` as a prefix is meant to match `scan0012`: when the prefix ends in a digit the next
/// character may continue the number.
fn prefix_ends_in_digit_run(prefix: &str) -> bool {
    prefix.chars().last().is_some_and(|c| c.is_ascii_digit())
}

/// The folded words of a value, one space between them: `Microsoft(R) Word 16.0` is
/// `microsoft r word 16 0`.
pub(crate) fn words_key(text: &str) -> String {
    fold(text)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn invalid(id: &str, path: &str) -> AppError {
    AppError::KnowledgePackInvalid {
        pack: id.to_string(),
        path: path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_vocabulary_loads_in_every_locale() {
        for (_, locale) in bundled_vocabularies() {
            let vocabulary = ProfileVocabulary::load(&["health-fr", "health-ch"])
                .unwrap_or_else(|error| panic!("{locale}: {error:?}"));
            assert!(!vocabulary.hash().is_empty());
        }
    }

    #[test]
    fn a_property_a_program_writes_is_generic_and_a_name_is_not() {
        let vocabulary = ProfileVocabulary::load(&[]).expect("loads");
        for generic in [
            "Microsoft Word",
            "MICROSOFT\u{ae} WORD 16.0",
            "Microsoft Office User",
            "admin",
            "Administrateur",
            "Canon iR-ADV C3530",
            "RICOH MP C3004",
            "scan0012",
            "Document1",
            "PScript5.dll Version 5.2.2",
            "Weda 4.2",
            "   ",
        ] {
            assert!(
                vocabulary.is_generic(generic),
                "{generic:?} must be generic"
            );
        }
        for real in [
            "Zorglub Metadatus",
            "Dr Martin",
            "Adminis Martin",
            "Brother Anselme",
            "Minutes of the review",
        ] {
            assert!(!vocabulary.is_generic(real), "{real:?} must not be generic");
        }
    }

    #[test]
    fn the_prefix_rule_needs_whole_leading_words() {
        let vocabulary = ProfileVocabulary::load(&[]).expect("loads");
        // `epson` is a scanner brand; `epsonite` is not the brand.
        assert!(vocabulary.is_generic("Epson WF-3720"));
        assert!(!vocabulary.is_generic("Epsonite Durand"));
    }

    #[test]
    fn the_users_own_name_is_generic_once_it_is_given() {
        let plain = ProfileVocabulary::load(&[]).expect("loads");
        assert!(!plain.is_generic("Zorglub Metadatus"));
        let with_name = plain.with_user_names(&["Zorglub Metadatus".to_string()]);
        assert!(with_name.is_generic("zorglub  METADATUS"));
    }

    #[test]
    fn a_title_that_is_a_file_name_is_recognised() {
        let vocabulary = ProfileVocabulary::load(&[]).expect("loads");
        assert!(vocabulary.looks_like_file_name("courrier-martin.docx"));
        assert!(vocabulary.looks_like_file_name("Scan 12.PDF"));
        assert!(!vocabulary.looks_like_file_name("Compte rendu. Suite"));
        assert!(!vocabulary.looks_like_file_name(".docx"));
        assert!(!vocabulary.looks_like_file_name("Version 2.5 du protocole"));
    }

    #[test]
    fn a_vocabulary_file_refuses_unknown_fields() {
        let body = r#"{"schema":1,"id":"base","locale":"fr-FR","surprise":true}"#;
        assert!(serde_json::from_str::<RawVocabulary>(body).is_err());
    }
}
