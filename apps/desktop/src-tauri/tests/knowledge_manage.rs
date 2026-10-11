//! Lot 8: the knowledge base's management operations, end to end.
//!
//! These tests drive the real Analyse over invented files in a temporary folder (the gateway is
//! faked), use the management operations through a second connection to the same index, and then
//! look at what the next Analyse does with the result. What only shows end to end: a name added by
//! hand is found in the next question and in the documents the next Analyse reads again, a name
//! deleted by hand does not come back, a split is not undone by a re-reading, the manual rows
//! outlive the two folder Resets, and nothing the dialog does touches a file or asks the AI for
//! anything.
//!
//! Every name here is invented. The documents are French because the pilot is; the code under test
//! holds no French word (the packs do).

mod common;

use assistant_cabinet_ai_lib::knowledge::manage::{
    self, EditEntity, ListQuery, ListView, NameTools, NewManualEntity, SplitRequest,
};
use assistant_cabinet_ai_lib::knowledge::packs::PackSet;
use assistant_cabinet_ai_lib::knowledge::store as kb;
use assistant_cabinet_ai_lib::knowledge::{IntegrityReport, KnowledgeMode};
use rusqlite::Connection;

use common::corpus::{clinic, doc, entity_id, plan_for, world};
use common::knowledge::{count, entity_named, mentions_of, snapshot, Lab};

fn tools(lab: &Lab) -> NameTools {
    let packs =
        PackSet::load_with_type_vocabulary(lab.locale, &lab.packs).expect("loads the packs");
    NameTools::from_packs(&packs, lab.locale)
}

fn clean(connection: &Connection) {
    assert_eq!(
        kb::integrity_check(connection).expect("checks"),
        IntegrityReport::default(),
        "the knowledge base must be consistent"
    );
}

/// The paths of the documents that mention an entity (mentions stored on it).
fn files_of(lab: &Lab, entity_id: i64) -> Vec<String> {
    let mut paths: Vec<String> = mentions_of(&lab.side(), entity_id)
        .into_iter()
        .map(|mention| mention.path)
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// The clinic of the question-time tests, plus a visit note that writes the doctor's nickname.
fn clinic_with_initials() -> Vec<(String, String)> {
    let mut files = clinic();
    files.push((
        "visite-jd.txt".to_string(),
        doc(
            4242,
            "mardi matin, Jeannot est passe pour un controle de routine.",
        ),
    ));
    files
}

// ------------------------------------------------------------------- a name added by hand

#[tokio::test]
async fn a_manual_alias_is_found_in_the_next_question_and_in_the_next_scan() {
    let mut lab = world(clinic_with_initials()).await;
    let jean = entity_id(&lab, "Jean Dupont");
    let initials_before = files_of(&lab, jean);
    assert!(
        !initials_before.contains(&"visite-jd.txt".to_string()),
        "nobody knows who Jeannot is yet: {initials_before:?}"
    );

    // The question side: before the alias nothing is found, after it Jean Dupont is.
    let question = "Que dit Jeannot du bilan ?";
    let before = plan_for(&lab, KnowledgeMode::Auto, question);
    assert!(
        before
            .analysis
            .as_ref()
            .is_none_or(|a| a.entities.is_empty()),
        "{:?}",
        before.analysis
    );
    let outcome =
        manage::add_alias(&lab.side(), &tools(&lab), jean, "Jeannot").expect("adds the alias");
    assert!(outcome.documents_due >= 1, "the documents are now due");
    let after = plan_for(&lab, KnowledgeMode::Auto, question);
    let analysis = after.analysis.as_ref().expect("reads the question");
    assert_eq!(analysis.entities.len(), 1);
    assert_eq!(analysis.entities[0].display, "Jean Dupont");

    // Nothing started by itself: the gateway saw no new request, the note is still not read again.
    let requests = lab.gateway.requests().len();
    assert_eq!(files_of(&lab, jean), initials_before);

    // The scan side: the Analyse the user starts reads the due documents again, from the stored
    // text, and finds the nickname.
    let summary = lab.analyse().await;
    assert!(
        summary
            .knowledge
            .as_ref()
            .is_some_and(|knowledge| knowledge.refreshed_sources >= 1),
        "{:?}",
        summary.knowledge
    );
    assert_eq!(
        lab.gateway.requests().len(),
        requests,
        "reading the documents again from their stored text asks the AI for nothing"
    );
    assert!(
        files_of(&lab, jean).contains(&"visite-jd.txt".to_string()),
        "{:?}",
        files_of(&lab, jean)
    );
    assert_eq!(manage::documents_due(&lab.side()).unwrap(), 0);
    clean(&lab.side());

    // A second Analyse over the same files changes nothing (I14).
    let settled = snapshot(&lab.side());
    lab.analyse().await;
    assert_eq!(snapshot(&lab.side()), settled);
}

#[tokio::test]
async fn a_manual_entity_is_found_in_the_documents_that_name_it() {
    let mut lab = world(clinic()).await;
    assert!(entity_named(&lab.side(), "Marc Lambert").is_none());
    lab.write(
        "courrier-lambert.txt",
        &doc(
            3131,
            "Courrier a Marc Lambert au sujet de la facture en attente.",
        ),
    );
    // Analysing now would already learn the name from the letter on its own (a first name and a
    // surname are a name), so the entity is created first and the letter written after the
    // creation: what is checked is that the by-hand entity, and not a new automatic one, is read.
    let created = manage::create_entity(
        &lab.side(),
        &tools(&lab),
        &NewManualEntity {
            name: "Marc Lambert".to_string(),
            type_id: "person".to_string(),
            subtype: None,
            disambiguator: None,
        },
    )
    .expect("creates the entity")
    .entity_id
    .expect("an id");
    lab.analyse().await;
    assert_eq!(
        entity_named(&lab.side(), "Marc Lambert").unwrap().entity_id,
        created,
        "the name resolves to the entity she typed, not to a second one"
    );
    assert_eq!(
        files_of(&lab, created),
        vec!["courrier-lambert.txt".to_string()]
    );
    clean(&lab.side());
}

// --------------------------------------------------------------------------------- deleting

#[tokio::test]
async fn a_deleted_false_positive_does_not_come_back_and_a_restored_one_does() {
    let mut lab = world(clinic()).await;
    let claire = entity_id(&lab, "Claire Martin");
    assert_eq!(
        files_of(&lab, claire),
        vec!["martin-controle.txt".to_string()]
    );

    let outcome = manage::delete_entity(&lab.side(), claire).expect("deletes");
    assert!(
        outcome.documents_due >= 1,
        "the documents are due: the next Analyse reads them"
    );
    assert_eq!(
        entity_named(&lab.side(), "Claire Martin").unwrap().status,
        "deleted"
    );

    // The Analyse reads every due document again and meets her name in the letter once more.
    let summary = lab.analyse().await;
    assert!(
        summary
            .knowledge
            .as_ref()
            .is_some_and(|knowledge| knowledge.refreshed_sources >= 1),
        "the re-reading really happened: {:?}",
        summary.knowledge
    );
    let row = entity_named(&lab.side(), "Claire Martin").unwrap();
    assert_eq!((row.entity_id, row.status.as_str()), (claire, "deleted"));
    assert!(files_of(&lab, claire).is_empty());
    assert_eq!(
        common::knowledge::entities(&lab.side())
            .iter()
            .filter(|entity| entity.canonical == "Claire Martin")
            .count(),
        1,
        "no second entity with her name"
    );
    let listed = manage::list_entities(
        &lab.side(),
        &ListQuery {
            text: "martin".to_string(),
            ..ListQuery::default()
        },
    )
    .unwrap();
    assert_eq!(listed.total, 0, "she is gone from the list");
    clean(&lab.side());

    // Restoring brings her back, and the next Analyse gives her mentions back.
    manage::restore_entity(&lab.side(), claire).expect("restores");
    lab.analyse().await;
    assert_eq!(
        entity_named(&lab.side(), "Claire Martin").unwrap().status,
        "active"
    );
    assert_eq!(
        files_of(&lab, claire),
        vec!["martin-controle.txt".to_string()]
    );
    clean(&lab.side());
}

// ------------------------------------------------------------------------- merge and split

#[tokio::test]
async fn a_merge_survives_the_next_analyse_and_an_unmerge_undoes_it() {
    let mut lab = world(clinic()).await;
    let (jean, claire) = (
        entity_id(&lab, "Jean Dupont"),
        entity_id(&lab, "Claire Martin"),
    );
    manage::merge(&lab.side(), jean, claire).expect("merges");
    lab.analyse().await;
    assert_eq!(
        entity_named(&lab.side(), "Claire Martin").unwrap().status,
        "merged"
    );
    let sheet = manage::entity_detail(&lab.side(), jean).expect("reads the sheet");
    assert_eq!(sheet.merged_in.len(), 1);
    let paths: Vec<_> = sheet
        .sources
        .iter()
        .map(|s| s.relative_path.as_str())
        .collect();
    assert!(paths.contains(&"martin-controle.txt"), "{paths:?}");
    assert!(paths.contains(&"dupont-bilan.txt"), "{paths:?}");
    clean(&lab.side());

    let settled = snapshot(&lab.side());
    lab.analyse().await;
    assert_eq!(
        snapshot(&lab.side()),
        settled,
        "a second pass changes nothing"
    );

    manage::unmerge(&lab.side(), claire).expect("unmerges");
    lab.analyse().await;
    assert_eq!(
        entity_named(&lab.side(), "Claire Martin").unwrap().status,
        "active"
    );
    assert_eq!(
        files_of(&lab, claire),
        vec!["martin-controle.txt".to_string()]
    );
    clean(&lab.side());
}

#[tokio::test]
async fn a_split_is_not_undone_by_the_next_analyse() {
    let mut lab = world(clinic()).await;
    let jean = entity_id(&lab, "Jean Dupont");
    assert_eq!(
        files_of(&lab, jean),
        vec![
            "dupont-bilan.txt".to_string(),
            "dupont-lettre.txt".to_string()
        ]
    );
    let letter: i64 = lab
        .side()
        .query_row(
            "SELECT source_id FROM kb_sources WHERE relative_path = 'dupont-lettre.txt'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let other = manage::split(
        &lab.side(),
        &tools(&lab),
        &SplitRequest {
            entity_id: jean,
            mention_ids: vec![],
            source_ids: vec![letter],
            name: "Jean Dupont du courrier".to_string(),
        },
    )
    .expect("splits")
    .entity_id
    .expect("an id");

    // The split made the documents due; the Analyse reads them again and meets "Jean Dupont" in the
    // letter once more.
    let summary = lab.analyse().await;
    assert!(summary
        .knowledge
        .as_ref()
        .is_some_and(|k| k.refreshed_sources >= 1));
    assert_eq!(files_of(&lab, jean), vec!["dupont-bilan.txt".to_string()]);
    assert_eq!(files_of(&lab, other), vec!["dupont-lettre.txt".to_string()]);
    clean(&lab.side());

    // Undoing it gives the letter back, and a further Analyse keeps it there.
    manage::undo_last_operation(&lab.side()).expect("undoes the split");
    lab.analyse().await;
    assert_eq!(
        files_of(&lab, jean),
        vec![
            "dupont-bilan.txt".to_string(),
            "dupont-lettre.txt".to_string()
        ]
    );
    assert!(entity_named(&lab.side(), "Jean Dupont du courrier").is_none());
    clean(&lab.side());
}

#[tokio::test]
async fn an_edited_automatic_entity_is_not_recreated_under_its_old_name() {
    let mut lab = world(clinic()).await;
    let claire = entity_id(&lab, "Claire Martin");
    manage::edit_entity(
        &lab.side(),
        &tools(&lab),
        claire,
        &EditEntity {
            name: Some("Claire Martin-Roux".to_string()),
            ..EditEntity::default()
        },
    )
    .expect("renames");
    lab.analyse().await;
    let people: Vec<String> = common::knowledge::entities(&lab.side())
        .into_iter()
        .filter(|entity| entity.canonical.contains("Martin"))
        .map(|entity| format!("{} / {}", entity.canonical, entity.status))
        .collect();
    assert_eq!(
        people,
        vec!["Claire Martin-Roux / active".to_string()],
        "the letter still says Claire Martin and leads to the renamed entity"
    );
    assert_eq!(
        files_of(&lab, claire),
        vec!["martin-controle.txt".to_string()]
    );
    clean(&lab.side());
}

// ---------------------------------------------------------------------------------- resets

#[tokio::test]
async fn what_she_wrote_survives_both_folder_resets_and_a_reanalysis() {
    let mut lab = world(clinic()).await;
    let created = manage::create_entity(
        &lab.side(),
        &tools(&lab),
        &NewManualEntity {
            name: "Anne Roux".to_string(),
            type_id: "person".to_string(),
            subtype: None,
            disambiguator: None,
        },
    )
    .unwrap()
    .entity_id
    .unwrap();
    manage::add_alias(&lab.side(), &tools(&lab), created, "A. Roux").unwrap();
    manage::set_attribute(
        &lab.side(),
        &tools(&lab),
        created,
        "spoken_form",
        "Anne Roux",
    )
    .unwrap();
    let jean = entity_id(&lab, "Jean Dupont");

    lab.analyse().await;
    assert!(
        entity_named(&lab.side(), "Anne Roux").is_some(),
        "a re-analysis keeps it"
    );

    // The Documents Reset: the sources and what only they held up go, her rows stay.
    lab.index.clear().expect("resets the documents");
    assert!(entity_named(&lab.side(), "Anne Roux").is_some());
    assert!(
        entity_named(&lab.side(), "Jean Dupont").is_none(),
        "an automatic entity is gone"
    );
    assert_eq!(count(&lab.side(), "kb_sources"), 0);
    // The Data Reset changes nothing more.
    lab.index
        .clear_tabular_inventories()
        .expect("resets the data");
    lab.index
        .clear_tabular_workbooks()
        .expect("resets the cell cache");
    let roux = entity_named(&lab.side(), "Anne Roux").unwrap();
    assert_eq!(roux.status, "active");
    assert_eq!(count(&lab.side(), "kb_aliases") >= 2, true);
    let _ = jean;
    clean(&lab.side());

    // The whole-base reset takes her rows too, and the log.
    manage::reset(&lab.side(), true).expect("resets the whole base");
    for table in [
        "kb_entities",
        "kb_aliases",
        "kb_attributes",
        "kb_ops_log",
        "kb_mentions",
    ] {
        assert_eq!(count(&lab.side(), table), 0, "{table}");
    }
    clean(&lab.side());
}

#[tokio::test]
async fn a_reset_without_manual_rows_keeps_her_rows_and_the_next_analyse_rebuilds_the_rest() {
    let mut lab = world(clinic()).await;
    let created = manage::create_entity(
        &lab.side(),
        &tools(&lab),
        &NewManualEntity {
            name: "Anne Roux".to_string(),
            type_id: "person".to_string(),
            subtype: None,
            disambiguator: None,
        },
    )
    .unwrap()
    .entity_id
    .unwrap();
    manage::delete_entity(&lab.side(), entity_id(&lab, "Claire Martin")).unwrap();

    let outcome = manage::reset(&lab.side(), false).expect("resets what the files taught");
    assert!(
        outcome.documents_due >= 12,
        "every document is due again: {outcome:?}"
    );
    assert!(entity_named(&lab.side(), "Jean Dupont").is_none());
    assert!(entity_named(&lab.side(), "Anne Roux").is_some());
    assert_eq!(
        entity_named(&lab.side(), "Claire Martin").unwrap().status,
        "deleted"
    );
    clean(&lab.side());

    // Nothing was asked of the AI to rebuild: the text is in the index.
    let requests = lab.gateway.requests().len();
    lab.analyse().await;
    assert_eq!(lab.gateway.requests().len(), requests);
    assert!(entity_named(&lab.side(), "Jean Dupont").is_some());
    assert_eq!(
        entity_named(&lab.side(), "Claire Martin").unwrap().status,
        "deleted"
    );
    assert_eq!(
        entity_named(&lab.side(), "Anne Roux").unwrap().entity_id,
        created
    );
    clean(&lab.side());
}

// ----------------------------------------------------------------------- the files stay as they are

#[tokio::test]
async fn nothing_the_dialog_does_touches_a_file_a_chunk_or_asks_the_ai_anything() {
    let mut lab = world(clinic()).await;
    let folder_before = folder_bytes(&lab);
    let index_before = index_rows(&lab.side());
    let requests = lab.gateway.requests().len();

    let side = lab.side();
    let tools = tools(&lab);
    let (jean, claire) = (
        entity_id(&lab, "Jean Dupont"),
        entity_id(&lab, "Claire Martin"),
    );
    let created = manage::create_entity(
        &side,
        &tools,
        &NewManualEntity {
            name: "Anne Roux".to_string(),
            type_id: "person".to_string(),
            subtype: None,
            disambiguator: None,
        },
    )
    .unwrap()
    .entity_id
    .unwrap();
    manage::add_alias(&side, &tools, jean, "Jeannot").unwrap();
    manage::edit_entity(
        &side,
        &tools,
        created,
        &EditEntity {
            name: Some("Anne Roux-Petit".to_string()),
            ..EditEntity::default()
        },
    )
    .unwrap();
    manage::merge(&side, jean, claire).unwrap();
    manage::unmerge(&side, claire).unwrap();
    manage::delete_entity(&side, claire).unwrap();
    manage::restore_entity(&side, claire).unwrap();
    manage::undo_last_operation(&side).unwrap();
    manage::list_entities(&side, &ListQuery::default()).unwrap();
    manage::list_entities(
        &side,
        &ListQuery {
            view: ListView::Deleted,
            ..ListQuery::default()
        },
    )
    .unwrap();
    manage::entity_detail(&side, jean).unwrap();
    manage::health_report(&side).unwrap();
    manage::possible_matches(&side, 0).unwrap();
    manage::overview(&side).unwrap();
    manage::reset(&side, false).unwrap();
    manage::reset(&side, true).unwrap();

    assert_eq!(
        folder_bytes(&lab),
        folder_before,
        "the files of the folder are byte for byte the same"
    );
    assert_eq!(
        index_rows(&lab.side()),
        index_before,
        "documents, chunks and cell caches are the same rows"
    );
    assert_eq!(
        lab.gateway.requests().len(),
        requests,
        "no request to the AI"
    );
    clean(&lab.side());
    // And the application keeps working: an Analyse after the whole-base reset rebuilds the base.
    lab.analyse().await;
    assert!(entity_named(&lab.side(), "Jean Dupont").is_some());
}

/// Every file of the work folder with its bytes.
fn folder_bytes(lab: &Lab) -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(lab.work_path())
        .expect("reads the folder")
        .map(|entry| {
            let path = entry.expect("an entry").path();
            (
                path.file_name().unwrap().to_string_lossy().to_string(),
                std::fs::read(&path).expect("reads the file"),
            )
        })
        .collect();
    files.sort();
    files
}

/// The rows of the tables the index owns, as text.
fn index_rows(connection: &Connection) -> Vec<String> {
    let mut rows = Vec::new();
    for sql in [
        "SELECT relative_path || '|' || sha256 || '|' || empty FROM documents ORDER BY 1",
        "SELECT chunk_id || '|' || text || '|' || length(embedding) FROM chunks ORDER BY 1",
        "SELECT chunk_id || '|' || text FROM chunks_fts ORDER BY 1",
        "SELECT relative_path || '|' || inventory_json FROM tabular_inventories ORDER BY 1",
        "SELECT workbook_id || '|' || workbook_json FROM tabular_workbooks ORDER BY 1",
    ] {
        let mut statement = connection.prepare(sql).expect("prepares");
        let found: Vec<String> = statement
            .query_map([], |row| row.get(0))
            .expect("queries")
            .collect::<Result<_, _>>()
            .expect("reads");
        rows.extend(found);
    }
    rows
}

// ------------------------------------------------------------- the dialog opens the index at once

#[tokio::test]
async fn several_commands_can_open_the_index_at_the_same_moment() {
    let lab = world(clinic()).await;
    let path = lab.index_path();
    // The dialog asks for the overview, the list and more in the same instant, each command opening
    // the index on its own: none of them may fail because another is opening it too.
    let handles: Vec<_> = (0..12)
        .map(|_| {
            let path = path.clone();
            std::thread::spawn(move || {
                let index = assistant_cabinet_ai_lib::index_store::IndexStore::open_at(&path)
                    .map_err(|error| error.code().to_string())?;
                drop(index);
                Ok::<(), String>(())
            })
        })
        .collect();
    for handle in handles {
        assert_eq!(handle.join().expect("no panic"), Ok(()));
    }
}

// -------------------------------------------------------------------------------- the migration

#[tokio::test]
async fn a_base_made_before_this_lot_gets_its_new_column_and_keeps_every_row() {
    let lab = world(clinic()).await;
    let mentions = count(&lab.side(), "kb_mentions");
    assert!(mentions > 0);

    // Take the column away, the way a base from lots 1 to 7 does not have it.
    let side = lab.side();
    side.execute_batch(
        "DROP VIEW kb_effective_mentions; ALTER TABLE kb_mentions DROP COLUMN split_from;",
    )
    .expect("makes the older shape");
    drop(side);

    let reopened = assistant_cabinet_ai_lib::index_store::IndexStore::open_at(&lab.index_path())
        .expect("opens the older base");
    drop(reopened);
    let side = lab.side();
    let columns: Vec<String> = {
        let mut statement = side.prepare("PRAGMA table_info(kb_mentions)").unwrap();
        let rows = statement
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap();
        rows.map(Result::unwrap).collect()
    };
    assert!(columns.contains(&"split_from".to_string()), "{columns:?}");
    assert_eq!(count(&side, "kb_mentions"), mentions, "no row was lost");
    let jean = entity_id(&lab, "Jean Dupont");
    assert!(manage::entity_detail(&side, jean).is_ok());
    clean(&side);
}
