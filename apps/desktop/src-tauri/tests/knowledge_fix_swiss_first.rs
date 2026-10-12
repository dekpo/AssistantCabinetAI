//! KB fix 4, batch 1 (`fix/kb-swiss-first`): what a Swiss practice meets on day one, and the cheap, harmful
//! defects of the register. Every test here was written first and seen failing, then the fix was made.
//!
//! - F-02 (KBD-26): `Dre`, `Dresse` and their kin are titles, not the first word of a name.
//! - F-03 (KBD-29): a commune written in a table never narrows the documents.
//! - F-14 (KBD-04): the name of a workbook in a question is a reference, not date words.
//! - F-16 (KBD-18, KBD-35): an AVS number, a policy number, an e-mail address or an acronym is not a file.
//! - F-24 (KBD-38): `CHF 1'450.00` is a number to the table engine.
//! - F-01 (KBD-16): the search of one document's chunks is an indexed lookup.
//!
//! Every name here is invented. The sandbox figures of the batch are in the batch report.

mod common;

use assistant_cabinet_ai_lib::analysis_scope::ScopeMode;
use assistant_cabinet_ai_lib::folder_questions::{self, FolderAnswer, QuestionRoute};
use assistant_cabinet_ai_lib::inventory::WorkFolderInventory;
use assistant_cabinet_ai_lib::knowledge::reduce::SideInput;
use assistant_cabinet_ai_lib::knowledge::targeting::TargetingRequest;
use assistant_cabinet_ai_lib::knowledge::KnowledgeMode;
use assistant_cabinet_ai_lib::tabular::engine::TabularValue;
use assistant_cabinet_ai_lib::tabular::structural::StructuralAnswer;
use assistant_cabinet_ai_lib::tabular_answer::{self, PendingAnswer, TabularAnswer};

use common::corpus::{clean, doc, filler, plan_with, world};
use common::knowledge::{entities, entity_named, small_world_policy, Lab};

fn swiss_world(files: Vec<(String, String)>) -> Lab {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    for (name, text) in &files {
        lab.write(name, text);
    }
    lab
}

// ---------------------------------------------------------------------------------------- F-02

/// Ten files: the Swiss doctors in four of them, six fillers.
fn dre_library() -> Vec<(String, String)> {
    let mut files = vec![
        (
            "courrier-endocrinologie.txt".to_string(),
            doc(
                1,
                "Courrier de la Dre Anne-Sophie Vuagniaux au sujet du suivi du diabete.",
            ),
        ),
        (
            "rapport-endocrinologie.txt".to_string(),
            doc(
                2,
                "Rapport signe par la Dre Anne-Sophie Vuagniaux, endocrinologue au CHUV.",
            ),
        ),
        (
            "courrier-pediatrie.txt".to_string(),
            doc(3, "Courrier de la Dre Sandra Pasquier, pediatre, a Geneve."),
        ),
        (
            "compte-rendu-pediatrie.txt".to_string(),
            doc(
                4,
                "Compte rendu de la Dresse Sandra Pasquier pour le dossier de l'enfant.",
            ),
        ),
    ];
    files.extend((1..=6).map(filler));
    files
}

#[tokio::test]
async fn a_swiss_woman_doctor_is_found_under_her_own_name() {
    let mut lab = swiss_world(dre_library());
    lab.analyse().await;
    let connection = lab.side();
    let all = entities(&connection);
    let names: Vec<&str> = all.iter().map(|row| row.canonical.as_str()).collect();
    for wanted in ["Anne-Sophie Vuagniaux", "Sandra Pasquier"] {
        let row = entity_named(&connection, wanted)
            .unwrap_or_else(|| panic!("no entity named {wanted:?} among {names:?}"));
        assert_eq!(row.type_id, "person", "{wanted}");
    }
    assert!(
        names
            .iter()
            .all(|name| !name.starts_with("Dre ") && !name.starts_with("Dresse ")),
        "a title became part of a name: {names:?}"
    );
    // One entity each: the two spellings of the title do not make two Sandra Pasquier.
    assert_eq!(
        all.iter()
            .filter(|row| row.canonical.contains("Pasquier"))
            .count(),
        1,
        "{names:?}"
    );
    clean(&lab);
}

#[tokio::test]
async fn the_question_with_and_without_the_given_name_keeps_the_same_files() {
    let mut lab = swiss_world(dre_library());
    lab.analyse().await;
    let surname = plan_with(
        &lab,
        KnowledgeMode::Auto,
        &TargetingRequest::default(),
        "Que dit la Dre Vuagniaux ?",
        small_world_policy(),
    );
    let full = plan_with(
        &lab,
        KnowledgeMode::Auto,
        &TargetingRequest::default(),
        "Quels courriers a \u{e9}crit la Dre Anne-Sophie Vuagniaux ?",
        small_world_policy(),
    );
    let mut a = surname.reduction.documents.candidates.clone();
    let mut b = full.reduction.documents.candidates.clone();
    a.sort();
    b.sort();
    assert_eq!(
        a,
        ["courrier-endocrinologie.txt", "rapport-endocrinologie.txt"],
        "{:?}",
        surname.reduction.documents.skipped
    );
    assert_eq!(a, b);
}

// ---------------------------------------------------------------------------------------- F-03

/// A commune that a table lists and that a topic question names: twelve documents, four of which say
/// the word, and a workbook with a `Commune` column.
async fn commune_world() -> Lab {
    let mut files = vec![
        (
            "hug-cardiologie.txt".to_string(),
            doc(
                1,
                "Rapport des Hopitaux universitaires de Gen\u{e8}ve, service de cardiologie.",
            ),
        ),
        (
            "hug-neurologie.txt".to_string(),
            doc(
                2,
                "Courrier des Hopitaux universitaires de Gen\u{e8}ve, service de neurologie.",
            ),
        ),
        (
            "hug-urgences.txt".to_string(),
            doc(
                3,
                "Compte rendu des urgences de Gen\u{e8}ve, nuit du week-end.",
            ),
        ),
        (
            "chuv-courrier.txt".to_string(),
            doc(4, "Courrier du CHUV a Lausanne, service d'endocrinologie."),
        ),
    ];
    files.extend((1..=8).map(filler));
    let mut lab = swiss_world(files);
    lab.write_data(
        "patients.csv",
        "Nom;Commune\nRochat;Gen\u{e8}ve\nCuendet;Gen\u{e8}ve\nPittet;Lausanne\nChappuis;Sion\n",
    );
    lab.analyse().await;
    lab.analyse_data();
    lab
}

#[tokio::test]
async fn a_commune_of_a_table_never_narrows_the_documents() {
    let lab = commune_world().await;
    let vocabulary = lab.vocabulary();
    let documents = lab.documents();
    let workbooks = lab.workbooks();
    let plan = |question: &str| {
        lab.plan(
            &vocabulary,
            KnowledgeMode::Auto,
            &TargetingRequest::default(),
            question,
            SideInput::new(&documents),
            SideInput::new(&workbooks),
            small_world_policy(),
        )
    };
    let commune = plan("Quels documents viennent de Gen\u{e8}ve ?");
    assert!(
        !commune.reduction.documents.is_applied(),
        "the documents were reduced to {:?}",
        commune.reduction.documents.candidates
    );
    assert!(
        commune.reduction.documents.capped.is_empty(),
        "the cap dropped files"
    );

    // Since fix 4, batch 2 the hospital is an organisation the base knows by name (the vocabulary
    // lists it), so a question that names it is narrowed to the hospital's own files. The commune
    // adds nothing to it: the file that only says `Gen\u{e8}ve` (the urgences) is not taken in.
    let hospital = plan("Quels documents concernent les Hopitaux universitaires de Gen\u{e8}ve ?");
    let mut candidates = hospital.reduction.documents.candidates.clone();
    candidates.sort();
    assert_eq!(
        candidates,
        ["hug-cardiologie.txt", "hug-neurologie.txt"],
        "{:?}",
        hospital.reduction.documents.skipped
    );
    assert!(hospital.reduction.documents.capped.is_empty());
}

// ---------------------------------------------------------------------------------------- F-16

async fn routing_world() -> Lab {
    let mut files = vec![
        (
            "courrier-hug.txt".to_string(),
            doc(
                1,
                "Courrier des HUG au sujet de Sophie Rochat, numero AVS 756.3052.5778.43.",
            ),
        ),
        (
            "assurance.txt".to_string(),
            doc(
                2,
                "Attestation de la CSS Assurance, police numero 6.126.452-9, jean.dupont@email.com, tel 04.50.12.34.56.",
            ),
        ),
        (
            "FAC-2026-014.txt".to_string(),
            doc(3, "Facture numero FAC-2026-014 du laboratoire."),
        ),
        (
            "neurologie.txt".to_string(),
            doc(4, "Compte rendu de neurologie du mois de mars."),
        ),
    ];
    files.extend((1..=6).map(filler));
    let mut lab = swiss_world(files);
    lab.analyse().await;
    lab
}

fn route(lab: &Lab, question: &str) -> QuestionRoute {
    let inventory = WorkFolderInventory::discover(lab.work_path(), Some(&lab.index)).unwrap();
    folder_questions::route(&inventory, &lab.index, question, "fr-FR")
}

#[tokio::test]
async fn an_identifier_or_an_acronym_goes_to_the_documents_not_to_the_file_router() {
    let lab = routing_world().await;
    for question in [
        "\u{c0} qui appartient le num\u{e9}ro AVS 756.3052.5778.43 ?",
        "Quel document porte la police n\u{b0} 6.126.452-9 ?",
        "Quels documents concernent HUG ?",
        "Quels courriers viennent du CHUV ?",
        "\u{c0} qui est l'adresse jean.dupont@email.com ?",
        "Qui a le t\u{e9}l\u{e9}phone 04.50.12.34.56 ?",
    ] {
        let routed = route(&lab, question);
        assert!(
            matches!(routed, QuestionRoute::GlobalRetrieval),
            "{question:?} -> {routed:?}"
        );
    }
}

#[tokio::test]
async fn a_real_file_name_still_names_its_file() {
    let lab = routing_world().await;
    for (question, file) in [
        ("Montre la facture FAC-2026-014", "FAC-2026-014.txt"),
        ("Que dit neurologie.txt ?", "neurologie.txt"),
        ("Que dit neurologie ?", "neurologie.txt"),
    ] {
        match route(&lab, question) {
            QuestionRoute::TargetedRetrieval { file: found } => {
                assert_eq!(found.relative_path, file, "{question:?}")
            }
            other => panic!("{question:?} -> {other:?}"),
        }
    }
    // A name with an extension that no file carries is still reported as absent.
    match route(&lab, "Que dit contrat-2026.pdf ?") {
        QuestionRoute::Deterministic(FolderAnswer::NoMatchingFile { query }) => {
            assert_eq!(query, "contrat-2026.pdf")
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn the_questions_about_the_folder_are_still_answered_from_the_folder() {
    let lab = routing_world().await;
    for question in ["Liste mes documents", "Combien de documents ai-je ?"] {
        let routed = route(&lab, question);
        assert!(
            matches!(routed, QuestionRoute::Deterministic(_)),
            "{question:?} -> {routed:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------- F-24

const SWISS_PATIENTS: &str = "Date;Patient;Montant\n\
    14.10.2026;Sophie Rochat;CHF 1'450.00\n\
    02.10.2026;Mathieu Cuendet;CHF 380.50\n\
    21.10.2026;Isabelle Pittet;CHF 1'020.00\n\
    05.09.2026;Nathalie Chappuis;CHF 95.00\n\
    18.09.2026;Pascal Maillard;CHF 2'310.25\n\
    30.09.2026;C\u{e9}line Bovet;CHF 560.00\n\
    12.10.2026;Alain Jaccard;CHF 75.00\n\
    26.10.2026;Christelle D\u{e9}l\u{e8}ze;CHF 640.80\n\
    09.10.2026;Gr\u{e9}goire Crettenand;CHF 1'999.99\n\
    15.09.2026;Marie-Claude Vuilleumier;CHF 410.00\n";

const FRENCH_PATIENTS: &str = "Date;Patient;Montant\n\
    14/10/2026;Jean Dupont;1 450,00 \u{20ac}\n\
    02/10/2026;Marie Martin;380,50 \u{20ac}\n\
    21/10/2026;Claire Martin;1 020,00 \u{20ac}\n\
    05/09/2026;Luc Martin;95,00 \u{20ac}\n\
    18/09/2026;Laurence Fabre;2 310,25 \u{20ac}\n\
    30/09/2026;Fr\u{e9}d\u{e9}ric Girard;560,00 \u{20ac}\n\
    12/10/2026;Lucas Roy;75,00 \u{20ac}\n\
    26/10/2026;Pierre Thomas;640,80 \u{20ac}\n\
    09/10/2026;Thomas Pierre;1 999,99 \u{20ac}\n\
    15/09/2026;Marie Dupont;410,00 \u{20ac}\n";

fn ask_data(lab: &Lab, question: &str) -> TabularAnswer {
    let folder = lab.data_folder();
    match tabular_answer::prepare(
        question,
        &folder,
        &ScopeMode::WholeFolder,
        "fr-FR",
        &lab.index,
        false,
    ) {
        Ok(PendingAnswer::Done(answer)) => answer,
        Ok(_) => panic!("{question:?}: the engine handed the question to the model"),
        Err(error) => panic!("{question:?}: {error:?}"),
    }
}

fn number_of(question: &str, answer: &TabularAnswer) -> f64 {
    match answer {
        TabularAnswer::Value {
            value: TabularValue::Sum(a) | TabularValue::Max(a),
            ..
        } => a.value,
        other => panic!("{question:?}: expected a computed number, got {other:?}"),
    }
}

#[test]
fn swiss_amounts_are_computed_by_the_engine_like_the_french_ones() {
    let mut lab = Lab::new();
    lab.write_data("patients-suisse.csv", SWISS_PATIENTS);
    lab.write_data("patients-france.csv", FRENCH_PATIENTS);
    lab.analyse_data();
    for file in ["patients-suisse.csv", "patients-france.csv"] {
        let question = format!("Quelle est la somme des montants dans {file} ?");
        let total = number_of(&question, &ask_data(&lab, &question));
        assert!((total - 8941.54).abs() < 1e-6, "{file}: {total}");
        let question = format!("Quel est le montant maximum dans {file} ?");
        let maximum = number_of(&question, &ask_data(&lab, &question));
        assert!((maximum - 2310.25).abs() < 1e-6, "{file}: {maximum}");
    }
}

#[test]
fn every_swiss_spelling_of_an_amount_is_one_number() {
    let mut lab = Lab::new();
    lab.write_data(
        "montants.csv",
        "Libelle;Montant\n\
         a;CHF 1'450.00\n\
         b;1\u{2019}020.50 CHF\n\
         c;Fr. 380.50\n\
         d;95.00 SFr.\n\
         e;chf 10.00\n",
    );
    lab.analyse_data();
    let question = "Quelle est la somme des montants ?";
    let total = number_of(question, &ask_data(&lab, question));
    assert!((total - 2956.0).abs() < 1e-6, "{total}");
}

#[test]
fn an_apostrophe_groups_thousands_and_is_never_a_decimal_mark() {
    let mut lab = Lab::new();
    lab.write_data(
        "montants.csv",
        "Libelle;Montant
         b;CHF 100.00
         c;CHF 20.50
         a;CHF 1'45
         d;CHF 3'000.00
         e;CHF 15.00
         f;CHF 8.00
",
    );
    lab.analyse_data();
    let question = "Quelle est la somme des montants ?";
    // `1'45` is not a number (an apostrophe groups three digits, it never marks a decimal): the
    // column is refused, never summed as 1.45 or 145.
    match ask_data(&lab, question) {
        TabularAnswer::Nudge { reason, .. } => assert_eq!(
            reason,
            Some(assistant_cabinet_ai_lib::tabular::engine::NotAnswerableReason::NonNumericColumn)
        ),
        other => panic!("{other:?}"),
    }
}

// ---------------------------------------------------------------------------------------- F-14

/// The row count an answer states, and whether the question was read as a count of the whole sheet (no
/// filter at all): a structural row count or an unfiltered count are the same fact.
fn whole_sheet_rows(answer: &TabularAnswer) -> Option<usize> {
    match answer {
        TabularAnswer::Structural {
            answer: StructuralAnswer::RowCount { rows, .. },
            ..
        } => Some(*rows),
        TabularAnswer::Value {
            value: TabularValue::Count(rows),
            locator,
            ..
        } if locator.filters.is_empty() => Some(*rows),
        _ => None,
    }
}

#[test]
fn the_name_of_a_workbook_is_a_reference_not_a_date() {
    let mut lab = Lab::new();
    lab.write_data(
        "ventes-2025.csv",
        "Date;Client;Montant\n\
         15/03/2024;Alpha;100\n\
         20/06/2024;Beta;200\n\
         25/11/2024;Gamma;300\n\
         14/01/2025;Alpha;150\n\
         18/05/2025;Beta;250\n\
         27/09/2025;Gamma;350\n",
    );
    lab.write_data(
        "factures-fournisseurs-2026.csv",
        "Num\u{e9}ro;Fournisseur;Montant\nFAC-2026-001;MedSupply;100\nFAC-2026-002;Papeterie;50\n",
    );
    lab.analyse_data();
    for (question, rows) in [
        ("Combien de lignes dans ventes-2025.csv ?", 6),
        ("Combien de lignes ? dans ventes-2025.csv", 6),
        ("Combien de lignes dans factures-fournisseurs-2026.csv ?", 2),
        ("Combien de lignes dans Ventes-2025.CSV ?", 6),
    ] {
        let answer = ask_data(&lab, question);
        assert_eq!(
            whole_sheet_rows(&answer),
            Some(rows),
            "{question:?}: {answer:?}"
        );
    }
}

#[test]
fn a_year_the_user_really_asks_for_is_still_a_filter() {
    let mut lab = Lab::new();
    lab.write_data(
        "ventes-2025.csv",
        "Date;Client;Montant\n\
         15/03/2024;Alpha;100\n\
         20/06/2024;Beta;200\n\
         14/01/2025;Alpha;150\n\
         18/05/2025;Beta;250\n\
         27/09/2025;Gamma;350\n",
    );
    lab.write_data("autre.csv", "Date;Client\n15/03/2024;Alpha\n");
    lab.analyse_data();
    let question = "Combien de lignes en 2025 dans ventes-2025.csv ?";
    match ask_data(&lab, question) {
        TabularAnswer::Value {
            value: TabularValue::Count(n),
            ..
        } => assert_eq!(n, 3, "{question:?}"),
        other => panic!("{question:?}: {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------- F-01

#[tokio::test]
async fn the_chunks_of_one_document_are_found_through_an_index() {
    let lab = world(vec![("a.txt".to_string(), doc(1, "Un texte."))]).await;
    let connection = lab.side();
    let indexes: Vec<String> = connection
        .prepare("SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = 'chunks'")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(
        indexes.iter().any(|name| name == "chunks_by_document"),
        "{indexes:?}"
    );
    let plan: Vec<String> = connection
        .prepare("EXPLAIN QUERY PLAN SELECT chunk_id FROM chunks WHERE relative_path = ?1")
        .unwrap()
        .query_map(["a.txt"], |row| row.get::<_, String>(3))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(
        plan.iter()
            .all(|step| step.contains("USING") && step.contains("INDEX")),
        "{plan:?}"
    );
}

// ---------------------------------------------------------------------------------------- F-20

#[tokio::test]
async fn the_analyse_summary_counts_the_files_it_could_not_read_by_format() {
    let mut lab = Lab::new();
    lab.write("lettre.txt", "Un courrier lisible.\n");
    for name in [
        "a.odt",
        "b.ODT",
        "dossier/c.odt",
        "note.rtf",
        "mail.eml",
        "reunion.vtt",
    ] {
        lab.write(name, "contenu\n");
    }
    // The system's own files are not documents the user expects to be read.
    lab.write("Thumbs.db", "x");
    lab.write("desktop.ini", "x");
    // A workbook belongs to the Data Folder, a file without an extension to nobody: neither is
    // reported as a document that was not read.
    lab.write("classeur.xlsx", "x");
    lab.write("LISEZMOI", "x");
    let summary = lab.analyse().await;
    let found: Vec<(String, usize)> = summary
        .unread_formats
        .iter()
        .map(|format| (format.extension.clone(), format.count))
        .collect();
    assert_eq!(
        found,
        [
            ("odt".to_string(), 3),
            ("eml".to_string(), 1),
            ("rtf".to_string(), 1),
            ("vtt".to_string(), 1),
        ]
    );
    assert_eq!(summary.indexed_files, 1);
}

#[tokio::test]
async fn a_folder_with_only_readable_files_reports_no_unread_format() {
    let mut lab = Lab::new();
    lab.write("lettre.txt", "Un courrier lisible.\n");
    let summary = lab.analyse().await;
    assert!(summary.unread_formats.is_empty());
}

// ---------------------------------------------------------------------------------------- F-02, shared surname

/// A doctor and a patient who share a surname: the doctor is always written `Dre ...`, the patient
/// `Monsieur ...`. The ground truth of the sandbox has exactly this pair (`Dre Anne-Sophie
/// Vuagniaux`, `Monsieur Gilles Vuagniaux`).
fn shared_surname_library() -> Vec<(String, String)> {
    let mut files = vec![
        (
            "courrier-endocrinologie.txt".to_string(),
            doc(
                1,
                "Courrier de la Dre Anne-Sophie Vuagniaux au sujet du suivi du diabete.",
            ),
        ),
        (
            "rapport-endocrinologie.txt".to_string(),
            doc(
                2,
                "Rapport signe par la Dre Anne-Sophie Vuagniaux, endocrinologue au CHUV.",
            ),
        ),
        (
            "ordonnance-patient.txt".to_string(),
            doc(
                3,
                "Ordonnance pour Monsieur Gilles Vuagniaux, traitement de six mois.",
            ),
        ),
    ];
    files.extend((1..=9).map(filler));
    files
}

#[tokio::test]
async fn a_title_and_a_surname_seen_together_outrank_the_surname_alone() {
    let mut lab = swiss_world(shared_surname_library());
    lab.analyse().await;
    let titled = plan_with(
        &lab,
        KnowledgeMode::Auto,
        &TargetingRequest::default(),
        "Que dit la Dre Vuagniaux ?",
        small_world_policy(),
    );
    let analysis = titled.analysis.as_ref().expect("the question was read");
    assert!(
        analysis.ambiguities.is_empty(),
        "the title names the doctor: {:?}",
        analysis.ambiguities
    );
    let mut files = titled.reduction.documents.candidates.clone();
    files.sort();
    assert_eq!(
        files,
        ["courrier-endocrinologie.txt", "rapport-endocrinologie.txt"],
        "{:?}",
        titled.reduction.documents.skipped
    );

    // Without the title the surname names two people: the question stays a question.
    let bare = plan_with(
        &lab,
        KnowledgeMode::Auto,
        &TargetingRequest::default(),
        "Que dit Vuagniaux ?",
        small_world_policy(),
    );
    let bare_analysis = bare.analysis.as_ref().expect("the question was read");
    assert_eq!(bare_analysis.ambiguities.len(), 1);
    assert!(!bare.reduction.documents.is_applied());
}

/// The register's own case (KBD-04, raised to S2 by lot 11): the invoices workbook of the sandbox,
/// named by hand, holds a year in its name and invoice numbers that end with it.
#[test]
fn the_sandbox_invoices_workbook_named_by_hand_is_counted_whole() {
    let truth = common::validation::Truth::load();
    let book = truth
        .books
        .iter()
        .find(|b| b.path == "factures-fournisseurs-2026.xlsx")
        .expect("the invoices workbook is in the sandbox");
    // `workbooks.json`: one sheet, `Factures`, 39 rows.
    let rows = 39;
    let mut lab = Lab::new();
    lab.write_data_bytes(
        &book.path,
        &std::fs::read(truth.root.join("data").join(&book.path)).unwrap(),
    );
    lab.analyse_data();
    for question in [
        "Combien de lignes dans factures-fournisseurs-2026.xlsx ?",
        "Combien de lignes dans Factures-Fournisseurs-2026.xlsx ?",
    ] {
        let answer = ask_data(&lab, question);
        assert_eq!(
            whole_sheet_rows(&answer),
            Some(rows),
            "{question:?}: {answer:?}"
        );
    }
}

/// The Analyse summary of the documents on the sandbox, tier by tier: what the user is told about the
/// files that were not read. The register counts 21 of 200 (7 `.odt`, 7 `.rtf`, 2 `.eml`, 2 `.vtt`, 1
/// `.odp`, 1 `.pptx`, 1 `.ppt`). `cargo test --release --test knowledge_fix_swiss_first print_the_unread -- --ignored --nocapture`
#[tokio::test]
#[ignore]
async fn print_the_unread_formats_of_the_sandbox_tiers() {
    let truth = common::validation::Truth::load();
    for tier in [30usize, 100, 200] {
        let mut lab = Lab::new();
        lab.packs = vec!["health-fr", "health-ch"];
        for doc in truth.docs_in(tier) {
            let to = lab.work_path().join(&doc.path);
            std::fs::create_dir_all(to.parent().unwrap()).unwrap();
            std::fs::copy(truth.root.join("documents").join(&doc.path), to).unwrap();
        }
        let summary = lab.analyse().await;
        let total: usize = summary.unread_formats.iter().map(|f| f.count).sum();
        let list: Vec<String> = summary
            .unread_formats
            .iter()
            .map(|f| format!("{} .{}", f.count, f.extension))
            .collect();
        println!(
            "tier {tier}: {total} file(s) not read ({}), scanned {}, indexed {}",
            list.join(", "),
            summary.scanned_files,
            summary.indexed_files
        );
        if tier == 200 {
            assert_eq!(total, 21, "the register counts 21 unread files of 200");
        }
    }
}

/// What the targeting line of the human test will say, from the same code on the same files: the library of
/// the sandbox (`ACAI_TIER`, default 200) with all its workbooks, mode `Auto`. `cargo test --release --test knowledge_fix_swiss_first print_the_targeting -- --ignored --nocapture`
#[tokio::test]
#[ignore]
async fn print_the_targeting_lines_of_the_human_test() {
    use assistant_cabinet_ai_lib::knowledge::targeting::QueryVocabulary;
    use common::validation::{library, Replay, Truth};
    let truth = Truth::load();
    let tier: usize = std::env::var("ACAI_TIER")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200);
    let lib = library(&truth, tier, &["health-fr", "health-ch"]).await;
    let vocabulary = QueryVocabulary::load("fr-FR", &["health-fr", "health-ch"]).unwrap();
    let replay = Replay::new(&lib, &vocabulary);
    for question in [
        "\u{c0} qui appartient le num\u{e9}ro AVS 756.3052.5778.43 ?",
        "Que dit la Dre Vuagniaux ?",
        "Quels courriers a \u{e9}crit la Dre Anne-Sophie Vuagniaux ?",
        "Quels documents viennent de Gen\u{e8}ve ?",
        "Quels documents concernent HUG ?",
    ] {
        let asked = replay.ask(
            question,
            KnowledgeMode::Auto,
            &TargetingRequest::default(),
            None,
            true,
        );
        println!(
            "{question}\n    route {:?}, selected {}",
            asked.route,
            asked.selected.len()
        );
        if let Some(plan) = &asked.plan {
            if let Some(analysis) = &plan.analysis {
                for e in &analysis.entities {
                    println!(
                        "    name {} / {} / {:?} / {:.2}",
                        e.display, e.type_id, e.status, e.confidence
                    );
                }
                for a in &analysis.ambiguities {
                    println!("    ambiguity on {:?}", a.span);
                }
            }
            let d = &plan.reduction.documents;
            let w = &plan.reduction.data;
            println!(
                "    documents: {:?} skipped {:?} kept {} of {}; workbooks: {:?} skipped {:?} kept {} of {}",
                d.outcome,
                d.skipped.map(|r| r.as_code()),
                d.candidates.len(),
                d.selected,
                w.outcome,
                w.skipped.map(|r| r.as_code()),
                w.candidates.len(),
                w.selected
            );
        }
    }
}

/// The table answers of the human test, in the engine's own words (`Debug`), on the sandbox's invoices workbook
/// and on the two amount tables the helper writes. `cargo test --test knowledge_fix_swiss_first print_the_data -- --ignored --nocapture`
#[test]
#[ignore]
fn print_the_data_answers_of_the_human_test() {
    let truth = common::validation::Truth::load();
    let mut lab = Lab::new();
    lab.write_data_bytes(
        "factures-fournisseurs-2026.xlsx",
        &std::fs::read(
            truth
                .root
                .join("data")
                .join("factures-fournisseurs-2026.xlsx"),
        )
        .unwrap(),
    );
    lab.write_data("patients-suisse.csv", SWISS_PATIENTS);
    lab.write_data("patients-france.csv", FRENCH_PATIENTS);
    lab.analyse_data();
    for question in [
        "Combien de lignes dans factures-fournisseurs-2026.xlsx ?",
        "Quelle est la somme des montants dans patients-suisse.csv ?",
        "Quel est le montant maximum dans patients-suisse.csv ?",
        "Quelle est la somme des montants dans patients-france.csv ?",
        "Quel est le montant maximum dans patients-france.csv ?",
    ] {
        println!("{question}\n    -> {:?}", ask_data(&lab, question));
    }
}
