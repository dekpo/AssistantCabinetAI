//! P-2 (`docs/LEXICAL-RETRIEVAL-DESIGN.md`): "what is the latest letter received" is a ranking of the
//! inventory, answered with no model, and it says how the files ranked. French test data.

mod common;

use std::time::{Duration, SystemTime};

use assistant_cabinet_ai_lib::folder_questions::{route, FolderAnswer, NoCorpus, QuestionRoute};
use assistant_cabinet_ai_lib::inventory::WorkFolderInventory;
use common::knowledge::Lab;

const LOCALE: &str = "fr-FR";

fn modified_on(lab: &Lab, name: &str, day: &str) {
    let seconds = chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d")
        .expect("a date")
        .and_hms_opt(12, 0, 0)
        .expect("a time")
        .and_utc()
        .timestamp();
    let file = std::fs::File::options()
        .write(true)
        .open(lab.work_path().join(name))
        .expect("opens the file");
    file.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds as u64))
        .expect("sets the time");
}

fn ranking(inventory: &WorkFolderInventory, question: &str, locale: &str) -> FolderAnswer {
    match route(inventory, &NoCorpus, question, locale) {
        QuestionRoute::Deterministic(answer) => answer,
        other => panic!("{question:?} is a ranking, not {other:?}"),
    }
}

fn listed(answer: &FolderAnswer) -> Vec<(String, String, String)> {
    match answer {
        FolderAnswer::RankedFile { ranking, .. } => ranking
            .iter()
            .map(|entry| {
                (
                    entry.relative_path.clone(),
                    entry.date.clone(),
                    entry.basis.clone(),
                )
            })
            .collect(),
        other => panic!("expected a ranking, got {other:?}"),
    }
}

#[tokio::test]
async fn the_latest_letter_is_named_among_the_readable_ones_and_the_answer_says_how_they_ranked() {
    let mut lab = Lab::new();
    lab.write(
        "2026-10-01_courrier-dermatologie.txt",
        "Courrier de dermatologie.\n",
    );
    lab.write("2026-03-05_courrier-orl.txt", "Courrier d'ORL.\n");
    lab.write(
        "courrier-sans-date.txt",
        "Courrier sans date dans le nom.\n",
    );
    lab.write("2026-11-20_facture-medsupply.txt", "Facture MedSupply.\n");
    lab.write("courrier-vide.txt", "");
    lab.analyse().await;
    modified_on(&lab, "courrier-sans-date.txt", "2026-06-01");
    let inventory =
        WorkFolderInventory::discover(lab.work_path(), Some(&lab.index)).expect("discovers");

    let latest = ranking(
        &inventory,
        "Quel est le dernier courrier re\u{e7}u ?",
        LOCALE,
    );
    assert_eq!(
        listed(&latest),
        vec![
            (
                "2026-10-01_courrier-dermatologie.txt".to_string(),
                "2026-10-01".to_string(),
                "filename".to_string()
            ),
            (
                "courrier-sans-date.txt".to_string(),
                "2026-06-01".to_string(),
                "modified".to_string()
            ),
            (
                "2026-03-05_courrier-orl.txt".to_string(),
                "2026-03-05".to_string(),
                "filename".to_string()
            ),
        ],
        "the invoice is not a letter and the empty file is not readable"
    );
    let FolderAnswer::RankedFile {
        latest: is_latest,
        noun,
        candidates,
        unranked,
        mixed_basis,
        ..
    } = &latest
    else {
        unreachable!()
    };
    assert_eq!(
        (
            *is_latest,
            noun.as_deref(),
            *candidates,
            *unranked,
            *mixed_basis
        ),
        (true, Some("courrier"), 3, 0, true)
    );

    // The earliest, and the noun first.
    assert_eq!(
        listed(&ranking(
            &inventory,
            "Quel est le premier courrier ?",
            LOCALE
        ))[0]
            .0,
        "2026-03-05_courrier-orl.txt"
    );
    assert_eq!(
        listed(&ranking(
            &inventory,
            "Le courrier le plus r\u{e9}cent",
            LOCALE
        ))[0]
            .0,
        "2026-10-01_courrier-dermatologie.txt"
    );
    // A noun that names no kind covers every readable document.
    let any = ranking(&inventory, "Quel est le dernier document ?", LOCALE);
    assert_eq!(listed(&any)[0].0, "2026-11-20_facture-medsupply.txt");
    if let FolderAnswer::RankedFile {
        noun, candidates, ..
    } = &any
    {
        assert_eq!((noun.as_deref(), *candidates), (None, 4));
    }
    // The plural meets the singular in a name.
    assert_eq!(
        listed(&ranking(
            &inventory,
            "Quelle est la derni\u{e8}re facture ?",
            LOCALE
        ))[0]
            .0,
        "2026-11-20_facture-medsupply.txt"
    );
    // The English pack has its own frame.
    assert_eq!(
        listed(&ranking(
            &inventory,
            "What is the latest document?",
            "en-US"
        ))[0]
            .0,
        "2026-11-20_facture-medsupply.txt"
    );
}

#[tokio::test]
async fn any_other_word_is_a_filter_and_the_sentence_keeps_its_road() {
    let mut lab = Lab::new();
    lab.write(
        "2026-10-01_courrier-dermatologie.txt",
        "Courrier de dermatologie.\n",
    );
    lab.analyse().await;
    let inventory =
        WorkFolderInventory::discover(lab.work_path(), Some(&lab.index)).expect("discovers");

    for question in [
        "Quel est le dernier courrier de dermatologie re\u{e7}u ?",
        "Que dit le dernier courrier ?",
        "R\u{e9}sume le dernier courrier.",
        "Quel est le dernier courrier de 2026 ?",
    ] {
        assert!(
            !matches!(
                route(&inventory, &NoCorpus, question, LOCALE),
                QuestionRoute::Deterministic(FolderAnswer::RankedFile { .. })
            ),
            "{question:?} is not a ranking"
        );
    }
}
