//! Explorer step 8B: any two folders, never a duplicate (`docs/DECISIONS.md`, "any two folders, never a
//! duplicate"; `docs/SESSION-EXPLORER/EXPLORER-08B-same-or-nested-folders.md`).
//!
//! The Documents folder and the Data folder may be separate, the same folder, tables inside documents or
//! documents inside tables. In each layout one physical file is read once, stored under one key, listed once
//! and cited once: the rows of the index, the vectors the gateway computes and the lists of the finder and
//! of the family views are counted against the files **walked on the disk by this test**, never against the
//! code under test.
//!
//! These tests start a fake gateway on a local socket (the document pass asks for vectors): run them one at a
//! time, on their own, as `docs/TROUBLESHOOTING.md` says (`cargo test --test explorer_nested_folders -- --test-threads=1`).

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use assistant_cabinet_ai_lib::analysis_scope::AnalysisScope;
use assistant_cabinet_ai_lib::data_folder;
use assistant_cabinet_ai_lib::discovery::{discover_all, discover_family};
use assistant_cabinet_ai_lib::family_view::{family_views, FamilyViews};
use assistant_cabinet_ai_lib::file_key::Root;
use assistant_cabinet_ai_lib::file_key::{physical_identity, FamilyRoots};
use assistant_cabinet_ai_lib::file_profile::build::Domain;
use assistant_cabinet_ai_lib::file_record::{FileKind, ProcessingStatus};
use assistant_cabinet_ai_lib::finder::{find, FinderQuery, FinderScope, InventoryFile};
use assistant_cabinet_ai_lib::indexing::{self, IndexSummary};
use assistant_cabinet_ai_lib::inventory::{FileHashCache, WorkFolderInventory};
use common::knowledge::{gateway_client, tabular_snapshot, Lab};
use rusqlite::Connection;

/// The two folders of a layout.
struct Layout {
    name: &'static str,
    documents: PathBuf,
    data: PathBuf,
}

fn layouts(root: &Path) -> Vec<Layout> {
    vec![
        Layout {
            name: "separate",
            documents: root.join("separate").join("Docs"),
            data: root.join("separate").join("Data"),
        },
        Layout {
            name: "same folder",
            documents: root.join("same").join("Both"),
            data: root.join("same").join("Both"),
        },
        Layout {
            name: "tables inside documents",
            documents: root.join("tables-in-docs").join("Docs"),
            data: root.join("tables-in-docs").join("Docs").join("Tables"),
        },
        Layout {
            name: "documents inside tables",
            documents: root.join("docs-in-tables").join("Data").join("Docs"),
            data: root.join("docs-in-tables").join("Data"),
        },
    ]
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

/// A passage no other file shares, so two files are never one by content.
fn text_of(tag: &str) -> String {
    format!("Passage of the file {tag}. It says something only the file {tag} says, about the topic {tag}.")
}

/// A workbook no other file shares.
fn table_of(tag: &str) -> String {
    format!(
        "date,fournisseur,montant\n2026-01-05,Alpha,100\n2026-02-01,Beta,150\n2026-03-02,Gamma,300\n2026-03-10,{tag},400\n"
    )
}

/// One document and one workbook at the top of each folder, one of each in a sub-folder, and the same
/// name in a sub-folder of both folders. In the layouts where the folders are one or nested, some of
/// these are the same file or sit inside the other folder: the test never counts what it wrote.
fn fill(layout: &Layout, root: &Path) {
    for (folder, who) in [(&layout.documents, "docs"), (&layout.data, "data")] {
        let tag = |name: &str| {
            let absolute = folder.join(name);
            absolute
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace(['\\', '/'], "-")
        };
        write(
            &folder.join(format!("lettre-{who}.txt")),
            &text_of(&tag(&format!("lettre-{who}"))),
        );
        write(
            &folder.join(format!("suivi-{who}.csv")),
            &table_of(&tag(&format!("suivi-{who}"))),
        );
        write(
            &folder.join("sub").join("note.txt"),
            &text_of(&tag("sub-note")),
        );
        write(
            &folder.join("sub").join("stock.csv"),
            &table_of(&tag("sub-stock")),
        );
    }
}

/// Every file under either folder, walked here: the physical files, each once, whatever the layout.
fn physical_files(layout: &Layout) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut documents = BTreeSet::new();
    let mut tables = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for folder in [&layout.documents, &layout.data] {
        for file in discover_all(folder) {
            let absolute = PathBuf::from(&file.absolute_path);
            if !seen.insert(physical_identity(&absolute)) {
                continue;
            }
            match file.extension.as_str() {
                "txt" | "md" => documents.insert(file.absolute_path),
                "csv" => tables.insert(file.absolute_path),
                _ => false,
            };
        }
    }
    (documents, tables)
}

fn column(path: &Path, sql: &str) -> Vec<String> {
    let connection = Connection::open(path).unwrap();
    let mut statement = connection.prepare(sql).unwrap();
    let found = statement
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    found
}

fn count(path: &Path, sql: &str) -> usize {
    let connection = Connection::open(path).unwrap();
    connection
        .query_row(sql, [], |row| row.get::<_, i64>(0))
        .unwrap() as usize
}

async fn document_pass(lab: &mut Lab, layout: &Layout) -> IndexSummary {
    let mut context = lab.context();
    let url = lab.gateway.url.clone();
    indexing::run_cancellable(
        &mut lab.index,
        &gateway_client(),
        &url,
        "cabinet-embed",
        &layout.documents,
        Some(layout.data.as_path()),
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

fn tabular_pass(lab: &mut Lab, layout: &Layout) -> IndexSummary {
    let mut context = lab.context();
    data_folder::analyse_cancellable(
        &layout.data,
        Some(layout.documents.as_path()),
        &mut lab.index,
        "en-US",
        &|_| {},
        Some(&mut context),
        &|| false,
    )
    .expect("the tabular pass completes")
}

fn views(lab: &Lab, layout: &Layout) -> FamilyViews {
    family_views(
        Some(&layout.documents),
        Some(&layout.data),
        Some(&lab.index),
        &FileHashCache::new(),
        &FileHashCache::new(),
    )
    .unwrap()
}

/// The inputs the gateway was asked to embed, over every request.
fn vectors_asked(lab: &Lab) -> usize {
    lab.gateway
        .requests()
        .iter()
        .map(|request| request.inputs.len())
        .sum()
}

/// The physical files a list of keys of `family` stands for, by the one resolver, each checked to exist.
fn files_of_keys(layout: &Layout, family: Domain, keys: &[String]) -> Vec<String> {
    let (own, other) = match family {
        Domain::Documents => (&layout.documents, &layout.data),
        Domain::Data => (&layout.data, &layout.documents),
    };
    let roots = FamilyRoots::new(family, own, Some(other));
    keys.iter()
        .map(|key| {
            let path = roots
                .resolve(key)
                .unwrap_or_else(|| panic!("the key {key} resolves to a file"));
            assert!(path.is_file(), "{key} names a file that exists");
            physical_identity(&path)
        })
        .collect()
}

/// A list holds each physical file once and exactly the files expected.
fn assert_once(label: &str, found: Vec<String>, expected: &BTreeSet<String>) {
    let unique: BTreeSet<&String> = found.iter().collect();
    assert_eq!(
        unique.len(),
        found.len(),
        "{label}: a file is listed twice: {found:?}"
    );
    assert_eq!(
        found.len(),
        expected.len(),
        "{label}: one entry per physical file"
    );
}

/// Everything the index holds, counted against the physical files.
fn assert_one_row_per_physical_file(lab: &Lab, layout: &Layout, before_vectors: usize) {
    let (documents, tables) = physical_files(layout);
    let index = lab.index_path();
    let label = layout.name;
    let expected_documents: BTreeSet<String> = documents
        .iter()
        .map(|p| physical_identity(Path::new(p)))
        .collect();
    let expected_tables: BTreeSet<String> = tables
        .iter()
        .map(|p| physical_identity(Path::new(p)))
        .collect();

    let document_keys = column(&index, "SELECT relative_path FROM documents");
    assert_once(
        &format!("{label}: documents"),
        files_of_keys(layout, Domain::Documents, &document_keys),
        &expected_documents,
    );
    let table_keys = column(&index, "SELECT relative_path FROM tabular_inventories");
    assert_once(
        &format!("{label}: tabular_inventories"),
        files_of_keys(layout, Domain::Data, &table_keys),
        &expected_tables,
    );

    // Vectors: the gateway is asked for each passage once, and once for the one-line description the pass
    // makes of each document (`Title: ...`). Every text here is unique to its file, so any text asked twice
    // would be a file read twice.
    let asked: Vec<String> = lab
        .gateway
        .requests()
        .into_iter()
        .flat_map(|request| request.inputs)
        .collect();
    let distinct: BTreeSet<&String> = asked.iter().collect();
    assert_eq!(
        distinct.len(),
        asked.len(),
        "{label}: a text was sent to the gateway twice"
    );
    let chunk_files = column(&index, "SELECT DISTINCT relative_path FROM chunks");
    assert_eq!(
        chunk_files.len(),
        documents.len(),
        "{label}: every document has passages, once"
    );
    assert_eq!(
        asked.len() - before_vectors,
        count(&index, "SELECT COUNT(*) FROM chunks") + documents.len(),
        "{label}: one vector per passage and one per document description, no more"
    );

    // The profiles and the knowledge base, per family.
    for (domain, expected) in [
        ("documents", &expected_documents),
        ("data", &expected_tables),
    ] {
        let family = if domain == "documents" {
            Domain::Documents
        } else {
            Domain::Data
        };
        let profiles = column(
            &index,
            &format!("SELECT relative_path FROM file_profiles WHERE domain = '{domain}'"),
        );
        assert_once(
            &format!("{label}: file_profiles {domain}"),
            files_of_keys(layout, family, &profiles),
            expected,
        );
        let sources = column(
            &index,
            &format!("SELECT relative_path FROM kb_sources WHERE domain = '{domain}'"),
        );
        let resolved = files_of_keys(layout, family, &sources);
        let unique: BTreeSet<&String> = resolved.iter().collect();
        assert_eq!(
            unique.len(),
            resolved.len(),
            "{label}: kb_sources {domain} holds a file twice"
        );
        assert!(
            resolved.len() <= expected.len(),
            "{label}: kb_sources {domain}"
        );
    }

    // The same folder has no foreign key anywhere, and no layout writes a second key for a file.
    if layout.documents == layout.data {
        for sql in [
            "SELECT relative_path FROM documents",
            "SELECT relative_path FROM chunks",
            "SELECT relative_path FROM tabular_inventories",
            "SELECT relative_path FROM file_profiles",
            "SELECT relative_path FROM kb_sources",
        ] {
            let leading: Vec<String> = column(&index, sql)
                .into_iter()
                .filter(|key| key.starts_with('/'))
                .collect();
            assert!(
                leading.is_empty(),
                "{label}: a foreign key exists in the same folder: {leading:?}"
            );
        }
    }
}

/// The finder's list, the family views and a scope's resolution each hold a physical file once.
fn assert_listed_once(lab: &Lab, layout: &Layout) {
    let (documents, tables) = physical_files(layout);
    let label = layout.name;

    for (family, expected) in [(Domain::Documents, &documents), (Domain::Data, &tables)] {
        let (own, other) = match family {
            Domain::Documents => (&layout.documents, &layout.data),
            Domain::Data => (&layout.data, &layout.documents),
        };
        let listed = discover_family(own, Some(other), family);
        let keys: Vec<String> = listed
            .iter()
            .map(|file| file.relative_path.clone())
            .collect();
        let expected_identities: BTreeSet<String> = expected
            .iter()
            .map(|p| physical_identity(Path::new(p)))
            .collect();
        assert_once(
            &format!("{label}: discover_family {family:?}"),
            files_of_keys(layout, family, &keys),
            &expected_identities,
        );

        // The finder answers from that list: every hit is one file.
        let inventory: Vec<InventoryFile> = listed.iter().map(InventoryFile::from).collect();
        let query = FinderQuery {
            limit: Some(10_000),
            ..FinderQuery::default()
        };
        let result = find(
            &lab.side(),
            family,
            &inventory,
            &query,
            &FinderScope::WholeFolder,
        )
        .expect("the finder answers");
        let hits: Vec<String> = result
            .files
            .iter()
            .map(|hit| hit.relative_path.clone())
            .collect();
        let unique: BTreeSet<&String> = hits.iter().collect();
        assert_eq!(
            unique.len(),
            hits.len(),
            "{label}: the finder lists a file twice"
        );
        assert_eq!(
            hits.len(),
            expected.len(),
            "{label}: the finder lists each {family:?} file"
        );
    }

    let seen = views(lab, layout);
    let documents_view = seen.documents.expect("the document family exists");
    let documents_in_view: Vec<String> = documents_view
        .all_files()
        .iter()
        .filter(|record| record.kind.is_document())
        .map(|record| record.relative_path.clone())
        .collect();
    let expected_documents: BTreeSet<String> = documents
        .iter()
        .map(|p| physical_identity(Path::new(p)))
        .collect();
    assert_once(
        &format!("{label}: the document family view"),
        files_of_keys(layout, Domain::Documents, &documents_in_view),
        &expected_documents,
    );
    // The whole-folder scope resolves to the same list.
    let resolution = AnalysisScope::whole_folder(1).resolve(&documents_view);
    let resolved: Vec<String> = resolution
        .inventory
        .all_files()
        .iter()
        .filter(|record| record.kind.is_document())
        .map(|record| record.relative_path.clone())
        .collect();
    assert_eq!(
        resolved.len(),
        documents.len(),
        "{label}: the scope resolves each document once"
    );

    let data_view = seen.data.expect("the tabular family exists");
    let tables_in_view: Vec<String> = data_view
        .files()
        .all_files()
        .iter()
        .filter(|record| record.kind == FileKind::TabularCandidate)
        .map(|record| record.relative_path.clone())
        .collect();
    let expected_tables: BTreeSet<String> = tables
        .iter()
        .map(|p| physical_identity(Path::new(p)))
        .collect();
    assert_once(
        &format!("{label}: the tabular family view"),
        files_of_keys(layout, Domain::Data, &tables_in_view),
        &expected_tables,
    );
}

/// The folder's own listing and the other folder's give a file the same key, hence the same state.
fn assert_both_listings_agree(lab: &Lab, layout: &Layout) {
    let cache = FileHashCache::new();
    let (documents, _) = WorkFolderInventory::discover_in(
        Root::Documents,
        &layout.documents,
        Some(&layout.data),
        Some(&lab.index),
        &cache,
    )
    .unwrap();
    let (data, _) = WorkFolderInventory::discover_in(
        Root::Data,
        &layout.data,
        Some(&layout.documents),
        Some(&lab.index),
        &cache,
    )
    .unwrap();
    let states = |listing: &WorkFolderInventory| -> BTreeMap<String, ProcessingStatus> {
        listing
            .all_files()
            .iter()
            .filter(|record| record.kind.is_document() || record.kind == FileKind::TabularCandidate)
            .map(|record| {
                let absolute = record
                    .relative_path
                    .split('/')
                    .fold(listing.root().to_path_buf(), |path, part| path.join(part));
                (physical_identity(&absolute), record.processing_status)
            })
            .collect()
    };
    let (from_documents, from_data) = (states(&documents), states(&data));
    let mut shared = 0;
    for (identity, state) in &from_documents {
        if let Some(other) = from_data.get(identity) {
            shared += 1;
            assert_eq!(
                state, other,
                "{}: one file, one state in both listings",
                layout.name
            );
        }
    }
    let overlapping = layout.name != "separate";
    assert_eq!(
        shared > 0,
        overlapping,
        "{}: the listings share files exactly when the folders do",
        layout.name
    );
}

#[tokio::test]
async fn in_every_layout_one_physical_file_is_one_row_one_vector_and_one_entry() {
    let base = tempfile::tempdir().unwrap();
    for layout in layouts(base.path()) {
        let mut lab = Lab::new();
        fill(&layout, base.path());

        document_pass(&mut lab, &layout).await;
        tabular_pass(&mut lab, &layout);
        assert_one_row_per_physical_file(&lab, &layout, 0);
        assert_listed_once(&lab, &layout);
        assert_both_listings_agree(&lab, &layout);
    }
}

#[tokio::test]
async fn a_second_pass_changes_nothing_and_a_removed_file_loses_its_one_row() {
    let base = tempfile::tempdir().unwrap();
    for layout in layouts(base.path()) {
        let mut lab = Lab::new();
        fill(&layout, base.path());
        document_pass(&mut lab, &layout).await;
        tabular_pass(&mut lab, &layout);
        let (documents, tables) = physical_files(&layout);
        let index = lab.index_path();
        let tabular_before = tabular_snapshot(&lab.side());
        let document_rows = column(
            &index,
            "SELECT relative_path || '|' || sha256 FROM documents ORDER BY 1",
        );
        let asked = vectors_asked(&lab);

        // Nothing new to read: every file unchanged, no passage sent again.
        let again = document_pass(&mut lab, &layout).await;
        assert_eq!(again.scanned_files, documents.len(), "{}", layout.name);
        assert_eq!(
            again.unchanged_files,
            documents.len(),
            "{}: every document is unchanged",
            layout.name
        );
        assert_eq!(again.indexed_files, 0, "{}", layout.name);
        assert_eq!(
            vectors_asked(&lab),
            asked,
            "{}: no vector computed again",
            layout.name
        );
        assert_eq!(
            column(
                &index,
                "SELECT relative_path || '|' || sha256 FROM documents ORDER BY 1"
            ),
            document_rows,
            "{}",
            layout.name
        );
        let again = tabular_pass(&mut lab, &layout);
        assert_eq!(again.scanned_files, tables.len(), "{}", layout.name);
        assert_eq!(
            tabular_snapshot(&lab.side()),
            tabular_before,
            "{}: the tabular tables are the same",
            layout.name
        );

        // Removing one document and one workbook removes exactly one row of each.
        let gone_document = PathBuf::from(documents.iter().next().unwrap());
        let gone_table = PathBuf::from(tables.iter().next().unwrap());
        std::fs::remove_file(&gone_document).unwrap();
        std::fs::remove_file(&gone_table).unwrap();
        let summary = document_pass(&mut lab, &layout).await;
        assert_eq!(
            summary.removed_files.len(),
            1,
            "{}: one document forgotten",
            layout.name
        );
        let summary = tabular_pass(&mut lab, &layout);
        assert_eq!(
            summary.removed_files.len(),
            1,
            "{}: one workbook forgotten",
            layout.name
        );
        assert_eq!(
            count(&index, "SELECT COUNT(*) FROM documents"),
            documents.len() - 1,
            "{}",
            layout.name
        );
        assert_eq!(
            count(&index, "SELECT COUNT(*) FROM tabular_inventories"),
            tables.len() - 1,
            "{}",
            layout.name
        );
        assert_eq!(
            count(
                &index,
                "SELECT COUNT(*) FROM file_profiles WHERE domain = 'documents'"
            ),
            documents.len() - 1,
            "{}",
            layout.name
        );
    }
}

#[tokio::test]
async fn the_same_folder_gives_the_tabular_snapshot_of_a_plain_pass() {
    // No foreign file exists when both folders are one: the pass is the plain pass over that folder.
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("Both");
    let layout = Layout {
        name: "same folder",
        documents: folder.clone(),
        data: folder.clone(),
    };
    fill(&layout, root.path());

    let mut across = Lab::new();
    tabular_pass(&mut across, &layout);
    let mut plain = Lab::new();
    data_folder::analyse_cancellable(
        &folder,
        None,
        &mut plain.index,
        "en-US",
        &|_| {},
        None,
        &|| false,
    )
    .unwrap();

    // The knowledge base took part in one pass and not in the other: compare the engine's own tables.
    let engine = |lab: &Lab| {
        let mut snapshot = tabular_snapshot(&lab.side());
        snapshot.remove("kb_column_semantics");
        snapshot
    };
    assert_eq!(engine(&across), engine(&plain));
}

#[tokio::test]
async fn changing_the_folders_forgets_the_old_keys_and_reads_the_files_natively() {
    let root = tempfile::tempdir().unwrap();
    let separate = Layout {
        name: "separate",
        documents: root.path().join("Docs"),
        data: root.path().join("Data"),
    };
    fill(&separate, root.path());
    let mut lab = Lab::new();
    document_pass(&mut lab, &separate).await;
    tabular_pass(&mut lab, &separate);
    let index = lab.index_path();
    assert!(
        column(&index, "SELECT relative_path FROM tabular_inventories")
            .iter()
            .any(|key| key.starts_with("/documents/")),
        "the workbooks of the Documents folder were read under a foreign key"
    );

    // The Data folder becomes the Documents folder itself.
    let same = Layout {
        name: "same folder",
        documents: separate.documents.clone(),
        data: separate.documents.clone(),
    };
    document_pass(&mut lab, &same).await;
    tabular_pass(&mut lab, &same);

    let (documents, tables) = physical_files(&same);
    for sql in [
        "SELECT relative_path FROM documents",
        "SELECT relative_path FROM tabular_inventories",
        "SELECT relative_path FROM file_profiles",
        "SELECT relative_path FROM kb_sources",
    ] {
        let stale: Vec<String> = column(&index, sql)
            .into_iter()
            .filter(|key| key.starts_with('/'))
            .collect();
        assert!(
            stale.is_empty(),
            "nothing is left under a stale key: {stale:?}"
        );
    }
    assert_eq!(
        count(&index, "SELECT COUNT(*) FROM documents"),
        documents.len()
    );
    assert_eq!(
        count(&index, "SELECT COUNT(*) FROM tabular_inventories"),
        tables.len()
    );
}

#[test]
fn a_path_under_neither_folder_and_a_way_out_of_a_folder_are_not_read() {
    let root = tempfile::tempdir().unwrap();
    let layout = layouts(root.path()).remove(2);
    fill(&layout, root.path());
    write(&root.path().join("elsewhere").join("stray.txt"), "stray");
    write(
        &root
            .path()
            .join("tables-in-docs")
            .join("Docs2")
            .join("near.txt"),
        "near",
    );

    let keys: Vec<String> =
        discover_family(&layout.documents, Some(&layout.data), Domain::Documents)
            .into_iter()
            .map(|file| file.relative_path)
            .collect();
    assert!(keys
        .iter()
        .all(|key| !key.contains("stray") && !key.contains("near")));

    let roots = FamilyRoots::new(Domain::Documents, &layout.documents, Some(&layout.data));
    assert_eq!(roots.resolve("../Docs2/near.txt"), None);
    assert_eq!(roots.resolve("/data/../../elsewhere/stray.txt"), None);
    assert_eq!(
        roots.key_of(&root.path().join("elsewhere").join("stray.txt")),
        None
    );
    // A folder whose name only starts like the Documents folder is not inside it.
    assert_eq!(
        roots.key_of(
            &root
                .path()
                .join("tables-in-docs")
                .join("Docs2")
                .join("near.txt")
        ),
        None
    );
}

#[cfg(windows)]
#[tokio::test]
async fn a_folder_that_differs_only_by_case_is_the_same_folder() {
    let root = tempfile::tempdir().unwrap();
    let proper = layouts(root.path()).remove(2);
    fill(&proper, root.path());
    // The Tables folder typed in lower case: the same folder on Windows.
    let lowered = Layout {
        name: "tables inside documents, lower case",
        documents: proper.documents.clone(),
        data: PathBuf::from(proper.data.to_string_lossy().to_lowercase()),
    };

    let mut lab = Lab::new();
    document_pass(&mut lab, &lowered).await;
    tabular_pass(&mut lab, &lowered);

    let (documents, tables) = physical_files(&proper);
    assert_eq!(
        count(&lab.index_path(), "SELECT COUNT(*) FROM documents"),
        documents.len()
    );
    assert_eq!(
        count(
            &lab.index_path(),
            "SELECT COUNT(*) FROM tabular_inventories"
        ),
        tables.len()
    );
    assert_eq!(
        count(&lab.index_path(), "SELECT COUNT(*) FROM chunks") + documents.len(),
        vectors_asked(&lab),
        "each passage and each description was sent once"
    );
}
