//! Explorer step 8: a file of the other family is analysed in place, read only, under the folder it sits in
//! (`docs/DECISIONS.md`, "root and family"; `docs/SESSION-EXPLORER/EXPLORER-08-in-place.md`).
//!
//! A workbook sitting in the Documents folder is read by the tabular pass and answers a table question; a
//! document sitting in the Data folder is read by the document pass and answers a document question. The
//! same relative path in both folders is two files. Nothing is copied, moved or modified. A pass that
//! meets no file of the other family writes exactly what it wrote before.
//!
//! These tests start a fake gateway on a local socket (the document pass asks for vectors): run them one
//! at a time, on their own, as `docs/TROUBLESHOOTING.md` says.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use assistant_cabinet_ai_lib::analysis_scope::{ScopeEntry, ScopeMode};
use assistant_cabinet_ai_lib::data_folder;
use assistant_cabinet_ai_lib::error::AppError;
use assistant_cabinet_ai_lib::family_view::{family_views, FamilyViews};
use assistant_cabinet_ai_lib::file_key::Root;
use assistant_cabinet_ai_lib::file_profile::build::Domain;
use assistant_cabinet_ai_lib::file_record::{ProcessingStatus, Readability};
use assistant_cabinet_ai_lib::finder::facts::folder_facts;
use assistant_cabinet_ai_lib::gateway::GatewayClient;
use assistant_cabinet_ai_lib::indexing::{self, IndexSummary};
use assistant_cabinet_ai_lib::inventory::{FileHashCache, WorkFolderInventory};
use assistant_cabinet_ai_lib::retrieval::{self, RetrievalScope};
use assistant_cabinet_ai_lib::tabular::engine::TabularValue;
use assistant_cabinet_ai_lib::tabular_answer::{self, ModelAssist, TabularAnswer};
use assistant_cabinet_ai_lib::viewer_source;
use common::knowledge::{gateway_client, tabular_snapshot, Lab};
use rusqlite::Connection;

/// Ten invoices, three of them in March 2026: "how many invoices in March 2026" is 3.
fn factures_csv() -> String {
    "date,fournisseur,montant\n\
     2026-01-05,Alpha,100\n\
     2026-01-15,Beta,150\n\
     2026-02-01,Gamma,200\n\
     2026-02-10,Alpha,250\n\
     2026-02-20,Beta,300\n\
     2026-03-02,Gamma,300\n\
     2026-03-10,Alpha,400\n\
     2026-03-20,Beta,500\n\
     2026-04-01,Gamma,600\n\
     2026-04-15,Alpha,650\n"
        .to_string()
}

fn write(root: &Path, relative: &str, content: &[u8]) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

/// The document pass of the application: the work folder, and the Data folder as the other one.
async fn document_pass(lab: &mut Lab, with_other_folder: bool) -> IndexSummary {
    let mut context = lab.context();
    let url = lab.gateway.url.clone();
    let other = lab.data.path().to_path_buf();
    indexing::run_cancellable(
        &mut lab.index,
        &gateway_client(),
        &url,
        "cabinet-embed",
        lab.work.path(),
        with_other_folder.then_some(other.as_path()),
        None,
        None,
        lab.locale,
        &|_| {},
        Some(&mut context),
        &|| false,
    )
    .await
    .expect("the document pass completes")
}

/// The tabular pass of the application: the Data folder, and the Documents folder as the other one.
fn tabular_pass(lab: &mut Lab, with_other_folder: bool) -> IndexSummary {
    data_folder::analyse_cancellable(
        lab.data.path(),
        with_other_folder.then(|| lab.work.path()),
        &mut lab.index,
        "en-US",
        &|_| {},
        None,
        &|| false,
    )
    .expect("the tabular pass completes")
}

fn views(lab: &Lab) -> FamilyViews {
    family_views(
        Some(lab.work.path()),
        Some(lab.data.path()),
        Some(&lab.index),
        &FileHashCache::new(),
        &FileHashCache::new(),
    )
    .unwrap()
}

fn rows(path: &Path, sql: &str) -> Vec<String> {
    let connection = Connection::open(path).unwrap();
    let mut statement = connection.prepare(sql).unwrap();
    let found = statement
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    found
}

/// Every file under `root` with its bytes: what "nothing was copied, moved or modified" compares.
fn listing(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, folder: &Path, into: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(base, &path, into);
            } else {
                let relative = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                into.insert(relative, std::fs::read(&path).unwrap());
            }
        }
    }
    let mut found = BTreeMap::new();
    walk(root, root, &mut found);
    found
}

#[tokio::test]
async fn a_workbook_in_the_documents_folder_is_analysed_in_place_and_answers_a_table_question() {
    let mut lab = Lab::new();
    // A name a rename would have changed, if any pass ever renamed a workbook.
    write(
        lab.work.path(),
        "Suivi des factures 2026.csv",
        factures_csv().as_bytes(),
    );
    let before = listing(lab.work.path());

    let summary = tabular_pass(&mut lab, true);

    assert_eq!(summary.scanned_files, 1, "the tabular pass read it");
    assert_eq!(summary.indexed_files, 1);
    let key = "/documents/Suivi des factures 2026.csv";
    assert_eq!(
        rows(
            &lab.index_path(),
            "SELECT relative_path FROM tabular_inventories"
        ),
        vec![key.to_string()]
    );

    // Green, in the family's view and in the folder's own listing.
    let seen = views(&lab);
    let data = seen.data.expect("the tabular family exists");
    let record = data.files().find_by_relative_path(key).unwrap();
    assert_eq!(
        (record.readability, record.processing_status),
        (Readability::Readable, ProcessingStatus::Indexed)
    );
    let (physical, _) = WorkFolderInventory::discover_in(
        Root::Documents,
        lab.work.path(),
        Some(lab.data.path()),
        Some(&lab.index),
        &FileHashCache::new(),
    )
    .unwrap();
    let record = physical
        .find_by_relative_path("Suivi des factures 2026.csv")
        .unwrap();
    assert_eq!(record.processing_status, ProcessingStatus::Indexed);

    // It answers a table question, ticked under its key.
    let scope = ScopeMode::Explicit(vec![ScopeEntry {
        relative_path: key.to_string(),
        pinned_id: data.files().find_by_relative_path(key).unwrap().id.clone(),
        added_at: 1,
        sheet_names: Vec::new(),
    }]);
    let pending = tabular_answer::prepare_if_data_only(
        "How many invoices in March 2026?",
        &data,
        &scope,
        "en-US",
        &lab.index,
    )
    .unwrap()
    .expect("the question is about the table");
    let gateway = GatewayClient::new().unwrap();
    let assist = ModelAssist {
        gateway: &gateway,
        // Never dialled: the question classifies on its own.
        server_url: "http://127.0.0.1:9",
        model_alias: "cabinet-chat",
        idle_timeout: std::time::Duration::from_millis(200),
    };
    let answer = tabular_answer::resolve(
        pending,
        "How many invoices in March 2026?",
        "en-US",
        &assist,
    )
    .await;
    let TabularAnswer::Value { value, .. } = answer else {
        panic!("expected a computed value, got {answer:?}");
    };
    assert_eq!(value, TabularValue::Count(3));

    // Read only: the folder holds the same files with the same bytes.
    assert_eq!(listing(lab.work.path()), before);
    assert!(listing(lab.data.path()).is_empty());
}

#[tokio::test]
async fn a_document_in_the_data_folder_is_analysed_in_place_and_answers_a_document_question() {
    let mut lab = Lab::new();
    write(
        lab.data.path(),
        "Courriers/lettre-cabinet.txt",
        b"The cabinet closes for the summer on the first of August and reopens on the first of September.",
    );
    let before = listing(lab.data.path());

    let summary = document_pass(&mut lab, true).await;

    assert_eq!(summary.scanned_files, 1, "the document pass read it");
    let key = "/data/Courriers/lettre-cabinet.txt";
    assert_eq!(
        rows(&lab.index_path(), "SELECT relative_path FROM documents"),
        vec![key.to_string()]
    );

    // Green in the Data folder's own listing, and in the document family's view.
    let (physical, _) = WorkFolderInventory::discover_in(
        Root::Data,
        lab.data.path(),
        Some(lab.work.path()),
        Some(&lab.index),
        &FileHashCache::new(),
    )
    .unwrap();
    let record = physical
        .find_by_relative_path("Courriers/lettre-cabinet.txt")
        .unwrap();
    assert_eq!(
        (record.readability, record.processing_status),
        (Readability::Readable, ProcessingStatus::Indexed)
    );
    let documents = views(&lab).documents.unwrap();
    assert!(documents.find_by_relative_path(key).is_some());

    // It answers a document question: the passage comes back, cited under its key.
    let scoped = [key.to_string()];
    let found = retrieval::search_scoped(
        &lab.index,
        "When does the cabinet close for the summer?",
        &[0.5; 8],
        RetrievalScope::Files(&scoped),
    )
    .unwrap();
    assert!(!found.is_empty());
    assert!(found.iter().all(|passage| passage.relative_path == key));
    assert!(found[0].text.contains("first of August"));

    // The profile of the file is stored under the document family and the same key.
    assert_eq!(
        rows(
            &lab.index_path(),
            "SELECT domain || ':' || relative_path FROM file_profiles"
        ),
        vec![format!("documents:{key}")]
    );

    // Read only.
    assert_eq!(listing(lab.data.path()), before);
    assert!(listing(lab.work.path()).is_empty());
}

#[tokio::test]
async fn the_same_relative_path_in_both_folders_does_not_collide() {
    let mut lab = Lab::new();
    write(
        lab.work.path(),
        "rapport.txt",
        b"The apple harvest was early this year.",
    );
    write(
        lab.data.path(),
        "rapport.txt",
        b"The banana shipment was late this year.",
    );
    let mut other_sheet = factures_csv();
    other_sheet.push_str("2026-05-01,Delta,10\n");
    write(lab.work.path(), "table.csv", factures_csv().as_bytes());
    write(lab.data.path(), "table.csv", other_sheet.as_bytes());

    document_pass(&mut lab, true).await;
    tabular_pass(&mut lab, true);

    assert_eq!(
        rows(
            &lab.index_path(),
            "SELECT relative_path FROM documents ORDER BY 1"
        ),
        vec!["/data/rapport.txt", "rapport.txt"]
    );
    assert_eq!(
        rows(
            &lab.index_path(),
            "SELECT relative_path FROM tabular_inventories ORDER BY 1"
        ),
        vec!["/documents/table.csv", "table.csv"]
    );
    assert_eq!(
        rows(
            &lab.index_path(),
            "SELECT DISTINCT chunk_id FROM chunks ORDER BY 1"
        )
        .len(),
        2,
        "two files, two passages: the chunk ids carry the key"
    );

    // Each is found by its own words and within its own key only.
    let own = ["rapport.txt".to_string()];
    let foreign = ["/data/rapport.txt".to_string()];
    let apples = retrieval::search_scoped(
        &lab.index,
        "apple harvest",
        &[0.5; 8],
        RetrievalScope::Files(&own),
    )
    .unwrap();
    let bananas = retrieval::search_scoped(
        &lab.index,
        "banana shipment",
        &[0.5; 8],
        RetrievalScope::Files(&foreign),
    )
    .unwrap();
    assert!(apples[0].text.contains("apple"));
    assert!(bananas[0].text.contains("banana"));
    assert!(apples
        .iter()
        .all(|passage| passage.relative_path == "rapport.txt"));
    assert!(bananas
        .iter()
        .all(|passage| passage.relative_path == "/data/rapport.txt"));

    // Both listings tell the two workbooks apart: the folder's own file stays at its path.
    let seen = views(&lab);
    let data = seen.data.unwrap();
    let own = data.files().find_by_relative_path("table.csv").unwrap();
    let foreign = data
        .files()
        .find_by_relative_path("/documents/table.csv")
        .unwrap();
    assert_ne!(own.sha256, foreign.sha256);
    assert_eq!(foreign.processing_status, ProcessingStatus::Indexed);
    assert_eq!(own.processing_status, ProcessingStatus::Indexed);
}

#[tokio::test]
async fn a_pass_that_meets_no_file_of_the_other_family_writes_what_it_always_wrote() {
    // The tabular snapshot is byte-identical for a Data folder without foreign files, whether or not the
    // Documents folder is passed (the Documents folder here holds documents only).
    let mut with = Lab::new();
    let mut without = Lab::new();
    for lab in [&with, &without] {
        write(lab.data.path(), "factures.csv", factures_csv().as_bytes());
        write(
            lab.data.path(),
            "Clients/liste.csv",
            b"nom,ville\nAlpha,Lyon\n",
        );
        write(
            lab.work.path(),
            "lettre.txt",
            b"A letter with nothing to do with any table.",
        );
    }

    tabular_pass(&mut with, true);
    tabular_pass(&mut without, false);
    assert_eq!(
        tabular_snapshot(&with.side()),
        tabular_snapshot(&without.side())
    );

    // The document pass, with the Data folder passed or not, stores the same documents.
    document_pass(&mut with, true).await;
    document_pass(&mut without, false).await;
    for sql in [
        "SELECT relative_path || '|' || sha256 FROM documents ORDER BY 1",
        "SELECT chunk_id || '|' || text FROM chunks ORDER BY 1",
        "SELECT domain || ':' || relative_path FROM file_profiles ORDER BY 1",
    ] {
        assert_eq!(
            rows(&with.index_path(), sql),
            rows(&without.index_path(), sql),
            "{sql}"
        );
    }
}

#[tokio::test]
async fn a_text_pass_over_both_folders_never_rewrites_the_tabular_tables() {
    let mut lab = Lab::new();
    write(lab.data.path(), "factures.csv", factures_csv().as_bytes());
    write(lab.work.path(), "Suivi.csv", factures_csv().as_bytes());
    write(
        lab.work.path(),
        "lettre.txt",
        b"A letter about the first invoice of March.",
    );
    write(
        lab.data.path(),
        "note.txt",
        b"A note kept next to the tables.",
    );
    tabular_pass(&mut lab, true);
    let before = tabular_snapshot(&lab.side());

    document_pass(&mut lab, true).await;
    lab.write("autre.txt", "Another letter, added later.");
    document_pass(&mut lab, true).await;

    assert_eq!(tabular_snapshot(&lab.side()), before);
}

#[tokio::test]
async fn a_foreign_file_that_leaves_its_folder_is_forgotten_by_its_own_pass() {
    let mut lab = Lab::new();
    write(lab.work.path(), "tableau.csv", factures_csv().as_bytes());
    write(
        lab.data.path(),
        "lettre.txt",
        b"A short letter that will be removed.",
    );
    tabular_pass(&mut lab, true);
    document_pass(&mut lab, true).await;
    assert_eq!(
        rows(
            &lab.index_path(),
            "SELECT relative_path FROM tabular_inventories"
        ),
        vec!["/documents/tableau.csv"]
    );

    std::fs::remove_file(lab.work.path().join("tableau.csv")).unwrap();
    std::fs::remove_file(lab.data.path().join("lettre.txt")).unwrap();
    let tables = tabular_pass(&mut lab, true);
    let documents = document_pass(&mut lab, true).await;

    assert_eq!(tables.removed_files, vec!["/documents/tableau.csv"]);
    assert_eq!(documents.removed_files, vec!["/data/lettre.txt"]);
    assert!(rows(
        &lab.index_path(),
        "SELECT relative_path FROM tabular_inventories"
    )
    .is_empty());
    assert!(rows(&lab.index_path(), "SELECT relative_path FROM documents").is_empty());
}

#[tokio::test]
async fn the_facts_of_a_foreign_file_are_found_under_its_key() {
    // Explorer step 7 reads the facts of a family (`folder_facts`); a file read in place carries them
    // under the key it was stored with, and the client places it by the folder that key names.
    let mut lab = Lab::new();
    write(
        lab.data.path(),
        "Courriers/2026-02-06_lettre.txt",
        b"A letter dated by its name.",
    );
    write(lab.work.path(), "table.csv", factures_csv().as_bytes());
    document_pass(&mut lab, true).await;
    tabular_pass(&mut lab, true);

    let documents = folder_facts(&lab.side(), Domain::Documents).unwrap();
    let letter = documents
        .files
        .iter()
        .find(|file| file.relative_path == "/data/Courriers/2026-02-06_lettre.txt")
        .expect("the document of the Data folder has facts under its key");
    assert_eq!(letter.years, vec!["2026"]);

    let tables = folder_facts(&lab.side(), Domain::Data).unwrap();
    let table = tables
        .files
        .iter()
        .find(|file| file.relative_path == "/documents/table.csv")
        .expect("the workbook of the Documents folder has facts under its key");
    assert!(table.years.contains(&"2026".to_string()));
}

#[tokio::test]
async fn without_the_other_folder_a_pass_reads_its_own_folder_alone() {
    let mut lab = Lab::new();
    write(lab.work.path(), "tableau.csv", factures_csv().as_bytes());
    write(lab.data.path(), "lettre.txt", b"A short letter.");

    let tables = tabular_pass(&mut lab, false);
    let documents = document_pass(&mut lab, false).await;

    assert_eq!(tables.scanned_files, 0);
    assert_eq!(documents.scanned_files, 0);
    assert!(rows(
        &lab.index_path(),
        "SELECT relative_path FROM tabular_inventories"
    )
    .is_empty());
}

#[tokio::test]
async fn hidden_and_system_files_follow_the_extension_rule_and_are_never_renamed() {
    // The rule, stated once: the walk lists every regular file, and the family is decided by the
    // extension alone. An Office lock file keeps its workbook extension, so the tabular pass reads it and
    // reports it as a workbook with no table; `.DS_Store`, `Thumbs.db` and `desktop.ini` belong to no
    // family, so no pass reads them and the document pass does not report them as an unread format. No
    // rename ever touches a hidden file or a lock file.
    let mut lab = Lab::new();
    write(lab.work.path(), "~$Suivi.xlsx", b"lock");
    write(lab.work.path(), ".DS_Store", b"x");
    write(lab.work.path(), "Thumbs.db", b"x");
    write(lab.work.path(), "desktop.ini", b"x");
    let before = listing(lab.work.path());

    let tables = tabular_pass(&mut lab, true);
    let documents = document_pass(&mut lab, true).await;
    let renamed = assistant_cabinet_ai_lib::filename_sanitizer::sanitize_documents(
        lab.work.path(),
        Some(lab.data.path()),
    );

    assert_eq!(tables.scanned_files, 1);
    assert_eq!(tables.empty_files, vec!["/documents/~$Suivi.xlsx"]);
    assert_eq!(documents.scanned_files, 0);
    assert!(documents.unread_formats.is_empty());
    assert!(renamed.renamed.is_empty() && renamed.failed.is_empty());
    assert_eq!(listing(lab.work.path()), before);
}

#[test]
fn a_foreign_key_opens_nothing_where_the_roots_are_not_known() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "data/lettre.txt", b"hello");
    // The viewer is handed the key by a caller that does not know about roots: it refuses, it does not open
    // another file.
    let refused = viewer_source::read(root.path(), "/data/lettre.txt");
    assert!(matches!(refused, Err(AppError::ViewerFileUnreadable)));
}

#[test]
fn a_symbolic_link_in_the_other_folder_is_not_followed() {
    #[cfg(unix)]
    {
        use assistant_cabinet_ai_lib::index_store::IndexStore;

        let documents = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        write(outside.path(), "secret.csv", factures_csv().as_bytes());
        std::os::unix::fs::symlink(
            outside.path().join("secret.csv"),
            documents.path().join("link.csv"),
        )
        .unwrap();
        let data = tempfile::tempdir().unwrap();
        let app = tempfile::tempdir().unwrap();
        let mut index = IndexStore::open_at(&app.path().join("index.sqlite3")).unwrap();

        let summary = data_folder::analyse_cancellable(
            data.path(),
            Some(documents.path()),
            &mut index,
            "en-US",
            &|_| {},
            None,
            &|| false,
        )
        .unwrap();

        assert_eq!(summary.scanned_files, 0);
    }
}

// --- A file named in a question, wherever it sits (explorer step 9 follow-up) ------------------------------

/// What the two gates of a mixed selection (a document and a workbook both ticked) make of a question.
/// `gate`: the data-only shortcut (`None` hands the question to the mixed tier). `table_part`: what the
/// mixed tier's own table half is (`prepare_for_mixed`), where an answer that is not a computed value ends
/// the whole answer with no generation.
struct MixedRouting {
    gate: Option<TabularAnswer>,
    table_part: TabularAnswer,
}

fn mixed_routing(
    lab: &Lab,
    ticked_workbook: &str,
    ticked_document: &str,
    question: &str,
    locale: &str,
) -> MixedRouting {
    let data = views(lab).data.expect("the tabular family exists");
    let scope = ScopeMode::Explicit(vec![ScopeEntry {
        relative_path: ticked_workbook.to_string(),
        pinned_id: data
            .files()
            .find_by_relative_path(ticked_workbook)
            .unwrap()
            .id
            .clone(),
        added_at: 1,
        sheet_names: Vec::new(),
    }]);
    let gate =
        match tabular_answer::prepare_if_data_only(question, &data, &scope, locale, &lab.index)
            .unwrap()
        {
            None => None,
            Some(tabular_answer::PendingAnswer::Done(answer)) => Some(answer),
            Some(tabular_answer::PendingAnswer::TryModel { .. }) => {
                panic!("the gate never hands the model to this tier")
            }
        };
    let table_part = match tabular_answer::prepare_for_mixed(
        question,
        &data,
        &scope,
        locale,
        &lab.index,
        &[ticked_document.to_string()],
    )
    .unwrap()
    {
        tabular_answer::PendingAnswer::Done(answer) => answer,
        tabular_answer::PendingAnswer::TryModel { fallback, .. } => fallback,
    };
    MixedRouting { gate, table_part }
}

/// The table half of a question that has none: the same sign `prepare_for_mixed` leaves for a question the
/// classifier cannot read, so the documents answer it alone.
fn has_no_table_part(answer: &TabularAnswer) -> bool {
    matches!(answer, TabularAnswer::Nudge { reason: None, .. })
}

#[test]
fn a_document_of_the_data_folder_named_in_a_mixed_question_is_answered_from_the_documents() {
    let mut lab = Lab::new();
    write(
        lab.data.path(),
        "Test/factures.csv",
        factures_csv().as_bytes(),
    );
    write(
        lab.data.path(),
        "Test/publipostage/modele_lettre.docx",
        b"a document of the Data folder",
    );
    tabular_pass(&mut lab, true);

    // The owner's own question, word for word (21 October 2026), with the letter ticked under its key.
    let routing = mixed_routing(
        &lab,
        "Test/factures.csv",
        "/data/Test/publipostage/modele_lettre.docx",
        "que dit la lettre modele_lettre.docx ?",
        "fr-FR",
    );

    assert!(routing.gate.is_none(), "got {:?}", routing.gate);
    assert!(
        has_no_table_part(&routing.table_part),
        "the table engine must not answer for a document, got {:?}",
        routing.table_part
    );
}

#[test]
fn a_document_of_the_documents_folder_named_in_a_mixed_question_is_answered_from_the_documents() {
    let mut lab = Lab::new();
    write(lab.data.path(), "factures.csv", factures_csv().as_bytes());
    write(
        lab.work.path(),
        "lettre-cabinet.txt",
        b"The cabinet closes in August.",
    );
    tabular_pass(&mut lab, true);

    let routing = mixed_routing(
        &lab,
        "factures.csv",
        "lettre-cabinet.txt",
        "que dit la lettre lettre-cabinet.txt ?",
        "fr-FR",
    );

    assert!(routing.gate.is_none(), "got {:?}", routing.gate);
    assert!(
        has_no_table_part(&routing.table_part),
        "the table engine must not answer for a document, got {:?}",
        routing.table_part
    );
}

#[test]
fn a_document_named_without_any_trigger_word_is_answered_from_the_documents_too() {
    // No "the letter": nothing but the file name says it is a document.
    let mut lab = Lab::new();
    write(lab.data.path(), "factures.csv", factures_csv().as_bytes());
    write(
        lab.data.path(),
        "Test/modele_lettre.docx",
        b"a document of the Data folder",
    );
    tabular_pass(&mut lab, true);

    let routing = mixed_routing(
        &lab,
        "factures.csv",
        "/data/Test/modele_lettre.docx",
        "Resume modele_lettre.docx",
        "fr-FR",
    );

    assert!(routing.gate.is_none(), "got {:?}", routing.gate);
    assert!(
        has_no_table_part(&routing.table_part),
        "got {:?}",
        routing.table_part
    );
}

#[test]
fn a_workbook_named_in_a_mixed_question_but_not_ticked_is_still_said_not_selected() {
    let mut lab = Lab::new();
    write(lab.data.path(), "factures.csv", factures_csv().as_bytes());
    write(lab.data.path(), "achats.csv", factures_csv().as_bytes());
    write(
        lab.work.path(),
        "lettre-cabinet.txt",
        b"The cabinet closes in August.",
    );
    tabular_pass(&mut lab, true);

    let routing = mixed_routing(
        &lab,
        "factures.csv",
        "lettre-cabinet.txt",
        "How many invoices in achats.csv?",
        "en-US",
    );

    let not_selected = TabularAnswer::FileNotSelected {
        query: "achats.csv".to_string(),
    };
    assert_eq!(routing.gate, Some(not_selected.clone()));
    assert_eq!(routing.table_part, not_selected);
}
