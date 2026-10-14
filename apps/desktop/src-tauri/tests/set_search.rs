//! Step 7 of the file finder work (`docs/LEXICAL-RETRIEVAL-DESIGN.md` 7, `docs/DECISIONS.md` 14 October
//! 2026): the sentence that asks which files talk about X is answered by the finder from the index, with no
//! model and no embedding, and every other sentence keeps its road.
//!
//! French test data, as the pilot writes it. Every name is invented.

mod common;

use assistant_cabinet_ai_lib::discovery::discover_all;
use assistant_cabinet_ai_lib::file_profile::vocabulary::ProfileVocabulary;
use assistant_cabinet_ai_lib::finder::set_search::{answer, identifier_forms};
use assistant_cabinet_ai_lib::finder::InventoryFile;
use assistant_cabinet_ai_lib::folder_questions::{
    route, set_question, FolderAnswer, NoCorpus, QuestionRoute,
};
use assistant_cabinet_ai_lib::inventory::WorkFolderInventory;
use assistant_cabinet_ai_lib::knowledge::packs::PackSet;
use common::knowledge::Lab;
use common::validation::{library, Truth};

const LOCALE: &str = "fr-FR";

fn x(question: &str) -> Option<(String, bool, Option<String>)> {
    set_question(question, LOCALE).map(|set| (set.text, set.quoted, set.kind))
}

#[test]
fn the_frame_reads_the_sentences_that_ask_which_files_talk_about_x() {
    for (question, text, kind) in [
        ("Quels documents parlent de la LAMal ?", "LAMal", None),
        ("Quels documents parlent de LAMal", "LAMal", None),
        (
            "quels documents viennent de Gen\u{e8}ve ?",
            "Gen\u{e8}ve",
            None,
        ),
        ("Quels documents mentionnent Annecy ?", "Annecy", None),
        ("Y a-t-il des documents qui parlent de Nyon ?", "Nyon", None),
        (
            "Combien de documents mentionnent Lausanne ?",
            "Lausanne",
            None,
        ),
        (
            "Quels courriers \u{e9}voquent la franchise annuelle ?",
            "franchise annuelle",
            None,
        ),
        (
            "Quels courriers sont li\u{e9}s \u{e0} Lausanne ?",
            "Lausanne",
            None,
        ),
        ("Quelles lettres mentionnent Nyon ?", "Nyon", Some("letter")),
        (
            "Quels r\u{e9}sultats parlent de Staphylococcus aureus ?",
            "Staphylococcus aureus",
            Some("lab_result"),
        ),
        (
            "Quelles factures concernent MedSupply ?",
            "MedSupply",
            Some("invoice"),
        ),
        ("Quels documents parlent d'Annecy ?", "Annecy", None),
        ("Quels documents concernent HUG ?", "HUG", None),
        (
            "Quels documents contiennent 756.3052.5778.43 ?",
            "756.3052.5778.43",
            None,
        ),
    ] {
        let found = x(question).unwrap_or_else(|| panic!("{question:?} is a set question"));
        assert_eq!(
            (found.0.as_str(), found.2.as_deref()),
            (text, kind),
            "{question:?}"
        );
    }
    let quoted = x("Quels documents mentionnent \"franchise annuelle de base\" ?")
        .expect("a quoted phrase is a set question");
    assert_eq!(
        (quoted.0.as_str(), quoted.1),
        ("franchise annuelle de base", true)
    );
    let guillemets = x("Quels documents mentionnent \u{ab} Hopitaux universitaires de Geneve et du Valais \u{bb} ?")
        .expect("a quoted phrase may be long");
    assert!(guillemets.1);
}

#[test]
fn a_filter_a_long_term_or_a_question_about_meaning_keeps_its_road() {
    for question in [
        // A filter word between the noun and the verb.
        "Quels courriers de 2026 parlent de la LAMal ?",
        "Quels documents r\u{e9}cents parlent de la LAMal ?",
        "Quels documents analys\u{e9}s parlent de Nyon ?",
        // X is a sentence, not a term.
        "Quels documents parlent de ce que Marie Martin a dit au dernier rendez-vous ?",
        "Quels documents parlent de la prise en charge des frais de transport ?",
        // The interrogative as X.
        "Quels documents parlent de quoi ?",
        // What a document says.
        "Que dit le courrier de Marie Dupont ?",
        "R\u{e9}sume le dossier de Fatima Benali.",
        "Quel taux de glyc\u{e9}mie pour Camille ?",
        "Combien de fichiers ?",
        "Combien de documents ?",
        "Liste tous les fichiers",
        // No verb of the frame.
        "Quels documents sont r\u{e9}cents ?",
        "",
    ] {
        assert!(
            x(question).is_none(),
            "{question:?} is not a set question: {:?}",
            x(question)
        );
    }
}

#[test]
fn the_router_sends_the_set_sentence_to_the_search_and_the_others_where_they_went() {
    let root = std::path::Path::new("unused");
    let empty = WorkFolderInventory::from_records(root, Vec::new());
    let routed = route(
        &empty,
        &NoCorpus,
        "Quels documents parlent de la LAMal ?",
        LOCALE,
    );
    let QuestionRoute::SetSearch(set) = routed else {
        panic!("a set sentence goes to the search, not {routed:?}");
    };
    assert_eq!(set.text, "LAMal");

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
    // The English pack has its own frame.
    assert!(matches!(
        route(
            &empty,
            &NoCorpus,
            "Which documents mention Geneva?",
            "en-US"
        ),
        QuestionRoute::SetSearch(_)
    ));
}

fn inventory(folder: &std::path::Path) -> Vec<InventoryFile> {
    discover_all(folder)
        .iter()
        .map(InventoryFile::from)
        .collect()
}

fn listing(answer: &FolderAnswer) -> (usize, Vec<&str>, bool) {
    match answer {
        FolderAnswer::DocumentSearch {
            total,
            listed,
            identifier,
            ..
        } => (
            *total,
            listed.iter().map(|d| d.relative_path.as_str()).collect(),
            *identifier,
        ),
        other => panic!("expected a document search, got {other:?}"),
    }
}

#[tokio::test]
async fn the_search_answers_from_the_index_with_no_gateway_call_and_lists_the_files_it_found() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    lab.write(
        "2026-03-10_courrier-caisse.txt",
        "Courrier de la caisse\nObjet : LAMal, franchise annuelle de 300 francs.\nNumero AVS 756.3052.5778.43\n",
    );
    lab.write(
        "2026-04-02_decompte-css.txt",
        "Decompte CSS\nLa LAMal prend en charge le sejour.\nAssure AVS 7563052577843\n",
    );
    lab.write(
        "2026-05-02_note.txt",
        "Note sans rapport avec la caisse. Annecy.\n",
    );
    lab.write("avs-en-clair.txt", "Assure : 756 3052 5778 43\n");
    lab.analyse().await;
    let requests_before = lab.gateway.requests().len();
    let files = inventory(lab.work_path());
    let connection = lab.side();

    // A word search: the files that hold the word, each with its passage, and the total stated.
    let set =
        set_question("Quels documents parlent de la LAMal ?", LOCALE).expect("a set question");
    let found = answer(&connection, &files, &set, Vec::new()).expect("the search answers");
    let (total, listed, identifier) = listing(&found);
    assert_eq!((total, identifier), (2, false));
    assert_eq!(
        listed,
        vec![
            "2026-03-10_courrier-caisse.txt",
            "2026-04-02_decompte-css.txt"
        ]
    );
    if let FolderAnswer::DocumentSearch {
        terms,
        prefix,
        listed,
        coverage,
        ..
    } = &found
    {
        assert_eq!(terms, &vec!["lamal".to_string()]);
        assert!(!prefix, "a finished sentence is a whole-word search");
        assert!(listed
            .iter()
            .all(|d| d.excerpt.as_deref().is_some_and(|e| e.contains("LAMal"))));
        assert_eq!(coverage.searched, 4);
    }

    // A quoted phrase is one phrase.
    let phrase = set_question(
        "Quels documents mentionnent \"franchise annuelle\" ?",
        LOCALE,
    )
    .expect("a set question");
    let found = answer(&connection, &files, &phrase, Vec::new()).expect("answers");
    assert_eq!(listing(&found).1, vec!["2026-03-10_courrier-caisse.txt"]);

    // The word is a word: a start of it finds nothing.
    let start = set_question("Quels documents parlent de LAMa ?", LOCALE).expect("a set question");
    let found = answer(&connection, &files, &start, Vec::new()).expect("answers");
    assert_eq!(listing(&found).0, 0);

    assert_eq!(
        lab.gateway.requests().len(),
        requests_before,
        "a set search spends no gateway call"
    );
}

#[tokio::test]
async fn an_identifier_is_found_in_every_spelling_it_was_written_in() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    lab.write(
        "2026-03-10_courrier-caisse.txt",
        "Courrier de la caisse\nNumero AVS 756.3052.5778.43\n",
    );
    lab.write(
        "2026-04-02_decompte-css.txt",
        "Decompte CSS\nAssure AVS 7563052577843\n",
    );
    lab.write("avs-en-clair.txt", "Assure : 756 3052 5778 43\n");
    lab.write("autre-assure.txt", "Assure AVS 756.9163.8555.59\n");
    lab.analyse().await;
    let files = inventory(lab.work_path());
    let connection = lab.side();
    let packs = PackSet::load(LOCALE, &["health-fr", "health-ch"]).expect("loads the packs");
    let vocabulary = ProfileVocabulary::load(&["health-fr", "health-ch"]).expect("loads");

    for typed in ["756.3052.5778.43", "7563052577843", "756 3052 5778 43"] {
        let forms = identifier_forms(typed, &packs, &vocabulary);
        assert_eq!(forms, vec!["7563052577843".to_string()], "{typed}");
        let set = set_question(&format!("Quels documents contiennent {typed} ?"), LOCALE)
            .unwrap_or_else(|| panic!("{typed} is a term of a set sentence"));
        let found = answer(&connection, &files, &set, forms).expect("answers");
        let (total, mut listed, identifier) = listing(&found);
        listed.sort_unstable();
        assert!(identifier);
        assert_eq!(
            (total, listed),
            (
                3,
                vec![
                    "2026-03-10_courrier-caisse.txt",
                    "2026-04-02_decompte-css.txt",
                    "avs-en-clair.txt"
                ]
            ),
            "{typed} must meet the number written the other ways, and not the other person's"
        );
    }
    assert!(identifier_forms("Quels documents parlent de Nyon ?", &packs, &vocabulary).is_empty());
}

#[tokio::test]
async fn the_six_set_sentences_of_the_final_test_find_their_files_and_nothing_else() {
    let truth = Truth::load();
    let library = library(&truth, 200, &["health-fr", "health-ch"]).await;
    let files = inventory(library.lab.work_path());
    let connection = library.lab.side();
    let requests_before = library.lab.gateway.requests().len();

    let mut expected_total = 0;
    let mut found_total = 0;
    let mut wrong_total = 0;
    for (id, question) in [
        ("Q010", "Quels documents parlent de la LAMal ?"),
        (
            "Q011",
            "Quels courriers \u{e9}voquent la franchise annuelle ?",
        ),
        ("Q012", "Quels documents viennent de Gen\u{e8}ve ?"),
        ("Q013", "Quels courriers sont li\u{e9}s \u{e0} Lausanne ?"),
        ("Q014", "Y a-t-il des documents qui parlent de Nyon ?"),
        ("Q015", "Quels documents mentionnent Annecy ?"),
    ] {
        let truth_question = truth
            .questions
            .iter()
            .find(|q| q.id == id)
            .unwrap_or_else(|| panic!("{id} is in the sandbox"));
        let expected: Vec<&str> = truth_question
            .relevant_readable
            .iter()
            .filter(|path| truth.doc(path).is_some_and(|doc| doc.index <= 200))
            .map(String::as_str)
            .collect();
        let set =
            set_question(question, LOCALE).unwrap_or_else(|| panic!("{id} is a set question"));
        let found = answer(&connection, &files, &set, Vec::new()).expect("answers");
        let (total, _, _) = listing(&found);
        let FolderAnswer::DocumentSearch { listed, others, .. } = &found else {
            unreachable!()
        };
        let mut paths: Vec<&str> = listed.iter().map(|d| d.relative_path.as_str()).collect();
        paths.extend(others.iter().map(String::as_str));
        let hit = expected.iter().filter(|path| paths.contains(path)).count();
        let wrong: Vec<&&str> = paths
            .iter()
            .filter(|path| !expected.contains(path))
            .collect();
        println!(
            "{id} {question}: expected {} found {hit} listed {total} wrong {} kind {:?}",
            expected.len(),
            wrong.len(),
            set.kind
        );
        expected_total += expected.len();
        found_total += hit;
        wrong_total += wrong.len();
    }
    println!("six sentences: {found_total} of {expected_total}, wrong files {wrong_total}");
    assert_eq!(expected_total, 111);
    assert!(found_total >= 105, "{found_total} of {expected_total}");
    assert_eq!(wrong_total, 0, "no wrong file");
    assert_eq!(
        library.lab.gateway.requests().len(),
        requests_before,
        "the six sentences spend no gateway call"
    );

    // P-2: the latest letter received is named, with the rank it took.
    let inventory = assistant_cabinet_ai_lib::inventory::WorkFolderInventory::discover(
        library.lab.work_path(),
        Some(&library.lab.index),
    )
    .expect("discovers the library");
    let ranked = match route(
        &inventory,
        &NoCorpus,
        "Quel est le dernier courrier re\u{e7}u ?",
        LOCALE,
    ) {
        QuestionRoute::Deterministic(FolderAnswer::RankedFile {
            ranking,
            candidates,
            ..
        }) => {
            println!(
                "latest letter among {candidates} readable ones: {:?}",
                ranking.first()
            );
            ranking
        }
        other => panic!("the rank sentence is a ranking, not {other:?}"),
    };
    // The truth is the latest name date among the readable files whose path has the word "courrier".
    let latest_date = truth
        .docs_in(200)
        .filter(|doc| doc.readable && doc.product_reads())
        .filter(|doc| {
            doc.path
                .split(|c: char| !c.is_alphanumeric())
                .any(|word| word == "courrier" || word == "courriers")
        })
        .filter_map(|doc| {
            let name = doc.path.rsplit('/').next().unwrap_or(&doc.path);
            name.get(..10)
                .filter(|date| chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok())
                .map(str::to_string)
        })
        .max()
        .expect("the library holds dated letters");
    assert_eq!(ranked[0].date, latest_date);
    assert_eq!(ranked[0].basis, "filename");
    assert!(
        ranked.windows(2).all(|pair| pair[0].date >= pair[1].date),
        "the ranking is in date order"
    );

    // Every health insurance number of the library, typed without a separator, finds every file that holds
    // it, however the file wrote it. The truth is a plain pattern over the stored passages, not the packs.
    let pattern = regex::Regex::new(r"\b756[. ]?\d{4}[. ]?\d{4}[. ]?\d{2}\b").expect("pattern");
    let mut holders: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
        std::collections::BTreeMap::new();
    let mut statement = connection
        .prepare("SELECT relative_path, text FROM chunks")
        .expect("prepares");
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .expect("reads the passages");
    for (path, text) in rows.flatten() {
        for found in pattern.find_iter(&text) {
            let compact: String = found
                .as_str()
                .chars()
                .filter(char::is_ascii_digit)
                .collect();
            holders.entry(compact).or_default().insert(path.clone());
        }
    }
    let packs = PackSet::load(LOCALE, &["health-fr", "health-ch"]).expect("loads the packs");
    let vocabulary = ProfileVocabulary::load(&["health-fr", "health-ch"]).expect("loads");
    let (mut numbers_found, mut files_found, mut files_expected, mut wrong) = (0, 0, 0, 0);
    for (compact, files_holding) in &holders {
        let set = set_question(&format!("Quels documents contiennent {compact} ?"), LOCALE)
            .expect("a set question");
        let forms = identifier_forms(&set.text, &packs, &vocabulary);
        let found = answer(&connection, &files, &set, forms).expect("answers");
        let FolderAnswer::DocumentSearch { listed, others, .. } = &found else {
            unreachable!()
        };
        let paths: std::collections::BTreeSet<String> = listed
            .iter()
            .map(|d| d.relative_path.clone())
            .chain(others.iter().cloned())
            .collect();
        let hit = files_holding.intersection(&paths).count();
        files_expected += files_holding.len();
        files_found += hit;
        wrong += paths.difference(files_holding).count();
        if hit == files_holding.len() {
            numbers_found += 1;
        }
    }
    println!(
        "health insurance numbers typed without separators: {numbers_found} of {} numbers fully found, files {files_found} of {files_expected}, wrong files {wrong}",
        holders.len()
    );
    assert!(!holders.is_empty());
    assert_eq!(
        files_found, files_expected,
        "every file that holds a number is found"
    );
    assert_eq!(wrong, 0);
}
