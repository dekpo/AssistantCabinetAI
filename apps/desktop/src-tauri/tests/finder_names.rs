//! Tab 1 of the long-documents chain, item 3: X of the set sentence is a name. When it is exactly one entity of
//! the knowledge base the answer is the files linked to it and the files that write its surfaces; two
//! entities or none leave X a phrase. Every name is invented.

mod common;

use assistant_cabinet_ai_lib::discovery::discover_all;
use assistant_cabinet_ai_lib::finder::names::{KnowledgeNames, NameResolver};
use assistant_cabinet_ai_lib::finder::set_search::{answer, answer_with_names};
use assistant_cabinet_ai_lib::finder::InventoryFile;
use assistant_cabinet_ai_lib::folder_questions::{
    route, set_question, FolderAnswer, NoCorpus, QuestionRoute,
};
use assistant_cabinet_ai_lib::inventory::WorkFolderInventory;
use assistant_cabinet_ai_lib::knowledge::manage::{self, NameTools};
use assistant_cabinet_ai_lib::knowledge::packs::PackSet;
use common::knowledge::{entity_named, Lab};

const LOCALE: &str = "fr-FR";

fn inventory(folder: &std::path::Path) -> Vec<InventoryFile> {
    discover_all(folder)
        .iter()
        .map(InventoryFile::from)
        .collect()
}

fn paths(answer: &FolderAnswer) -> (Option<String>, Vec<String>) {
    let FolderAnswer::DocumentSearch {
        entity,
        listed,
        others,
        total,
        ..
    } = answer
    else {
        panic!("expected a document search, got {answer:?}");
    };
    let mut found: Vec<String> = listed
        .iter()
        .map(|d| d.relative_path.clone())
        .chain(others.iter().cloned())
        .collect();
    assert_eq!(*total, found.len(), "the total is the number of files");
    found.sort();
    (entity.clone(), found)
}

async fn lab() -> Lab {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    lab.write(
        "2026-05-20_ordonnance.txt",
        "Ordonnance du 20 mai 2026\nCabinet medical des Voirons\nPatient : Marie Martin\nOmeprazole, un comprime matin et soir.\n",
    );
    lab.write(
        "2026-06-03_courrier-dermatologie.txt",
        "Courrier du 3 juin 2026\nHopitaux universitaires de Geneve\nMadame Marie Martin a ete recue pour une consultation de dermatologie.\n",
    );
    lab.write(
        "2026-07-01_note-service.txt",
        "Note de service\nLe planning des vacances a ete adopte.\n",
    );
    lab.write(
        "2026-07-09_courrier-roy.txt",
        "Courrier du 9 juillet 2026\nPatient : Pierre Roy\nConsultation de controle.\n",
    );
    lab.write(
        "2026-07-10_courrier-roy-paul.txt",
        "Courrier du 10 juillet 2026\nPatient : Paul Roy\nConsultation de controle.\n",
    );
    lab.analyse().await;
    lab
}

#[tokio::test]
async fn a_name_that_is_one_entity_finds_its_files_and_two_entities_stay_a_phrase() {
    let lab = lab().await;
    let files = inventory(lab.work_path());
    let connection = lab.side();
    let vocabulary = lab.vocabulary();
    let names = KnowledgeNames {
        connection: &connection,
        vocabulary: &vocabulary,
    };

    // One entity: the files that write the name, named as the entity.
    let set =
        set_question("Quels documents parlent de Marie Martin ?", LOCALE).expect("a set question");
    let resolved = names
        .resolve_one(&set.text)
        .expect("Marie Martin is one entity");
    assert!(resolved.display.contains("Martin"), "{resolved:?}");
    let found = answer_with_names(&connection, &files, &set, Vec::new(), &names).expect("answers");
    let (entity, listed) = paths(&found);
    assert!(entity.is_some_and(|name| name.contains("Martin")));
    assert_eq!(
        listed,
        vec![
            "2026-05-20_ordonnance.txt".to_string(),
            "2026-06-03_courrier-dermatologie.txt".to_string()
        ]
    );

    // Without the port the sentence is the word search it was, and finds the same two files here.
    let plain = answer(&connection, &files, &set, Vec::new()).expect("answers");
    assert_eq!(paths(&plain).0, None);

    // Two entities answer to the surname: never chosen between, X stays a phrase.
    let set = set_question("Quels documents parlent de Roy ?", LOCALE).expect("a set question");
    assert!(
        names.resolve_one(&set.text).is_none(),
        "two people named Roy are not one entity"
    );
    let found = answer_with_names(&connection, &files, &set, Vec::new(), &names).expect("answers");
    let (entity, listed) = paths(&found);
    assert_eq!(entity, None);
    assert_eq!(
        listed,
        vec![
            "2026-07-09_courrier-roy.txt".to_string(),
            "2026-07-10_courrier-roy-paul.txt".to_string()
        ]
    );

    // No entity at all: a phrase, as before.
    let set = set_question("Quels documents parlent de Nyon ?", LOCALE).expect("a set question");
    assert!(names.resolve_one(&set.text).is_none());
    let found = answer_with_names(&connection, &files, &set, Vec::new(), &names).expect("answers");
    assert_eq!(paths(&found), (None, Vec::new()));
}

#[tokio::test]
async fn another_name_of_the_entity_finds_the_files_that_write_the_first_one() {
    let mut lab = lab().await;
    // The owner teaches the base a second name; the next Analyse reads the files again with it.
    let packs =
        PackSet::load_with_type_vocabulary(lab.locale, &lab.packs).expect("loads the packs");
    let marie = entity_named(&lab.side(), "Marie Martin").expect("the base knows her");
    manage::add_alias(
        &lab.side(),
        &NameTools::from_packs(&packs, lab.locale),
        marie.entity_id,
        "Mimi Martin",
    )
    .expect("adds the alias");
    lab.write(
        "2026-08-02_rappel.txt",
        "Rappel de rendez-vous
Patient : Mimi Martin
Prevoir une consultation de suivi.
",
    );
    lab.analyse().await;
    let files = inventory(lab.work_path());
    let connection = lab.side();
    let vocabulary = lab.vocabulary();
    let names = KnowledgeNames {
        connection: &connection,
        vocabulary: &vocabulary,
    };
    let set =
        set_question("Quels documents parlent de Mimi Martin ?", LOCALE).expect("a set question");

    // The words she typed meet the one file that writes them.
    let plain = paths(&answer(&connection, &files, &set, Vec::new()).expect("answers"));
    assert_eq!(
        plain,
        (None, vec!["2026-08-02_rappel.txt".to_string()]),
        "the words alone"
    );
    // The name is the entity: its three files, whichever of its names each one writes.
    let (entity, named) =
        paths(&answer_with_names(&connection, &files, &set, Vec::new(), &names).expect("answers"));
    assert!(entity.is_some_and(|name| name.contains("Marie Martin")));
    assert_eq!(
        named,
        vec![
            "2026-05-20_ordonnance.txt".to_string(),
            "2026-06-03_courrier-dermatologie.txt".to_string(),
            "2026-08-02_rappel.txt".to_string()
        ]
    );
}

#[test]
fn the_sentences_about_what_a_document_says_keep_their_road() {
    let root = std::path::Path::new("unused");
    let empty = WorkFolderInventory::from_records(root, Vec::new());
    for question in [
        "Que dit le courrier de Marie Dupont ?",
        "R\u{e9}sume le dossier de Fatima Benali.",
    ] {
        assert_eq!(
            route(&empty, &NoCorpus, question, LOCALE),
            QuestionRoute::GlobalRetrieval,
            "{question:?} keeps its road"
        );
    }
    assert!(matches!(
        route(
            &empty,
            &NoCorpus,
            "Quels documents parlent de Marie Dupont ?",
            LOCALE
        ),
        QuestionRoute::SetSearch(_)
    ));
}
