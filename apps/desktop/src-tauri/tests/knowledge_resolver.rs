//! The deterministic resolver, one test per row of the table in `docs/SESSION-KB-00-master.md`
//! section 7, plus the rules that keep it safe: a suppressed name stays suppressed, ambiguity is
//! judged inside the selection first, a fuzzy match is a question and never a merge, a manual alias
//! beats an automatic one, a guess cannot drive a reduction.
//!
//! The first half runs against the in-memory lookup, so each rule is read without a database. The
//! second half runs the same rules through the real store (`StoreLookup`), to prove the two agree.
//!
//! **No test asserts a literal phonetic key.** Keys may change in a later calibration; what is
//! asserted is behaviour (two names the contract file lists as colliding are offered as a possible
//! match, never linked), and the keys of two names are only ever compared with each other.
//!
//! The words come from the real French base pack, so a title or a particle missing from the pack
//! fails here. Every name is invented.

use std::collections::BTreeSet;

use assistant_cabinet_ai_lib::knowledge::normalize::TitleSet;
use assistant_cabinet_ai_lib::knowledge::packs::{IdentifierStrength, PackSet};
use assistant_cabinet_ai_lib::knowledge::phonetic::{FrenchPhonetic, PhoneticEncoder};
use assistant_cabinet_ai_lib::knowledge::resolve::{
    aliases_for, AliasRequest, DeterministicResolver, EntityLookup, EntityResolver, IdentifierRef,
    InMemoryLookup, PossibleReason, Resolution, ResolveContext, ResolveMode, Signal, SourceSet,
    StoreLookup, SurfaceInput, UnresolvedReason, Via, CANDIDATE_CONFIDENCE_CAP,
};
use assistant_cabinet_ai_lib::knowledge::store as kb;
use assistant_cabinet_ai_lib::knowledge::{
    AliasDraft, AliasKind, AttributeDraft, Domain, EntityDraft, EntityRef, EntityStatus,
    EntityTypeId, KnowledgeDelta, MentionDraft, MentionLocator, Method, Origin, SourceRef,
    CURRENT_KB_VERSION,
};
use rusqlite::Connection;

// ----------------------------------------------------------------------------- harness

fn titles() -> TitleSet {
    PackSet::load("fr-FR", &[])
        .expect("the base pack")
        .title_set()
}

fn memory() -> InMemoryLookup {
    InMemoryLookup::new(titles(), Box::new(FrenchPhonetic))
}

fn person() -> EntityTypeId {
    EntityTypeId::person()
}

struct Ask<'a> {
    mode: ResolveMode,
    selection: Option<&'a SourceSet>,
    source_entities: Option<&'a BTreeSet<i64>>,
}

fn ingest<'a>() -> Ask<'a> {
    Ask {
        mode: ResolveMode::Ingestion,
        selection: None,
        source_entities: None,
    }
}

fn query<'a>(selection: Option<&'a SourceSet>) -> Ask<'a> {
    Ask {
        mode: ResolveMode::Query,
        selection,
        source_entities: None,
    }
}

fn run(lookup: &dyn EntityLookup, ask: &Ask<'_>, surface: &SurfaceInput) -> Resolution {
    let set = titles();
    let encoder = FrenchPhonetic;
    DeterministicResolver.resolve(
        surface,
        &ResolveContext {
            mode: ask.mode,
            titles: &set,
            encoder: &encoder,
            lookup,
            selection: ask.selection,
            source_entities: ask.source_entities,
        },
    )
}

fn resolve(lookup: &dyn EntityLookup, ask: &Ask<'_>, text: &str) -> Resolution {
    run(lookup, ask, &SurfaceInput::new(text))
}

fn existing(answer: &Resolution) -> (i64, Signal, f32) {
    match answer {
        Resolution::Existing {
            entity,
            signal,
            confidence,
            ..
        } => (entity.entity_id, *signal, confidence.value()),
        other => panic!("expected Existing, got {other:?}"),
    }
}

fn ids(answer: &Resolution) -> BTreeSet<i64> {
    answer.entity_ids().into_iter().collect()
}

fn set(values: &[i64]) -> BTreeSet<i64> {
    values.iter().copied().collect()
}

fn selection(values: &[i64]) -> SourceSet {
    SourceSet::new(values.iter().copied())
}

// ----------------------------------------------------------------------------- signal 1: identifiers

#[test]
fn an_exact_identifier_links_whatever_the_name() {
    let mut world = memory();
    let jean = world.add_person("Jean Dupont");
    world.add_identifier(jean, "email", "JEANDUPONTEXAMPLEORG");
    let surface = SurfaceInput::new("J.D.")
        .of_type(person())
        .with_identifier(IdentifierRef {
            scheme: "email".to_string(),
            normalized: "JEANDUPONTEXAMPLEORG".to_string(),
            strength: IdentifierStrength::Exact,
        });
    let answer = run(&world, &ingest(), &surface);
    assert_eq!(existing(&answer), (jean, Signal::Identifier, 1.0));
    match answer {
        Resolution::Existing { via, .. } => assert_eq!(via, Via::Identifier),
        _ => unreachable!(),
    }
}

#[test]
fn an_identifier_two_entities_carry_is_a_question_not_a_link() {
    let mut world = memory();
    let a = world.add_person("Jean Dupont");
    let b = world.add_person("Marie Dupont");
    world.add_identifier(a, "email", "SHARED");
    world.add_identifier(b, "email", "SHARED");
    let surface = SurfaceInput::new("Someone").with_identifier(IdentifierRef {
        scheme: "email".to_string(),
        normalized: "SHARED".to_string(),
        strength: IdentifierStrength::Exact,
    });
    let answer = run(&world, &ingest(), &surface);
    assert!(matches!(answer, Resolution::Ambiguous { .. }));
    assert_eq!(ids(&answer), set(&[a, b]));
}

#[test]
fn a_possible_identifier_like_a_phone_number_never_links() {
    let mut world = memory();
    let jean = world.add_person("Jean Dupont");
    world.add_identifier(jean, "phone", "0612345678");
    let surface = SurfaceInput::new("Marie Dupont").with_identifier(IdentifierRef {
        scheme: "phone".to_string(),
        normalized: "0612345678".to_string(),
        strength: IdentifierStrength::Possible,
    });
    match run(&world, &ingest(), &surface) {
        Resolution::PossibleMatch {
            entities,
            reason,
            create_separate,
            ..
        } => {
            assert_eq!(entities[0].entity_id, jean);
            assert_eq!(reason, PossibleReason::SharedIdentifier);
            assert!(create_separate, "ingestion keeps the two apart");
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn an_unknown_identifier_alone_makes_a_supported_entity() {
    let world = memory();
    let surface = SurfaceInput::new("")
        .found_by(Method::Identifier)
        .with_identifier(IdentifierRef {
            scheme: "invoice_number".to_string(),
            normalized: "FA20260042".to_string(),
            strength: IdentifierStrength::Exact,
        });
    match run(&world, &ingest(), &surface) {
        Resolution::NewEntity {
            status,
            signal,
            confidence,
        } => {
            assert_eq!(status, EntityStatus::Active);
            assert_eq!(signal, Signal::Identifier);
            assert_eq!(confidence.value(), 1.0);
        }
        other => panic!("unexpected {other:?}"),
    }
    // The same in a question: nothing to resolve.
    assert!(matches!(
        run(&world, &query(None), &surface),
        Resolution::Unresolved {
            reason: UnresolvedReason::NoMatch
        }
    ));
}

#[test]
fn an_identifier_keeps_to_the_type_that_was_asked_for() {
    let mut world = memory();
    let org = world.add("organization", "Acme Holdings");
    world.add_identifier(org, "siret", "12345678900012");
    let surface = SurfaceInput::new("Jean Dupont")
        .of_type(person())
        .with_identifier(IdentifierRef {
            scheme: "siret".to_string(),
            normalized: "12345678900012".to_string(),
            strength: IdentifierStrength::Exact,
        });
    assert!(matches!(
        run(&world, &ingest(), &surface),
        Resolution::NewEntity { .. }
    ));
}

// ----------------------------------------------------------------------------- signals 2 and 3: names

#[test]
fn the_same_name_in_any_dress_is_the_same_entity() {
    let mut world = memory();
    let helene = world.add_person("H\u{e9}l\u{e8}ne Moreau");
    for written in [
        "H\u{e9}l\u{e8}ne Moreau",
        "helene moreau",
        "HELENE MOREAU",
        "Dr. H\u{e9}l\u{e8}ne Moreau",
        "Docteur Helene  Moreau",
        "H\u{e9}l\u{e8}ne-Moreau",
        // NFD: the accents as separate marks, as a Mac stores a file name.
        "He\u{301}le\u{300}ne Moreau",
    ] {
        let answer = resolve(&world, &ingest(), written);
        assert_eq!(existing(&answer).0, helene, "{written:?}");
        assert_eq!(existing(&answer).1, Signal::CanonicalName, "{written:?}");
    }
}

#[test]
fn a_name_in_the_other_order_is_found_through_the_reordered_alias() {
    let mut world = memory();
    let jean = world.add_person("Jean Dupont");
    let answer = resolve(&world, &ingest(), "DUPONT, Jean");
    assert_eq!(existing(&answer), (jean, Signal::Alias, 0.9));
    match answer {
        Resolution::Existing { via, .. } => assert_eq!(via, Via::Alias(AliasKind::Reordered)),
        _ => unreachable!(),
    }
}

#[test]
fn an_elided_article_does_not_change_the_name() {
    let mut world = memory();
    let place = world.add("organization", "H\u{f4}pital Saint Michel");
    let hospital = world.add("organization", "l'H\u{f4}pital Central");
    let a = resolve(&world, &ingest(), "H\u{f4}pital Saint-Michel");
    assert_eq!(existing(&a).0, place);
    let b = resolve(&world, &ingest(), "H\u{f4}pital Central");
    assert_eq!(existing(&b).0, hospital);
}

#[test]
fn homonyms_with_different_disambiguators_stay_apart() {
    let mut world = memory();
    let old = world.add_with(
        "person",
        "Jean Dupont",
        EntityStatus::Active,
        Origin::Automatic,
        "1950",
    );
    let young = world.add_with(
        "person",
        "Jean Dupont",
        EntityStatus::Active,
        Origin::Automatic,
        "1985",
    );
    let bare = resolve(&world, &ingest(), "Jean Dupont");
    assert!(matches!(bare, Resolution::Ambiguous { .. }));
    assert_eq!(ids(&bare), set(&[old, young]));

    let told = run(
        &world,
        &ingest(),
        &SurfaceInput::new("Jean Dupont").with_disambiguator("1985"),
    );
    assert_eq!(existing(&told).0, young);
}

#[test]
fn a_manual_entity_or_alias_beats_an_automatic_one() {
    let mut world = memory();
    let guessed = world.add_with(
        "person",
        "Pierre Martin",
        EntityStatus::Active,
        Origin::Automatic,
        "",
    );
    let typed = world.add_with(
        "person",
        "Paul Martin",
        EntityStatus::Active,
        Origin::Manual,
        "",
    );
    world.add_alias(guessed, "PM", AliasKind::Abbreviation, Origin::Automatic);
    world.add_alias(typed, "PM", AliasKind::Abbreviation, Origin::Manual);
    let answer = resolve(&world, &ingest(), "PM");
    assert_eq!(existing(&answer), (typed, Signal::Alias, 0.95));

    // Two typed rows are a real tie.
    world.add_alias(guessed, "PM", AliasKind::Manual, Origin::Manual);
    let tie = resolve(&world, &ingest(), "PM");
    assert!(matches!(tie, Resolution::Ambiguous { .. }), "{tie:?}");
}

#[test]
fn a_name_the_user_deleted_resolves_to_nothing_and_creates_nothing() {
    let mut world = memory();
    world.add_person("Marie Dupont");
    world.tombstone("person", "Jean Dupont");
    for mode in [ingest(), query(None)] {
        match resolve(&world, &mode, "Dr Jean Dupont") {
            Resolution::Unresolved { reason } => assert_eq!(reason, UnresolvedReason::Suppressed),
            other => panic!("unexpected {other:?}"),
        }
    }
    // Another name of the same type is not affected.
    assert!(matches!(
        resolve(&world, &ingest(), "Marie Dupont"),
        Resolution::Existing { .. }
    ));
}

#[test]
fn a_tombstone_of_one_type_does_not_suppress_another() {
    let mut world = memory();
    world.tombstone("person", "Acme");
    let org = SurfaceInput::new("Acme").of_type(EntityTypeId::organization());
    assert!(matches!(
        run(&world, &ingest(), &org),
        Resolution::NewEntity { .. }
    ));
    let who = SurfaceInput::new("Acme").of_type(person());
    assert!(matches!(
        run(&world, &ingest(), &who),
        Resolution::Unresolved {
            reason: UnresolvedReason::Suppressed
        }
    ));
}

#[test]
fn a_type_that_was_asked_for_filters_the_hits() {
    let mut world = memory();
    world.add("organization", "Dupont Freres");
    let who = SurfaceInput::new("Dupont Freres").of_type(person());
    assert!(matches!(
        run(&world, &ingest(), &who),
        Resolution::NewEntity { .. }
    ));
    let org = SurfaceInput::new("Dupont Freres").of_type(EntityTypeId::organization());
    assert!(matches!(
        run(&world, &ingest(), &org),
        Resolution::Existing { .. }
    ));
}

// ----------------------------------------------------------------------------- initials and surnames

#[test]
fn an_initial_and_a_surname_links_only_when_the_source_cannot_mean_anyone_else() {
    let mut world = memory();
    let jean = world.add_person("Jean Dupont");
    let in_source = set(&[jean]);
    let ask = Ask {
        mode: ResolveMode::Ingestion,
        selection: None,
        source_entities: Some(&in_source),
    };
    let answer = resolve(&world, &ask, "J. Dupont");
    assert_eq!(existing(&answer).0, jean);
    match answer {
        Resolution::Existing {
            via, confidence, ..
        } => {
            assert_eq!(via, Via::Alias(AliasKind::Initial));
            assert!(confidence.value() < 0.8, "never enough to narrow a scope");
        }
        _ => unreachable!(),
    }

    // Without the list of the source's entities there is no way to know: it stands apart.
    assert!(matches!(
        resolve(&world, &ingest(), "J. Dupont"),
        Resolution::NewEntity {
            status: EntityStatus::Candidate,
            ..
        }
    ));
}

#[test]
fn an_initial_stands_apart_when_the_surname_is_shared_in_the_source() {
    let mut world = memory();
    let jean = world.add_person("Jean Dupont");
    let marie = world.add_person("Marie Dupont");
    let in_source = set(&[jean, marie]);
    let ask = Ask {
        mode: ResolveMode::Ingestion,
        selection: None,
        source_entities: Some(&in_source),
    };
    // Only Jean carries the initial J, but Marie is in the source with the same surname: not safe.
    assert!(matches!(
        resolve(&world, &ask, "J. Dupont"),
        Resolution::NewEntity {
            status: EntityStatus::Candidate,
            ..
        }
    ));
}

#[test]
fn an_initial_two_full_names_share_is_not_linked_in_ingestion() {
    let mut world = memory();
    let jean = world.add_person("Jean Dupont");
    let jacques = world.add_person("Jacques Dupont");
    let in_source = set(&[jean, jacques]);
    let ask = Ask {
        mode: ResolveMode::Ingestion,
        selection: None,
        source_entities: Some(&in_source),
    };
    assert!(matches!(
        resolve(&world, &ask, "J. Dupont"),
        Resolution::NewEntity {
            status: EntityStatus::Candidate,
            ..
        }
    ));
}

#[test]
fn a_surname_alone_in_ingestion_links_only_to_the_one_entity_of_its_source() {
    let mut world = memory();
    let pierre = world.add_person("Pierre Martin");
    let paul = world.add_person("Paul Martin");

    let one = set(&[pierre]);
    let ask = Ask {
        mode: ResolveMode::Ingestion,
        selection: None,
        source_entities: Some(&one),
    };
    let answer = resolve(&world, &ask, "Dr Martin");
    assert_eq!(existing(&answer).0, pierre);
    assert!(existing(&answer).2 < 0.8);

    let both = set(&[pierre, paul]);
    let ask = Ask {
        mode: ResolveMode::Ingestion,
        selection: None,
        source_entities: Some(&both),
    };
    assert!(matches!(
        resolve(&world, &ask, "Dr Martin"),
        Resolution::Unresolved {
            reason: UnresolvedReason::SurnameOnly
        }
    ));

    // Known elsewhere, absent from this source: no new alias, no guess.
    let none = BTreeSet::new();
    let ask = Ask {
        mode: ResolveMode::Ingestion,
        selection: None,
        source_entities: Some(&none),
    };
    assert!(matches!(
        resolve(&world, &ask, "Dr Martin"),
        Resolution::Unresolved {
            reason: UnresolvedReason::SurnameOnly
        }
    ));
}

#[test]
fn a_surname_nobody_holds_becomes_a_candidate_not_a_certainty() {
    let world = memory();
    match run(
        &world,
        &ingest(),
        &SurfaceInput::new("Dr Lambert").found_by(Method::TitleSurname),
    ) {
        Resolution::NewEntity {
            status, confidence, ..
        } => {
            assert_eq!(status, EntityStatus::Candidate);
            assert!((confidence.value() - 0.7).abs() < 1e-6);
        }
        other => panic!("unexpected {other:?}"),
    }
}

// ----------------------------------------------------------------------------- signal 6: fuzzy, never a merge

#[test]
fn a_misspelling_is_a_possible_match_and_never_a_link() {
    let mut world = memory();
    let jean = world.add_person("Jean Dupont");
    // The contract file (phonetic-fr.json) lists Dupont and Dupond as the same sound.
    for written in ["Jean Dupond", "Dupond"] {
        match resolve(&world, &ingest(), written) {
            Resolution::PossibleMatch {
                entities,
                reason,
                create_separate,
                ..
            } => {
                assert_eq!(entities.len(), 1, "{written}");
                assert_eq!(entities[0].entity_id, jean, "{written}");
                assert_eq!(reason, PossibleReason::Phonetic);
                assert!(
                    create_separate,
                    "{written}: ingestion keeps a separate entity"
                );
            }
            other => panic!("{written}: unexpected {other:?}"),
        }
    }
    // In a question it is only a "did you mean", nothing to create.
    match resolve(&world, &query(None), "Jean Dupond") {
        Resolution::PossibleMatch {
            create_separate, ..
        } => assert!(!create_separate),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn a_man_and_a_woman_who_sound_alike_are_a_question_to_the_user() {
    let mut world = memory();
    world.add_person("Reine Dubois");
    // Whether the two share a key is the encoder's business; what must hold is that they are
    // never linked.
    let answer = resolve(&world, &query(None), "Rene Dubois");
    assert!(
        matches!(
            answer,
            Resolution::PossibleMatch { .. } | Resolution::Unresolved { .. }
        ),
        "{answer:?}"
    );
    let encoder = FrenchPhonetic;
    let set = titles();
    let key = |text: &str| {
        encoder.encode_name(
            &assistant_cabinet_ai_lib::knowledge::normalize::normalize_name(text, &set),
        )
    };
    if key("Rene Dubois") == key("Reine Dubois") {
        assert!(matches!(answer, Resolution::PossibleMatch { .. }));
    }
}

#[test]
fn a_short_word_is_never_bridged_by_the_fuzzy_rule() {
    let mut world = memory();
    world.add_person("Lea Smith");
    // One edit on a three-letter word: too much of the name changes.
    assert!(matches!(
        resolve(&world, &ingest(), "Lee Smith"),
        Resolution::NewEntity { .. }
    ));
}

#[test]
fn the_same_words_in_another_order_are_a_possible_match() {
    let mut world = memory();
    let marie = world.add_person("Marie Claire Dupont");
    // The reordered alias covers surname-first; this order has no alias.
    match resolve(&world, &ingest(), "Claire Marie Dupont") {
        Resolution::PossibleMatch {
            entities, reason, ..
        } => {
            assert_eq!(entities[0].entity_id, marie);
            assert_eq!(reason, PossibleReason::TokenSet);
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn two_words_that_differ_are_not_a_fuzzy_match() {
    let mut world = memory();
    world.add_person("Jean Dupont");
    assert!(matches!(
        resolve(&world, &ingest(), "Jane Dupond"),
        Resolution::NewEntity { .. }
    ));
}

// ----------------------------------------------------------------------------- signal 7 and the status of a new entity

#[test]
fn a_new_entity_starts_active_only_when_something_backs_it() {
    let world = memory();
    let case = |text: &str, type_id: EntityTypeId, method: Method| match run(
        &world,
        &ingest(),
        &SurfaceInput::new(text).of_type(type_id).found_by(method),
    ) {
        Resolution::NewEntity {
            status,
            signal,
            confidence,
        } => (status, signal, confidence.value()),
        other => panic!("{text}: unexpected {other:?}"),
    };
    assert_eq!(
        case("Jean Dupont", person(), Method::CapitalisedName),
        (EntityStatus::Candidate, Signal::None, 0.4)
    );
    assert_eq!(
        case("Dr Jean Dupont", person(), Method::TitleFullName),
        (EntityStatus::Active, Signal::StrongPattern, 0.85)
    );
    assert_eq!(
        case(
            "Dupont SARL",
            EntityTypeId::organization(),
            Method::OrganizationMarker
        ),
        (EntityStatus::Active, Signal::StrongPattern, 0.8)
    );
    assert_eq!(
        case("Dupont", person(), Method::ColumnValue),
        (EntityStatus::Active, Signal::ColumnValue, 0.95)
    );
    // A surname alone is a guess whatever found it, unless a column vouched for it.
    assert_eq!(
        case("Lambert", person(), Method::TitleFullName).0,
        EntityStatus::Candidate
    );
    assert_eq!(
        case("Jean Dupont", person(), Method::FilenameToken),
        (EntityStatus::Candidate, Signal::None, 0.5)
    );
}

// ----------------------------------------------------------------------------- query mode and the selection

#[test]
fn a_full_name_inside_the_selection_is_resolved() {
    let mut world = memory();
    let jean = world.add_person("Jean Dupont");
    world.mention(jean, 1);
    let chosen = selection(&[1, 2]);
    let answer = resolve(&world, &query(Some(&chosen)), "Dr Jean Dupont");
    assert_eq!(existing(&answer), (jean, Signal::CanonicalName, 0.95));
}

#[test]
fn a_surname_is_judged_inside_the_selection_first() {
    let mut world = memory();
    let pierre = world.add_person("Pierre Martin");
    let paul = world.add_person("Paul Martin");
    world.mention(pierre, 1);
    world.mention(paul, 2);

    // Two Martins exist, one is inside the selection: resolved, and the other is never named.
    let only_first = selection(&[1]);
    let answer = resolve(&world, &query(Some(&only_first)), "Dr Martin");
    assert_eq!(existing(&answer), (pierre, Signal::Alias, 0.8));

    // Both inside: ambiguous, and the answer names exactly those two.
    let both = selection(&[1, 2]);
    let answer = resolve(&world, &query(Some(&both)), "Martin");
    assert!(matches!(answer, Resolution::Ambiguous { .. }));
    assert_eq!(ids(&answer), set(&[pierre, paul]));

    // Neither inside: unresolved, and it says it is known elsewhere without naming anyone.
    let elsewhere = selection(&[3]);
    match resolve(&world, &query(Some(&elsewhere)), "Martin") {
        Resolution::Unresolved { reason } => {
            assert_eq!(reason, UnresolvedReason::NotInSelection);
        }
        other => panic!("unexpected {other:?}"),
    }

    // No selection at all: the whole store answers.
    assert!(matches!(
        resolve(&world, &query(None), "Martin"),
        Resolution::Ambiguous { .. }
    ));
}

#[test]
fn an_ambiguous_answer_never_names_an_entity_outside_the_selection() {
    let mut world = memory();
    let inside_a = world.add_person("Pierre Martin");
    let inside_b = world.add_person("Paul Martin");
    let outside = world.add_person("Paule Martin");
    world.mention(inside_a, 1);
    world.mention(inside_b, 1);
    world.mention(outside, 9);
    let chosen = selection(&[1]);
    let answer = resolve(&world, &query(Some(&chosen)), "Martin");
    assert_eq!(ids(&answer), set(&[inside_a, inside_b]));
    assert!(!answer.entity_ids().contains(&outside));
}

#[test]
fn an_entity_outside_the_selection_is_not_resolved_by_its_full_name() {
    let mut world = memory();
    let jean = world.add_person("Jean Dupont");
    world.mention(jean, 5);
    let chosen = selection(&[1]);
    assert!(matches!(
        resolve(&world, &query(Some(&chosen)), "Jean Dupont"),
        Resolution::Unresolved {
            reason: UnresolvedReason::NotInSelection
        }
    ));
}

#[test]
fn an_initial_and_a_surname_in_a_question_is_offered_never_assumed() {
    let mut world = memory();
    let jean = world.add_person("Jean Dupont");
    world.mention(jean, 1);
    let chosen = selection(&[1]);
    match resolve(&world, &query(Some(&chosen)), "J. Dupont") {
        Resolution::PossibleMatch {
            entities,
            reason,
            create_separate,
            ..
        } => {
            assert_eq!(entities[0].entity_id, jean);
            assert_eq!(reason, PossibleReason::Initial);
            assert!(!create_separate);
        }
        other => panic!("unexpected {other:?}"),
    }

    let jacques = world.add_person("Jacques Dupont");
    world.mention(jacques, 1);
    assert!(matches!(
        resolve(&world, &query(Some(&chosen)), "J. Dupont"),
        Resolution::Ambiguous { .. }
    ));
}

#[test]
fn a_guess_can_rank_but_never_narrow() {
    let mut world = memory();
    let guess = world.add_with(
        "person",
        "Jean Dupont",
        EntityStatus::Candidate,
        Origin::Automatic,
        "",
    );
    world.mention(guess, 1);
    let chosen = selection(&[1]);
    let (_, _, confidence) = existing(&resolve(&world, &query(Some(&chosen)), "Jean Dupont"));
    assert!(
        confidence <= CANDIDATE_CONFIDENCE_CAP + 1e-6,
        "{confidence}"
    );
    assert!(confidence < 0.8, "the reduction threshold");
    // The same entity seen while reading a file is linked at the normal strength: ingestion is
    // how a candidate gets its second sighting.
    let (_, _, in_ingestion) = existing(&resolve(&world, &ingest(), "Jean Dupont"));
    assert!(in_ingestion > 0.8);
}

#[test]
fn a_title_seen_with_a_person_is_remembered_as_an_alias() {
    let mut world = memory();
    let jean = world.add_person("Jean Dupont");
    world.add_alias(jean, "Dr Dupont", AliasKind::TitleForm, Origin::Automatic);
    world.mention(jean, 1);
    let chosen = selection(&[1]);
    let answer = resolve(&world, &query(Some(&chosen)), "Dr. Dupont");
    assert_eq!(existing(&answer).0, jean);
    assert!(
        existing(&answer).2 <= 0.8 + 1e-6,
        "a surname is never certain"
    );
}

#[test]
fn nothing_known_is_unresolved_in_a_question_and_new_in_a_file() {
    let world = memory();
    match resolve(&world, &query(None), "Zoe Quinn") {
        Resolution::Unresolved { reason } => assert_eq!(reason, UnresolvedReason::NoMatch),
        other => panic!("unexpected {other:?}"),
    }
    assert!(matches!(
        resolve(&world, &ingest(), "Zoe Quinn"),
        Resolution::NewEntity { .. }
    ));
}

#[test]
fn a_name_made_only_of_titles_resolves_to_nothing() {
    let world = memory();
    for mode in [ingest(), query(None)] {
        assert!(matches!(
            resolve(&world, &mode, "Dr"),
            Resolution::Unresolved {
                reason: UnresolvedReason::Empty
            }
        ));
        assert!(matches!(
            resolve(&world, &mode, "   "),
            Resolution::Unresolved {
                reason: UnresolvedReason::Empty
            }
        ));
    }
}

#[test]
fn a_resolution_names_its_outcome_and_its_codes() {
    let mut world = memory();
    world.add_person("Jean Dupont");
    let answer = resolve(&world, &ingest(), "Jean Dupont");
    assert_eq!(answer.outcome_code(), "existing");
    assert_eq!(answer.confidence().value(), 0.95);
    assert_eq!(Signal::CanonicalName.as_code(), "canonical_name");
    assert_eq!(Signal::CanonicalName.rank(), 2);
    assert_eq!(PossibleReason::TokenSet.as_code(), "token_set");
    assert_eq!(
        UnresolvedReason::NotInSelection.as_code(),
        "not_in_selection"
    );
    assert_eq!(
        Method::from_code(Method::GazetteerHit.as_code()),
        Some(Method::GazetteerHit)
    );
}

// ----------------------------------------------------------------------------- the same rules through the real store

fn connection() -> Connection {
    let connection = Connection::open_in_memory().expect("in-memory database");
    assistant_cabinet_ai_lib::index_store::configure_connection(&connection).unwrap();
    kb::ensure_schema(&connection).unwrap();
    connection
}

fn source(path: &str) -> SourceRef {
    SourceRef {
        domain: Domain::Documents,
        relative_path: path.to_string(),
        content_id: format!("sha-{path}"),
    }
}

/// Learn `names` (all persons) in one document, with the aliases a real pass would give them.
fn learn(connection: &mut Connection, path: &str, names: &[&str]) {
    let set = titles();
    let encoder = FrenchPhonetic;
    let mut delta = KnowledgeDelta::empty(source(path));
    delta.kb_version = CURRENT_KB_VERSION;
    for (index, name) in names.iter().enumerate() {
        let temp = index as u32 + 1;
        let normalized =
            assistant_cabinet_ai_lib::knowledge::normalize::normalize_name(name, &set).joined;
        delta.entities.push(EntityDraft {
            temp_id: temp,
            type_id: "person".to_string(),
            subtype: None,
            canonical_name: name.to_string(),
            normalized_name: normalized,
            disambiguator: String::new(),
            status: EntityStatus::Active,
        });
        for alias in aliases_for(
            &AliasRequest {
                type_id: &person(),
                canonical: name,
                titles_seen: &[],
                acronym: None,
            },
            &set,
            &encoder,
        ) {
            delta.aliases.push(AliasDraft {
                entity: EntityRef::Draft(temp),
                display: alias.display,
                normalized: alias.normalized,
                phonetic_key: alias.phonetic_key,
                kind: alias.kind,
                confidence: alias.confidence,
            });
        }
        delta.mentions.push(MentionDraft {
            entity: EntityRef::Draft(temp),
            alias_normalized: None,
            role: None,
            locator: MentionLocator::Chunk {
                chunk_id: format!("{path}#p1#s{temp}"),
            },
            occurrence_count: 1,
            confidence: 0.9,
            method: Method::GazetteerHit.as_code().to_string(),
            extractor_version: 1,
        });
    }
    let tx = connection.transaction().unwrap();
    kb::apply_delta(&tx, &delta).unwrap();
    tx.commit().unwrap();
}

fn entity_id(connection: &Connection, normalized: &str) -> i64 {
    kb::find_entities_by_normalized(connection, normalized).unwrap()[0].entity_id
}

fn source_set(connection: &Connection, paths: &[&str]) -> SourceSet {
    let scope: Vec<(String, String)> = paths
        .iter()
        .map(|path| (path.to_string(), format!("sha-{path}")))
        .collect();
    SourceSet::new(kb::source_ids_for_scope(connection, Domain::Documents, &scope).unwrap())
}

fn with_store<R>(connection: &Connection, body: impl FnOnce(&StoreLookup<'_>) -> R) -> R {
    let set = titles();
    let lookup = StoreLookup::new(connection, &set);
    body(&lookup)
}

#[test]
fn the_store_resolves_a_name_in_any_dress() {
    let mut connection = connection();
    learn(
        &mut connection,
        "a.txt",
        &["H\u{e9}l\u{e8}ne Moreau", "Jean Dupont"],
    );
    let helene = entity_id(&connection, "helene moreau");
    with_store(&connection, |lookup| {
        for written in [
            "Dr H\u{e9}l\u{e8}ne Moreau",
            "MOREAU, Helene",
            "helene-moreau",
        ] {
            let answer = resolve(lookup, &ingest(), written);
            assert_eq!(existing(&answer).0, helene, "{written}");
        }
    });
}

#[test]
fn the_store_judges_a_surname_inside_the_selection() {
    let mut connection = connection();
    learn(&mut connection, "one.txt", &["Pierre Martin"]);
    learn(&mut connection, "two.txt", &["Paul Martin"]);
    let pierre = entity_id(&connection, "pierre martin");
    let paul = entity_id(&connection, "paul martin");

    let first = source_set(&connection, &["one.txt"]);
    let both = source_set(&connection, &["one.txt", "two.txt"]);
    let neither = source_set(&connection, &[]);
    assert_eq!(first.len(), 1);
    assert_eq!(both.len(), 2);

    with_store(&connection, |lookup| {
        assert_eq!(
            existing(&resolve(lookup, &query(Some(&first)), "Dr Martin")),
            (pierre, Signal::Alias, 0.8)
        );
        let answer = resolve(lookup, &query(Some(&both)), "Martin");
        assert!(matches!(answer, Resolution::Ambiguous { .. }));
        assert_eq!(ids(&answer), set(&[pierre, paul]));
        // An empty selection allows nothing, not everything.
        assert!(matches!(
            resolve(lookup, &query(Some(&neither)), "Martin"),
            Resolution::Unresolved {
                reason: UnresolvedReason::NotInSelection
            }
        ));
    });
}

#[test]
fn a_file_that_changed_no_longer_counts_for_the_old_selection() {
    let mut connection = connection();
    learn(&mut connection, "one.txt", &["Pierre Martin"]);
    // The selection pins the file by content: this one is another file with the same name.
    let stale = SourceSet::new(
        kb::source_ids_for_scope(
            &connection,
            Domain::Documents,
            &[("one.txt".to_string(), "sha-another".to_string())],
        )
        .unwrap(),
    );
    assert!(stale.is_empty());
    with_store(&connection, |lookup| {
        assert!(matches!(
            resolve(lookup, &query(Some(&stale)), "Pierre Martin"),
            Resolution::Unresolved { .. }
        ));
    });
}

#[test]
fn the_store_offers_a_misspelling_and_the_other_word_order() {
    let mut connection = connection();
    learn(
        &mut connection,
        "a.txt",
        &["Jean Dupont", "Marie Claire Dupont"],
    );
    let jean = entity_id(&connection, "jean dupont");
    let marie = entity_id(&connection, "marie claire dupont");
    with_store(&connection, |lookup| {
        match resolve(lookup, &ingest(), "Jean Dupond") {
            Resolution::PossibleMatch {
                entities, reason, ..
            } => {
                assert_eq!(entities[0].entity_id, jean);
                assert_eq!(reason, PossibleReason::Phonetic);
            }
            other => panic!("unexpected {other:?}"),
        }
        match resolve(lookup, &ingest(), "Claire Marie Dupont") {
            Resolution::PossibleMatch {
                entities, reason, ..
            } => {
                assert_eq!(entities[0].entity_id, marie);
                assert_eq!(reason, PossibleReason::TokenSet);
            }
            other => panic!("unexpected {other:?}"),
        }
    });
}

#[test]
fn the_store_finds_an_entity_by_an_identifier_attribute() {
    let mut connection = connection();
    learn(&mut connection, "a.txt", &["Jean Dupont"]);
    let jean = entity_id(&connection, "jean dupont");
    kb::set_attribute(
        &connection,
        jean,
        "identifier:email",
        "JEANDUPONTEXAMPLEORG",
        Origin::Manual,
        None,
    )
    .unwrap();
    let surface = SurfaceInput::new("someone")
        .of_type(person())
        .with_identifier(IdentifierRef {
            scheme: "email".to_string(),
            normalized: "JEANDUPONTEXAMPLEORG".to_string(),
            strength: IdentifierStrength::Exact,
        });
    with_store(&connection, |lookup| {
        assert_eq!(
            existing(&run(lookup, &ingest(), &surface)),
            (jean, Signal::Identifier, 1.0)
        );
    });
}

#[test]
fn an_identifier_that_is_an_entity_is_found_by_its_value() {
    let connection = connection();
    kb::create_entity(
        &connection,
        &assistant_cabinet_ai_lib::knowledge::NewEntity {
            type_id: "identifier".to_string(),
            subtype: Some("invoice".to_string()),
            canonical_name: "FA-2026-0042".to_string(),
            normalized_name: "FA20260042".to_string(),
            disambiguator: String::new(),
            status: EntityStatus::Active,
            origin: Origin::Automatic,
        },
    )
    .unwrap();
    let surface = SurfaceInput::new("FA-2026-0042")
        .of_type(EntityTypeId::new("identifier").unwrap())
        .with_identifier(IdentifierRef {
            scheme: "invoice_number".to_string(),
            normalized: "FA20260042".to_string(),
            strength: IdentifierStrength::Exact,
        });
    with_store(&connection, |lookup| {
        assert_eq!(
            existing(&run(lookup, &ingest(), &surface)).1,
            Signal::Identifier
        );
    });
}

#[test]
fn the_store_honours_a_tombstone_and_follows_a_merge() {
    let mut connection = connection();
    learn(
        &mut connection,
        "a.txt",
        &["Jean Dupont", "Jeanne Dupont", "Marie Curie"],
    );
    let jean = entity_id(&connection, "jean dupont");
    let jeanne = entity_id(&connection, "jeanne dupont");
    let marie = entity_id(&connection, "marie curie");

    // Merge Jeanne into Jean: her name now leads to the survivor.
    let tx = connection.transaction().unwrap();
    kb::merge_entities(&tx, jean, jeanne).unwrap();
    tx.commit().unwrap();
    // Delete Marie: a tombstone.
    let tx = connection.transaction().unwrap();
    kb::delete_entity(&tx, marie).unwrap();
    tx.commit().unwrap();

    with_store(&connection, |lookup| {
        assert_eq!(
            existing(&resolve(lookup, &ingest(), "Jeanne Dupont")).0,
            jean
        );
        match resolve(lookup, &ingest(), "Marie Curie") {
            Resolution::Unresolved { reason } => assert_eq!(reason, UnresolvedReason::Suppressed),
            other => panic!("unexpected {other:?}"),
        }
    });
}

#[test]
fn the_store_prefers_the_alias_the_user_typed() {
    let mut connection = connection();
    learn(&mut connection, "a.txt", &["Pierre Martin", "Paul Martin"]);
    let pierre = entity_id(&connection, "pierre martin");
    let paul = entity_id(&connection, "paul martin");
    for (id, origin) in [(pierre, Origin::Automatic), (paul, Origin::Manual)] {
        kb::add_alias(
            &connection,
            id,
            &assistant_cabinet_ai_lib::knowledge::NewAlias {
                display: "PM".to_string(),
                normalized: "pm".to_string(),
                phonetic_key: String::new(),
                kind: AliasKind::Abbreviation,
                confidence: 1.0,
            },
            origin,
        )
        .unwrap();
    }
    with_store(&connection, |lookup| {
        assert_eq!(
            existing(&resolve(lookup, &ingest(), "PM")),
            (paul, Signal::Alias, 0.95)
        );
    });
}

#[test]
fn a_lookup_error_means_unresolved_never_new() {
    let mut connection = connection();
    learn(&mut connection, "a.txt", &["Jean Dupont"]);
    // The knowledge base becomes unreadable under the resolver's feet.
    connection
        .execute_batch("DROP VIEW kb_effective_mentions; DROP VIEW kb_effective_entity;")
        .unwrap();
    with_store(&connection, |lookup| {
        for mode in [ingest(), query(None)] {
            match resolve(lookup, &mode, "Jean Dupont") {
                Resolution::Unresolved { reason } => {
                    assert_eq!(reason, UnresolvedReason::LookupFailed);
                }
                other => panic!("unexpected {other:?}"),
            }
        }
        assert!(lookup.failed());
    });
}

#[test]
fn resolving_changes_nothing_in_the_store() {
    let mut connection = connection();
    learn(&mut connection, "a.txt", &["Jean Dupont", "Pierre Martin"]);
    let before = kb::table_counts(&connection).unwrap();
    let chosen = source_set(&connection, &["a.txt"]);
    with_store(&connection, |lookup| {
        for text in [
            "Jean Dupont",
            "Martin",
            "J. Dupont",
            "Jean Dupond",
            "Zoe Quinn",
            "Dr",
        ] {
            resolve(lookup, &ingest(), text);
            resolve(lookup, &query(Some(&chosen)), text);
        }
    });
    assert_eq!(kb::table_counts(&connection).unwrap(), before);
    assert!(kb::integrity_check(&connection).unwrap().is_clean());
}

#[test]
fn an_attribute_draft_reaches_the_resolver_through_a_pass() {
    // A delta that states an identifier for an entity is enough for the next file to be linked by it.
    let mut connection = connection();
    let set = titles();
    let mut delta = KnowledgeDelta::empty(source("b.txt"));
    delta.kb_version = CURRENT_KB_VERSION;
    delta.entities.push(EntityDraft {
        temp_id: 1,
        type_id: "person".to_string(),
        subtype: None,
        canonical_name: "Jean Dupont".to_string(),
        normalized_name: "jean dupont".to_string(),
        disambiguator: String::new(),
        status: EntityStatus::Active,
    });
    delta.attributes.push(AttributeDraft {
        entity: EntityRef::Draft(1),
        key: "identifier:email".to_string(),
        value: "JD".to_string(),
    });
    delta.mentions.push(MentionDraft {
        entity: EntityRef::Draft(1),
        alias_normalized: None,
        role: None,
        locator: MentionLocator::Filename,
        occurrence_count: 1,
        confidence: 0.5,
        method: Method::FilenameToken.as_code().to_string(),
        extractor_version: 1,
    });
    let tx = connection.transaction().unwrap();
    kb::apply_delta(&tx, &delta).unwrap();
    tx.commit().unwrap();
    let lookup = StoreLookup::new(&connection, &set);
    let hits = lookup.by_identifier("email", "JD");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].canonical_name, "Jean Dupont");
}

// ----------------------------------------------------------------------------- the contract file

fn cases_file() -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("knowledge")
        .join("resolver-cases.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    serde_json::from_str(&text).expect("resolver-cases.json is valid JSON")
}

fn numbers(value: &serde_json::Value) -> Option<Vec<i64>> {
    value
        .as_array()
        .map(|items| items.iter().filter_map(|item| item.as_i64()).collect())
}

/// Runs every row of `tests/fixtures/knowledge/resolver-cases.json` and prints the table. The owner
/// edits that file; a wrong row fails by name.
///
/// ```text
/// cargo test --test knowledge_resolver the_resolver_cases_hold -- --nocapture
/// ```
#[test]
fn the_resolver_cases_hold() {
    let file = cases_file();
    let mut world = memory();
    let mut names: std::collections::BTreeMap<String, i64> = std::collections::BTreeMap::new();
    for entry in file["world"].as_array().expect("world") {
        let name = entry["name"].as_str().expect("name");
        let id = world.add(entry["type"].as_str().expect("type"), name);
        for source in numbers(&entry["sources"]).unwrap_or_default() {
            world.mention(id, source);
        }
        names.insert(name.to_string(), id);
    }
    for entry in file["deleted"].as_array().into_iter().flatten() {
        world.tombstone(
            entry["type"].as_str().expect("type"),
            entry["name"].as_str().expect("name"),
        );
    }
    let name_of = |id: i64| -> String {
        names
            .iter()
            .find(|(_, known)| **known == id)
            .map(|(name, _)| name.clone())
            .unwrap_or_else(|| format!("#{id}"))
    };

    let mut failures = Vec::new();
    println!();
    println!(
        "{:<24} {:<9} {:<9} {:<15} {:<42} {:<17} ok",
        "text", "mode", "selection", "answer", "points at", "reason"
    );
    for case in file["cases"].as_array().expect("cases") {
        let text = case["text"].as_str().expect("text");
        let mode = match case["mode"].as_str().expect("mode") {
            "query" => ResolveMode::Query,
            "ingestion" => ResolveMode::Ingestion,
            other => panic!("unknown mode {other}"),
        };
        let selection_ids = numbers(&case["selection"]);
        let chosen = selection_ids.as_ref().map(|ids| selection(ids));
        let in_source: Option<BTreeSet<i64>> = case["in_source"].as_array().map(|items| {
            items
                .iter()
                .map(|item| names[item.as_str().expect("a name")])
                .collect()
        });
        let ask = Ask {
            mode,
            selection: chosen.as_ref(),
            source_entities: in_source.as_ref(),
        };
        let mut surface = SurfaceInput::new(text);
        if let Some(type_id) = case["type"].as_str() {
            surface = surface.of_type(EntityTypeId::new(type_id).expect("a type"));
        }
        let answer = run(&world, &ask, &surface);

        let outcome = answer.outcome_code();
        let reason = match &answer {
            Resolution::PossibleMatch { reason, .. } => reason.as_code(),
            Resolution::Unresolved { reason } => reason.as_code(),
            _ => "",
        };
        let mut who: Vec<String> = answer.entity_ids().into_iter().map(name_of).collect();
        who.sort();

        let mut wanted: Vec<String> = case["who"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|item| item.as_str().expect("a name").to_string())
            .collect();
        wanted.sort();

        let mut problems = Vec::new();
        if outcome != case["outcome"].as_str().expect("outcome") {
            problems.push(format!("answer {outcome}"));
        }
        if let Some(expected) = case["reason"].as_str() {
            if reason != expected {
                problems.push(format!("reason {reason:?}"));
            }
        }
        if who != wanted {
            problems.push(format!("points at {who:?}"));
        }
        println!(
            "{:<24} {:<9} {:<9} {:<15} {:<42} {:<17} {}",
            text,
            case["mode"].as_str().unwrap(),
            selection_ids
                .as_ref()
                .map(|ids| format!("{ids:?}"))
                .unwrap_or_else(|| "-".to_string()),
            outcome,
            who.join(" + "),
            reason,
            if problems.is_empty() { "yes" } else { "NO" }
        );
        if !problems.is_empty() {
            failures.push(format!(
                "{text:?} ({}): {}",
                case["mode"],
                problems.join(", ")
            ));
        }
    }
    println!();
    assert!(
        failures.is_empty(),
        "rows that do not hold:\n{}",
        failures.join("\n")
    );
}
