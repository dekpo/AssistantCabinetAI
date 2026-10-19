//! Step 6 of the file finder work (`docs/LEXICAL-RETRIEVAL-DESIGN.md` 6, 9): the engine that finds files
//! with no model, no embedding and no file read, over the profile tables, the passages and the knowledge
//! base. Every document and every name is invented.

#![allow(clippy::field_reassign_with_default)]

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

use assistant_cabinet_ai_lib::discovery::discover_all;
use assistant_cabinet_ai_lib::file_profile::build::Domain;
use assistant_cabinet_ai_lib::finder::typed::apply_typed_text;
use assistant_cabinet_ai_lib::finder::{
    find, DateRange, EntityFilter, FinderQuery, FinderResult, FinderScope, InventoryFile,
};
use common::knowledge::{tabular_snapshot, Lab};
use rusqlite::Connection;
use serde::Deserialize;

const PRESCRIPTION: &str = "Ordonnance du 20 mai 2026\nCabinet medical des Voirons\nPatient : Marie Martin\nOmeprazole, un comprime matin et soir.\n";
const INVOICE: &str = "Facture FAC-2026-014\nMedSupply SA\nFacture n FAC-2026-014\nDate : 14 avril 2026\nClient : Cabinet medical des Voirons\nTotal HT : CHF 326.30\n";
const LETTER: &str = "Courrier du 3 juin 2026\nHopitaux universitaires de Geneve\nMadame Marie Martin a ete recue aux HUG pour une consultation de dermatologie.\nElle presente une lesion cutanee du bras gauche.\n";
const MINUTES: &str = "Compte rendu de la reunion de service\nDate : 12 mars 2026\nLe planning des vacances a ete adopte.\n";

fn inventory(folder: &std::path::Path) -> Vec<InventoryFile> {
    discover_all(folder)
        .iter()
        .map(InventoryFile::from)
        .collect()
}

fn query(text: &str) -> FinderQuery {
    FinderQuery {
        text: Some(text.to_string()),
        limit: Some(10_000),
        ..FinderQuery::default()
    }
}

fn paths(result: &FinderResult) -> Vec<&str> {
    result
        .files
        .iter()
        .map(|hit| hit.relative_path.as_str())
        .collect()
}

fn whole(
    connection: &Connection,
    domain: Domain,
    files: &[InventoryFile],
    query: &FinderQuery,
) -> FinderResult {
    find(connection, domain, files, query, &FinderScope::WholeFolder).expect("the finder answers")
}

fn entity_id(connection: &Connection, like: &str) -> i64 {
    connection
        .query_row(
            "SELECT entity_id FROM kb_entities WHERE canonical_name LIKE ?1 AND status = 'active'",
            [like],
            |row| row.get(0),
        )
        .expect("the base knows the entity")
}

async fn analysed_lab() -> Lab {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    lab.write("2026-05-20_ordonnance.txt", PRESCRIPTION);
    lab.write("2026-04-14_facture-medsupply-fac-2026-014.txt", INVOICE);
    lab.write("courriers/2026-06-03_courrier-dermatologie.txt", LETTER);
    lab.write("reunions/compte-rendu-service.txt", MINUTES);
    lab.analyse().await;
    lab
}

// ---------------------------------------------------------------------------------------------
// The name: every file is findable by what the file system knows, before any Analyse
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct NameQuery {
    kind: String,
    query: String,
    expected: Vec<String>,
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("the repository root is three levels above src-tauri")
        .to_path_buf()
}

fn emit_queries(out: &std::path::Path) -> Vec<NameQuery> {
    let root = repository_root();
    let mut last_error = String::new();
    for python in ["python", "python3", "py"] {
        let result = Command::new(python)
            .current_dir(&root)
            .args([
                "scripts/file_profile_score.py",
                "--sandbox",
                "fixtures/knowledge-sandbox",
                "--emit-name-queries",
            ])
            .arg(out)
            .output();
        match result {
            Ok(output) if output.status.success() => {
                let text = std::fs::read_to_string(out).expect("the script wrote the queries");
                return serde_json::from_str(&text).expect("the queries are JSON");
            }
            Ok(output) => last_error = String::from_utf8_lossy(&output.stderr).to_string(),
            Err(error) => last_error = error.to_string(),
        }
    }
    panic!("no Python could run scripts/file_profile_score.py: {last_error}");
}

fn sandbox_inventory() -> Vec<InventoryFile> {
    let root = repository_root().join("fixtures/knowledge-sandbox");
    let mut files = Vec::new();
    for file in ["documents.json", "workbooks.json"] {
        let text = std::fs::read_to_string(root.join(file)).expect("reads the truth");
        let rows: Vec<serde_json::Value> = serde_json::from_str(&text).expect("the truth is JSON");
        for row in rows {
            let path = row["path"].as_str().expect("a path").to_string();
            let extension = path.rsplit_once('.').map_or("", |(_, e)| e).to_lowercase();
            files.push(InventoryFile {
                relative_path: path,
                extension,
            });
        }
    }
    files
}

#[test]
fn every_name_query_finds_every_file_it_must_through_the_finders_own_entry() {
    let directory = tempfile::tempdir().expect("temporary folder");
    let queries = emit_queries(&directory.path().join("queries.json"));
    let files = sandbox_inventory();
    // An index that holds nothing: the files are known by the file system alone.
    let lab = Lab::new();
    let connection = lab.side();

    assert!(queries.len() >= 700, "{}", queries.len());
    let mut missed: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for item in &queries {
        // The sandbox holds both kinds of file; each is searched in the folder it belongs to.
        let mut found: Vec<String> = Vec::new();
        for domain in [Domain::Documents, Data] {
            let scoped: Vec<InventoryFile> = files
                .iter()
                .filter(|file| {
                    matches!(file.extension.as_str(), "csv" | "xls" | "xlsx" | "xlsm")
                        == (domain == Data)
                })
                .cloned()
                .collect();
            let result = whole(&connection, domain, &scoped, &query(&item.query));
            found.extend(result.files.iter().map(|hit| hit.relative_path.clone()));
        }
        let lost: Vec<String> = item
            .expected
            .iter()
            .filter(|path| !found.contains(path))
            .cloned()
            .collect();
        if !lost.is_empty() {
            missed
                .entry(format!("{}|{}", item.kind, item.query))
                .or_default()
                .extend(lost);
        }
    }
    assert!(
        missed.is_empty(),
        "{} of {} name queries missed a file: {:?}",
        missed.len(),
        queries.len(),
        missed.iter().take(10).collect::<Vec<_>>()
    );
}

use Domain::Data;

#[tokio::test]
async fn a_file_never_analysed_is_found_by_its_name_and_says_what_it_is_not() {
    let lab = Lab::new();
    lab.write("2026-10-01_courrier-dermatologie.pdf", "x");
    lab.write("scan-mercredi.jpg", "x");
    lab.write("notes.pages", "x");
    let files = inventory(lab.work_path());
    let connection = lab.side();

    let result = whole(&connection, Domain::Documents, &files, &query("derma"));
    assert_eq!(paths(&result), vec!["2026-10-01_courrier-dermatologie.pdf"]);
    let hit = &result.files[0];
    assert_eq!(hit.card.state, "unanalysed");
    assert_eq!(hit.card.format, "pdf");
    assert!(hit
        .reasons
        .iter()
        .any(|reason| reason.kind == "word" && reason.detail.as_deref() == Some("name")));
    assert!(hit.excerpt.is_none());

    // A format the product does not read is findable by name and says so.
    let unsupported = whole(&connection, Domain::Documents, &files, &query("notes"));
    assert_eq!(unsupported.files[0].card.state, "unsupported");

    // The coverage states what could not be searched in the text: nothing is analysed here.
    assert_eq!(result.coverage.searched, 0);
    assert_eq!(result.coverage.unanalysed, 2);
    assert_eq!(result.coverage.unreadable, 1);
    assert_eq!(result.total, 1);
}

// ---------------------------------------------------------------------------------------------
// Words: the profile, the passages, and why a file was found
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn a_word_is_found_in_the_profile_and_in_the_passages_and_the_name_ranks_first() {
    let lab = analysed_lab().await;
    let files = inventory(lab.work_path());
    let connection = lab.side();

    // In a passage only: no file name holds "lesion".
    let lesion = whole(&connection, Domain::Documents, &files, &query("lesion"));
    assert_eq!(
        paths(&lesion),
        vec!["courriers/2026-06-03_courrier-dermatologie.txt"]
    );
    let hit = &lesion.files[0];
    assert!(hit
        .reasons
        .iter()
        .any(|reason| reason.kind == "word" && reason.detail.as_deref() == Some("passage")));
    let excerpt = hit.excerpt.as_deref().expect("the passage is shown");
    assert!(excerpt.to_lowercase().contains("lesion"), "{excerpt}");
    assert_eq!(hit.locator.as_ref().and_then(|l| l.page), Some(1));

    // A word of a title is a profile hit, found while it is typed.
    let title = whole(&connection, Domain::Documents, &files, &query("reunion"));
    assert!(paths(&title).contains(&"reunions/compte-rendu-service.txt"));

    // A prefix of the last word, as in a field being typed; a prefix in the middle is not a word.
    let typed = whole(&connection, Domain::Documents, &files, &query("ordo"));
    assert_eq!(paths(&typed), vec!["2026-05-20_ordonnance.txt"]);
    let middle = whole(
        &connection,
        Domain::Documents,
        &files,
        &query("ordo cabinet"),
    );
    assert!(middle.files.is_empty(), "{:?}", paths(&middle));

    // The finder says it is a word search and which words it read.
    let search = lesion.text_search.as_ref().expect("a word search");
    assert_eq!(search.terms, vec!["lesion"]);

    // The name outranks a passage: "dermatologie" is in the name of one file and the text of the same.
    let derma = whole(&connection, Domain::Documents, &files, &query("martin"));
    assert!(derma.total >= 2, "{:?}", paths(&derma));
    assert_eq!(
        whole(&connection, Domain::Documents, &files, &query("")).total,
        4
    );
}

#[tokio::test]
async fn the_name_of_a_file_outranks_the_same_word_in_a_passage() {
    let mut lab = Lab::new();
    lab.write("rapport-annuel.txt", "Le bilan de l annee.\n");
    lab.write("notes-diverses.txt", "Voir le rapport annuel du service.\n");
    lab.analyse().await;
    let files = inventory(lab.work_path());
    let connection = lab.side();

    let result = whole(&connection, Domain::Documents, &files, &query("rapport"));
    assert_eq!(
        paths(&result),
        vec!["rapport-annuel.txt", "notes-diverses.txt"]
    );
}

#[tokio::test]
async fn a_search_never_leaves_its_own_folder() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write("planning.txt", "Planning du cabinet.\n");
    lab.write_data(
        "planning.csv",
        "Patient;Date du rendez-vous\nMarie Martin;2026-05-20\n",
    );
    lab.analyse().await;
    lab.analyse_data();
    let documents = inventory(lab.work_path());
    let data = inventory(lab.data.path());
    let connection = lab.side();

    let in_documents = whole(
        &connection,
        Domain::Documents,
        &documents,
        &query("planning"),
    );
    assert_eq!(paths(&in_documents), vec!["planning.txt"]);
    assert_eq!(in_documents.files[0].domain, Domain::Documents);
    let in_data = whole(&connection, Domain::Data, &data, &query("planning"));
    assert_eq!(paths(&in_data), vec!["planning.csv"]);
    assert_eq!(in_data.files[0].domain, Domain::Data);
}

// ---------------------------------------------------------------------------------------------
// Entities and roles
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn an_entity_filter_reads_the_base_and_the_profile_and_a_role_reads_the_profile_only() {
    let lab = analysed_lab().await;
    let files = inventory(lab.work_path());
    let connection = lab.side();
    let marie = entity_id(&connection, "%Martin%");

    let mut by_entity = FinderQuery::default();
    by_entity.entities = vec![EntityFilter {
        entity_id: marie,
        role: None,
    }];
    let found = whole(&connection, Domain::Documents, &files, &by_entity);
    assert!(
        paths(&found).contains(&"courriers/2026-06-03_courrier-dermatologie.txt")
            && paths(&found).contains(&"2026-05-20_ordonnance.txt"),
        "{:?}",
        paths(&found)
    );
    assert!(!paths(&found).contains(&"reunions/compte-rendu-service.txt"));
    let prescription = found
        .files
        .iter()
        .find(|hit| hit.relative_path == "2026-05-20_ordonnance.txt")
        .expect("found");
    assert!(prescription
        .reasons
        .iter()
        .any(|reason| reason.kind == "entity" && reason.detail.as_deref() == Some("subject")));
    assert!(prescription
        .card
        .entities
        .iter()
        .any(|entity| entity.entity_id == Some(marie) && entity.role == "subject"));

    // A role is read from the profile only: she is the subject of the prescription (a `Patient :` label)
    // and of nothing else, the letter names her without a label.
    let mut as_subject = FinderQuery::default();
    as_subject.entities = vec![EntityFilter {
        entity_id: marie,
        role: Some("subject".to_string()),
    }];
    let subject = whole(&connection, Domain::Documents, &files, &as_subject);
    assert_eq!(paths(&subject), vec!["2026-05-20_ordonnance.txt"]);

    // Two entities narrow together: no file holds both her and the other person.
    let organisation = entity_id(&connection, "%universitaires%");
    let mut both = FinderQuery::default();
    both.entities = vec![
        EntityFilter {
            entity_id: marie,
            role: None,
        },
        EntityFilter {
            entity_id: organisation,
            role: None,
        },
    ];
    let both = whole(&connection, Domain::Documents, &files, &both);
    assert!(paths(&both)
        .iter()
        .all(|path| *path != "reunions/compte-rendu-service.txt"));
    assert!(both.total <= found.total);

    // The entity facet counts files, with the entity's name.
    let facet = found
        .facets
        .by_entity
        .iter()
        .find(|facet| facet.value == marie.to_string())
        .expect("the entity is in its own facet");
    assert_eq!(facet.count, found.total);
    assert!(facet
        .label
        .as_deref()
        .is_some_and(|label| label.contains("Martin")));
}

#[tokio::test]
async fn a_link_without_an_entity_is_a_surface_and_never_a_filter() {
    let lab = analysed_lab().await;
    let files = inventory(lab.work_path());
    let connection = lab.side();
    let marie = entity_id(&connection, "%Martin%");
    // The link of the prescription loses its entity: only the surface stays, as when the base forgets.
    connection
        .execute(
            "UPDATE file_profile_entities SET entity_id = NULL WHERE relative_path = '2026-05-20_ordonnance.txt'",
            [],
        )
        .expect("edits the profile");
    connection
        .execute("DELETE FROM kb_mentions WHERE source_id IN (SELECT source_id FROM kb_sources WHERE relative_path = '2026-05-20_ordonnance.txt')", [])
        .expect("edits the base");

    let mut as_subject = FinderQuery::default();
    as_subject.entities = vec![EntityFilter {
        entity_id: marie,
        role: Some("subject".to_string()),
    }];
    assert!(whole(&connection, Domain::Documents, &files, &as_subject)
        .files
        .is_empty());
    // The same surface is still a word of the profile.
    let by_word = whole(&connection, Domain::Documents, &files, &query("martin"));
    assert!(paths(&by_word).contains(&"2026-05-20_ordonnance.txt"));
}

// ---------------------------------------------------------------------------------------------
// Filters, facets, scope and coverage
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_filters_narrow_together_and_the_facets_count_what_is_left() {
    let lab = analysed_lab().await;
    let files = inventory(lab.work_path());
    let connection = lab.side();

    let everything = whole(
        &connection,
        Domain::Documents,
        &files,
        &FinderQuery::default(),
    );
    assert_eq!(everything.total, 4);
    let types: Vec<(&str, usize)> = everything
        .facets
        .by_type
        .iter()
        .map(|facet| (facet.value.as_str(), facet.count))
        .collect();
    assert!(
        types.contains(&("invoice", 1)) && types.contains(&("prescription", 1)),
        "{types:?}"
    );
    assert_eq!(
        everything
            .facets
            .by_format
            .iter()
            .map(|f| (f.value.as_str(), f.count))
            .collect::<Vec<_>>(),
        vec![("txt", 4)]
    );
    assert!(everything
        .facets
        .by_folder
        .iter()
        .any(|facet| facet.value == "courriers" && facet.count == 1));
    assert!(everything
        .facets
        .by_year
        .iter()
        .any(|facet| facet.value == "2026" && facet.count >= 3));

    let mut invoices = FinderQuery::default();
    invoices.kinds = vec!["invoice".to_string()];
    let found = whole(&connection, Domain::Documents, &files, &invoices);
    assert_eq!(
        paths(&found),
        vec!["2026-04-14_facture-medsupply-fac-2026-014.txt"]
    );
    assert_eq!(
        found.files[0].card.business_type.as_deref(),
        Some("invoice")
    );
    assert_eq!(
        found.files[0].card.document_date.as_deref(),
        Some("2026-04-14")
    );

    let mut april_to_may = FinderQuery::default();
    april_to_may.date = Some(DateRange {
        from: Some("2026-04".to_string()),
        to: Some("2026-05".to_string()),
    });
    let dated = whole(&connection, Domain::Documents, &files, &april_to_may);
    assert_eq!(
        paths(&dated),
        vec![
            "2026-04-14_facture-medsupply-fac-2026-014.txt",
            "2026-05-20_ordonnance.txt"
        ],
        "the end of a range written as a month includes the whole month"
    );

    let mut in_folder = FinderQuery::default();
    in_folder.folders = vec!["courriers".to_string()];
    assert_eq!(
        paths(&whole(&connection, Domain::Documents, &files, &in_folder)),
        vec!["courriers/2026-06-03_courrier-dermatologie.txt"]
    );

    let mut narrowed = query("martin");
    narrowed.kinds = vec!["prescription".to_string()];
    narrowed.formats = vec!["txt".to_string()];
    narrowed.states = vec!["analysed".to_string()];
    assert_eq!(
        paths(&whole(&connection, Domain::Documents, &files, &narrowed)),
        vec!["2026-05-20_ordonnance.txt"]
    );
    let mut wrong_format = query("martin");
    wrong_format.formats = vec!["pdf".to_string()];
    assert!(whole(&connection, Domain::Documents, &files, &wrong_format)
        .files
        .is_empty());
}

#[tokio::test]
async fn the_selection_scope_is_a_hard_limit_and_the_pages_of_a_result_add_up() {
    let lab = analysed_lab().await;
    let files = inventory(lab.work_path());
    let connection = lab.side();

    let selection = FinderScope::Selection(vec!["2026-05-20_ordonnance.txt".to_string()]);
    let inside = find(
        &connection,
        Domain::Documents,
        &files,
        &query("martin"),
        &selection,
    )
    .expect("the finder answers");
    assert_eq!(paths(&inside), vec!["2026-05-20_ordonnance.txt"]);
    let outside = find(
        &connection,
        Domain::Documents,
        &files,
        &query("reunion"),
        &selection,
    )
    .expect("the finder answers");
    assert!(
        outside.files.is_empty(),
        "nothing outside the selection is searched"
    );
    assert_eq!(outside.coverage.searched, 1, "the coverage is the scope's");
    let nothing = find(
        &connection,
        Domain::Documents,
        &files,
        &FinderQuery::default(),
        &FinderScope::Selection(Vec::new()),
    )
    .expect("the finder answers");
    assert_eq!(nothing.total, 0);

    let mut first = FinderQuery::default();
    first.limit = Some(3);
    let page_one = whole(&connection, Domain::Documents, &files, &first);
    first.offset = 3;
    let page_two = whole(&connection, Domain::Documents, &files, &first);
    assert_eq!(
        (page_one.total, page_two.total),
        (4, 4),
        "the total never shrinks"
    );
    assert_eq!((page_one.files.len(), page_two.files.len()), (3, 1));
}

#[tokio::test]
async fn the_coverage_counts_what_could_not_be_searched() {
    let mut lab = Lab::new();
    lab.write("a-lettre.txt", "Une lettre de test.\n");
    lab.write("b-vide.txt", "");
    lab.analyse().await;
    lab.write("c-nouveau.pdf", "x");
    lab.write("d-notes.pages", "x");
    let files = inventory(lab.work_path());
    let connection = lab.side();

    let result = whole(
        &connection,
        Domain::Documents,
        &files,
        &FinderQuery::default(),
    );
    assert_eq!(result.total, 4);
    let state_of = |path: &str| {
        result
            .files
            .iter()
            .find(|hit| hit.relative_path == path)
            .map(|hit| hit.card.state.clone())
            .expect("listed")
    };
    assert_eq!(state_of("a-lettre.txt"), "analysed");
    assert_eq!(state_of("b-vide.txt"), "empty");
    assert_eq!(state_of("c-nouveau.pdf"), "unanalysed");
    assert_eq!(state_of("d-notes.pages"), "unsupported");
    let coverage = &result.coverage;
    assert_eq!(
        (
            coverage.searched,
            coverage.unreadable,
            coverage.unanalysed,
            coverage.partial
        ),
        (1, 2, 1, 0)
    );
    assert!(result
        .facets
        .by_state
        .iter()
        .any(|facet| facet.value == "unanalysed" && facet.count == 1));

    // A workbook whose free-text column was cut is partial.
    connection
        .execute(
            "INSERT INTO file_profile_fields (domain, relative_path, kind, detail, value, source, position, chosen)
             VALUES ('documents', 'a-lettre.txt', 'identifiers_truncated', '', '200', 'text', 99, 0)",
            [],
        )
        .expect("marks the file");
    let partial = whole(
        &connection,
        Domain::Documents,
        &files,
        &FinderQuery::default(),
    );
    assert_eq!(partial.coverage.partial, 1);
}

// ---------------------------------------------------------------------------------------------
// Workbooks: cell words, and the spreadsheet tables are only read
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn a_workbook_is_found_by_a_word_of_its_cells_and_nothing_of_the_spreadsheet_tables_changes()
{
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write_data(
        "rendez-vous.csv",
        "Patient;Date du rendez-vous;Motif\nMarie Martin;2026-05-20;suivi dermatologie\nPaul Roy;2026-06-02;controle\n",
    );
    lab.analyse_data();
    let data = inventory(lab.data.path());
    let connection = lab.side();
    let before = tabular_snapshot(&connection);

    let by_cell = whole(&connection, Domain::Data, &data, &query("dermatologie"));
    assert_eq!(paths(&by_cell), vec!["rendez-vous.csv"]);
    assert!(by_cell.files[0]
        .reasons
        .iter()
        .any(|reason| reason.kind == "word" && reason.detail.as_deref() == Some("cells")));
    let by_header = whole(&connection, Domain::Data, &data, &query("motif"));
    assert_eq!(paths(&by_header), vec!["rendez-vous.csv"]);
    let mut july = FinderQuery::default();
    july.date = Some(DateRange {
        from: Some("2026-07".to_string()),
        to: None,
    });
    assert!(whole(&connection, Domain::Data, &data, &july)
        .files
        .is_empty());
    let mut june = FinderQuery::default();
    june.date = Some(DateRange {
        from: Some("2026-06".to_string()),
        to: Some("2026-06".to_string()),
    });
    assert_eq!(
        paths(&whole(&connection, Domain::Data, &data, &june)),
        vec!["rendez-vous.csv"],
        "a date column that spans the month overlaps it"
    );

    assert_eq!(
        before,
        tabular_snapshot(&connection),
        "the finder only reads the spreadsheet tables"
    );
}

// ---------------------------------------------------------------------------------------------
// What the left panel's field means: an extension is a format, a lone date is a date
// ---------------------------------------------------------------------------------------------

/// The query the `find_files` command builds from what was typed: the same function, over the same
/// inventory.
fn typed(files: &[InventoryFile], text: &str) -> FinderQuery {
    let mut asked = query(text);
    apply_typed_text(&mut asked, files);
    asked
}

fn found_by_typing(
    connection: &Connection,
    domain: Domain,
    files: &[InventoryFile],
    text: &str,
) -> Vec<String> {
    let mut found: Vec<String> = whole(connection, domain, files, &typed(files, text))
        .files
        .iter()
        .map(|hit| hit.relative_path.clone())
        .collect();
    found.sort();
    found
}

#[tokio::test]
async fn a_date_typed_finds_the_files_dated_so_before_and_after_an_analyse() {
    let mut lab = Lab::new();
    let workbook = "Produit;Quantite\nCompresses;4\nPansements;10\n";
    lab.write_data("2026-02-06_commandes.csv", workbook);
    lab.write_data("2026-02-20_stock.csv", workbook);
    lab.write_data("archive/2026-03-04_stock.csv", workbook);
    lab.write_data("2025-12-01_ancien.csv", workbook);
    lab.write_data("ventes.csv", workbook);
    let files = inventory(lab.data.path());

    let check = |connection: &Connection, when: &str| {
        let day = vec!["2026-02-06_commandes.csv".to_string()];
        for text in ["2026-02-06", "06/02/2026", "06-02-2026"] {
            assert_eq!(
                found_by_typing(connection, Data, &files, text),
                day,
                "{text:?} {when}"
            );
        }
        assert_eq!(
            found_by_typing(connection, Data, &files, "2026-02"),
            vec!["2026-02-06_commandes.csv", "2026-02-20_stock.csv"],
            "a month {when}"
        );
        assert_eq!(
            found_by_typing(connection, Data, &files, "2026"),
            vec![
                "2026-02-06_commandes.csv",
                "2026-02-20_stock.csv",
                "archive/2026-03-04_stock.csv"
            ],
            "a year {when}"
        );
        let hit = &whole(connection, Data, &files, &typed(&files, "2026-02-06")).files[0];
        assert!(
            hit.reasons
                .iter()
                .any(|reason| reason.kind == "date" && reason.detail.as_deref() == Some("name")),
            "the hit says it was found by its date {when}"
        );
        assert_eq!(
            found_by_typing(connection, Data, &files, "2027"),
            Vec::<String>::new(),
            "a year nobody carries {when}"
        );
        assert_eq!(
            found_by_typing(connection, Data, &files, "2026-02-30"),
            Vec::<String>::new(),
            "an impossible day is only words {when}"
        );
    };
    check(&lab.side(), "before the Analyse");
    lab.analyse_data();
    check(&lab.side(), "after the Analyse");
}

#[tokio::test]
async fn a_year_typed_is_a_date_and_a_word_but_never_a_passage_search() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    lab.write("2026-05-20_ordonnance.txt", PRESCRIPTION);
    lab.write("2026-04-14_facture-medsupply-fac-2026-014.txt", INVOICE);
    lab.write("facture-fac-2026-015.txt", &INVOICE.replace("014", "015"));
    lab.write(
        "note-interne.txt",
        "Reunion de service.\nLe planning de 2026 a ete adopte.\nLe fichier pdf est joint.\n",
    );
    lab.analyse().await;
    let files = inventory(lab.work_path());
    let connection = lab.side();

    // The dated files are found, by their name or by the date in their profile.
    let year = whole(
        &connection,
        Domain::Documents,
        &files,
        &typed(&files, "2026"),
    );
    let mut found = paths(&year);
    found.sort();
    for dated in [
        "2026-04-14_facture-medsupply-fac-2026-014.txt",
        "2026-05-20_ordonnance.txt",
        "facture-fac-2026-015.txt",
    ] {
        assert!(found.contains(&dated), "{dated} is dated 2026: {found:?}");
    }
    // The passages are not searched for a year: no hit rests on the text of a passage.
    assert!(year.files.iter().all(|hit| hit
        .reasons
        .iter()
        .all(|reason| reason.detail.as_deref() != Some("passage"))));
    // An identifier typed in full is words, not a date.
    assert_eq!(
        found_by_typing(&connection, Domain::Documents, &files, "FAC-2026-014"),
        vec!["2026-04-14_facture-medsupply-fac-2026-014.txt"]
    );
    // The same word typed beside another word still reaches the passages.
    assert_eq!(
        found_by_typing(&connection, Domain::Documents, &files, "planning 2026"),
        vec!["note-interne.txt"]
    );
}

#[tokio::test]
async fn an_extension_typed_is_a_format_and_never_a_word_of_the_text() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    lab.write("2026-05-20_ordonnance.txt", PRESCRIPTION);
    lab.write("rapport-mensuel.txt", MINUTES);
    lab.write("rapport-annuel.md", MINUTES);
    lab.write("courriers/note-interne.txt", "Le fichier pdf est joint.\n");
    lab.analyse().await;
    let files = inventory(lab.work_path());
    let connection = lab.side();
    let docs = Domain::Documents;

    let every_txt = vec![
        "2026-05-20_ordonnance.txt",
        "courriers/note-interne.txt",
        "rapport-mensuel.txt",
    ];
    for text in [".txt", "txt", ".TXT", "Txt"] {
        assert_eq!(
            found_by_typing(&connection, docs, &files, text),
            every_txt,
            "{text:?}"
        );
    }
    assert_eq!(
        found_by_typing(&connection, docs, &files, "rapport .md"),
        vec!["rapport-annuel.md"]
    );
    assert_eq!(
        found_by_typing(&connection, docs, &files, "rapport"),
        vec!["rapport-annuel.md", "rapport-mensuel.txt"]
    );
    // `pdf` is the format, not the word of a note's text: no file here is a pdf.
    assert_eq!(
        found_by_typing(&connection, docs, &files, "pdf"),
        Vec::<String>::new()
    );
    assert_eq!(
        found_by_typing(&connection, docs, &files, ".pdf"),
        Vec::<String>::new()
    );
    // Beside another word it is still the format: the word `fichier` among the pdf files, none here.
    assert_eq!(
        found_by_typing(&connection, docs, &files, "fichier pdf"),
        Vec::<String>::new()
    );
    // A filter-only query lists every file of the format, whatever their state.
    let listed = whole(&connection, docs, &files, &typed(&files, ".md"));
    assert_eq!(listed.total, 1);
    assert!(listed.text_search.is_none());
}

#[tokio::test]
async fn punctuation_alone_finds_nothing_and_a_name_with_its_extension_still_finds_the_file() {
    let lab = Lab::new();
    lab.write("2026-02-06_rapport.docx", "x");
    lab.write("coeur.pdf", "x");
    let files = inventory(lab.work_path());
    let connection = lab.side();
    let docs = Domain::Documents;

    for text in ["!!", ".", "...", "-", "?"] {
        assert_eq!(
            found_by_typing(&connection, docs, &files, text),
            Vec::<String>::new(),
            "{text:?}"
        );
    }
    assert_eq!(
        found_by_typing(&connection, docs, &files, "rapport.docx"),
        vec!["2026-02-06_rapport.docx"]
    );
    assert_eq!(
        found_by_typing(&connection, docs, &files, "c\u{153}ur"),
        vec!["coeur.pdf"]
    );
}

#[tokio::test]
async fn the_date_filter_reads_the_name_of_a_file_that_was_never_analysed() {
    let lab = Lab::new();
    lab.write("2026-04-10_courrier.pdf", "x");
    lab.write("sans-date.pdf", "x");
    let files = inventory(lab.work_path());
    let connection = lab.side();

    let mut april = FinderQuery::default();
    april.date = Some(DateRange {
        from: Some("2026-04".to_string()),
        to: Some("2026-04".to_string()),
    });
    assert_eq!(
        paths(&whole(&connection, Domain::Documents, &files, &april)),
        vec!["2026-04-10_courrier.pdf"]
    );
}

// ---------------------------------------------------------------------------------------------
// The contract with the webview
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_webviews_json_is_read_and_a_paths_only_query_reads_nothing_beyond_the_files() {
    let query: FinderQuery = serde_json::from_str(
        r#"{"text":"lesion","pathsOnly":true,"limit":100000,
            "entities":[{"entityId":3,"role":"subject"},{"entityId":4}],
            "date":{"from":"2026-01"},"kinds":["letter"],"formats":["pdf"],"states":["analysed"]}"#,
    )
    .expect("the TypeScript query is read");
    assert!(query.paths_only);
    assert_eq!(query.entities[0].role.as_deref(), Some("subject"));
    assert_eq!(query.entities[1].role, None);
    assert_eq!(
        query.date.as_ref().and_then(|d| d.from.as_deref()),
        Some("2026-01")
    );

    let whole: FinderScope = serde_json::from_str(r#"{"kind":"wholeFolder"}"#).expect("whole");
    assert_eq!(whole, FinderScope::WholeFolder);
    let selection: FinderScope =
        serde_json::from_str(r#"{"kind":"selection","paths":["a.pdf","b/c.pdf"]}"#).expect("some");
    assert_eq!(
        selection,
        FinderScope::Selection(vec!["a.pdf".to_string(), "b/c.pdf".to_string()])
    );
    let domain: Domain = serde_json::from_str(r#""documents""#).expect("domain");
    assert_eq!(domain, Domain::Documents);

    let lab = analysed_lab().await;
    let files = inventory(lab.work_path());
    let connection = lab.side();
    let full = whole_result(&connection, &files, "lesion", false);
    let light = whole_result(&connection, &files, "lesion", true);
    assert_eq!(paths(&full), paths(&light));
    assert!(full.files[0].excerpt.is_some() && full.files[0].locator.is_some());
    assert!(light.files[0].excerpt.is_none() && light.files[0].locator.is_none());
    assert!(light.files[0].card.entities.is_empty());
    assert_eq!(light.total, full.total);

    // What the webview receives uses camelCase keys.
    let json = serde_json::to_value(&full).expect("serialises");
    assert!(json["files"][0]["relativePath"].is_string());
    assert!(
        json["files"][0]["card"]["businessType"].is_null()
            || json["files"][0]["card"]["businessType"].is_string()
    );
    assert!(json["facets"]["byType"].is_array());
    assert!(json["coverage"]["unanalysed"].is_number());
    assert_eq!(json["textSearch"]["terms"][0], "lesion");
}

fn whole_result(
    connection: &Connection,
    files: &[InventoryFile],
    text: &str,
    paths_only: bool,
) -> FinderResult {
    let mut asked = query(text);
    asked.paths_only = paths_only;
    whole(connection, Domain::Documents, files, &asked)
}
