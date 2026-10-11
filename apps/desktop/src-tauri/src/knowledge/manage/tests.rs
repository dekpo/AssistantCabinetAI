//! The management operations, one by one, each followed by the integrity report. Every name here is
//! invented.

use std::time::Duration;

use super::*;
use crate::index_store::IndexStore;
use crate::knowledge::phonetic::FrenchPhonetic;
use crate::knowledge::{
    AliasDraft, ColumnSemanticsDraft, EntityDraft, EntityRef, IntegrityReport, KnowledgeDelta,
    MentionDraft, MentionLocator, SourceRef,
};

struct World {
    _directory: tempfile::TempDir,
    path: std::path::PathBuf,
    index: IndexStore,
    tools: NameTools,
}

impl World {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("temp folder");
        let path = directory.path().join("index.sqlite3");
        let index = IndexStore::open_at(&path).expect("opens");
        let titles = TitleSet::new().with_titles(["dr", "docteur", "m", "mme"]);
        Self {
            _directory: directory,
            path,
            index,
            tools: NameTools::new(titles, Box::new(FrenchPhonetic), BTreeSet::new()),
        }
    }

    fn conn(&self) -> &Connection {
        self.index.connection()
    }

    /// A document with one chunk of `text` and one active entity per `(type, name)`, each with an
    /// alias and a mention in that chunk, written the way a pass writes them.
    fn seed(&mut self, path: &str, text: &str, names: &[(&str, &str)]) {
        let sha = format!("sha-{path}");
        let chunk_id = format!("{path}#p1#s1");
        self.conn()
            .execute(
                "INSERT OR REPLACE INTO documents (relative_path, sha256, empty) VALUES (?1, ?2, 0)",
                params![path, sha],
            )
            .unwrap();
        self.conn()
            .execute(
                "INSERT OR REPLACE INTO chunks
                    (chunk_id, relative_path, page_number, section, text, embedding)
                 VALUES (?1, ?2, 1, 1, ?3, x'00')",
                params![chunk_id, path, text],
            )
            .unwrap();
        let mut delta = KnowledgeDelta::empty(SourceRef {
            domain: Domain::Documents,
            relative_path: path.to_string(),
            content_id: sha,
        });
        delta.kb_version = CURRENT_KB_VERSION;
        for (position, (type_id, name)) in names.iter().enumerate() {
            delta.entities.push(EntityDraft {
                temp_id: position as u32,
                type_id: type_id.to_string(),
                subtype: None,
                canonical_name: name.to_string(),
                normalized_name: name.to_lowercase(),
                disambiguator: String::new(),
                status: EntityStatus::Active,
            });
            delta.aliases.push(AliasDraft {
                entity: EntityRef::Draft(position as u32),
                display: name.to_string(),
                normalized: name.to_lowercase(),
                phonetic_key: String::new(),
                kind: AliasKind::CanonicalVariant,
                confidence: 1.0,
            });
            delta
                .mentions
                .push(mention(EntityRef::Draft(position as u32), &chunk_id));
        }
        let tx = self.index.transaction().unwrap();
        store::apply_delta(&tx, &delta).unwrap();
        tx.commit().unwrap();
    }

    fn id(&self, name: &str) -> i64 {
        self.conn()
            .query_row(
                "SELECT entity_id FROM kb_entities WHERE canonical_name = ?1",
                [name],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| panic!("no entity named {name}"))
    }

    fn status(&self, name: &str) -> String {
        self.conn()
            .query_row(
                "SELECT status FROM kb_entities WHERE canonical_name = ?1",
                [name],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn clean(&self) {
        assert_eq!(
            store::integrity_check(self.conn()).unwrap(),
            IntegrityReport::default()
        );
    }

    fn count(&self, sql: &str) -> i64 {
        self.conn().query_row(sql, [], |row| row.get(0)).unwrap()
    }

    fn mentions_of(&self, entity_id: i64) -> i64 {
        self.conn()
            .query_row(
                "SELECT COUNT(*) FROM kb_mentions WHERE entity_id = ?1",
                [entity_id],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn create(&self, name: &str, type_id: &str) -> i64 {
        create_entity(
            self.conn(),
            &self.tools,
            &NewManualEntity {
                name: name.to_string(),
                type_id: type_id.to_string(),
                subtype: None,
                disambiguator: None,
            },
        )
        .unwrap()
        .entity_id
        .unwrap()
    }
}

fn mention(entity: EntityRef, chunk_id: &str) -> MentionDraft {
    MentionDraft {
        entity,
        alias_normalized: None,
        role: None,
        locator: MentionLocator::Chunk {
            chunk_id: chunk_id.to_string(),
        },
        occurrence_count: 1,
        confidence: 0.9,
        method: "title_full_name".to_string(),
        extractor_version: 1,
    }
}

fn code<T>(result: Result<T, AppError>) -> String {
    match result {
        Ok(_) => "ok".to_string(),
        Err(error) => error.code().to_string(),
    }
}

// ------------------------------------------------------------------------------ create, edit

#[test]
fn a_created_entity_is_manual_active_and_answers_to_its_name() {
    let world = World::new();
    let outcome = create_entity(
        world.conn(),
        &world.tools,
        &NewManualEntity {
            name: "  Jean   Dupont ".to_string(),
            type_id: "person".to_string(),
            subtype: None,
            disambiguator: None,
        },
    )
    .unwrap();
    let id = outcome.entity_id.unwrap();
    let entity = store::get_entity(world.conn(), id).unwrap().unwrap();
    assert_eq!(entity.canonical_name, "Jean Dupont");
    assert_eq!(entity.normalized_name, "jean dupont");
    assert_eq!(
        (entity.status, entity.origin),
        (EntityStatus::Active, Origin::Manual)
    );
    let aliases = store::entity_with_aliases(world.conn(), id)
        .unwrap()
        .unwrap()
        .aliases;
    assert!(aliases.iter().all(|alias| alias.origin == Origin::Manual));
    assert!(aliases
        .iter()
        .any(|alias| alias.normalized == "jean dupont"));
    assert_eq!(
        last_operation(world.conn()).unwrap().map(|line| line.kind),
        Some("create".to_string())
    );
    world.clean();
}

#[test]
fn creating_refuses_an_identifier_an_empty_name_a_title_alone_and_a_taken_name() {
    let world = World::new();
    let input = |name: &str, type_id: &str| NewManualEntity {
        name: name.to_string(),
        type_id: type_id.to_string(),
        subtype: None,
        disambiguator: None,
    };
    assert_eq!(
        code(create_entity(
            world.conn(),
            &world.tools,
            &input("X-1", "identifier")
        )),
        "knowledge_operation_refused"
    );
    assert_eq!(
        code(create_entity(
            world.conn(),
            &world.tools,
            &input("   ", "person")
        )),
        "knowledge_name_invalid"
    );
    assert_eq!(
        code(create_entity(
            world.conn(),
            &world.tools,
            &input("Dr", "person")
        )),
        "knowledge_name_invalid",
        "a title alone is not a name"
    );
    assert_eq!(
        code(create_entity(
            world.conn(),
            &world.tools,
            &input(&"x".repeat(MAX_TEXT_CHARS + 1), "term")
        )),
        "knowledge_name_invalid"
    );
    world.create("Jean Dupont", "person");
    assert_eq!(
        code(create_entity(
            world.conn(),
            &world.tools,
            &input("Docteur JEAN Dupont", "person")
        )),
        "knowledge_name_taken",
        "titles and capitals do not make another name"
    );
    // The same name for another type is another entity.
    world.create("Jean Dupont", "organization");
    world.clean();
}

#[test]
fn an_alias_is_added_once_and_only_a_manual_one_is_removed() {
    let mut world = World::new();
    world.seed("a.txt", "Jean Dupont", &[("person", "Jean Dupont")]);
    let id = world.id("Jean Dupont");
    let first = add_alias(world.conn(), &world.tools, id, "Docteur JD").unwrap();
    let alias_id = first.alias_id.unwrap();
    assert_eq!(first.shared_with, 0);
    let again = add_alias(world.conn(), &world.tools, id, "docteur jd").unwrap();
    assert_eq!(
        again.alias_id,
        Some(alias_id),
        "the same folded name is one alias"
    );

    // A name another entity already answers to is reported as shared.
    world.seed("b.txt", "Jacques Dupont", &[("person", "Jacques Dupont")]);
    let other = world.id("Jacques Dupont");
    let shared = add_alias(world.conn(), &world.tools, other, "JD").unwrap();
    assert_eq!(shared.shared_with, 1, "Jean Dupont already answers to jd");

    // An automatic alias is not hers to edit.
    let automatic: i64 =
        world.count("SELECT alias_id FROM kb_aliases WHERE normalized = 'jean dupont'");
    assert_eq!(
        code(remove_alias(world.conn(), automatic)),
        "knowledge_operation_refused"
    );
    remove_alias(world.conn(), alias_id).unwrap();
    assert!(!store::entity_with_aliases(world.conn(), id)
        .unwrap()
        .unwrap()
        .aliases
        .iter()
        .any(|alias| alias.normalized == "jd"));
    assert_eq!(
        code(remove_alias(world.conn(), alias_id)),
        "knowledge_entity_not_found"
    );
    world.clean();
}

#[test]
fn an_alias_on_an_identifier_or_a_deleted_entity_is_refused() {
    let mut world = World::new();
    world.seed(
        "a.txt",
        "x",
        &[("identifier", "inv-1"), ("person", "Jean Dupont")],
    );
    assert_eq!(
        code(add_alias(
            world.conn(),
            &world.tools,
            world.id("inv-1"),
            "Facture"
        )),
        "knowledge_operation_refused"
    );
    let id = world.id("Jean Dupont");
    delete_entity(world.conn(), id).unwrap();
    assert_eq!(
        code(add_alias(world.conn(), &world.tools, id, "JD")),
        "knowledge_operation_refused"
    );
    assert_eq!(
        code(add_alias(world.conn(), &world.tools, 999_999, "JD")),
        "knowledge_entity_not_found"
    );
    world.clean();
}

#[test]
fn editing_an_automatic_entity_makes_it_manual_and_keeps_the_old_name() {
    let mut world = World::new();
    world.seed("a.txt", "Jean Dupond", &[("person", "Jean Dupond")]);
    let id = world.id("Jean Dupond");
    edit_entity(
        world.conn(),
        &world.tools,
        id,
        &EditEntity {
            name: Some("Jean Dupont".to_string()),
            ..EditEntity::default()
        },
    )
    .unwrap();
    let entity = store::get_entity(world.conn(), id).unwrap().unwrap();
    assert_eq!(entity.canonical_name, "Jean Dupont");
    assert_eq!(entity.normalized_name, "jean dupont");
    assert_eq!(
        entity.origin,
        Origin::Manual,
        "she has spoken: it survives a re-analysis"
    );
    let aliases = store::entity_with_aliases(world.conn(), id)
        .unwrap()
        .unwrap()
        .aliases;
    assert!(aliases
        .iter()
        .any(|alias| alias.normalized == "jean dupond"));
    assert!(aliases
        .iter()
        .any(|alias| alias.normalized == "jean dupont"));
    assert_eq!(world.mentions_of(id), 1, "its mentions are kept");

    // A name another entity holds is refused; nothing changes.
    world.seed("b.txt", "Anne Roux", &[("person", "Anne Roux")]);
    let before = world.count("SELECT COUNT(*) FROM kb_ops_log");
    assert_eq!(
        code(edit_entity(
            world.conn(),
            &world.tools,
            id,
            &EditEntity {
                name: Some("anne roux".to_string()),
                ..EditEntity::default()
            },
        )),
        "knowledge_name_taken"
    );
    assert_eq!(world.count("SELECT COUNT(*) FROM kb_ops_log"), before);
    world.clean();
}

#[test]
fn editing_the_type_is_allowed_between_the_manual_types_only() {
    let mut world = World::new();
    world.seed(
        "a.txt",
        "x",
        &[("person", "Conseil de Ordre"), ("identifier", "inv-1")],
    );
    let id = world.id("Conseil de Ordre");
    edit_entity(
        world.conn(),
        &world.tools,
        id,
        &EditEntity {
            type_id: Some("organization".to_string()),
            ..EditEntity::default()
        },
    )
    .unwrap();
    assert_eq!(
        store::get_entity(world.conn(), id)
            .unwrap()
            .unwrap()
            .type_id,
        "organization"
    );
    for (entity, to) in [(id, "identifier"), (world.id("inv-1"), "term")] {
        assert_eq!(
            code(edit_entity(
                world.conn(),
                &world.tools,
                entity,
                &EditEntity {
                    type_id: Some(to.to_string()),
                    ..EditEntity::default()
                },
            )),
            "knowledge_operation_refused"
        );
    }
    world.clean();
}

#[test]
fn an_attribute_is_manual_and_a_personal_key_is_refused() {
    let mut world = World::new();
    world.seed("a.txt", "x", &[("person", "Jean Dupont")]);
    let id = world.id("Jean Dupont");
    let tools = NameTools::new(
        TitleSet::new(),
        Box::new(FrenchPhonetic),
        BTreeSet::from(["email".to_string()]),
    );
    set_attribute(world.conn(), &tools, id, "spoken_form", "docteur Dupont").unwrap();
    for key in ["email", "identifier:nir", "Not A Key"] {
        assert!(
            set_attribute(world.conn(), &tools, id, key, "v").is_err(),
            "{key}"
        );
    }
    let detail = entity_detail(world.conn(), id).unwrap();
    assert_eq!(
        detail.attributes,
        vec![AttributeView {
            key: "spoken_form".to_string(),
            value: "docteur Dupont".to_string(),
            origin: "manual".to_string()
        }]
    );
    remove_attribute(world.conn(), id, "spoken_form", "docteur Dupont").unwrap();
    assert!(entity_detail(world.conn(), id)
        .unwrap()
        .attributes
        .is_empty());
    assert_eq!(
        code(remove_attribute(
            world.conn(),
            id,
            "spoken_form",
            "docteur Dupont"
        )),
        "knowledge_operation_refused"
    );
    world.clean();
}

// ------------------------------------------------------------------------ merge and unmerge

#[test]
fn a_merge_is_a_redirect_that_can_be_undone_and_chains_stay_one_hop_long() {
    let mut world = World::new();
    world.seed(
        "a.txt",
        "x",
        &[
            ("person", "Jean Dupont"),
            ("person", "J. Dupont"),
            ("person", "Jeanne Dupont"),
        ],
    );
    let (a, b, c) = (
        world.id("Jean Dupont"),
        world.id("J. Dupont"),
        world.id("Jeanne Dupont"),
    );
    merge(world.conn(), a, b).unwrap();
    assert_eq!(world.status("J. Dupont"), "merged");
    world.clean();

    // A redirect leaves the absorbed entity's mentions where they were: the survivor shows them.
    assert_eq!(world.mentions_of(b), 1);
    assert_eq!(entity_detail(world.conn(), a).unwrap().mention_count, 2);
    assert_eq!(entity_detail(world.conn(), b).unwrap().effective_id, a);

    // Merging the survivor elsewhere re-points what was redirected to it.
    merge(world.conn(), c, a).unwrap();
    world.clean();
    let target: i64 = world.count(&format!(
        "SELECT merged_into FROM kb_entities WHERE entity_id = {b}"
    ));
    assert_eq!(target, c);

    // Refusals: the same entity, and a cycle (the absorbed one already stands for the survivor).
    assert_eq!(code(merge(world.conn(), c, c)), "knowledge_merge_refused");
    assert_eq!(code(merge(world.conn(), b, a)), "knowledge_merge_refused");
    world.clean();

    unmerge(world.conn(), a).unwrap();
    assert_eq!(world.status("Jean Dupont"), "active");
    world.clean();
}

#[test]
fn two_types_are_never_merged() {
    let mut world = World::new();
    world.seed(
        "a.txt",
        "x",
        &[("person", "Dupont"), ("organization", "Dupont SARL")],
    );
    assert_eq!(
        code(merge(
            world.conn(),
            world.id("Dupont"),
            world.id("Dupont SARL")
        )),
        "knowledge_merge_refused"
    );
    world.clean();
}

// -------------------------------------------------------------------------- delete, restore

#[test]
fn delete_is_a_tombstone_and_restore_brings_the_entity_back() {
    let mut world = World::new();
    world.seed("a.txt", "Cabinet Test", &[("person", "Cabinet Test")]);
    let id = world.id("Cabinet Test");
    delete_entity(world.conn(), id).unwrap();
    assert_eq!(world.status("Cabinet Test"), "deleted");
    assert_eq!(world.mentions_of(id), 0);
    let page = list_entities(world.conn(), &ListQuery::default()).unwrap();
    assert_eq!(page.total, 0, "a tombstone leaves the live list");
    let deleted = list_entities(
        world.conn(),
        &ListQuery {
            view: ListView::Deleted,
            ..ListQuery::default()
        },
    )
    .unwrap();
    assert_eq!(deleted.rows.len(), 1);
    restore_entity(world.conn(), id).unwrap();
    assert_eq!(world.status("Cabinet Test"), "active");
    world.clean();
}

// -------------------------------------------------------------------------------------- split

fn two_people_one_name(world: &mut World) -> (i64, Vec<i64>) {
    world.seed(
        "a.txt",
        "Jean Dupont habite Lyon",
        &[("person", "Jean Dupont")],
    );
    world.seed(
        "b.txt",
        "Jean Dupont habite Nice",
        &[("person", "Jean Dupont")],
    );
    let id = world.id("Jean Dupont");
    let mentions: Vec<i64> = {
        let mut statement = world
            .conn()
            .prepare(
                "SELECT m.mention_id FROM kb_mentions m
                 JOIN kb_sources s ON s.source_id = m.source_id
                 WHERE m.entity_id = ?1 ORDER BY s.relative_path",
            )
            .unwrap();
        let rows = statement.query_map([id], |row| row.get(0)).unwrap();
        rows.collect::<Result<_, _>>().unwrap()
    };
    assert_eq!(mentions.len(), 2);
    (id, mentions)
}

fn split_request(id: i64, mentions: Vec<i64>, name: &str) -> SplitRequest {
    SplitRequest {
        entity_id: id,
        mention_ids: mentions,
        source_ids: vec![],
        name: name.to_string(),
    }
}

#[test]
fn a_split_moves_the_chosen_mentions_and_notes_where_they_came_from() {
    let mut world = World::new();
    let (id, mentions) = two_people_one_name(&mut world);
    let outcome = split(
        world.conn(),
        &world.tools,
        &split_request(id, vec![mentions[1]], "Jean Dupont 2"),
    )
    .unwrap();
    let new_id = outcome.entity_id.unwrap();
    assert_ne!(new_id, id);
    assert_eq!(world.mentions_of(id), 1);
    assert_eq!(world.mentions_of(new_id), 1);
    let (origin, from): (String, i64) = world
        .conn()
        .query_row(
            "SELECT origin, split_from FROM kb_mentions WHERE mention_id = ?1",
            [mentions[1]],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((origin.as_str(), from), ("manual", id));
    let entity = store::get_entity(world.conn(), new_id).unwrap().unwrap();
    assert_eq!(
        (entity.origin, entity.type_id.as_str()),
        (Origin::Manual, "person")
    );
    let detail = entity_detail(world.conn(), new_id).unwrap();
    assert_eq!(detail.sources[0].mentions[0].split_from, Some(id));
    world.clean();
}

#[test]
fn a_split_by_source_moves_every_mention_of_that_source() {
    let mut world = World::new();
    let (id, _) = two_people_one_name(&mut world);
    let source_id: i64 =
        world.count("SELECT source_id FROM kb_sources WHERE relative_path = 'b.txt'");
    let outcome = split(
        world.conn(),
        &world.tools,
        &SplitRequest {
            entity_id: id,
            mention_ids: vec![],
            source_ids: vec![source_id],
            name: "Jean Dupont de Nice".to_string(),
        },
    )
    .unwrap();
    assert_eq!(world.mentions_of(outcome.entity_id.unwrap()), 1);
    world.clean();
}

#[test]
fn a_split_that_leaves_nothing_or_reuses_a_name_is_refused() {
    let mut world = World::new();
    let (id, mentions) = two_people_one_name(&mut world);
    assert_eq!(
        code(split(
            world.conn(),
            &world.tools,
            &split_request(id, mentions.clone(), "Autre")
        )),
        "knowledge_operation_refused",
        "all of them"
    );
    assert_eq!(
        code(split(
            world.conn(),
            &world.tools,
            &split_request(id, vec![], "Autre")
        )),
        "knowledge_operation_refused",
        "none of them"
    );
    assert_eq!(
        code(split(
            world.conn(),
            &world.tools,
            &split_request(id, vec![999_999], "Autre")
        )),
        "knowledge_operation_refused",
        "a mention that is not this entity's"
    );
    assert_eq!(
        code(split(
            world.conn(),
            &world.tools,
            &split_request(id, vec![mentions[0]], "JEAN dupont")
        )),
        "knowledge_name_taken",
        "two entities with one name are one ambiguity for the scanner"
    );
    assert_eq!(
        code(split(
            world.conn(),
            &world.tools,
            &split_request(id, vec![mentions[0]], "  ")
        )),
        "knowledge_name_invalid"
    );
    assert_eq!(world.mentions_of(id), 2, "nothing moved");
    world.clean();
}

#[test]
fn a_reanalysis_does_not_put_a_split_mention_back() {
    let mut world = World::new();
    let (id, mentions) = two_people_one_name(&mut world);
    let new_id = split(
        world.conn(),
        &world.tools,
        &split_request(id, vec![mentions[1]], "Jean Dupont 2"),
    )
    .unwrap()
    .entity_id
    .unwrap();

    // The same document is read again and still finds the name Jean Dupont in the same chunk.
    world.seed(
        "b.txt",
        "Jean Dupont habite Nice",
        &[("person", "Jean Dupont")],
    );
    assert_eq!(
        world.mentions_of(id),
        1,
        "the original gets no mention back at b.txt"
    );
    assert_eq!(
        world.mentions_of(new_id),
        1,
        "the moved mention is still hers"
    );

    // Another place of the same file is not affected.
    let tx = world.index.transaction().unwrap();
    let mut delta = KnowledgeDelta::empty(SourceRef {
        domain: Domain::Documents,
        relative_path: "b.txt".to_string(),
        content_id: "sha-b.txt".to_string(),
    });
    delta.kb_version = CURRENT_KB_VERSION;
    delta
        .mentions
        .push(mention(EntityRef::Existing(id), "b.txt#p1#s1"));
    delta
        .mentions
        .push(mention(EntityRef::Existing(id), "b.txt#p2#s1"));
    store::apply_delta(&tx, &delta).unwrap();
    tx.commit().unwrap();
    assert_eq!(
        world.mentions_of(id),
        2,
        "page 2 is a new place, page 1 stays moved"
    );
    assert_eq!(world.mentions_of(new_id), 1);
    world.clean();
}

#[test]
fn undoing_a_split_puts_every_mention_back_and_removes_the_new_entity() {
    let mut world = World::new();
    let (id, mentions) = two_people_one_name(&mut world);
    let before = world.count("SELECT COUNT(*) FROM kb_mentions");
    let new_id = split(
        world.conn(),
        &world.tools,
        &split_request(id, vec![mentions[1]], "Jean Dupont 2"),
    )
    .unwrap()
    .entity_id
    .unwrap();
    undo_last_operation(world.conn()).unwrap();
    assert_eq!(world.mentions_of(id), 2);
    assert!(store::get_entity(world.conn(), new_id).unwrap().is_none());
    assert_eq!(world.count("SELECT COUNT(*) FROM kb_mentions"), before);
    let (origin, from): (String, Option<i64>) = world
        .conn()
        .query_row(
            "SELECT origin, split_from FROM kb_mentions WHERE mention_id = ?1",
            [mentions[1]],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((origin.as_str(), from), ("automatic", None));
    world.clean();
}

// -------------------------------------------------------------------------------------- undo

#[test]
fn undo_walks_back_one_operation_at_a_time_and_never_toggles() {
    let mut world = World::new();
    world.seed(
        "a.txt",
        "x",
        &[("person", "Jean Dupont"), ("person", "J. Dupont")],
    );
    let (a, b) = (world.id("Jean Dupont"), world.id("J. Dupont"));
    let created = world.create("Anne Roux", "person");
    merge(world.conn(), a, b).unwrap();
    delete_entity(world.conn(), created).unwrap();

    // Newest first: the deletion, then the merge, then the creation.
    undo_last_operation(world.conn()).unwrap();
    assert_eq!(world.status("Anne Roux"), "active");
    assert_eq!(
        last_operation(world.conn()).unwrap().map(|line| line.kind),
        Some("merge".to_string()),
        "the restore made by the undo is not the next thing to undo"
    );
    undo_last_operation(world.conn()).unwrap();
    assert_eq!(world.status("J. Dupont"), "active");
    undo_last_operation(world.conn()).unwrap();
    assert!(store::get_entity(world.conn(), created).unwrap().is_none());
    world.clean();

    // Nothing left that can be undone.
    assert_eq!(
        code(undo_last_operation(world.conn())),
        "knowledge_nothing_to_undo"
    );
    assert_eq!(
        world.count("SELECT COUNT(*) FROM kb_entities WHERE origin = 'manual'"),
        0
    );
}

#[test]
fn undoing_an_unmerge_merges_again() {
    let mut world = World::new();
    world.seed(
        "a.txt",
        "x",
        &[("person", "Jean Dupont"), ("person", "J. Dupont")],
    );
    let (a, b) = (world.id("Jean Dupont"), world.id("J. Dupont"));
    merge(world.conn(), a, b).unwrap();
    unmerge(world.conn(), b).unwrap();
    undo_last_operation(world.conn()).unwrap();
    assert_eq!(world.status("J. Dupont"), "merged");
    world.clean();
}

#[test]
fn an_operation_that_cannot_be_undone_stops_the_stack() {
    let mut world = World::new();
    world.seed("a.txt", "x", &[("person", "Jean Dupont")]);
    let id = world.id("Jean Dupont");
    let alias = add_alias(world.conn(), &world.tools, id, "JD")
        .unwrap()
        .alias_id
        .unwrap();
    remove_alias(world.conn(), alias).unwrap();
    let last = last_operation(world.conn()).unwrap().unwrap();
    assert_eq!((last.kind.as_str(), last.undoable), ("remove_alias", false));
    assert_eq!(
        code(undo_last_operation(world.conn())),
        "knowledge_nothing_to_undo",
        "the log holds ids, not the text she typed: removing an alias is confirmed, not undone"
    );
    world.clean();
}

#[test]
fn an_undo_the_base_has_moved_on_from_is_refused_and_changes_nothing() {
    let mut world = World::new();
    let created = world.create("Anne Roux", "person");
    // A pass finds her in a document: the entity now holds a mention.
    world.seed("a.txt", "Anne Roux", &[("person", "Anne Roux")]);
    assert_eq!(world.mentions_of(created), 1);
    let before = world.count("SELECT COUNT(*) FROM kb_ops_log");
    assert_eq!(
        code(undo_last_operation(world.conn())),
        "knowledge_operation_refused"
    );
    assert_eq!(world.count("SELECT COUNT(*) FROM kb_ops_log"), before);
    assert_eq!(world.status("Anne Roux"), "active");
    world.clean();
}

// -------------------------------------------------------------------------- possible matches

#[test]
fn accepting_a_pair_merges_it_and_a_dismissed_pair_is_never_offered_again() {
    let mut world = World::new();
    world.seed(
        "a.txt",
        "x",
        &[
            ("person", "Jean Dupont"),
            ("person", "Dupond"),
            ("person", "Marie Roux"),
            ("person", "Marie Rou"),
        ],
    );
    let (a, b, c, d) = (
        world.id("Jean Dupont"),
        world.id("Dupond"),
        world.id("Marie Roux"),
        world.id("Marie Rou"),
    );
    assert!(store::record_possible_match(world.conn(), a, b, "same_sound", 0.8).unwrap());
    assert!(store::record_possible_match(world.conn(), c, d, "same_sound", 0.7).unwrap());
    assert_eq!(possible_matches(world.conn(), 0).unwrap().total, 2);

    accept_possible_match(world.conn(), a, b).unwrap();
    assert_eq!(world.status("Dupond"), "merged");
    let state: String = world
        .conn()
        .query_row(
            "SELECT state FROM kb_possible_matches WHERE entity_a = ?1",
            [a.min(b)],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(state, "merged");

    dismiss_possible_match(world.conn(), d, c).unwrap();
    assert_eq!(possible_matches(world.conn(), 0).unwrap().total, 0);
    assert!(
        !store::record_possible_match(world.conn(), c, d, "same_sound", 0.7).unwrap(),
        "a pair she dismissed is not written again by a later pass"
    );
    assert_eq!(
        code(dismiss_possible_match(world.conn(), c, d)),
        "knowledge_operation_refused"
    );
    assert_eq!(
        code(accept_possible_match(world.conn(), c, d)),
        "knowledge_operation_refused"
    );

    // Undoing the dismissal offers the pair again; undoing the merge reopens the other one.
    undo_last_operation(world.conn()).unwrap();
    assert_eq!(possible_matches(world.conn(), 0).unwrap().total, 1);
    undo_last_operation(world.conn()).unwrap();
    assert_eq!(possible_matches(world.conn(), 0).unwrap().total, 2);
    world.clean();
}

#[test]
fn a_pair_of_different_types_cannot_be_accepted_and_stays_open() {
    let mut world = World::new();
    world.seed(
        "a.txt",
        "x",
        &[("person", "Dupont"), ("organization", "Dupont SARL")],
    );
    let (a, b) = (world.id("Dupont"), world.id("Dupont SARL"));
    store::record_possible_match(world.conn(), a, b, "same_sound", 0.6).unwrap();
    assert_eq!(
        code(accept_possible_match(world.conn(), a, b)),
        "knowledge_merge_refused"
    );
    assert_eq!(possible_matches(world.conn(), 0).unwrap().total, 1);
    world.clean();
}

// ---------------------------------------------------------------------------------- the lists

#[test]
fn the_list_pages_filters_and_searches() {
    let mut world = World::new();
    let names: Vec<(&str, String)> = (0..60)
        .map(|number| ("person", format!("Personne {number:02}")))
        .collect();
    let borrowed: Vec<(&str, &str)> = names.iter().map(|(t, n)| (*t, n.as_str())).collect();
    world.seed("a.txt", "x", &borrowed);
    world.seed(
        "b.txt",
        "x",
        &[
            ("organization", "Mairie de Lyon"),
            ("person", "Personne 03"),
        ],
    );

    let first = list_entities(world.conn(), &ListQuery::default()).unwrap();
    assert_eq!(
        (first.total, first.rows.len(), first.page_size),
        (61, PAGE_SIZE, PAGE_SIZE)
    );
    let second = list_entities(
        world.conn(),
        &ListQuery {
            page: 1,
            ..ListQuery::default()
        },
    )
    .unwrap();
    assert_eq!(second.rows.len(), 11);
    assert_eq!(
        first.rows[0].name, "Mairie de Lyon",
        "ordered by folded name"
    );

    let by_type = list_entities(
        world.conn(),
        &ListQuery {
            type_id: Some("organization".into()),
            ..ListQuery::default()
        },
    )
    .unwrap();
    assert_eq!(by_type.total, 1);

    // The search starts a word with whatever she typed, in any case or accent, and finds an alias.
    let query = |text: &str| ListQuery {
        text: text.to_string(),
        ..ListQuery::default()
    };
    let by_text = list_entities(world.conn(), &query("MAIR")).unwrap();
    assert_eq!(
        by_text
            .rows
            .iter()
            .map(|row| row.name.as_str())
            .collect::<Vec<_>>(),
        ["Mairie de Lyon"]
    );
    let id = world.id("Mairie de Lyon");
    add_alias(world.conn(), &world.tools, id, "Hotel de ville").unwrap();
    assert_eq!(
        list_entities(world.conn(), &query("h\u{f4}tel de v"))
            .unwrap()
            .total,
        1
    );
    assert!(
        list_entities(world.conn(), &query("sonne 0"))
            .unwrap()
            .total
            >= 10,
        "the folded name is also searched inside"
    );
    assert_eq!(
        list_entities(world.conn(), &query("%")).unwrap().total,
        0,
        "a percent sign is not a wildcard"
    );

    // Problems: an entity seen once. Personne 03 is named in two files.
    let problems = list_entities(
        world.conn(),
        &ListQuery {
            problems_only: true,
            ..ListQuery::default()
        },
    )
    .unwrap();
    assert_eq!(problems.total, 60, "all but the one named in two files");
    let manual = list_entities(
        world.conn(),
        &ListQuery {
            origin: Some("manual".into()),
            ..ListQuery::default()
        },
    )
    .unwrap();
    assert_eq!(manual.total, 0);
    assert_eq!(
        code(list_entities(
            world.conn(),
            &ListQuery {
                origin: Some("robot".into()),
                ..ListQuery::default()
            }
        )),
        "knowledge_operation_refused"
    );
    world.clean();
}

#[test]
fn the_sheet_shows_where_each_association_comes_from() {
    let mut world = World::new();
    let text = "Report of 12 March. Doctor Jean Dupont recommends a check.";
    world.seed("rapports/mars.txt", text, &[("person", "Jean Dupont")]);
    let id = world.id("Jean Dupont");
    // A table mention.
    let tx = world.index.transaction().unwrap();
    let mut delta = KnowledgeDelta::empty(SourceRef {
        domain: Domain::Data,
        relative_path: "rdv.xlsx".to_string(),
        content_id: "wb-1".to_string(),
    });
    delta.kb_version = CURRENT_KB_VERSION;
    delta.mentions.push(MentionDraft {
        entity: EntityRef::Existing(id),
        alias_normalized: None,
        role: Some("provider".to_string()),
        locator: MentionLocator::Cells {
            sheet: "Mars".to_string(),
            column_name: "Medecin".to_string(),
            first_row: Some(2),
            sample_rows: vec![2, 5, 9, 11],
        },
        occurrence_count: 4,
        confidence: 0.95,
        method: "column_value".to_string(),
        extractor_version: 1,
    });
    delta.column_semantics.push(ColumnSemanticsDraft {
        sheet: "Mars".to_string(),
        column_name: "Medecin".to_string(),
        semantic_type: "person".to_string(),
        subtype: None,
        role: Some("provider".to_string()),
        confidence: 0.9,
        reason: "header_exact".to_string(),
    });
    store::apply_delta(&tx, &delta).unwrap();
    tx.commit().unwrap();

    let detail = entity_detail(world.conn(), id).unwrap();
    assert_eq!(detail.mention_count, 2);
    assert_eq!(detail.sources.len(), 2);
    let documents = detail
        .sources
        .iter()
        .find(|source| source.domain == "documents")
        .unwrap();
    assert_eq!(documents.relative_path, "rapports/mars.txt");
    let chunk = &documents.mentions[0];
    assert_eq!(
        (
            chunk.locator.kind.as_str(),
            chunk.locator.page,
            chunk.locator.section
        ),
        ("chunk", Some(1), Some(1))
    );
    assert_eq!(
        (chunk.method.as_str(), chunk.origin.as_str()),
        ("title_full_name", "automatic")
    );
    let excerpt = chunk.excerpt.as_ref().expect("a passage");
    assert!(excerpt.text.contains("Jean Dupont"), "{excerpt:?}");
    let table = detail
        .sources
        .iter()
        .find(|source| source.domain == "data")
        .unwrap();
    let cells = &table.mentions[0];
    assert_eq!(cells.locator.sheet.as_deref(), Some("Mars"));
    assert_eq!(cells.locator.column.as_deref(), Some("Medecin"));
    assert_eq!(
        cells.locator.sample_rows,
        vec![2, 5, 9],
        "three rows are kept"
    );
    assert_eq!(
        (cells.role.as_deref(), cells.occurrences),
        (Some("provider"), 4)
    );
    assert!(cells.excerpt.is_none());
    assert_eq!(detail.columns[0].relative_path, "rdv.xlsx");
    assert_eq!(detail.columns[0].columns[0].reason, "header_exact");
    assert!(detail.columns[0].columns[0].read);
    world.clean();
}

#[test]
fn an_unknown_entity_has_no_sheet() {
    let world = World::new();
    assert_eq!(
        code(entity_detail(world.conn(), 12_345)),
        "knowledge_entity_not_found"
    );
}

#[test]
fn page_and_section_are_read_from_the_end_of_the_chunk_id() {
    assert_eq!(page_and_section("a/b.pdf#p3#s2"), (Some(3), Some(2)));
    assert_eq!(page_and_section("a#b/c.pdf#p12#s1"), (Some(12), Some(1)));
    assert_eq!(page_and_section("no-markers"), (None, None));
}

#[test]
fn an_excerpt_is_a_window_around_the_name_and_says_where_it_was_cut() {
    let long = format!("{} Jean Dupont {}", "mot ".repeat(100), "fin ".repeat(100));
    let excerpt = excerpt_of(&long, &["jean dupont".to_string()]);
    assert!(excerpt.text.contains("Jean Dupont"));
    assert!(excerpt.text.chars().count() <= EXCERPT_CHARS);
    assert!(excerpt.cut_before && excerpt.cut_after);
    let short = excerpt_of("Jean\nDupont", &["jean dupont".to_string()]);
    assert_eq!(short.text, "Jean Dupont");
    assert!(!short.cut_before && !short.cut_after);
    assert_eq!(excerpt_of("", &["x".to_string()]).text, "");
}

#[test]
fn the_health_report_counts_and_lists_ids() {
    let mut world = World::new();
    world.seed(
        "a.txt",
        "x",
        &[("person", "Jean Dupont"), ("organization", "Mairie")],
    );
    world.seed("b.txt", "x", &[("person", "Jean Dupont")]);
    // A document read with nothing found in it, and a workbook with no usable column.
    world
        .conn()
        .execute(
            "INSERT INTO documents (relative_path, sha256, empty) VALUES ('vide.txt', 'sha-v', 0)",
            [],
        )
        .unwrap();
    let tx = world.index.transaction().unwrap();
    let mut empty = KnowledgeDelta::empty(SourceRef {
        domain: Domain::Documents,
        relative_path: "vide.txt".into(),
        content_id: "sha-v".into(),
    });
    empty.kb_version = CURRENT_KB_VERSION;
    store::apply_delta(&tx, &empty).unwrap();
    let mut sheet = KnowledgeDelta::empty(SourceRef {
        domain: Domain::Data,
        relative_path: "nombres.csv".into(),
        content_id: "wb".into(),
    });
    sheet.kb_version = CURRENT_KB_VERSION;
    store::apply_delta(&tx, &sheet).unwrap();
    tx.commit().unwrap();

    let report = health_report(world.conn()).unwrap();
    assert_eq!(
        report
            .seen_once
            .items
            .iter()
            .map(|brief| brief.name.as_str())
            .collect::<Vec<_>>(),
        ["Mairie"]
    );
    assert_eq!(
        report
            .documents_without_entity
            .items
            .iter()
            .map(|source| source.relative_path.as_str())
            .collect::<Vec<_>>(),
        ["vide.txt"]
    );
    assert_eq!(report.workbooks_without_usable_column.total, 1);
    let names: BTreeSet<_> = report
        .in_documents_not_in_tables
        .items
        .iter()
        .map(|brief| brief.name.clone())
        .collect();
    assert_eq!(
        names,
        BTreeSet::from(["Jean Dupont".to_string(), "Mairie".to_string()])
    );
    assert_eq!((report.candidates, report.probable_duplicates), (0, 0));
    world.clean();
}

#[test]
fn the_overview_counts_what_is_there() {
    let mut world = World::new();
    world.seed("a.txt", "x", &[("person", "Jean Dupont")]);
    world.create("Anne Roux", "person");
    let overview = overview(world.conn()).unwrap();
    assert_eq!(
        (
            overview.entities_live,
            overview.entities_manual,
            overview.documents_read
        ),
        (2, 1, 1)
    );
    assert!(overview.consistent);
    assert_eq!(
        overview.documents_due, 1,
        "adding a name makes the read document due"
    );
    assert_eq!(
        overview.last_operation.map(|line| line.kind),
        Some("create".to_string())
    );
}

// ------------------------------------------------------------------------------------- reset

#[test]
fn a_reset_without_manual_keeps_what_she_wrote_and_what_she_deleted() {
    let mut world = World::new();
    world.seed(
        "a.txt",
        "x",
        &[("person", "Jean Dupont"), ("person", "Faux Nom")],
    );
    let keep = world.create("Anne Roux", "person");
    delete_entity(world.conn(), world.id("Faux Nom")).unwrap();
    let outcome = reset(world.conn(), false).unwrap();
    assert_eq!(world.count("SELECT COUNT(*) FROM kb_sources"), 0);
    assert_eq!(world.count("SELECT COUNT(*) FROM kb_mentions"), 0);
    assert!(
        store::get_entity(world.conn(), keep).unwrap().is_some(),
        "manual entity kept"
    );
    assert_eq!(world.status("Faux Nom"), "deleted", "the tombstone is kept");
    assert_eq!(
        world.count("SELECT COUNT(*) FROM kb_entities WHERE canonical_name = 'Jean Dupont'"),
        0
    );
    assert_eq!(outcome.documents_due, 1, "the document is due again");
    // The reset is a barrier: what came before cannot be undone.
    assert_eq!(
        code(undo_last_operation(world.conn())),
        "knowledge_nothing_to_undo"
    );
    world.clean();
}

#[test]
fn a_reset_with_manual_empties_the_base_and_the_log() {
    let mut world = World::new();
    world.seed("a.txt", "x", &[("person", "Jean Dupont")]);
    world.create("Anne Roux", "person");
    delete_entity(world.conn(), world.id("Jean Dupont")).unwrap();
    reset(world.conn(), true).unwrap();
    for table in [
        "kb_entities",
        "kb_aliases",
        "kb_mentions",
        "kb_sources",
        "kb_ops_log",
        "kb_attributes",
    ] {
        assert_eq!(
            world.count(&format!("SELECT COUNT(*) FROM {table}")),
            0,
            "{table}"
        );
    }
    assert_eq!(
        world.count("SELECT COUNT(*) FROM kb_entity_types"),
        6,
        "the seeds stay"
    );
    world.clean();
}

// ------------------------------------------------------------------------------- invariants

#[test]
fn no_operation_touches_a_document_a_chunk_or_a_table_row() {
    let mut world = World::new();
    world.seed(
        "a.txt",
        "Jean Dupont et J. Dupont",
        &[("person", "Jean Dupont"), ("person", "J. Dupont")],
    );
    let snapshot = |world: &World| -> Vec<String> {
        let mut rows = Vec::new();
        for sql in [
            "SELECT relative_path || '|' || sha256 || '|' || empty FROM documents",
            "SELECT chunk_id || '|' || text || '|' || length(embedding) FROM chunks",
            "SELECT chunk_id || '|' || text FROM chunks_fts",
            "SELECT relative_path || '|' || inventory_json FROM tabular_inventories",
            "SELECT workbook_id || '|' || workbook_json FROM tabular_workbooks",
        ] {
            let mut statement = world.conn().prepare(sql).unwrap();
            let found: Vec<String> = statement
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            rows.push(format!("{sql} -> {found:?}"));
        }
        rows
    };
    let before = snapshot(&world);
    let (a, b) = (world.id("Jean Dupont"), world.id("J. Dupont"));
    let created = world.create("Anne Roux", "person");
    add_alias(world.conn(), &world.tools, a, "JD").unwrap();
    merge(world.conn(), a, b).unwrap();
    unmerge(world.conn(), b).unwrap();
    delete_entity(world.conn(), created).unwrap();
    restore_entity(world.conn(), created).unwrap();
    undo_last_operation(world.conn()).unwrap();
    reset(world.conn(), false).unwrap();
    reset(world.conn(), true).unwrap();
    assert_eq!(snapshot(&world), before);
}

#[test]
fn a_busy_database_gives_the_clean_code_and_changes_nothing() {
    let mut world = World::new();
    world.seed("a.txt", "x", &[("person", "Jean Dupont")]);
    world
        .conn()
        .busy_timeout(Duration::from_millis(50))
        .unwrap();
    let id = world.id("Jean Dupont");
    let other = Connection::open(&world.path).unwrap();
    other.execute_batch("BEGIN IMMEDIATE").unwrap();

    let before = world.count("SELECT COUNT(*) FROM kb_ops_log");
    let tools = &world.tools;
    let refused = [
        code(delete_entity(world.conn(), id)),
        code(add_alias(world.conn(), tools, id, "JD")),
        code(create_entity(
            world.conn(),
            tools,
            &NewManualEntity {
                name: "X".into(),
                type_id: "term".into(),
                subtype: None,
                disambiguator: None,
            },
        )),
        code(undo_last_operation(world.conn())),
        code(reset(world.conn(), true)),
        code(merge(world.conn(), id, id + 1)),
    ];
    assert!(
        refused.iter().all(|code| code == "knowledge_unavailable"),
        "{refused:?}"
    );
    other.execute_batch("ROLLBACK").unwrap();

    assert_eq!(world.count("SELECT COUNT(*) FROM kb_ops_log"), before);
    assert_eq!(world.status("Jean Dupont"), "active");
    delete_entity(world.conn(), id).unwrap();
    world.clean();
}

// ------------------------------------------------------------------------ the bridge contract

#[test]
fn the_requests_the_interface_sends_are_read_as_written() {
    let query: ListQuery = serde_json::from_str(
        r#"{"text":"dup","typeId":null,"origin":"manual","view":"deleted","problemsOnly":true,"page":2}"#,
    )
    .unwrap();
    assert_eq!(
        (query.text.as_str(), query.type_id, query.origin.as_deref()),
        ("dup", None, Some("manual"))
    );
    assert_eq!(
        (query.view, query.problems_only, query.page),
        (ListView::Deleted, true, 2)
    );

    let input: NewManualEntity =
        serde_json::from_str(r#"{"name":"Anne","typeId":"person"}"#).unwrap();
    assert_eq!(
        (input.type_id.as_str(), input.subtype, input.disambiguator),
        ("person", None, None)
    );

    let edit: EditEntity =
        serde_json::from_str(r#"{"name":null,"typeId":"organization","disambiguator":""}"#)
            .unwrap();
    assert_eq!(
        (edit.name, edit.type_id.as_deref()),
        (None, Some("organization"))
    );

    let request: SplitRequest =
        serde_json::from_str(r#"{"entityId":4,"mentionIds":[1,2],"sourceIds":[],"name":"X 2"}"#)
            .unwrap();
    assert_eq!(
        (
            request.entity_id,
            request.mention_ids,
            request.name.as_str()
        ),
        (4, vec![1, 2], "X 2")
    );
}

#[test]
fn the_answers_the_interface_reads_use_the_names_it_expects() {
    let mut world = World::new();
    world.seed("a.txt", "Jean Dupont", &[("person", "Jean Dupont")]);
    let id = world.id("Jean Dupont");
    let outcome =
        serde_json::to_value(add_alias(world.conn(), &world.tools, id, "JD").unwrap()).unwrap();
    for key in ["entityId", "aliasId", "sharedWith", "documentsDue"] {
        assert!(outcome.get(key).is_some(), "Outcome.{key}");
    }
    let detail = serde_json::to_value(entity_detail(world.conn(), id).unwrap()).unwrap();
    for key in [
        "entity",
        "effectiveId",
        "survivor",
        "aliases",
        "mergedIn",
        "sources",
        "mentionCount",
        "relations",
        "attributes",
        "possibleMatches",
        "columns",
    ] {
        assert!(detail.get(key).is_some(), "EntityDetail.{key}");
    }
    let mention = &detail["sources"][0]["mentions"][0];
    for key in [
        "mentionId",
        "entityId",
        "locator",
        "excerpt",
        "method",
        "confidence",
        "origin",
        "occurrences",
        "splitFrom",
    ] {
        assert!(mention.get(key).is_some(), "MentionView.{key}");
    }
    for key in [
        "kind",
        "page",
        "section",
        "sheet",
        "column",
        "firstRow",
        "sampleRows",
    ] {
        assert!(mention["locator"].get(key).is_some(), "LocatorView.{key}");
    }
    let page =
        serde_json::to_value(list_entities(world.conn(), &ListQuery::default()).unwrap()).unwrap();
    for key in ["total", "page", "pageSize", "rows"] {
        assert!(page.get(key).is_some(), "EntityPage.{key}");
    }
    for key in [
        "entityId",
        "typeId",
        "subtype",
        "name",
        "disambiguator",
        "status",
        "origin",
        "mergedInto",
        "mentions",
        "sources",
        "aliases",
        "openMatches",
    ] {
        assert!(page["rows"][0].get(key).is_some(), "EntityRow.{key}");
    }
    let overview = serde_json::to_value(overview(world.conn()).unwrap()).unwrap();
    for key in [
        "entitiesLive",
        "entitiesCandidate",
        "entitiesManual",
        "documentsRead",
        "documentsDue",
        "consistent",
        "lastOperation",
    ] {
        assert!(overview.get(key).is_some(), "Overview.{key}");
    }
    assert_eq!(overview["lastOperation"]["kind"], "add_alias");
    let health = serde_json::to_value(health_report(world.conn()).unwrap()).unwrap();
    for key in [
        "seenOnce",
        "candidates",
        "documentsWithoutEntity",
        "workbooksWithoutUsableColumn",
        "probableDuplicates",
        "inDocumentsNotInTables",
    ] {
        assert!(health.get(key).is_some(), "HealthReport.{key}");
    }
    assert!(health["seenOnce"].get("total").is_some() && health["seenOnce"].get("items").is_some());
}

#[test]
fn a_merged_entity_names_its_survivor_on_its_sheet() {
    let mut world = World::new();
    world.seed(
        "a.txt",
        "x",
        &[("person", "Jean Dupont"), ("person", "J. Dupont")],
    );
    let (a, b) = (world.id("Jean Dupont"), world.id("J. Dupont"));
    assert!(entity_detail(world.conn(), a).unwrap().survivor.is_none());
    merge(world.conn(), a, b).unwrap();
    let sheet = entity_detail(world.conn(), b).unwrap();
    assert_eq!(
        sheet.survivor.map(|brief| brief.name),
        Some("Jean Dupont".to_string())
    );
    assert_eq!(sheet.effective_id, a);
}

#[test]
fn a_write_raises_the_generation_of_the_name_list_and_reports_the_documents_due() {
    let mut world = World::new();
    world.seed("a.txt", "x", &[("person", "Jean Dupont")]);
    assert_eq!(documents_due(world.conn()).unwrap(), 0);
    let epoch = |world: &World| current_epoch(world.conn()).unwrap();
    let before = epoch(&world);
    let outcome = add_alias(world.conn(), &world.tools, world.id("Jean Dupont"), "JD").unwrap();
    assert_eq!(epoch(&world), before + 1);
    assert_eq!(outcome.documents_due, 1);
    // An attribute adds no name to look for.
    set_attribute(
        world.conn(),
        &world.tools,
        world.id("Jean Dupont"),
        "spoken_form",
        "Jean",
    )
    .unwrap();
    assert_eq!(epoch(&world), before + 1);
}
