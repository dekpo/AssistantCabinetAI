//! KB lot 11 - the walk: the human test's files and questions replayed on a temporary copy, so that every figure
//! the protocol states as an expectation is produced by the same code on the same files, and so that the
//! lifecycle (a new file, a changed file, a deleted file, a renamed file, the three Resets, a library that grows and
//! shrinks) is proved to leave the base clean.
//!
//! The window cannot be walked here: what only the owner can see (the targeting line, the buttons, the card
//! counts) is said so in the protocol. What can be computed is computed here.
//!
//! ```text
//! cargo test --release --test knowledge_validation_walk -- --nocapture
//! ```
//!
//! The lifecycle test is not `#[ignore]`: it asserts the invariants (I3, I14, no orphan row after any step).

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use assistant_cabinet_ai_lib::filename_sanitizer;
use assistant_cabinet_ai_lib::knowledge::normalize::TitleSet;
use assistant_cabinet_ai_lib::knowledge::store as kb;
use assistant_cabinet_ai_lib::knowledge::targeting::{QueryVocabulary, TargetingRequest};
use assistant_cabinet_ai_lib::knowledge::{IntegrityReport, KnowledgeMode};

use common::knowledge::{count, snapshot, Lab};
use common::validation::*;

const PACKS: [&str; 2] = ["health-fr", "health-ch"];

/// The folders exactly as the helper writes them: every file of the first `tier` documents, readable by the
/// product or not, and every workbook.
fn lab_like_the_helper(truth: &Truth, tier: usize) -> Lab {
    let mut lab = Lab::new();
    lab.packs = PACKS.to_vec();
    lab.mode = KnowledgeMode::Suggest;
    for doc in truth.docs_in(tier) {
        let to = lab.work_path().join(&doc.path);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(truth.root.join("documents").join(&doc.path), to).unwrap();
    }
    for book in &truth.books {
        lab.write_data_bytes(
            &book.path,
            &std::fs::read(truth.root.join("data").join(&book.path)).unwrap(),
        );
    }
    lab
}

fn grow(truth: &Truth, lab: &Lab, from: usize, to: usize) {
    for doc in truth
        .docs
        .iter()
        .filter(|d| d.index > from && d.index <= to)
    {
        let target = lab.work_path().join(&doc.path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::copy(truth.root.join("documents").join(&doc.path), target).unwrap();
    }
}

fn shrink(truth: &Truth, lab: &Lab, to: usize) {
    for doc in truth.docs.iter().filter(|d| d.index > to) {
        let _ = std::fs::remove_file(lab.work_path().join(&doc.path));
    }
}

fn entities_by_type(lab: &Lab) -> BTreeMap<String, i64> {
    let connection = lab.side();
    let mut statement = connection
        .prepare("SELECT type_id || ':' || status, COUNT(*) FROM kb_entities WHERE status IN ('candidate','active') GROUP BY 1")
        .expect("prepares");
    statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("queries")
        .map(|r| r.expect("reads"))
        .collect()
}

/// What the `names` command of the helper lists and fingerprints: type, status and canonical name, sorted.
fn entity_list(lab: &Lab) -> Vec<(String, String, String)> {
    let connection = lab.side();
    let mut statement = connection
        .prepare("SELECT type_id, status, canonical_name FROM kb_entities WHERE status IN ('candidate','active') ORDER BY type_id, canonical_name, status")
        .expect("prepares");
    statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .expect("queries")
        .map(|r| r.expect("reads"))
        .collect()
}

fn entity_exists(lab: &Lab, name: &str) -> bool {
    let connection = lab.side();
    connection
        .query_row(
            "SELECT COUNT(*) FROM kb_entities WHERE status IN ('candidate','active') AND canonical_name = ?1",
            [name],
            |row| row.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false)
}

fn files_of(lab: &Lab, name: &str) -> Vec<String> {
    let connection = lab.side();
    let mut statement = connection
        .prepare(
            "SELECT DISTINCT s.relative_path FROM kb_entities e JOIN kb_mentions m ON m.entity_id = e.entity_id
             JOIN kb_sources s ON s.source_id = m.source_id WHERE e.status IN ('candidate','active') AND e.canonical_name = ?1
             AND s.domain = 'documents' ORDER BY 1",
        )
        .expect("prepares");
    statement
        .query_map([name], |row| row.get::<_, String>(0))
        .expect("queries")
        .map(|r| r.expect("reads"))
        .collect()
}

fn clean(lab: &Lab) {
    assert_eq!(
        kb::integrity_check(&lab.side()).expect("checks"),
        IntegrityReport::default()
    );
}

/// One pass the way the Analyse button runs it: the clean names first, then the documents, then the workbooks.
async fn analyse(truth: &Truth, lab: &mut Lab, tier: usize) -> usize {
    let report = filename_sanitizer::sanitize_folder(lab.work_path());
    let (ocr, rasterizer) = perfect_ocr(truth, tier);
    let mut context = lab.context();
    let url = lab.gateway.url.clone();
    let summary = lab
        .analyse_through(&mut context, &url, Some(&ocr), Some(&rasterizer))
        .await;
    lab.analyse_data();
    println!(
        "    pass: analysed {} (unchanged {}), removed {:?}, renamed {}, knowledge {:?}",
        summary.indexed_files,
        summary.unchanged_files,
        summary.removed_files,
        report.renamed.len(),
        summary.knowledge
    );
    report.renamed.len()
}

/// The line the window will draw under the answer to `text` in `Automatique`, from the same code: the documents
/// kept out of the documents selected, and the names used. The laboratory is handed back untouched.
fn print_the_line(lab: Lab, tier: usize, step: &str, text: &str) -> Lab {
    let vocabulary = QueryVocabulary::load("fr-FR", &PACKS).unwrap();
    let library = Library {
        lab,
        tier,
        product_files: Vec::new(),
        analyse_ms: 0,
        summary: None,
        passes: Vec::new(),
    };
    {
        let replay = Replay::new(&library, &vocabulary);
        let asked = replay.ask(
            text,
            KnowledgeMode::Auto,
            &TargetingRequest::default(),
            None,
            false,
        );
        match &asked.plan {
            None => println!(
                "    {step} `{text}`: no search (route {})",
                asked.route.code()
            ),
            Some(plan) => {
                let documents = &plan.reduction.documents;
                let names: Vec<String> = plan
                    .analysis
                    .as_ref()
                    .map(|a| a.entities.iter().map(|e| e.display.clone()).collect())
                    .unwrap_or_default();
                println!(
                    "    {step} `{text}`: selected {}, outcome {:?}, kept {}, skipped {:?}, names {names:?}",
                    asked.selected.len(), documents.outcome, documents.candidates.len(), documents.skipped.map(|r| r.as_code())
                );
            }
        }
    }
    library.lab
}

#[tokio::test]
async fn the_lifecycle_of_the_protocol_leaves_the_base_clean() {
    let truth = Truth::load();
    let mut lab = lab_like_the_helper(&truth, 30);
    analyse(&truth, &mut lab, 30).await;
    clean(&lab);
    let after_first = entities_by_type(&lab);
    let first_snapshot = snapshot(&lab.side());
    println!("A. first Analyse of 30 documents: {after_first:?}");

    // I14: a second Analyse over unchanged files writes nothing
    analyse(&truth, &mut lab, 30).await;
    assert_eq!(
        snapshot(&lab.side()),
        first_snapshot,
        "a second pass over unchanged files changes nothing"
    );

    // a question changes nothing (I3), in the three modes
    let vocabulary = QueryVocabulary::load("fr-FR", &PACKS).unwrap();
    let before = snapshot(&lab.side());
    {
        let library = Library {
            lab,
            tier: 30,
            product_files: Vec::new(),
            analyse_ms: 0,
            summary: None,
            passes: Vec::new(),
        };
        let replay = Replay::new(&library, &vocabulary);
        for mode in [
            KnowledgeMode::Off,
            KnowledgeMode::Suggest,
            KnowledgeMode::Auto,
        ] {
            for text in [
                "Que dit le dossier de Madame Zenobie Quarante ?",
                "Quels documents concernent Hyacinthe Vaudroz ?",
                "Que dit le Dr Ernest Blondeau ?",
                "Que dit Leopold Tschumi ?",
            ] {
                replay.ask(text, mode, &TargetingRequest::default(), None, false);
            }
        }
        lab = library.lab;
    }
    assert_eq!(
        snapshot(&lab.side()),
        before,
        "a question about a name nobody wrote creates no entity"
    );
    assert!(!entity_exists(&lab, "Zenobie Quarante"));

    // C1. a new letter about a person the library has never seen
    lab.write("courriers-entrants/courrier-quenneville-nouveau.txt", "Madame Sidonie Quenneville\n12 rue des Acacias, 74100 Annemasse\n\nObjet : demande de rendez-vous.\nMadame Sidonie Quenneville souhaite un rendez-vous de suivi dans le courant du mois d'octobre 2026.\nElle rappelle que sa caisse maladie est la CSS Assurance.\n");
    let before_add = entities_by_type(&lab);
    analyse(&truth, &mut lab, 30).await;
    clean(&lab);
    assert!(
        entity_exists(&lab, "Sidonie Quenneville"),
        "the new person is in the base after the next Analyse"
    );
    println!(
        "C1. new letter: {before_add:?} -> {:?}",
        entities_by_type(&lab)
    );
    lab = print_the_line(lab, 30, "C1", "Que dit Madame Sidonie Quenneville ?");

    // C2. a name with spaces and an accent: the Analyse renames the file first, and the base follows the new name
    lab.write(
        "Courrier Quenneville \u{e9}t\u{e9} 2026.txt",
        "Madame Sidonie Quenneville confirme son rendez-vous du 14 octobre 2026.\n",
    );
    let renamed = analyse(&truth, &mut lab, 30).await;
    clean(&lab);
    assert_eq!(renamed, 1, "one file is renamed");
    assert!(lab
        .work_path()
        .join("Courrier-Quenneville-ete-2026.txt")
        .is_file());
    let files = files_of(&lab, "Sidonie Quenneville");
    assert!(
        files
            .iter()
            .any(|f| f == "Courrier-Quenneville-ete-2026.txt"),
        "{files:?}"
    );
    println!(
        "C2. renamed to Courrier-Quenneville-ete-2026.txt; Sidonie Quenneville is in {files:?}"
    );
    lab = print_the_line(lab, 30, "C2", "Que dit Madame Sidonie Quenneville ?");

    // C3. the three Resets of the protocol (placed before the files are changed or deleted, so the rebuilt list can be compared name for name), then Analyse both cards again: the base is a function of the files, so the
    // list of names (what `names` prints and fingerprints) comes back the same
    let listed_before = entity_list(&lab);
    let tables_before = count(&lab.side(), "kb_sources");
    lab.index.clear().expect("clears the documents");
    clean(&lab);
    let after_reset = entities_by_type(&lab);
    analyse(&truth, &mut lab, 30).await;
    clean(&lab);
    println!(
        "C3a. documents Reset: {after_reset:?}; re-analysed: {:?}; sources before {tables_before}",
        entities_by_type(&lab)
    );
    lab.index
        .clear_tabular_inventories()
        .expect("clears the inventories");
    lab.index
        .clear_tabular_workbooks()
        .expect("clears the cache");
    clean(&lab);
    lab.analyse_data();
    clean(&lab);
    println!(
        "C3b. data Reset and re-analyse: {:?}",
        entities_by_type(&lab)
    );
    assistant_cabinet_ai_lib::knowledge::manage::reset(&lab.side(), true)
        .expect("resets the knowledge");
    println!("C3c. whole knowledge Reset: {:?}", entities_by_type(&lab));
    clean(&lab);
    analyse(&truth, &mut lab, 30).await;
    clean(&lab);
    let listed_after = entity_list(&lab);
    println!(
        "C3d. after the rebuild: {} names, before the Resets {}; identical list: {}",
        listed_after.len(),
        listed_before.len(),
        listed_after == listed_before
    );
    if listed_after != listed_before {
        for row in listed_before.iter().filter(|r| !listed_after.contains(r)) {
            println!("    only before: {row:?}");
        }
        for row in listed_after.iter().filter(|r| !listed_before.contains(r)) {
            println!("    only after: {row:?}");
        }
    }
    assert_eq!(
        listed_after, listed_before,
        "the base rebuilt from the same files holds the same names"
    );

    // C4. a changed letter: the old surname leaves that file, the new one arrives (the first text letter about one
    // patient: the surname is changed wherever it is written)
    let target = truth
        .docs_in(30)
        .find(|d| d.format == "txt" && d.kind == "lettre_confrere" && d.about.len() == 1)
        .map(|d| (d.path.clone(), d.about[0].clone()));
    let (path, person_id) =
        target.expect("a text letter about one patient in the first 30 documents");
    let person = &truth.entities[&person_id];
    let (given, family) = person.canonical.split_once(' ').expect("a full name");
    let changed_name = format!("{given} Qu\u{e9}m\u{e9}ner");
    let full = lab.work_path().join(&path);
    let text = std::fs::read_to_string(&full).unwrap();
    let before_files = files_of(&lab, &person.canonical);
    std::fs::write(&full, text.replace(family, "Qu\u{e9}m\u{e9}ner")).unwrap();
    analyse(&truth, &mut lab, 30).await;
    clean(&lab);
    let after_files = files_of(&lab, &person.canonical);
    assert!(
        !after_files.contains(&path),
        "the changed letter no longer names {}",
        person.canonical
    );
    assert!(
        entity_exists(&lab, &changed_name),
        "{changed_name} is a new name of the base"
    );
    println!(
        "C4. changed {path}: {} in {} file(s) before, {} after; {changed_name} exists",
        person.canonical,
        before_files.len(),
        after_files.len()
    );
    lab = print_the_line(lab, 30, "C4", &format!("Que dit {changed_name} ?"));
    println!("    names now (C4): {:?}", entities_by_type(&lab));

    // C5. a deleted letter: its associations go, the others stay
    let victim = truth
        .docs_in(30)
        .find(|d| d.format == "pdf" && d.mentions.iter().any(|m| m.entity == "p-rochat-sophie"))
        .map(|d| d.path.clone());
    if let Some(path) = victim {
        let before_files = files_of(&lab, "Sophie Rochat");
        std::fs::remove_file(lab.work_path().join(&path)).unwrap();
        analyse(&truth, &mut lab, 30).await;
        clean(&lab);
        let after_files = files_of(&lab, "Sophie Rochat");
        assert!(!after_files.contains(&path));
        if before_files.contains(&path) {
            assert_eq!(
                after_files.len() + 1,
                before_files.len(),
                "only the deleted file left"
            );
        }
        println!(
            "C5. deleted {path}: Sophie Rochat in {} file(s) before, {} after",
            before_files.len(),
            after_files.len()
        );
        println!("    names now (C5): {:?}", entities_by_type(&lab));
        lab = print_the_line(lab, 30, "C5", "Résume le dossier de Sophie Rochat.");
        lab = print_the_line(
            lab,
            30,
            "C4",
            "Quels courriers viennent de la CSS Assurance ?",
        );
    }

    // C6. the library grows to 100, then shrinks back to 30: names follow the files
    grow(&truth, &lab, 30, 100);
    analyse(&truth, &mut lab, 100).await;
    clean(&lab);
    let at_100 = entities_by_type(&lab);
    shrink(&truth, &lab, 30);
    analyse(&truth, &mut lab, 30).await;
    clean(&lab);
    let back_at_30 = entities_by_type(&lab);
    println!(
        "C6. 30 -> 100: {at_100:?}
    100 -> 30: {back_at_30:?}"
    );
    println!(
        "C6 total names: at 100 {}, back at 30 {}",
        at_100.values().sum::<i64>(),
        back_at_30.values().sum::<i64>()
    );
    assert!(at_100.values().sum::<i64>() > back_at_30.values().sum::<i64>());
    let _ = TitleSet::new();
    let _ = Path::new("");
}

/// What the extractor makes of one sentence at a time: the minimal text behind each defect the sandbox found.
/// Prints a table; asserts nothing (the rows of the defects register say what was expected and what happens).
#[tokio::test]
#[ignore]
async fn probe_the_extractor_one_sentence_at_a_time() {
    let probes: Vec<(&str, &str)> = vec![
        ("title at the end of a line", "Le guide a ete relu par Pr\nJean-Luc Rapin et par la direction."),
        ("given name and surname on two lines", "Le dossier a ete transmis a Jean\nDupont pour validation."),
        ("title and given name on two lines", "Le dossier a ete transmis au Dr Jean\nDupont pour validation."),
        ("recipient at the start of an address block", "Madame, Monsieur,\n\nAnne Marchand\n12 rue Pasteur, 74100 Annemasse\n\nNous avons traite votre dossier."),
        ("recipient with a civility on its own line", "Madame\nSophie Rochat\nAvenue de la Gare 14, 1003 Lausanne\n\nNous avons traite votre dossier."),
        ("the Swiss title Dre", "Courrier de la Dre Sandra Pasquier, pediatre, a Geneve."),
        ("the Swiss title Dresse", "Courrier de la Dresse Sandra Pasquier, pediatre, a Geneve."),
        ("hospital, lowercase words after the marker", "Compte rendu des Hopitaux universitaires de Geneve, service de cardiologie."),
        ("hospital, lowercase words after the marker (centre)", "Courrier du Centre hospitalier universitaire vaudois, service de neurologie."),
        ("acronym alone", "Courrier du CHUV et des HUG au sujet du patient."),
        ("insurer with no marker", "Attestation de la SUVA et courrier de l'URSSAF Rhone-Alpes."),
        ("hospital with no marker", "Courrier des Hospices Civils de Lyon et du CHU Grenoble Alpes."),
        ("organisation after a sentence-initial word", "Entre Bureautique Leman SA et la Papeterie du Lac SARL, un accord a ete signe."),
        ("legal suffix followed by a full stop", "Le bailleur est Dupont Immobilier SA. Le loyer est indexe."),
        ("organisation with an ampersand", "Les factures viennent du Garage Martin & Fils SARL et de l'Etude Chappuis & Associes."),
        ("an address fragment repeated", "Pharmacie, 12 place de l'Hotel de Ville, 74100 Annemasse. Ouverte du lundi au samedi."),
        ("weak title and a shared surname", "M. Dupont est attendu mardi a 10 h."),
        ("two people whose names are swapped", "Rendez-vous de Lucas Roy mardi et de Roy Lucas jeudi."),
        ("surname first, upper case", "DUPONT Jean a ete recu en consultation."),
        ("particle and apostrophe", "Courrier a Matthieu de la Fontaine, Camille d'Aubigne et Sean O'Brien."),
        ("hyphen typed between given name and surname", "Le dossier de Jean-Dupont est complet."),
    ];
    let truth = Truth::load();
    let mut lab = Lab::new();
    lab.packs = PACKS.to_vec();
    lab.mode = KnowledgeMode::Suggest;
    for (index, (_, text)) in probes.iter().enumerate() {
        lab.write(&format!("probe-{index:02}.txt"), &format!("{text}\n"));
    }
    analyse(&truth, &mut lab, 30).await;
    let connection = lab.side();
    println!("| # | Probe | The base found |\n| --- | --- | --- |");
    for (index, (label, _)) in probes.iter().enumerate() {
        let mut statement = connection
            .prepare(
                "SELECT e.type_id, e.status, e.canonical_name FROM kb_entities e
                 JOIN kb_mentions m ON m.entity_id = e.entity_id JOIN kb_sources s ON s.source_id = m.source_id
                 WHERE s.relative_path = ?1 GROUP BY e.entity_id ORDER BY e.canonical_name",
            )
            .expect("prepares");
        let found: Vec<String> = statement
            .query_map([format!("probe-{index:02}.txt")], |row| {
                Ok(format!(
                    "{} {} `{}`",
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?
                ))
            })
            .expect("queries")
            .map(|r| r.expect("reads"))
            .collect();
        println!(
            "| {index} | {label} | {} |",
            if found.is_empty() {
                "nothing".to_string()
            } else {
                found.join(", ")
            }
        );
    }
}

/// What the application writes after each question, for the `report` command of the human test to read: the
/// questions are asked in `suggest` over the first 30 documents (fake vectors, so which files are cited means
/// nothing; the point is that the rows exist and that the helper adds them up as `diagnostics::aggregate` does).
/// The index is copied to `ACAI_VALIDATION_OUT/walk-data` so the helper can be run against it with
/// `ACAI_DATA_DIR`.
#[tokio::test]
#[ignore]
async fn record_questions_for_the_report_command() {
    use assistant_cabinet_ai_lib::knowledge::diagnostics::{
        self, RetrievalDiagnostics, RetrievalPath,
    };
    use assistant_cabinet_ai_lib::knowledge::targeting;
    let truth = Truth::load();
    let library = library(&truth, 30, &PACKS).await;
    let vocabulary = QueryVocabulary::load("fr-FR", &PACKS).unwrap();
    let replay = Replay::new(&library, &vocabulary);
    let mut at = 1_800_000_000i64;
    let mut recorded = 0;
    for q in truth
        .questions
        .iter()
        .filter(|q| {
            q.min_tier <= 30 && !matches!(q.category.as_str(), "data" | "leak" | "small_scope")
        })
        .take(80)
    {
        let asked = replay.ask(
            &q.text,
            KnowledgeMode::Suggest,
            &TargetingRequest::default(),
            None,
            false,
        );
        let Some(plan) = asked.plan.as_ref() else {
            continue;
        };
        let Some(searched) =
            replay.search(&asked, &q.text, &common::fake_gateway::vector_for(&q.text))
        else {
            continue;
        };
        let mut measured = RetrievalDiagnostics::new(RetrievalPath::Documents);
        measured.selected_documents = asked.selected.len();
        targeting::note_documents(&library.lab.index, plan, &searched, &mut measured);
        diagnostics::record_question(&library.lab.side(), at, &measured).expect("records");
        at += 60;
        recorded += 1;
    }
    let aggregates = diagnostics::aggregate(&library.lab.side(), 2000).expect("aggregates");
    println!("recorded {recorded} questions\n{aggregates:#?}");
    if let Some(out) = out_dir() {
        let folder = out.join("walk-data");
        std::fs::create_dir_all(&folder).unwrap();
        // a consistent copy of the database (the lab holds it open)
        library
            .lab
            .side()
            .execute(
                "VACUUM INTO ?1",
                [folder.join("index.sqlite3").display().to_string()],
            )
            .expect("copies the index");
        println!("index copied to {}", folder.display());
    }
}

/// What the folder router makes of a question before the knowledge base is ever consulted: a number with dots
/// (an AVS number) and an acronym can read as the name of a file. Prints the route and, for a folder answer, its
/// machine form.
#[tokio::test]
#[ignore]
async fn probe_the_folder_router() {
    use assistant_cabinet_ai_lib::inventory::WorkFolderInventory;
    let truth = Truth::load();
    let library = library(&truth, 30, &PACKS).await;
    let lab = &library.lab;
    let inventory = WorkFolderInventory::discover(lab.work_path(), Some(&lab.index)).unwrap();
    let questions = [
        "À qui appartient le numéro AVS 756.3052.5778.43 ?",
        "À qui appartient le numéro de sécurité sociale 1 58 03 74 443 192 09 ?",
        "Quels documents concernent HUG ?",
        "Quels courriers viennent du CHUV ?",
        "Quels documents concernent les Hôpitaux universitaires de Genève ?",
        "Quelle facture porte l'IBAN CH88 0053 1473 1495 6118 1 ?",
        "Quel document porte la police n° 6.126.452-9 ?",
        "Montre la facture FAC-2026-014",
        "Que dit le contrat CTR-2026-0041 ?",
        "Que dit le Docteur Pierre Martin ?",
        "Quels courriers viennent du Dr Pierre Martin ?",
        "Que dit Paul Martin ?",
        "Combien de documents ai-je ?",
        "Que dit M. Dupont ?",
    ];
    for text in questions {
        let route = assistant_cabinet_ai_lib::folder_questions::route(
            &inventory, &lab.index, text, "fr-FR",
        );
        println!("{text}\n    -> {route:?}");
    }
}

/// What the tabular tier made of a question, in one line: the engine's own answer, or the fact that it hands the
/// question to the model.
fn describe(
    pending: &Result<
        assistant_cabinet_ai_lib::tabular_answer::PendingAnswer,
        assistant_cabinet_ai_lib::error::AppError,
    >,
) -> String {
    use assistant_cabinet_ai_lib::tabular_answer::PendingAnswer;
    match pending {
        Ok(PendingAnswer::Done(answer)) => format!("{answer:?}"),
        Ok(_) => {
            "hands the question to the model (the engine could not settle it alone)".to_string()
        }
        Err(error) => format!("error {error:?}"),
    }
}

/// The data questions of the sandbox through the real tabular engine (the authority on a number, unchanged by
/// the knowledge base), with the answer the generator knows: the people of `rendez-vous-2026.xlsx` are written in
/// several forms (`Daniel Menoud`, `Menoud Daniel`, `Menoud D`), and the engine counts the cells that match what
/// was typed. Prints the question, the count the sheet really holds for that person, and what the engine said.
#[tokio::test]
#[ignore]
async fn report_the_data_questions() {
    use assistant_cabinet_ai_lib::analysis_scope::ScopeMode;
    use assistant_cabinet_ai_lib::tabular_answer;
    let truth = Truth::load();
    let library = library(&truth, 200, &PACKS).await;
    let folder = library.lab.data_folder();
    println!("| Id | Question | The sheet holds (all forms) | The engine answered |\n| --- | --- | --- | --- |");
    for q in truth.questions.iter().filter(|q| q.category == "data") {
        let pending = tabular_answer::prepare(
            &q.text,
            &folder,
            &ScopeMode::WholeFolder,
            "fr-FR",
            &library.lab.index,
            false,
        );
        let said: String = describe(&pending).chars().take(260).collect();
        println!(
            "| {} | {} | {:?} | {said} |",
            q.id, q.text, q.expected_value
        );
    }
}

/// What the Analyse button's summary will say at each size, on the real OCR engine and the real PDF rasteriser
/// (the embedding is the lab's fake: it changes no count). The folder cards show what is derived from these
/// fields; the exact sentences are in the interface catalogue. Needs the bundled Tesseract and pdfium.
#[tokio::test]
#[ignore]
async fn print_the_analyse_summaries_of_the_tiers() {
    use assistant_cabinet_ai_lib::ocr::tesseract::TesseractProvider;
    use assistant_cabinet_ai_lib::ocr::OcrProvider;
    use assistant_cabinet_ai_lib::raster::{self, PageRasterizer};
    use std::path::PathBuf;

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let binary = [
        "binaries/tesseract-x86_64-pc-windows-msvc.exe",
        "binaries/tesseract-aarch64-apple-darwin",
        "binaries/tesseract-x86_64-apple-darwin",
    ]
    .iter()
    .map(|p| root.join(p))
    .find(|p| p.exists())
    .expect("the bundled Tesseract is staged");
    let engine =
        TesseractProvider::new(binary, root.join("resources/tessdata")).expect("starts Tesseract");
    let library = [
        "resources/pdfium/pdfium.dll",
        "resources/pdfium/libpdfium.dylib",
        "resources/pdfium/libpdfium.so",
    ]
    .iter()
    .map(|p| root.join(p))
    .find(|p| p.exists())
    .expect("pdfium is staged");
    let rasterizer = raster::shared(&library).expect("binds pdfium");

    let truth = Truth::load();
    let mut lab = lab_like_the_helper(&truth, 30);
    let mut at = 30;
    for tier in [30usize, 100, 200] {
        if tier > at {
            grow(&truth, &lab, at, tier);
            at = tier;
        }
        let renamed = filename_sanitizer::sanitize_folder(lab.work_path())
            .renamed
            .len();
        let mut context = lab.context();
        let url = lab.gateway.url.clone();
        let started = std::time::Instant::now();
        let summary = lab
            .analyse_through(
                &mut context,
                &url,
                Some(&engine as &dyn OcrProvider),
                Some(rasterizer as &dyn PageRasterizer),
            )
            .await;
        let data = lab.analyse_data();
        let inventory = assistant_cabinet_ai_lib::inventory::WorkFolderInventory::discover(
            lab.work_path(),
            Some(&lab.index),
        )
        .unwrap();
        println!(
            "tier {tier}: files in the folder {}, scanned {}, indexed {}, unchanged {}, empty {:?}, read by OCR {}, too little text {:?}, failed {:?}, renamed {renamed}, removed {:?} ({} ms)",
            inventory.all_files().len(), summary.scanned_files, summary.indexed_files, summary.unchanged_files, summary.empty_files,
            summary.ocr_files.len(), summary.low_confidence_files, summary.failed_files.len(), summary.removed_files, started.elapsed().as_millis()
        );
        println!(
            "        data pass: scanned {}, indexed {}, empty {:?}",
            data.scanned_files, data.indexed_files, data.empty_files
        );
        println!(
            "        what the knowledge base says about the documents: {:?}",
            summary.knowledge
        );
        println!("        and about the tables: {:?}", data.knowledge);
    }
}

/// How the tabular engine reads the number and date formats of French-speaking Switzerland and of a French
/// export, written as text in a table: `CHF 1'450.00`, `1'450.00`, `1 450,00 €`, `450,50`, dates `14.10.2026`
/// and `14/10/2026`, an AVS number in a column. The engine is the authority on a number and is unchanged by the
/// knowledge base; this prints what it makes of formats a real library holds. Prints, asserts nothing.
#[tokio::test]
#[ignore]
async fn probe_the_swiss_number_and_date_formats() {
    use assistant_cabinet_ai_lib::analysis_scope::ScopeMode;
    use assistant_cabinet_ai_lib::tabular_answer;
    let mut lab = Lab::new();
    lab.packs = PACKS.to_vec();
    lab.mode = KnowledgeMode::Suggest;
    let swiss = "Date;Patient;Montant;N\u{b0} AVS\n\
        14.10.2026;Sophie Rochat;CHF 1'450.00;756.1234.5678.97\n\
        02.10.2026;Mathieu Cuendet;CHF 380.50;756.2345.6789.01\n\
        21.10.2026;Isabelle Pittet;CHF 1'020.00;756.3456.7890.12\n\
        05.09.2026;Nathalie Chappuis;CHF 95.00;756.4567.8901.23\n\
        18.09.2026;Pascal Maillard;CHF 2'310.25;756.5678.9012.34\n\
        30.09.2026;C\u{e9}line Bovet;CHF 560.00;756.6789.0123.45\n\
        12.10.2026;Alain Jaccard;CHF 75.00;756.7890.1234.56\n\
        26.10.2026;Christelle D\u{e9}l\u{e8}ze;CHF 640.80;756.8901.2345.67\n\
        09.10.2026;Gr\u{e9}goire Crettenand;CHF 1'999.99;756.9012.3456.78\n\
        15.09.2026;Marie-Claude Vuilleumier;CHF 410.00;756.0123.4567.89\n";
    lab.write_data("patients-suisse.csv", swiss);
    let french = "Date;Patient;Montant\n\
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
    lab.write_data("patients-france.csv", french);
    lab.analyse_data();
    let folder = lab.data_folder();
    println!("expected, in both files: total 8941.54, maximum 2310.25, 6 rows in octobre 2026, 1 row for the first patient");
    for question in [
        "Combien de lignes en octobre 2026 dans patients-suisse.csv ?",
        "Quelle est la somme des montants dans patients-suisse.csv ?",
        "Quel est le montant maximum dans patients-suisse.csv ?",
        "Combien de lignes pour Sophie Rochat dans patients-suisse.csv ?",
        "Combien de lignes en octobre 2026 dans patients-france.csv ?",
        "Quelle est la somme des montants dans patients-france.csv ?",
        "Quel est le montant maximum dans patients-france.csv ?",
    ] {
        let pending = tabular_answer::prepare(
            question,
            &folder,
            &ScopeMode::WholeFolder,
            "fr-FR",
            &lab.index,
            false,
        );
        let said: String = describe(&pending).chars().take(240).collect();
        println!("{question}\n    -> {said}");
    }
}

/// What the targeting line and the « which one? » row will say for the questions of the human test, from the same
/// code that the application runs on the same files: the number of files kept out of the selection, the names the
/// base used, the names it asks about. `ACAI_WALK_TIER` (default 30) is the size of the library. Read the result
/// next to the real run's table (`widened` depends on the closest passage by meaning, so it comes from there).
#[tokio::test]
#[ignore]
async fn print_what_the_targeting_line_will_say() {
    let tier: usize = std::env::var("ACAI_WALK_TIER")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);
    let truth = Truth::load();
    let library = library(&truth, tier, &PACKS).await;
    let vocabulary = QueryVocabulary::load("fr-FR", &PACKS).unwrap();
    let replay = Replay::new(&library, &vocabulary);
    let texts: Vec<String> = std::env::var("ACAI_WALK_QUESTIONS")
        .map(|v| v.split('|').map(str::to_string).collect())
        .unwrap_or_else(|_| {
            truth
                .questions
                .iter()
                .filter(|q| q.min_tier <= tier)
                .map(|q| q.text.clone())
                .collect()
        });
    for text in texts {
        let asked = replay.ask(
            &text,
            KnowledgeMode::Auto,
            &TargetingRequest::default(),
            None,
            false,
        );
        println!("{text}");
        let Some(plan) = &asked.plan else {
            println!(
                "    route {} (answered by the folder router, no search)",
                asked.route.code()
            );
            continue;
        };
        let documents = &plan.reduction.documents;
        println!(
            "    route {}, selected {}, outcome {:?}, kept {}, skipped {:?}, reduction {} %",
            asked.route.code(),
            asked.selected.len(),
            documents.outcome,
            documents.candidates.len(),
            documents.skipped.map(|r| r.as_code()),
            documents.reduction_percent()
        );
        if let Some(analysis) = &plan.analysis {
            for e in &analysis.entities {
                println!(
                    "    names used: {} ({}, {:.2}, {}), in {} document(s) of {}",
                    e.display,
                    e.type_id,
                    e.confidence,
                    e.method,
                    e.document_files.len(),
                    asked.selected.len()
                );
            }
            for a in &analysis.ambiguities {
                let options: Vec<String> = a
                    .candidates
                    .iter()
                    .map(|c| format!("{} ({} documents)", c.display, c.documents))
                    .collect();
                println!("    asks which one: {}", options.join(" / "));
            }
            for p in &analysis.possibles {
                let options: Vec<String> = p
                    .candidates
                    .iter()
                    .map(|c| format!("{} ({} documents)", c.display, c.documents))
                    .collect();
                println!("    did you mean: {}", options.join(" / "));
            }
        }
    }
}

// ------------------------------------------------------------------------------------ the final protocol

fn human_tests_folder() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/test-reports/knowledge-base-pass-1/human-tests")
}

fn copy_tree(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            std::fs::create_dir_all(&target).unwrap();
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// What `lot11_helper.py extra-files` writes: the Swiss and French amount tables, the three workbooks and the two
/// `neurologie.txt` of the batch 3 test.
fn write_extra_files(lab: &Lab) {
    let human = human_tests_folder();
    for folder in [
        human.join("lot-11-files").join("data"),
        human.join("fix-04-batch-3-files").join("data"),
    ] {
        for entry in std::fs::read_dir(folder).unwrap() {
            let entry = entry.unwrap();
            lab.write_data_bytes(
                entry.file_name().to_str().unwrap(),
                &std::fs::read(entry.path()).unwrap(),
            );
        }
    }
    copy_tree(
        &human.join("fix-04-batch-3-files").join("docs"),
        lab.work_path(),
    );
}

/// One pass the way the Analyse buttons run it, with the card's figures printed.
async fn card_pass(truth: &Truth, lab: &mut Lab, tier: usize, step: &str) {
    let renamed = filename_sanitizer::sanitize_folder(lab.work_path());
    let (ocr, rasterizer) = perfect_ocr(truth, tier);
    let mut context = lab.context();
    let url = lab.gateway.url.clone();
    let summary = lab
        .analyse_through(&mut context, &url, Some(&ocr), Some(&rasterizer))
        .await;
    let unread: Vec<String> = summary
        .unread_formats
        .iter()
        .map(|f| format!("{} .{}", f.count, f.extension))
        .collect();
    println!(
        "{step} documents card: analysed {}, unchanged {}, removed {}, renamed {}, read by OCR {}, empty {:?}, not read {unread:?}",
        summary.indexed_files,
        summary.unchanged_files,
        summary.removed_files.len(),
        renamed.renamed.len(),
        summary.ocr_files.len(),
        summary.empty_files
    );
    if let Some(k) = &summary.knowledge {
        println!(
            "{step}     names: {} found, {} already known, {} new ({} guesses), {} identifiers, {} documents re-read",
            k.entities_detected, k.matched_existing, k.created_new, k.candidates, k.identifiers, k.refreshed_sources
        );
    }
    let data = lab.analyse_data();
    println!(
        "{step} data card: read {}, analysed {}, unchanged {}, removed {}",
        data.scanned_files,
        data.indexed_files,
        data.unchanged_files,
        data.removed_files.len()
    );
    if let Some(k) = &data.knowledge {
        println!(
            "{step}     names: {} found, {} already known, {} identifiers, {} columns in {} workbooks, {} relations, {} documents re-read",
            k.entities_detected, k.matched_existing, k.identifiers, k.semantic_columns, k.workbooks_analysed, k.relations, k.refreshed_sources
        );
    }
    let inventory = assistant_cabinet_ai_lib::inventory::WorkFolderInventory::discover(
        lab.work_path(),
        Some(&lab.index),
    )
    .unwrap();
    let lit_before_the_fix = inventory
        .all_files()
        .iter()
        .filter(|f| {
            matches!(
                f.processing_status,
                assistant_cabinet_ai_lib::file_record::ProcessingStatus::Discovered
                    | assistant_cabinet_ai_lib::file_record::ProcessingStatus::Pending
            )
        })
        .count();
    println!(
        "{step} Analyser button of the documents card: number {} (before the KBD-47 fix it would read {lit_before_the_fix})",
        inventory.unanalysed_documents()
    );
    let status = inventory.summary();
    println!(
        "{step} documents card status line: {} files, {} analysed, {} unreadable",
        status.total_files, status.indexed_files, status.unreadable_files
    );
    let data_status = lab.data_folder().report().summary;
    println!(
        "{step} data card status line: {} files, {} analysed, {} unreadable",
        data_status.total_files, data_status.analysed_files, data_status.unreadable_files
    );
}

/// The two status lines the folder cards print under their title.
fn status_lines(lab: &Lab, step: &str) {
    let inventory = assistant_cabinet_ai_lib::inventory::WorkFolderInventory::discover(
        lab.work_path(),
        Some(&lab.index),
    )
    .unwrap();
    let status = inventory.summary();
    println!(
        "{step} documents card: {} files, {} analysed, {} unreadable; button number {}",
        status.total_files,
        status.indexed_files,
        status.unreadable_files,
        inventory.unanalysed_documents()
    );
    let data_folder = lab.data_folder();
    let data = data_folder.report().summary;
    let lit = data_folder
        .files()
        .all_files()
        .iter()
        .filter(|f| {
            matches!(
                f.processing_status,
                assistant_cabinet_ai_lib::file_record::ProcessingStatus::Discovered
                    | assistant_cabinet_ai_lib::file_record::ProcessingStatus::Pending
            )
        })
        .count();
    println!(
        "{step} data card: {} files, {} analysed, {} unreadable; button number {lit}",
        data.total_files, data.analysed_files, data.unreadable_files
    );
}

/// The `names` command of the helper, as it prints.
fn names_block(lab: &Lab, step: &str) {
    let rows = entity_list(lab);
    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    for (type_id, status, _) in &rows {
        *counts.entry((type_id.clone(), status.clone())).or_default() += 1;
    }
    for ((type_id, status), number) in &counts {
        println!("{step} names: {type_id:14} {status:10} {number}");
    }
    println!("{step} names: entities: {}", rows.len());
}

fn line(lab: Lab, step: &str, text: &str, selection: Option<&[String]>, workbooks: bool) -> Lab {
    let vocabulary = QueryVocabulary::load("fr-FR", &PACKS).unwrap();
    let library = Library {
        lab,
        tier: 0,
        product_files: Vec::new(),
        analyse_ms: 0,
        summary: None,
        passes: Vec::new(),
    };
    {
        let replay = Replay::new(&library, &vocabulary);
        let asked = replay.ask(
            text,
            KnowledgeMode::Auto,
            &TargetingRequest::default(),
            selection,
            workbooks,
        );
        println!("{step} `{text}`");
        match &asked.plan {
            None => println!(
                "    route {} (answered without a search)",
                asked.route.code()
            ),
            Some(plan) => {
                let d = &plan.reduction.documents;
                println!(
                    "    route {}, outcome {:?}, kept {} of {}, skipped {:?}, applied {}",
                    asked.route.code(),
                    d.outcome,
                    d.candidates.len(),
                    d.selected,
                    d.skipped.map(|r| r.as_code()),
                    d.is_applied()
                );
                if let Some(analysis) = &plan.analysis {
                    for e in &analysis.entities {
                        println!(
                            "    name {} ({}, {:?}, {:.2}) in {} document(s)",
                            e.display,
                            e.type_id,
                            e.status,
                            e.confidence,
                            e.document_files.len()
                        );
                    }
                    for a in &analysis.ambiguities {
                        let options: Vec<String> = a
                            .candidates
                            .iter()
                            .map(|c| format!("{} ({} documents)", c.display, c.documents))
                            .collect();
                        println!("    asks which one: {}", options.join(" / "));
                    }
                    for p in &analysis.possibles {
                        let options: Vec<String> = p
                            .candidates
                            .iter()
                            .map(|c| format!("{} ({} documents)", c.display, c.documents))
                            .collect();
                        println!("    did you mean: {}", options.join(" / "));
                    }
                }
                if d.is_applied() && d.candidates.len() <= 12 {
                    let mut files = d.candidates.clone();
                    files.sort();
                    for file in files {
                        println!("      keeps {file}");
                    }
                }
            }
        }
    }
    library.lab
}

fn data_answer(lab: &Lab, question: &str) -> String {
    use assistant_cabinet_ai_lib::analysis_scope::ScopeMode;
    use assistant_cabinet_ai_lib::tabular_answer::{self, PendingAnswer, TabularAnswer};
    match tabular_answer::prepare(
        question,
        &lab.data_folder(),
        &ScopeMode::WholeFolder,
        "fr-FR",
        &lab.index,
        false,
    ) {
        Ok(PendingAnswer::Done(TabularAnswer::Value { value, locator, .. })) => {
            format!("{value:?} filters={:?}", locator.filters)
        }
        Ok(PendingAnswer::Done(other)) => format!("{other:?}"),
        Ok(_) => "(handed to the model)".to_string(),
        Err(error) => format!("error {error:?}"),
    }
}

fn mixed_answers(lab: &Lab, question: &str) {
    use assistant_cabinet_ai_lib::analysis_scope::ScopeMode;
    use assistant_cabinet_ai_lib::folder_questions::{self, Selection};
    use assistant_cabinet_ai_lib::tabular_answer::{self, PendingAnswer};
    let inventory = assistant_cabinet_ai_lib::inventory::WorkFolderInventory::discover(
        lab.work_path(),
        Some(&lab.index),
    )
    .unwrap();
    let selection = Selection {
        full: &inventory,
        scoped: &inventory,
        narrowed: false,
        no_documents_chosen: false,
    };
    let data_only = match tabular_answer::prepare_if_data_only(
        question,
        &lab.data_folder(),
        &ScopeMode::WholeFolder,
        "fr-FR",
        &lab.index,
    )
    .unwrap()
    {
        None => "None (goes on to the mixed tier)".to_string(),
        Some(PendingAnswer::Done(answer)) => format!("data-only: {answer:?}"),
        Some(_) => "data-only: model".to_string(),
    };
    let folder =
        folder_questions::answer_in_mixed_selection(&selection, &lab.index, question, "fr-FR");
    println!("D `{question}`\n    {data_only}\n    folder answer in the mixed tier: {folder:?}");
}

/// The whole of the final human test, in its order, with every figure it states. Prints; the integrity of the
/// base is asserted after every pass (`clean`), the lifecycle invariants by the test above.
///
/// ```text
/// cargo test --release --test knowledge_validation_walk print_the_figures_of_the_final_protocol -- --ignored --nocapture
/// ```
#[tokio::test]
#[ignore]
async fn print_the_figures_of_the_final_protocol() {
    let truth = Truth::load();
    let mut lab = lab_like_the_helper(&truth, 30);

    println!("=== PART A (30 documents)");
    card_pass(&truth, &mut lab, 30, "A1/A2").await;
    clean(&lab);
    names_block(&lab, "A");
    for (n, text) in [
        "R\u{e9}sume le dossier de Sophie Rochat.",
        "Que dit le dossier de Jean Dupontet ?",
        "Quels courriers viennent du Dr Marc Bersier ?",
        "Quels courriers viennent de la CSS Assurance ?",
        "Montre la facture FAC-2026-014",
        "Quels documents parlent de la LAMal ?",
        "Que dit le dossier de Madame Z\u{e9}nobie Quarante ?",
        "Que dit J. Dupont ?",
        "Qu'a dit le Dr Martin ?",
        "Que dit le courrier de V\u{e9}ronique Monnier ?",
    ]
    .iter()
    .enumerate()
    {
        lab = line(lab, &format!("A6-{}", n + 1), text, None, false);
    }

    println!("=== PART B (30 documents)");
    lab = line(
        lab,
        "B3",
        "Quels courriers viennent du Dr Marc Bersier ?",
        None,
        false,
    );
    {
        let inventory = assistant_cabinet_ai_lib::inventory::WorkFolderInventory::discover(
            lab.work_path(),
            Some(&lab.index),
        )
        .unwrap();
        let without: Vec<String> = inventory
            .indexed_files()
            .into_iter()
            .map(|f| f.relative_path.clone())
            .filter(|p| !p.ends_with("2026-03-20_decompte-css-rochat-sophie.pdf"))
            .collect();
        println!(
            "B4: {} documents ticked of {}",
            without.len(),
            inventory.indexed_files().len()
        );
        lab = line(
            lab,
            "B4",
            "R\u{e9}sume le dossier de Sophie Rochat.",
            Some(&without),
            false,
        );
        lab = line(
            lab,
            "B4",
            "Que dit 2026-03-20_decompte-css-rochat-sophie.pdf ?",
            Some(&without),
            false,
        );
    }
    names_block(&lab, "B5 before");
    for text in [
        "Que dit le dossier de Madame Z\u{e9}nobie Quarante ?",
        "Quels documents concernent Hyacinthe Vaudroz ?",
        "Que dit le Dr Ernest Blondeau ?",
        "Que dit L\u{e9}opold Tschumi ?",
    ] {
        lab = line(lab, "B5", text, None, false);
    }
    names_block(&lab, "B5 after");

    println!("=== PART C (30 documents, lifecycle)");
    names_block(&lab, "C0");
    lab.write("courriers-entrants/courrier-quenneville-nouveau.txt", "Madame Sidonie Quenneville\n12 rue des Acacias, 74100 Annemasse\n\nObjet : demande de rendez-vous.\nMadame Sidonie Quenneville souhaite un rendez-vous de suivi dans le courant du mois d'octobre 2026.\nElle rappelle que sa caisse maladie est la CSS Assurance.\n");
    card_pass(&truth, &mut lab, 30, "C1").await;
    names_block(&lab, "C1");
    lab = line(
        lab,
        "C1",
        "Que dit Madame Sidonie Quenneville ?",
        None,
        false,
    );
    lab.write(
        "Courrier Quenneville \u{e9}t\u{e9} 2026.txt",
        "Madame Sidonie Quenneville confirme son rendez-vous du 14 octobre 2026.\n",
    );
    card_pass(&truth, &mut lab, 30, "C2").await;
    lab = line(
        lab,
        "C2",
        "Que dit Madame Sidonie Quenneville ?",
        None,
        false,
    );

    let listed_before = entity_list(&lab);
    names_block(&lab, "C3 before");
    lab.index.clear().expect("clears the documents");
    status_lines(&lab, "C3a after the Reset");
    card_pass(&truth, &mut lab, 30, "C3a").await;
    lab.index.clear_tabular_inventories().expect("clears");
    lab.index.clear_tabular_workbooks().expect("clears");
    status_lines(&lab, "C3b after the Reset");
    let data = lab.analyse_data();
    println!("C3b data pass: analysed {}", data.indexed_files);
    status_lines(&lab, "C3b after the Analyse");
    assistant_cabinet_ai_lib::knowledge::manage::reset(&lab.side(), true).expect("resets");
    status_lines(&lab, "C3c after the whole Reset");
    card_pass(&truth, &mut lab, 30, "C3c").await;
    names_block(&lab, "C3 after");
    println!("C3: identical list {}", entity_list(&lab) == listed_before);

    let target = truth
        .docs_in(30)
        .find(|d| d.format == "txt" && d.kind == "lettre_confrere" && d.about.len() == 1)
        .map(|d| (d.path.clone(), d.about[0].clone()))
        .expect("a text letter about one patient");
    let person = &truth.entities[&target.1];
    let (given, family) = person.canonical.split_once(' ').unwrap();
    let full = lab.work_path().join(&target.0);
    let text = std::fs::read_to_string(&full).unwrap();
    std::fs::write(&full, text.replace(family, "Qu\u{e9}m\u{e9}ner")).unwrap();
    println!(
        "C4 changed {}: {} -> {given} Quemener",
        target.0, person.canonical
    );
    card_pass(&truth, &mut lab, 30, "C4").await;
    names_block(&lab, "C4");
    lab = line(
        lab,
        "C4",
        "Que dit Laurence Qu\u{e9}m\u{e9}ner ?",
        None,
        false,
    );
    {
        let rows: Vec<(String, String)> = lab
            .side()
            .prepare("SELECT status, canonical_name FROM kb_entities WHERE canonical_name LIKE 'Laurence%' ORDER BY canonical_name")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        println!("C4 every Laurence of the base: {rows:?}");
    }
    println!(
        "C4 Laurence Fabre is in {:?}",
        files_of(&lab, "Laurence Fabre")
    );
    println!(
        "C4 Laurence Quemener is in {:?}",
        files_of(&lab, "Laurence Qu\u{e9}m\u{e9}ner")
    );

    let victim = truth
        .docs_in(30)
        .find(|d| d.format == "pdf" && d.mentions.iter().any(|m| m.entity == "p-rochat-sophie"))
        .map(|d| d.path.clone())
        .expect("a letter about Sophie Rochat");
    std::fs::remove_file(lab.work_path().join(&victim)).unwrap();
    println!("C5 deleted {victim}");
    card_pass(&truth, &mut lab, 30, "C5").await;
    names_block(&lab, "C5");
    lab = line(
        lab,
        "C5",
        "R\u{e9}sume le dossier de Sophie Rochat.",
        None,
        false,
    );
    lab = line(
        lab,
        "C5",
        "Quels courriers viennent de la CSS Assurance ?",
        None,
        false,
    );

    grow(&truth, &lab, 30, 100);
    card_pass(&truth, &mut lab, 100, "C6 up").await;
    names_block(&lab, "C6 at 100");
    shrink(&truth, &lab, 30);
    card_pass(&truth, &mut lab, 30, "C6 down").await;
    names_block(&lab, "C6 back at 30");

    println!("=== PART D (100 documents)");
    grow(&truth, &lab, 30, 100);
    card_pass(&truth, &mut lab, 100, "D1").await;
    names_block(&lab, "D1");
    for (n, text) in [
        "Qui renouvelle le code de l'armoire \u{e0} pharmacie ?",
        "\u{c0} qui signaler un \u{e9}cart de la cha\u{ee}ne du froid ?",
        "R\u{e9}sume le dossier de Fatima Benali.",
    ]
    .iter()
    .enumerate()
    {
        lab = line(lab, &format!("D2-{}", 11 + n), text, None, false);
    }

    println!("=== PART S (Switzerland and the fixes, 100 documents and the extra files)");
    write_extra_files(&lab);
    card_pass(&truth, &mut lab, 100, "S0").await;
    names_block(&lab, "S0");
    for (id, text) in [
        (
            "S1",
            "\u{c0} qui appartient le num\u{e9}ro AVS 756.3052.5778.43 ?",
        ),
        (
            "S2",
            "Quels courriers a \u{e9}crit la Dre Anne-Sophie Vuagniaux ?",
        ),
        ("S3", "Que dit la Dre Vuagniaux ?"),
        ("S4", "Quels documents concernent HUG ?"),
        ("S5", "Quels documents viennent de Gen\u{e8}ve ?"),
        ("S6", "Quels courriers viennent du CHUV ?"),
        (
            "S7",
            "Quels documents concernent les H\u{f4}pitaux universitaires de Gen\u{e8}ve ?",
        ),
        ("S8", "R\u{e9}sume le dossier de Giuseppe D'Amico."),
        ("S9a", "Quels documents parlent de Lucas Roy ?"),
        ("S9b", "Quels documents parlent de Roy Lucas ?"),
        ("S10", "Que dit le courrier de A\u{ef}cha N'Diaye ?"),
    ] {
        lab = line(lab, id, text, None, false);
    }
    println!("=== S tables");
    for (id, question) in [
        (
            "T1",
            "Quelle est la somme des montants dans patients-suisse.csv ?",
        ),
        (
            "T2",
            "Quel est le montant maximum dans patients-suisse.csv ?",
        ),
        (
            "T3",
            "Quelle est la somme des montants dans patients-france.csv ?",
        ),
        (
            "T4",
            "Combien de lignes dans factures-fournisseurs-2026.xlsx ?",
        ),
        ("T5", "Combien de lignes dans rdv-export-titre.csv ?"),
        (
            "T6a",
            "Combien de lignes pour Dr Jean Dupont dans rdv-medecins.csv ?",
        ),
        (
            "T6b",
            "Combien de lignes pour Jean Dupont dans rdv-medecins.csv ?",
        ),
        (
            "T7",
            "Quelle est la somme de montant pour Dr Jean Dupont dans rdv-medecins.csv ?",
        ),
        (
            "T8a",
            "Combien de lignes pour Dr Paul Martin dans rdv-medecins.csv ?",
        ),
        (
            "T8b",
            "Combien de lignes pour Martin dans rdv-medecins.csv ?",
        ),
        (
            "T9",
            "Combien de lignes pour Alice Archer en mars 2026 dans rdv-medecins.csv ?",
        ),
        (
            "T10",
            "Combien de lignes pour Alice Archer dans rdv-dates-ambigues.csv ?",
        ),
        (
            "T11a",
            "Combien de lignes pour Alice Archer en mars 2026 dans rdv-dates-ambigues.csv ?",
        ),
        (
            "T11b",
            "Combien de lignes en 2026 dans rdv-dates-ambigues.csv ?",
        ),
        (
            "T11c",
            "Combien de lignes pour Alice Archer en mars dans rdv-dates-ambigues.csv ?",
        ),
    ] {
        println!("{id} `{question}`\n    -> {}", data_answer(&lab, question));
    }
    println!("=== S files");
    {
        let inventory = assistant_cabinet_ai_lib::inventory::WorkFolderInventory::discover(
            lab.work_path(),
            Some(&lab.index),
        )
        .unwrap();
        for question in [
            "Que dit mars/neurologie.txt ?",
            "Que dit neurologie.txt ?",
            "Que dit avril/neurologie.txt ?",
        ] {
            let routed = assistant_cabinet_ai_lib::folder_questions::route(
                &inventory, &lab.index, question, "fr-FR",
            );
            println!("F `{question}`\n    -> {routed:?}");
        }
        println!("F documents indexed: {}", inventory.indexed_files().len());
    }
    println!("=== S mixed");
    for question in [
        "Combien de lignes ?",
        "Combien de documents ai-je ?",
        "Que disent mes documents \u{e0} propos de Jean Dupont ?",
    ] {
        mixed_answers(&lab, question);
    }

    println!("=== S knowledge window");
    {
        let connection = lab.side();
        let mut statement = connection
            .prepare(
                "SELECT e.type_id, e.status, e.canonical_name,
                        (SELECT COUNT(DISTINCT s.relative_path) FROM kb_mentions m
                           JOIN kb_sources s ON s.source_id = m.source_id
                          WHERE m.entity_id = e.entity_id AND s.domain = 'documents'),
                        (SELECT group_concat(a.display, ' | ') FROM kb_aliases a WHERE a.entity_id = e.entity_id)
                 FROM kb_entities e WHERE e.status IN ('active', 'candidate') AND e.type_id = 'organization'
                 ORDER BY e.canonical_name",
            )
            .unwrap();
        let rows: Vec<(String, String, String, i64, Option<String>)> = statement
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .unwrap()
            .map(|row| row.unwrap())
            .collect();
        for (_, status, name, documents, aliases) in rows {
            println!(
                "K org {status} / {name} / {documents} document(s) / {}",
                aliases.unwrap_or_default()
            );
        }
        let pairs: Vec<(String, String, String)> = connection
            .prepare(
                "SELECT a.canonical_name, b.canonical_name, p.reason
                 FROM kb_possible_matches p
                 JOIN kb_entities a ON a.entity_id = p.entity_a
                 JOIN kb_entities b ON b.entity_id = p.entity_b
                 WHERE p.reason <> 'initial' AND (a.canonical_name LIKE '%Roy%' OR b.canonical_name LIKE '%Roy%')",
            )
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .map(|row| row.unwrap())
            .collect();
        println!("K pairs around Roy: {pairs:?}");
        let persons: Vec<(String, String, i64)> = connection
            .prepare(
                "SELECT e.canonical_name, e.status,
                        (SELECT COUNT(DISTINCT s.relative_path) FROM kb_mentions m
                           JOIN kb_sources s ON s.source_id = m.source_id
                          WHERE m.entity_id = e.entity_id AND s.domain = 'documents')
                 FROM kb_entities e WHERE e.status IN ('active', 'candidate') AND e.type_id = 'person'
                   AND (e.canonical_name LIKE '%Roy%' OR e.canonical_name LIKE '%hotel%')",
            )
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .map(|row| row.unwrap())
            .collect();
        println!("K persons Roy/hotel: {persons:?}");
        let hotels: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM kb_entities WHERE status IN ('active','candidate') AND type_id = 'person' AND normalized_name LIKE '%hotel%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        println!("K persons whose name holds hotel: {hotels}");
    }
    {
        let id: i64 = lab
            .side()
            .query_row(
                "SELECT entity_id FROM kb_entities WHERE canonical_name = 'Lucas Roy' AND status IN ('active','candidate')",
                [],
                |row| row.get(0),
            )
            .expect("Lucas Roy");
        let show = |lab: &Lab, step: &str| {
            let cells = common::knowledge::cells_mentions_of(&lab.side(), id);
            let places: Vec<String> = cells
                .iter()
                .map(|m| {
                    format!(
                        "{} / {} ({}, rows {:?}, {} occurrence(s))",
                        m.path,
                        m.column.clone().unwrap_or_default(),
                        m.method,
                        m.sample_rows,
                        m.occurrences
                    )
                })
                .collect();
            println!(
                "K restore {step}: {} table mention(s) {places:?}",
                cells.len()
            );
        };
        show(&lab, "before");
        let outcome =
            assistant_cabinet_ai_lib::knowledge::manage::delete_entity(&lab.side(), id).unwrap();
        println!("K deleted: documents due {}", outcome.documents_due);
        let outcome =
            assistant_cabinet_ai_lib::knowledge::manage::restore_entity(&lab.side(), id).unwrap();
        println!("K restored: documents due {}", outcome.documents_due);
        show(&lab, "after the restore");
        lab.analyse_data();
        show(&lab, "after the Data Analyse");
        lab.analyse_data();
        show(&lab, "after a second Data Analyse");
    }

    println!("=== PART G (200 documents)");
    grow(&truth, &lab, 100, 200);
    card_pass(&truth, &mut lab, 200, "G1").await;
    names_block(&lab, "G1");
    for (id, text) in [
        (
            "G2-11",
            "Qui renouvelle le code de l'armoire \u{e0} pharmacie ?",
        ),
        (
            "G2-12",
            "\u{c0} qui signaler un \u{e9}cart de la cha\u{ee}ne du froid ?",
        ),
        ("G2-13", "R\u{e9}sume le dossier de Fatima Benali."),
        ("G2-15", "Quels documents concernent la Dre Vuagniaux ?"),
        ("G2-16", "Quels documents viennent de Gen\u{e8}ve ?"),
        ("G2-6", "Quels documents parlent de la LAMal ?"),
    ] {
        lab = line(lab, id, text, None, false);
    }

    if let Ok(dump) = std::env::var("ACAI_WALK_DUMP") {
        let mut listing = Vec::new();
        for (label, root) in [("docs", lab.work_path()), ("data", lab.data.path())] {
            let mut stack = vec![root.to_path_buf()];
            while let Some(folder) = stack.pop() {
                for entry in std::fs::read_dir(&folder).unwrap() {
                    let entry = entry.unwrap();
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                    } else {
                        let bytes = std::fs::read(&path).unwrap();
                        listing.push(format!(
                            "{label}/{} {}",
                            path.strip_prefix(root)
                                .unwrap()
                                .to_string_lossy()
                                .replace('\\', "/"),
                            bytes.len()
                        ));
                    }
                }
            }
        }
        listing.sort();
        std::fs::write(dump, listing.join("\n")).unwrap();
    }
}
