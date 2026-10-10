//! Keeping what the knowledge base derived in step with the code that derives it.
//!
//! Two things in the store are computed by code that can change between releases: the phonetic key
//! of every alias (the encoder's rules), and the fingerprint of the lexicon packs. Both are
//! recorded in `kb_meta`; this module compares the record with the code that is running and brings
//! the store up to date.
//!
//! None of it runs by itself. The locale decides which encoder and which packs apply, and the locale
//! belongs to the client, not to the index: an Analyse pass (lot 4) calls these two functions once,
//! before it reads the first file, with the locale and the packs it is about to use.

use rusqlite::Connection;

use super::normalize::TitleSet;
use super::packs::PackSet;
use super::phonetic::PhoneticEncoder;
use super::resolve::alias_phonetic_key;
use super::store;
use crate::error::AppError;

const PHONETIC_KEY: &str = "phonetic_version";
const PACKS_KEY: &str = "packs_hash";

/// What a refresh did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhoneticRefresh {
    /// The signature that was recorded before (`fr-rules:2`), empty when none was.
    pub previous: String,
    /// The signature of the encoder that ran, now recorded.
    pub signature: String,
    /// Aliases read again. Zero when the record already matched.
    pub recomputed: usize,
    /// Aliases whose key was different and was rewritten.
    pub changed: usize,
}

/// Make every stored alias key the one the active encoder gives, if the encoder is not the one that
/// wrote them. The encoder's `signature()` - its id, which carries the language, and its version -
/// is compared with `kb_meta.phonetic_version`; when they differ, every alias is encoded again from
/// its display form and the new signature is recorded, **all in one transaction**: a failure leaves
/// the old keys and the old signature, so the next pass tries again.
///
/// A key is derived data. Nothing is lost by recomputing it, and nothing user-written is touched.
pub fn ensure_phonetic_keys(
    connection: &Connection,
    titles: &TitleSet,
    encoder: &dyn PhoneticEncoder,
) -> Result<PhoneticRefresh, AppError> {
    let signature = encoder.signature();
    let previous = store::meta(connection, PHONETIC_KEY)?.unwrap_or_default();
    if previous == signature {
        return Ok(PhoneticRefresh {
            previous,
            signature,
            recomputed: 0,
            changed: 0,
        });
    }

    let tx = connection
        .unchecked_transaction()
        .map_err(|_| AppError::KnowledgeUnavailable)?;
    let rows = store::alias_phonetic_rows(&tx)?;
    let mut changed = 0;
    for row in &rows {
        let key = alias_phonetic_key(&row.display, row.kind, titles, encoder);
        if key != row.phonetic_key {
            store::set_alias_phonetic_key(&tx, row.alias_id, &key)?;
            changed += 1;
        }
    }
    store::set_meta(&tx, PHONETIC_KEY, &signature)?;
    tx.commit().map_err(|_| AppError::KnowledgeUnavailable)?;
    Ok(PhoneticRefresh {
        previous,
        signature,
        recomputed: rows.len(),
        changed,
    })
}

/// Record the fingerprint of the packs in use. Returns whether it differs from the one stored,
/// which tells the caller that the vocabulary changed since the last pass (a title added, a pack
/// activated) and that sources read with the old one may deserve a second reading.
pub fn record_packs(connection: &Connection, packs: &PackSet) -> Result<bool, AppError> {
    let previous = store::meta(connection, PACKS_KEY)?.unwrap_or_default();
    if previous == packs.hash() {
        return Ok(false);
    }
    store::set_meta(connection, PACKS_KEY, packs.hash())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::phonetic::{EnglishPhonetic, FrenchPhonetic};
    use crate::knowledge::resolve::{aliases_for, AliasRequest};
    use crate::knowledge::{EntityStatus, EntityTypeId, NewEntity, Origin};

    fn connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        crate::index_store::configure_connection(&connection).unwrap();
        store::ensure_schema(&connection).unwrap();
        connection
    }

    fn titles() -> TitleSet {
        TitleSet::new().with_titles(["dr"])
    }

    /// An entity with the aliases `aliases_for` makes under `encoder`.
    fn add_person(connection: &Connection, name: &str, encoder: &dyn PhoneticEncoder) -> i64 {
        let id = store::create_entity(
            connection,
            &NewEntity {
                type_id: "person".to_string(),
                subtype: None,
                canonical_name: name.to_string(),
                normalized_name: name.to_lowercase(),
                disambiguator: String::new(),
                status: EntityStatus::Active,
                origin: Origin::Automatic,
            },
        )
        .unwrap();
        let person = EntityTypeId::person();
        for alias in aliases_for(
            &AliasRequest {
                type_id: &person,
                canonical: name,
                titles_seen: &[],
                acronym: None,
            },
            &titles(),
            encoder,
        ) {
            store::add_alias(connection, id, &alias, Origin::Automatic).unwrap();
        }
        id
    }

    fn keys(connection: &Connection, entity_id: i64) -> Vec<String> {
        store::entity_with_aliases(connection, entity_id)
            .unwrap()
            .unwrap()
            .aliases
            .into_iter()
            .map(|alias| alias.phonetic_key)
            .collect()
    }

    #[test]
    fn a_first_run_on_an_empty_store_records_the_encoder() {
        let connection = connection();
        let refresh = ensure_phonetic_keys(&connection, &titles(), &FrenchPhonetic).unwrap();
        assert_eq!(refresh.previous, "");
        assert_eq!(refresh.signature, FrenchPhonetic.signature());
        assert_eq!((refresh.recomputed, refresh.changed), (0, 0));
        assert_eq!(
            store::meta(&connection, "phonetic_version")
                .unwrap()
                .unwrap(),
            FrenchPhonetic.signature()
        );
    }

    #[test]
    fn nothing_is_read_again_while_the_encoder_is_the_same() {
        let connection = connection();
        add_person(&connection, "Jean Dupont", &FrenchPhonetic);
        ensure_phonetic_keys(&connection, &titles(), &FrenchPhonetic).unwrap();
        let again = ensure_phonetic_keys(&connection, &titles(), &FrenchPhonetic).unwrap();
        assert_eq!(again.recomputed, 0);
    }

    #[test]
    fn a_change_of_encoder_rewrites_every_key_in_one_go() {
        let connection = connection();
        let id = add_person(&connection, "Jean Dupont", &FrenchPhonetic);
        ensure_phonetic_keys(&connection, &titles(), &FrenchPhonetic).unwrap();
        let french = keys(&connection, id);
        assert!(french.iter().all(|key| !key.is_empty()));

        // The user switched the language: the English rules encode the same names differently.
        let refresh = ensure_phonetic_keys(&connection, &titles(), &EnglishPhonetic).unwrap();
        assert_eq!(refresh.previous, FrenchPhonetic.signature());
        assert_eq!(refresh.signature, EnglishPhonetic.signature());
        assert_eq!(refresh.recomputed, french.len());
        assert!(refresh.changed > 0);
        let english = keys(&connection, id);
        assert_ne!(french, english);

        // And back: the keys are exactly what they were.
        ensure_phonetic_keys(&connection, &titles(), &FrenchPhonetic).unwrap();
        assert_eq!(keys(&connection, id), french);
    }

    #[test]
    fn an_encoder_that_was_not_the_one_that_wrote_the_keys_fixes_them() {
        // Keys written by an older rule set: blank here, and the record is for another version.
        let connection = connection();
        let id = add_person(&connection, "Jean Dupont", &FrenchPhonetic);
        connection
            .execute("UPDATE kb_aliases SET phonetic_key = 'OLD'", [])
            .unwrap();
        store::set_meta(&connection, "phonetic_version", "fr-rules:1").unwrap();

        let refresh = ensure_phonetic_keys(&connection, &titles(), &FrenchPhonetic).unwrap();
        assert!(refresh.changed > 0);
        assert!(keys(&connection, id).iter().all(|key| key != "OLD"));
    }

    #[test]
    fn a_failed_refresh_keeps_the_old_keys_and_the_old_record() {
        let connection = connection();
        add_person(&connection, "Jean Dupont", &FrenchPhonetic);
        ensure_phonetic_keys(&connection, &titles(), &FrenchPhonetic).unwrap();
        let before = store::alias_phonetic_rows(&connection).unwrap();
        // A trigger that refuses every key update makes the rewrite fail half way.
        connection
            .execute_batch(
                "CREATE TRIGGER refuse_key_update BEFORE UPDATE OF phonetic_key ON kb_aliases
                 BEGIN SELECT RAISE(ABORT, 'refused'); END;",
            )
            .unwrap();

        let result = ensure_phonetic_keys(&connection, &titles(), &EnglishPhonetic);
        assert!(result.is_err());
        assert_eq!(store::alias_phonetic_rows(&connection).unwrap(), before);
        assert_eq!(
            store::meta(&connection, "phonetic_version")
                .unwrap()
                .unwrap(),
            FrenchPhonetic.signature()
        );
    }

    #[test]
    fn a_refresh_gives_the_keys_a_new_alias_would_get() {
        // With "M" an ambiguous title, the initial alias of "Marie Dupont" is "M. Dupont": the
        // recomputation must read it as a letter, like `aliases_for` did when it made it.
        let weak = TitleSet::new().with_weak_titles(["m"]);
        let connection = connection();
        let id = store::create_entity(
            &connection,
            &NewEntity {
                type_id: "person".to_string(),
                subtype: None,
                canonical_name: "Marie Dupont".to_string(),
                normalized_name: "marie dupont".to_string(),
                disambiguator: String::new(),
                status: EntityStatus::Active,
                origin: Origin::Automatic,
            },
        )
        .unwrap();
        let person = EntityTypeId::person();
        let made = aliases_for(
            &AliasRequest {
                type_id: &person,
                canonical: "Marie Dupont",
                titles_seen: &[],
                acronym: None,
            },
            &weak,
            &FrenchPhonetic,
        );
        for alias in &made {
            store::add_alias(&connection, id, alias, Origin::Automatic).unwrap();
        }
        let before = keys(&connection, id);
        ensure_phonetic_keys(&connection, &weak, &EnglishPhonetic).unwrap();
        ensure_phonetic_keys(&connection, &weak, &FrenchPhonetic).unwrap();
        assert_eq!(keys(&connection, id), before);
    }

    #[test]
    fn the_packs_fingerprint_is_recorded_once_and_changes_with_the_packs() {
        let connection = connection();
        let base = PackSet::load("en-US", &[]).unwrap();
        let health = PackSet::load("en-US", &["health"]).unwrap();
        assert!(record_packs(&connection, &base).unwrap());
        assert!(!record_packs(&connection, &base).unwrap());
        assert_eq!(
            store::meta(&connection, "packs_hash").unwrap().unwrap(),
            base.hash()
        );
        assert!(record_packs(&connection, &health).unwrap());
    }
}
