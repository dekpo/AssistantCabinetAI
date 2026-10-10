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

use super::normalize::{normalize_name, surname_then_initial, TitleSet};
use super::packs::PackSet;
use super::phonetic::PhoneticEncoder;
use super::resolve::{
    alias_phonetic_key, aliases_for, AliasRequest, PossibleReason, SCORE_INITIAL,
};
use super::store;
use super::{AliasKind, EntityTypeId, Origin};
use crate::error::AppError;

const PHONETIC_KEY: &str = "phonetic_version";
const PACKS_KEY: &str = "packs_hash";
const ALIAS_GENERATION_KEY: &str = "alias_generation";

/// The generation of the rules `aliases_for` makes aliases by. An unrecorded base is generation 1.
/// Raise it when a change of those rules must reach entities that already exist, and add the
/// correction to [`ensure_alias_generation`].
///
/// 2: a name written surname then initial (`Dupont J`) has its surname alone and its initial form,
/// and no one-letter surname.
pub const ALIAS_GENERATION: u32 = 2;

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

/// What a refresh of the aliases did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasRefresh {
    /// The generation recorded before, empty when none was.
    pub previous: String,
    /// People read as a surname followed by an initial.
    pub entities: usize,
    /// Automatic aliases the current rules no longer make, deleted.
    pub removed: usize,
    /// Automatic aliases written or corrected.
    pub written: usize,
    /// Pairs recorded as possible matches (a full name this initial is compatible with).
    pub possible_matches: usize,
}

/// Bring the aliases of the people written as `Surname Initial` up to the current rules, if the
/// rules that made them are not the current generation (`ALIAS_GENERATION`, recorded in `kb_meta`).
///
/// Why it exists: re-reading a source finds its people already known, so it never makes their
/// aliases again, and an entity filed by the old rules (`Dupont J` with a surname `J` and an
/// initial `D. J`) would keep them for good. Nor would it ever meet `Jean Dupont` as a new name,
/// which is the moment a possible match is recorded. This pass corrects both, from the names the
/// base already holds, and reads no document.
///
/// Like every function of this module it runs only inside an Analyse the user started
/// (`KnowledgeContext::standard`), and in **one transaction**: a failure leaves the old aliases and
/// the old generation, so the next Analyse tries again. Only automatic aliases are removed; a
/// manual one is never touched. The titles seen with a person are kept: they are read back from the
/// title aliases it already has.
pub fn ensure_alias_generation(
    connection: &Connection,
    titles: &TitleSet,
    encoder: &dyn PhoneticEncoder,
) -> Result<AliasRefresh, AppError> {
    let previous = store::meta(connection, ALIAS_GENERATION_KEY)?.unwrap_or_default();
    let mut refresh = AliasRefresh {
        previous,
        entities: 0,
        removed: 0,
        written: 0,
        possible_matches: 0,
    };
    if refresh.previous == ALIAS_GENERATION.to_string() {
        return Ok(refresh);
    }

    let tx = connection
        .unchecked_transaction()
        .map_err(|_| AppError::KnowledgeUnavailable)?;
    let person = EntityTypeId::person();
    for (entity_id, canonical) in store::people_ending_in_a_letter(&tx)? {
        let name = normalize_name(&canonical, titles);
        let Some(form) = surname_then_initial(&name, titles) else {
            continue;
        };
        let Some(record) = store::entity_with_aliases(&tx, entity_id)? else {
            continue;
        };
        refresh.entities += 1;

        let titles_seen: Vec<String> = record
            .aliases
            .iter()
            .filter(|alias| alias.kind == AliasKind::TitleForm)
            .filter_map(|alias| alias.normalized.split(' ').next().map(str::to_string))
            .collect();
        let made = aliases_for(
            &AliasRequest {
                type_id: &person,
                canonical: &record.entity.canonical_name,
                titles_seen: &titles_seen,
                acronym: None,
            },
            titles,
            encoder,
        );
        for alias in &record.aliases {
            let rewritable = alias.origin == Origin::Automatic
                && matches!(
                    alias.kind,
                    AliasKind::Reordered
                        | AliasKind::Initial
                        | AliasKind::PartialSurname
                        | AliasKind::TitleForm
                );
            if rewritable && !made.iter().any(|new| new.normalized == alias.normalized) {
                refresh.removed += usize::from(store::remove_alias(&tx, alias.alias_id)?);
            }
        }
        for alias in &made {
            let unchanged = record.aliases.iter().any(|known| {
                known.normalized == alias.normalized
                    && known.kind == alias.kind
                    && known.display == alias.display
                    && known.phonetic_key == alias.phonetic_key
            });
            if !unchanged {
                store::add_alias(&tx, entity_id, alias, Origin::Automatic)?;
                refresh.written += 1;
            }
        }

        // The full names this initial fits: the same hint the resolver gives a new name.
        for (other, alias) in store::find_entities_by_alias(&tx, &form.initial_surname_key())? {
            let is_a_full_name = other.type_id == EntityTypeId::PERSON
                && alias.kind == AliasKind::Initial
                && surname_then_initial(&normalize_name(&other.canonical_name, titles), titles)
                    .is_none();
            if is_a_full_name
                && store::record_possible_match(
                    &tx,
                    entity_id,
                    other.entity_id,
                    PossibleReason::Initial.as_code(),
                    SCORE_INITIAL,
                )?
            {
                refresh.possible_matches += 1;
            }
        }
    }
    store::set_meta(&tx, ALIAS_GENERATION_KEY, &ALIAS_GENERATION.to_string())?;
    tx.commit().map_err(|_| AppError::KnowledgeUnavailable)?;
    Ok(refresh)
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
    use crate::knowledge::{EntityStatus, EntityTypeId, NewAlias, NewEntity};

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
        let health = PackSet::load("en-US", &["health-fr"]).unwrap();
        assert!(record_packs(&connection, &base).unwrap());
        assert!(!record_packs(&connection, &base).unwrap());
        assert_eq!(
            store::meta(&connection, "packs_hash").unwrap().unwrap(),
            base.hash()
        );
        assert!(record_packs(&connection, &health).unwrap());
    }

    /// A person filed by the rules before generation 2: `Dupont J` with a surname `J`, an initial
    /// `D. J`, and the reordered form.
    fn add_person_filed_by_the_old_rules(connection: &Connection, name: &str) -> i64 {
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
        let old = |display: &str, normalized: &str, kind: AliasKind| NewAlias {
            display: display.to_string(),
            normalized: normalized.to_string(),
            phonetic_key: "old".to_string(),
            kind,
            confidence: 0.6,
        };
        for alias in [
            old(name, &name.to_lowercase(), AliasKind::CanonicalVariant),
            old("J Dupont", "j dupont", AliasKind::Reordered),
            old("D. J", "d j", AliasKind::Initial),
            old("J", "j", AliasKind::PartialSurname),
            old("Dr J", "dr j", AliasKind::TitleForm),
        ] {
            store::add_alias(connection, id, &alias, Origin::Automatic).unwrap();
        }
        id
    }

    fn alias_forms(connection: &Connection, entity_id: i64) -> Vec<(String, AliasKind, Origin)> {
        store::entity_with_aliases(connection, entity_id)
            .unwrap()
            .unwrap()
            .aliases
            .into_iter()
            .map(|alias| (alias.normalized, alias.kind, alias.origin))
            .collect()
    }

    #[test]
    fn a_base_filed_by_the_old_rules_is_corrected_once_and_keeps_what_the_user_typed() {
        let connection = connection();
        let dupont = add_person_filed_by_the_old_rules(&connection, "Dupont J");
        store::add_alias(
            &connection,
            dupont,
            &NewAlias {
                display: "Madame D".to_string(),
                normalized: "madame d".to_string(),
                phonetic_key: String::new(),
                kind: AliasKind::Manual,
                confidence: 1.0,
            },
            Origin::Manual,
        )
        .unwrap();
        let jean = add_person(&connection, "Jean Dupont", &FrenchPhonetic);
        let sophie = add_person(&connection, "Sophie Martin", &FrenchPhonetic);

        let refresh = ensure_alias_generation(&connection, &titles(), &FrenchPhonetic).unwrap();
        assert_eq!(refresh.previous, "");
        assert_eq!(refresh.entities, 1, "only the surname-then-initial person");
        assert_eq!(refresh.possible_matches, 1, "Jean Dupont fits Dupont J");

        let mut forms = alias_forms(&connection, dupont);
        forms.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            forms,
            [
                ("dr dupont".to_string(), AliasKind::TitleForm, Origin::Automatic),
                ("dupont".to_string(), AliasKind::PartialSurname, Origin::Automatic),
                ("dupont j".to_string(), AliasKind::CanonicalVariant, Origin::Automatic),
                ("j dupont".to_string(), AliasKind::Initial, Origin::Automatic),
                ("madame d".to_string(), AliasKind::Manual, Origin::Manual),
            ],
            "no one-letter surname, no initial of the surname; the title seen is kept, on the surname"
        );
        // The other people are untouched, and the pair is recorded once, in one order.
        assert_eq!(alias_forms(&connection, jean).len(), 4);
        assert_eq!(alias_forms(&connection, sophie).len(), 4);
        let pairs: Vec<(i64, i64, String, String)> = connection
            .prepare("SELECT entity_a, entity_b, reason, state FROM kb_possible_matches")
            .unwrap()
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            pairs,
            [(
                dupont.min(jean),
                dupont.max(jean),
                "initial".to_string(),
                "open".to_string()
            )]
        );

        // The next Analyse finds the generation recorded and does nothing, even with a decision the
        // user took in between.
        connection
            .execute("UPDATE kb_possible_matches SET state = 'dismissed'", [])
            .unwrap();
        let again = ensure_alias_generation(&connection, &titles(), &FrenchPhonetic).unwrap();
        assert_eq!(again.previous, ALIAS_GENERATION.to_string());
        assert_eq!(
            (
                again.entities,
                again.removed,
                again.written,
                again.possible_matches
            ),
            (0, 0, 0, 0)
        );
        let state: String = connection
            .query_row("SELECT state FROM kb_possible_matches", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(state, "dismissed");
    }

    #[test]
    fn an_initial_that_no_full_name_fits_records_no_possible_match() {
        let connection = connection();
        add_person_filed_by_the_old_rules(&connection, "Martin A");
        add_person(&connection, "Sophie Martin", &FrenchPhonetic);
        let refresh = ensure_alias_generation(&connection, &titles(), &FrenchPhonetic).unwrap();
        assert_eq!(
            (refresh.entities, refresh.possible_matches),
            (1, 0),
            "A is not the initial of Sophie"
        );
    }

    #[test]
    fn a_failed_alias_refresh_keeps_the_old_aliases_and_the_old_record() {
        let connection = connection();
        let dupont = add_person_filed_by_the_old_rules(&connection, "Dupont J");
        // A trigger that refuses the first deletion stands for any failure half-way.
        connection
            .execute_batch(
                "CREATE TRIGGER refuse_delete BEFORE DELETE ON kb_aliases
                 BEGIN SELECT RAISE(ABORT, 'refused'); END;",
            )
            .unwrap();
        assert!(ensure_alias_generation(&connection, &titles(), &FrenchPhonetic).is_err());
        assert_eq!(alias_forms(&connection, dupont).len(), 5);
        assert_eq!(store::meta(&connection, "alias_generation").unwrap(), None);
    }
}
