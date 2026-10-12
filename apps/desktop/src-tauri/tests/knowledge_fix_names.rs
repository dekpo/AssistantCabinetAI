//! KB fix 4, batch 2 (`fix/kb-names-and-organisations`): the extractor reads what a real library writes.
//! Every test here was written first and seen failing, then the fix was made.
//!
//! - F-06 (KBD-27): organisations without a marker word (HUG, CHUV, SUVA, URSSAF...) and with lowercase words.
//! - F-07 (KBD-28, KBD-11): organisation names cut at the edges, an address fragment, an untitled run.
//! - F-08 (KBD-31, KBD-32, KBD-33): a name alone in an address block, a name split by a line break, a particle.
//! - F-09 (KBD-25, KBD-36): swapped given name and surname are two people; an apostrophe is part of a surname.
//! - F-10 (KBD-12, KBD-13): a promoted candidate is demoted; the strongest method of a place is kept.
//! - F-04 (KBD-20): a restored name has its table mention back.
//!
//! Every name here is invented. The sandbox figures of the batch are in the batch report.

mod common;

use assistant_cabinet_ai_lib::knowledge::manage;
use assistant_cabinet_ai_lib::knowledge::reduce::SideInput;
use assistant_cabinet_ai_lib::knowledge::targeting::TargetingRequest;
use assistant_cabinet_ai_lib::knowledge::{KnowledgeMode, CURRENT_KB_VERSION};

use common::corpus::{clean, filler, plan_with};
use common::knowledge::{
    cells_mentions_of, entities, entity_named, mentions_of, small_world_policy, EntityRow, Lab,
};

fn world(files: &[(&str, &str)]) -> Lab {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    for (name, text) in files {
        lab.write(name, text);
    }
    lab
}

fn named(lab: &Lab, canonical: &str) -> Option<EntityRow> {
    entity_named(&lab.side(), canonical)
}

fn canonicals(lab: &Lab, type_id: &str) -> Vec<String> {
    entities(&lab.side())
        .into_iter()
        .filter(|row| row.type_id == type_id)
        .map(|row| row.canonical)
        .collect()
}

fn files_of(lab: &Lab, entity_id: i64) -> Vec<String> {
    let mut files: Vec<String> = mentions_of(&lab.side(), entity_id)
        .into_iter()
        .map(|mention| mention.path)
        .collect();
    files.sort();
    files.dedup();
    files
}

#[test]
fn the_extractor_generation_was_raised_for_this_batch() {
    assert!(
        CURRENT_KB_VERSION >= 3,
        "documents read by the earlier extractor must be read again at the next Analyse"
    );
}

// ---------------------------------------------------------------------------------------- F-06

#[tokio::test]
async fn well_known_organisations_without_a_marker_word_are_organisations() {
    let mut lab = world(&[
        (
            "courrier-chuv.txt",
            "Courrier du CHUV au sujet du suivi de la patiente.\n",
        ),
        (
            "rapport-hug.txt",
            "Compte rendu des HUG, service de cardiologie.\n",
        ),
        (
            "attestation-suva.txt",
            "Attestation de la SUVA pour le dossier.\n",
        ),
        (
            "cotisations.txt",
            "Courrier de l'URSSAF Rh\u{f4}ne-Alpes sur les cotisations.\n",
        ),
        (
            "lyon.txt",
            "Courrier des Hospices Civils de Lyon et du CHU Grenoble Alpes.\n",
        ),
        (
            "federation.txt",
            "Le rapport de la F\u{e9}d\u{e9}ration des m\u{e9}decins suisses est joint.\n",
        ),
    ]);
    lab.analyse().await;
    let organisations = canonicals(&lab, "organization");
    for wanted in [
        "Centre hospitalier universitaire vaudois",
        "H\u{f4}pitaux universitaires de Gen\u{e8}ve",
        "SUVA",
        "URSSAF Rh\u{f4}ne-Alpes",
        "Hospices Civils de Lyon",
        "CHU Grenoble Alpes",
        "F\u{e9}d\u{e9}ration des m\u{e9}decins suisses",
    ] {
        let row = named(&lab, wanted)
            .unwrap_or_else(|| panic!("no entity named {wanted:?} among {organisations:?}"));
        assert_eq!(row.type_id, "organization", "{wanted}");
    }
    // The acronym and the full name are one organisation: the letter that writes only `CHUV`
    // is tied to the entity of the full name.
    let chuv = named(&lab, "Centre hospitalier universitaire vaudois").unwrap();
    assert_eq!(files_of(&lab, chuv.entity_id), ["courrier-chuv.txt"]);
    // Never a person.
    let people = canonicals(&lab, "person");
    for name in &people {
        for wrong in ["CHUV", "HUG", "SUVA", "URSSAF", "Hospices", "CHU "] {
            assert!(!name.contains(wrong), "{name:?} is filed as a person");
        }
    }
    clean(&lab);
}

#[tokio::test]
async fn an_ordinary_acronym_or_word_is_not_an_organisation() {
    let mut lab = world(&[
        (
            "bilan.txt",
            "Le bilan montre une IRM normale, un taux de TSH correct et une TVA de 8 pour cent.\n",
        ),
        (
            "autre.txt",
            "Il a envoy\u{e9} le PDF avec un hug amical, selon l'IBAN indiqu\u{e9}.\n",
        ),
    ]);
    lab.analyse().await;
    assert!(
        canonicals(&lab, "organization").is_empty(),
        "{:?}",
        canonicals(&lab, "organization")
    );
    clean(&lab);
}

#[tokio::test]
async fn a_marker_followed_by_lowercase_words_still_names_an_organisation() {
    let mut lab = world(&[
        (
            "zurich.txt",
            "Courrier des H\u{f4}pitaux universitaires de Zurich, service de cardiologie.\n",
        ),
        (
            "seconde.txt",
            "Rapport de la Clinique priv\u{e9}e Bois-Cerf sur le suivi.\n",
        ),
    ]);
    lab.analyse().await;
    for wanted in [
        "H\u{f4}pitaux universitaires de Zurich",
        "Clinique priv\u{e9}e Bois-Cerf",
    ] {
        let organisations = canonicals(&lab, "organization");
        let row = named(&lab, wanted)
            .unwrap_or_else(|| panic!("no entity named {wanted:?} among {organisations:?}"));
        assert_eq!(row.type_id, "organization");
    }
    clean(&lab);
}

// ---------------------------------------------------------------------------------------- F-07

#[tokio::test]
async fn an_organisation_is_not_cut_at_its_edges() {
    let mut lab = world(&[
        (
            "accord.txt",
            "Entre Bureautique L\u{e9}man SA et la Papeterie du Lac SARL, un accord a \u{e9}t\u{e9} sign\u{e9}.\n",
        ),
        (
            "factures.txt",
            "Les factures viennent du Garage Martin & Fils SARL et de l'\u{c9}tude Chappuis & Associ\u{e9}s.\n",
        ),
        (
            "bail.txt",
            "Le bailleur est Dupont Immobilier SA. Le loyer est index\u{e9}.\n",
        ),
        (
            "bail-bis.txt",
            "Courrier de Dupont Immobilier au locataire concernant le loyer.\n",
        ),
    ]);
    lab.analyse().await;
    let organisations = canonicals(&lab, "organization");
    for wanted in [
        "Bureautique L\u{e9}man SA",
        "Papeterie du Lac SARL",
        "Garage Martin & Fils SARL",
        "\u{c9}tude Chappuis & Associ\u{e9}s",
        "Dupont Immobilier SA",
    ] {
        assert!(
            organisations.iter().any(|name| name == wanted),
            "{wanted:?} not among {organisations:?}"
        );
    }
    for wrong in ["Entre", "Fils SARL", "l'\u{c9}tude"] {
        assert!(
            organisations.iter().all(|name| !name.starts_with(wrong)),
            "{organisations:?}"
        );
    }
    let people = canonicals(&lab, "person");
    assert!(
        people.iter().all(|name| !name.contains("Immobilier")
            && !name.contains("Garage")
            && !name.contains("tude")),
        "an organisation is filed as a person: {people:?}"
    );
    // The letter that writes the name without its legal suffix is tied to the organisation.
    let landlord = named(&lab, "Dupont Immobilier SA").unwrap();
    assert_eq!(
        files_of(&lab, landlord.entity_id),
        ["bail-bis.txt", "bail.txt"]
    );
    clean(&lab);
}

#[tokio::test]
async fn an_address_fragment_is_not_a_person() {
    let mut files: Vec<(String, String)> = (1..=3)
        .map(|n| {
            (
                format!("pharmacie-{n}.txt"),
                "Pharmacie, 12 place de l'H\u{f4}tel de Ville, 74100 Annemasse. Ouverte du lundi au samedi.\n"
                    .to_string(),
            )
        })
        .collect();
    files.extend((1..=2).map(|n| {
        (
            format!("rue-{n}.txt"),
            "Courrier envoy\u{e9} au 14 rue Jean Jaur\u{e8}s \u{e0} Annemasse ce mois-ci.\n"
                .to_string(),
        )
    }));
    files.push((
        "auteur.txt".to_string(),
        "Le rapport \u{e9}crit par Alice Archer est joint.\n".to_string(),
    ));
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    for (name, text) in &files {
        lab.write(name, text);
    }
    lab.analyse().await;
    let people = canonicals(&lab, "person");
    assert!(
        people
            .iter()
            .all(|name| !name.contains("Ville") && !name.contains("Jaur")),
        "an address is filed as a person: {people:?}"
    );
    // The control: a real untitled name is still a candidate.
    assert!(named(&lab, "Alice Archer").is_some(), "{people:?}");
    clean(&lab);
}

// ---------------------------------------------------------------------------------------- F-08

#[tokio::test]
async fn a_name_alone_in_an_address_block_is_read() {
    let mut lab = world(&[
        (
            "courrier-caisse-1.txt",
            "Madame, Monsieur,\n\nAnne Marchand\n12 rue Pasteur, 74100 Annemasse\n\nNous avons trait\u{e9} votre dossier.\n",
        ),
        (
            "courrier-caisse-2.txt",
            "Madame\nSophie Rochat\nAvenue de la Gare 14, 1003 Lausanne\n\nNous avons trait\u{e9} votre dossier.\n",
        ),
        (
            "courrier-caisse-3.txt",
            "Florence Rime\nRue du Rh\u{f4}ne 5\n1204 Gen\u{e8}ve\n\nVotre d\u{e9}compte est joint.\n",
        ),
    ]);
    lab.analyse().await;
    let people = canonicals(&lab, "person");
    for wanted in ["Anne Marchand", "Sophie Rochat", "Florence Rime"] {
        assert!(
            people.iter().any(|name| name == wanted),
            "{wanted:?} not among {people:?}"
        );
    }
    // The street line of an address block is never a person.
    assert!(
        people
            .iter()
            .all(|name| !name.contains("Rh\u{f4}ne") && !name.contains("Gare")),
        "{people:?}"
    );
    clean(&lab);
}

#[tokio::test]
async fn a_heading_alone_on_a_line_is_not_a_person() {
    let mut lab = world(&[
        (
            "a.txt",
            "Compte Rendu\nBilan Sanguin\nLe patient va bien et reviendra dans un mois.\n",
        ),
        (
            "b.txt",
            "Compte Rendu\nBilan Sanguin\nLes r\u{e9}sultats sont normaux dans l'ensemble.\n",
        ),
    ]);
    lab.analyse().await;
    assert!(
        canonicals(&lab, "person").is_empty(),
        "{:?}",
        canonicals(&lab, "person")
    );
    clean(&lab);
}

/// A PDF wraps its lines at the margin: several lines of the text stop there, about as long as the longest,
/// and a name can go on under the last of them.
fn wrapped(last_lines: [&str; 2]) -> String {
    format!(
        "Le guide pratique de la consultation a ete relu en detail par toute l'equipe medicale et\n\
         par plusieurs confreres qui ont ajoute leurs remarques sur chaque chapitre du document puis\n\
         {}\n{}\n",
        last_lines[0], last_lines[1]
    )
}

#[tokio::test]
async fn a_name_split_by_a_line_break_is_read() {
    let guide = wrapped([
        "le texte final a ete soumis pour une derniere validation a la direction du site au Pr",
        "Jean-Luc Rapin qui le relira avant la fin du mois.",
    ]);
    let transmission = wrapped([
        "le texte final a ete soumis pour une derniere validation a la direction du site a Jean",
        "Dupont qui le relira avant la fin du mois.",
    ]);
    let titled = wrapped([
        "le texte final a ete soumis pour une derniere validation a la direction du site au Dr Jean",
        "Martinet qui le relira avant la fin du mois.",
    ]);
    let mut lab = world(&[
        ("guide.txt", guide.as_str()),
        ("transmission.txt", transmission.as_str()),
        ("titre.txt", titled.as_str()),
    ]);
    lab.analyse().await;
    let people = canonicals(&lab, "person");
    for wanted in ["Jean-Luc Rapin", "Jean Dupont", "Jean Martinet"] {
        assert!(
            people.iter().any(|name| name == wanted),
            "{wanted:?} not among {people:?}"
        );
    }
    clean(&lab);
}

/// An invoice or a letterhead has one long line that ends on a place and a capital under it: not a
/// wrapped paragraph, so no one is called `Annemasse Solution`. (Found by the sandbox: an invoice line
/// `Client : Cabinet medical des Voirons, 8 rue du Leman, 74100 Annemasse` and `Solution hydroalcoolique`.)
#[tokio::test]
async fn a_letterhead_or_an_invoice_line_is_not_joined_to_the_next_one() {
    let mut lab = world(&[
        (
            "facture-1.txt",
            "Facture FAC-2026-114\nGarage du Lac\nClient : Cabinet medical des Voirons, 8 rue du Leman, 74100 Annemasse\nSolution hydroalcoolique 500 mL x 5\nCompresses sterile, lot de 50 x 12\n",
        ),
        (
            "facture-2.txt",
            "Facture FAC-2026-115\nGarage du Lac\nClient : Cabinet medical des Voirons, 8 rue du Leman, 74100 Annemasse\nSolution hydroalcoolique 500 mL x 2\nCompresses sterile, lot de 50 x 3\n",
        ),
    ]);
    lab.analyse().await;
    let people = canonicals(&lab, "person");
    assert!(
        people
            .iter()
            .all(|name| !name.contains("Annemasse") && !name.contains("Solution")),
        "{people:?}"
    );
    let organisations = canonicals(&lab, "organization");
    assert!(
        organisations.iter().all(|name| !name.contains("Solution")),
        "{organisations:?}"
    );
    clean(&lab);
}

#[tokio::test]
async fn a_particle_inside_a_name_does_not_cut_it() {
    let mut lab = world(&[(
        "courrier.txt",
        "Courrier \u{e0} Matthieu de la Fontaine, Camille d'Aubign\u{e9}, Anne van den Berg et Pierre du Pasquier.\n",
    )]);
    lab.analyse().await;
    let people = canonicals(&lab, "person");
    for wanted in [
        "Matthieu de la Fontaine",
        "Camille d'Aubign\u{e9}",
        "Anne van den Berg",
        "Pierre du Pasquier",
    ] {
        assert!(
            people.iter().any(|name| name == wanted),
            "{wanted:?} not among {people:?}"
        );
    }
    clean(&lab);
}

// ---------------------------------------------------------------------------------------- F-09

fn swapped_files() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "patient-fr.txt",
            "Monsieur Lucas Roy a \u{e9}t\u{e9} re\u{e7}u mardi pour un contr\u{f4}le.\n",
        ),
        (
            "patient-ch.txt",
            "Monsieur Roy Lucas a \u{e9}t\u{e9} re\u{e7}u jeudi pour un bilan.\n",
        ),
        (
            "patient-fr-2.txt",
            "Le dossier de Monsieur Lucas Roy est complet.\n",
        ),
    ]
}

fn swapped_world() -> Lab {
    world(&swapped_files())
}

/// Two people, `Lucas Roy` and `Roy Lucas`: a question for one searches the files of that one, not the
/// files that hold the other's name (the lexical net must not search the words the other way round).
#[tokio::test]
async fn a_question_for_one_of_two_swapped_names_searches_only_that_person() {
    let mut lab = swapped_world();
    for n in 1..=8 {
        let (name, text) = filler(n);
        lab.write(&name, &text);
    }
    lab.analyse().await;
    for (question, expected) in [
        (
            "Quels documents parlent de Lucas Roy ?",
            vec!["patient-fr-2.txt", "patient-fr.txt"],
        ),
        (
            "Quels documents parlent de Roy Lucas ?",
            vec!["patient-ch.txt"],
        ),
    ] {
        let plan = plan_with(
            &lab,
            KnowledgeMode::Auto,
            &TargetingRequest::default(),
            question,
            small_world_policy(),
        );
        let mut kept = plan.reduction.documents.candidates.clone();
        kept.sort();
        assert_eq!(kept, expected, "{question}");
    }
}

#[tokio::test]
async fn two_people_with_swapped_names_are_two_entities() {
    let mut lab = swapped_world();
    lab.analyse().await;
    let french = named(&lab, "Lucas Roy").expect("Lucas Roy");
    let swiss = named(&lab, "Roy Lucas").expect("Roy Lucas");
    assert_ne!(french.entity_id, swiss.entity_id);
    assert_eq!(
        files_of(&lab, french.entity_id),
        ["patient-fr-2.txt", "patient-fr.txt"]
    );
    assert_eq!(files_of(&lab, swiss.entity_id), ["patient-ch.txt"]);
    // They are linked as a pair the user may look at, never merged.
    let pairs: i64 = lab
        .side()
        .query_row("SELECT COUNT(*) FROM kb_possible_matches", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(pairs >= 1, "no possible match between the two");
    clean(&lab);
}

/// The usual way one person is written twice: a table lists `Dupont Jean`, a letter says `Jean Dupont`. Two
/// orders are kept apart only inside one folder (two tables, two letters), where each order is a person's own.
#[tokio::test]
async fn a_table_and_a_letter_that_write_the_two_orders_name_one_person() {
    let mut lab = world(&[(
        "recu.txt",
        "Monsieur Jean Dupont a \u{e9}t\u{e9} re\u{e7}u hier pour un contr\u{f4}le.\n",
    )]);
    lab.write_data(
        "patients.csv",
        "Nom complet;Commune\nDupont Jean;Lausanne\nPaul Cuendet;Nyon\nAlice Pittet;Sion\n",
    );
    lab.analyse().await;
    lab.analyse_data();
    let dupont: Vec<EntityRow> = entities(&lab.side())
        .into_iter()
        .filter(|row| row.type_id == "person" && row.canonical.to_lowercase().contains("dupont"))
        .collect();
    assert_eq!(dupont.len(), 1, "{dupont:?}");
    assert!(!cells_mentions_of(&lab.side(), dupont[0].entity_id).is_empty());
    clean(&lab);
}

/// Inside one folder the same two orders are two people: two tables, each in its own order.
#[tokio::test]
async fn two_tables_that_write_the_two_orders_name_two_people() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    lab.write_data(
        "patients-fr.csv",
        "Nom complet;Commune\nLucas Roy;Annemasse\nPaul Cuendet;Nyon\nAlice Pittet;Sion\n",
    );
    lab.write_data(
        "patients-ch.csv",
        "Nom complet;Commune\nRoy Lucas;Lausanne\nHugo Morel;Nyon\nAnne Duc;Sion\n",
    );
    lab.analyse_data();
    assert!(named(&lab, "Lucas Roy").is_some());
    assert!(named(&lab, "Roy Lucas").is_some());
    clean(&lab);
}

/// The tables keep the other order: the appointments sheet that writes `Rochat Sophie` stays among the
/// workbooks given to the engine when the question names `Sophie Rochat` (the engine filters the rows by the
/// name), while the documents of the other order are not searched.
#[tokio::test]
async fn a_question_for_a_name_keeps_the_workbook_that_writes_the_other_order() {
    let mut files: Vec<(String, String)> = (1..=8).map(filler).collect();
    files.push((
        "dossier.txt".to_string(),
        "Le dossier de Madame Sophie Rochat est complet.\n".to_string(),
    ));
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    for (name, text) in &files {
        lab.write(name, text);
    }
    lab.write_data(
        "patients.csv",
        "Nom complet;Commune\nSophie Rochat;Lausanne\nPaul Cuendet;Nyon\nAlice Pittet;Sion\n",
    );
    lab.write_data(
        "rdv.csv",
        "Patient;Date\nRochat Sophie;02/03/2026\nCuendet Paul;03/03/2026\nPittet Alice;04/03/2026\n",
    );
    for n in 1..=4 {
        lab.write_data(
            &format!("stock-{n}.csv"),
            "Article;Quantite
Gants;10
Papier;20
Stylos;30
",
        );
    }
    lab.analyse().await;
    lab.analyse_data();
    let vocabulary = lab.vocabulary();
    let documents = lab.documents();
    let workbooks = lab.workbooks();
    let plan = lab.plan(
        &vocabulary,
        KnowledgeMode::Auto,
        &TargetingRequest::default(),
        "Combien de rendez-vous pour Sophie Rochat ?",
        SideInput::new(&documents),
        SideInput::new(&workbooks),
        small_world_policy(),
    );
    let mut kept = plan.reduction.data.candidates.clone();
    kept.sort();
    assert_eq!(
        kept,
        ["patients.csv", "rdv.csv"],
        "{:?}",
        plan.reduction.data
    );
}

#[tokio::test]
async fn a_surname_written_first_in_capitals_still_joins_the_full_name() {
    let mut lab = world(&[
        (
            "recu.txt",
            "Monsieur Jean Dupont a \u{e9}t\u{e9} re\u{e7}u hier.\n",
        ),
        (
            "dossier.txt",
            "Le dossier de DUPONT Jean est complet depuis hier.\n",
        ),
    ]);
    lab.analyse().await;
    let dupont: Vec<EntityRow> = entities(&lab.side())
        .into_iter()
        .filter(|row| row.type_id == "person" && row.canonical.to_lowercase().contains("dupont"))
        .collect();
    assert_eq!(dupont.len(), 1, "{dupont:?}");
    assert_eq!(
        files_of(&lab, dupont[0].entity_id),
        ["dossier.txt", "recu.txt"]
    );
    clean(&lab);
}

#[tokio::test]
async fn a_question_for_a_swapped_name_never_picks_the_other_person() {
    let mut files: Vec<(String, String)> = vec![(
        "patient-fr.txt".to_string(),
        "Monsieur Lucas Roy a \u{e9}t\u{e9} re\u{e7}u mardi pour un contr\u{f4}le.\n".to_string(),
    )];
    files.extend((1..=8).map(filler));
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    for (name, text) in &files {
        lab.write(name, text);
    }
    lab.analyse().await;
    let plan = plan_with(
        &lab,
        KnowledgeMode::Auto,
        &TargetingRequest::default(),
        "Que dit Roy Lucas ?",
        small_world_policy(),
    );
    let analysis = plan.analysis.expect("the question was read");
    assert!(
        analysis.entities.is_empty(),
        "resolved to {:?}",
        analysis
            .entities
            .iter()
            .map(|entity| entity.display.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        analysis
            .possibles
            .iter()
            .any(|possible| possible.candidates.iter().any(|c| c.display == "Lucas Roy")),
        "{:?}",
        analysis.possibles
    );
    assert!(!plan.reduction.documents.is_applied());
}

/// Giuseppe is written the way a letter to him is addressed (alone on a line above his street); his
/// namesake H\u{e9}l\u{e8}ne is a row of a workbook. The two share the surname `D'Amico`.
fn apostrophe_world() -> Lab {
    let mut files: Vec<(String, String)> = vec![
        (
            "damico-g.txt".to_string(),
            "Case postale 2323, 1211 Gen\u{e8}ve 2\nGiuseppe D'Amico\n41 avenue des Alpes, 74200 Thonon-les-Bains\n\nMadame, Monsieur,\n"
                .to_string(),
        ),
        (
            "ndiaye.txt".to_string(),
            "Madame A\u{ef}cha N'Diaye a t\u{e9}l\u{e9}phon\u{e9} ce matin.\n".to_string(),
        ),
        (
            "ndiaye-bis.txt".to_string(),
            "Le dossier de Madame A\u{ef}cha N'Diaye est complet.\n".to_string(),
        ),
        (
            "obrien.txt".to_string(),
            "Courrier de Monsieur Sean O'Brien \u{e0} la direction.\n".to_string(),
        ),
    ];
    files.extend((1..=6).map(filler));
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    for (name, text) in &files {
        lab.write(name, text);
    }
    lab.write_data(
        "patients.csv",
        "Nom complet;Commune\nH\u{e9}l\u{e8}ne D'Amico;Lausanne\nPaul Cuendet;Nyon\nAlice Pittet;Sion\n",
    );
    lab
}

#[tokio::test]
async fn a_surname_with_an_apostrophe_is_one_entity() {
    let mut lab = apostrophe_world();
    lab.analyse().await;
    lab.analyse_data();
    let people = canonicals(&lab, "person");
    for wanted in [
        "Giuseppe D'Amico",
        "H\u{e9}l\u{e8}ne D'Amico",
        "A\u{ef}cha N'Diaye",
        "Sean O'Brien",
    ] {
        assert!(
            people.iter().any(|name| name == wanted),
            "{wanted:?} not among {people:?}"
        );
    }
    // No entity for a piece of a surname, and one entity for the person written twice.
    for piece in ["Amico", "D'Amico", "Diaye", "N'Diaye", "Brien", "O'Brien"] {
        assert!(
            people.iter().all(|name| name != piece),
            "{piece:?} is an entity of its own: {people:?}"
        );
    }
    assert_eq!(
        people.iter().filter(|name| name.contains("Diaye")).count(),
        1,
        "{people:?}"
    );
    clean(&lab);
}

#[tokio::test]
async fn a_question_for_the_full_name_with_an_apostrophe_never_picks_another_person() {
    let mut lab = apostrophe_world();
    lab.analyse().await;
    lab.analyse_data();
    let giuseppe = named(&lab, "Giuseppe D'Amico").expect("Giuseppe").entity_id;
    let helene = named(&lab, "H\u{e9}l\u{e8}ne D'Amico")
        .expect("H\u{e9}l\u{e8}ne")
        .entity_id;
    for question in [
        "R\u{e9}sume le dossier de Giuseppe D'Amico.",
        "Que dit le dossier de Giuseppe D'Amico ?",
    ] {
        let plan = plan_with(
            &lab,
            KnowledgeMode::Auto,
            &TargetingRequest::default(),
            question,
            small_world_policy(),
        );
        let analysis = plan.analysis.expect("the question was read");
        let resolved: Vec<i64> = analysis.entities.iter().map(|e| e.entity_id).collect();
        assert_eq!(resolved, [giuseppe], "{question}: {:?}", analysis.entities);
        assert!(!resolved.contains(&helene), "{question}");
    }
}

/// The first document to name a person decides which titles the base knows her with; a later one that writes
/// `Dr Jean Dupont` adds `Dr Dupont` (found by the sandbox once recipients alone on a line created people
/// before the letters that call them doctors).
#[tokio::test]
async fn a_title_seen_with_a_known_person_teaches_the_base_her_title_form() {
    let mut lab = world(&[
        (
            "a-lettre.txt",
            "Madame, Monsieur,\nJean Dupont\n12 rue Pasteur, 74100 Annemasse\n",
        ),
        (
            "z-note.txt",
            "Le Dr Jean Dupont a sign\u{e9} le rapport hier.\n",
        ),
        ("planning.txt", "Semaine 1 : Dr Dupont\n"),
    ]);
    lab.analyse().await;
    let dupont = named(&lab, "Jean Dupont").expect("Jean Dupont");
    let aliases: Vec<String> = lab
        .side()
        .prepare("SELECT normalized FROM kb_aliases WHERE entity_id = ?1 ORDER BY normalized")
        .unwrap()
        .query_map([dupont.entity_id], |row| row.get::<_, String>(0))
        .unwrap()
        .map(|row| row.unwrap())
        .collect();
    assert!(aliases.contains(&"dr dupont".to_string()), "{aliases:?}");
    clean(&lab);
}

// ---------------------------------------------------------------------------------------- F-10

#[tokio::test]
async fn a_name_two_files_agreed_on_is_a_candidate_again_when_one_leaves() {
    let mut lab = world(&[
        (
            "rapport.txt",
            "Le rapport \u{e9}crit par Alice Archer est joint.\n",
        ),
        (
            "merci.txt",
            "Un grand merci \u{e0} Alice Archer pour son aide.\n",
        ),
    ]);
    lab.analyse().await;
    assert_eq!(named(&lab, "Alice Archer").unwrap().status, "active");

    lab.remove("merci.txt");
    lab.analyse().await;
    assert_eq!(
        named(&lab, "Alice Archer").unwrap().status,
        "candidate",
        "one file no longer supports her"
    );

    lab.write(
        "merci.txt",
        "Un grand merci \u{e0} Alice Archer pour son aide.\n",
    );
    lab.analyse().await;
    assert_eq!(named(&lab, "Alice Archer").unwrap().status, "active");
    clean(&lab);
}

#[tokio::test]
async fn a_name_found_by_its_title_keeps_that_method_after_the_second_reading() {
    let mut lab = world(&[
        (
            "recu.txt",
            "Monsieur Jean Dupont a \u{e9}t\u{e9} re\u{e7}u hier.\n",
        ),
        (
            "suite.txt",
            "Le dossier de Monsieur Jean Dupont est complet.\n",
        ),
    ]);
    lab.analyse().await;
    let dupont = named(&lab, "Jean Dupont").expect("Jean Dupont");
    let methods: Vec<String> = mentions_of(&lab.side(), dupont.entity_id)
        .into_iter()
        .map(|mention| mention.method)
        .collect();
    assert_eq!(methods.len(), 2, "{methods:?}");
    assert!(
        methods.iter().all(|method| method == "title_full_name"),
        "{methods:?}"
    );
    clean(&lab);
}

// ---------------------------------------------------------------------------------------- F-04

#[tokio::test]
async fn a_restored_name_has_its_table_mention_back() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    lab.write_data(
        "patients.csv",
        "Nom complet;Commune\nSophie Rochat;Lausanne\nPaul Cuendet;Nyon\nAlice Pittet;Sion\n",
    );
    lab.analyse_data();
    let sophie = named(&lab, "Sophie Rochat").expect("read from the column");
    assert!(
        !cells_mentions_of(&lab.side(), sophie.entity_id).is_empty(),
        "read from the column"
    );

    manage::delete_entity(&lab.side(), sophie.entity_id).expect("deletes");
    lab.analyse_data();
    assert!(cells_mentions_of(&lab.side(), sophie.entity_id).is_empty());

    manage::restore_entity(&lab.side(), sophie.entity_id).expect("restores");
    // Nothing changed in the file: the Data pass reads it again only because the restore said so.
    lab.analyse_data();
    assert!(
        !cells_mentions_of(&lab.side(), sophie.entity_id).is_empty(),
        "the cells mention did not come back"
    );
    clean(&lab);
}

// ------------------------------------------------------------------------- the human test's files

/// What the targeting line of the human test will say, from the same code on the same files: the first
/// `ACAI_TIER` (default 100) documents of the sandbox with all its workbooks, mode `Auto`, a perfect text
/// recognition. `cargo test --release --test knowledge_fix_names print_the_targeting -- --ignored --nocapture`
#[tokio::test]
#[ignore]
async fn print_the_targeting_lines_of_the_human_test() {
    use assistant_cabinet_ai_lib::knowledge::targeting::QueryVocabulary;
    use common::validation::{library, Replay, Truth};
    let truth = Truth::load();
    let tier: usize = std::env::var("ACAI_TIER")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);
    let lib = library(&truth, tier, &["health-fr", "health-ch"]).await;
    let vocabulary = QueryVocabulary::load("fr-FR", &["health-fr", "health-ch"]).unwrap();
    let replay = Replay::new(&lib, &vocabulary);
    let questions: Vec<String> = match std::env::var("ACAI_QUESTIONS") {
        Ok(list) => list.split('|').map(str::to_string).collect(),
        Err(_) => vec![
            "Quels courriers viennent du CHUV ?".to_string(),
            "Quels documents concernent HUG ?".to_string(),
            "Quels documents concernent les H\u{f4}pitaux universitaires de Gen\u{e8}ve ?"
                .to_string(),
            "Quels documents parlent de Lucas Roy ?".to_string(),
            "Quels documents parlent de Roy Lucas ?".to_string(),
            "R\u{e9}sume le dossier de Giuseppe D'Amico.".to_string(),
            "Que dit le courrier de A\u{ef}cha N'Diaye ?".to_string(),
        ],
    };
    for question in &questions {
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
                        "    name {} / {} / {:?} / {:.2} / {} document(s)",
                        e.display,
                        e.type_id,
                        e.status,
                        e.confidence,
                        e.document_files.len()
                    );
                }
                for a in &analysis.ambiguities {
                    println!("    ambiguity on {:?}", a.span);
                }
                for p in &analysis.possibles {
                    println!(
                        "    did you mean {:?} for {:?} ({})",
                        p.candidates
                            .iter()
                            .map(|c| c.display.clone())
                            .collect::<Vec<_>>(),
                        p.span,
                        p.reason
                    );
                }
            }
            let d = &plan.reduction.documents;
            println!(
                "    documents: {:?} skipped {:?} kept {} of {}",
                d.outcome,
                d.skipped.map(|r| r.as_code()),
                d.candidates.len(),
                d.selected,
            );
            let mut files = d.candidates.clone();
            files.sort();
            for file in files.iter().take(12) {
                println!("      keeps {file}");
            }
        }
    }
}

/// The organisations and the recipients the base holds after the first `ACAI_TIER` (default 100) documents:
/// what the dialog and `show_kb_entities.py` will list. `cargo test --release --test knowledge_fix_names print_the_names -- --ignored --nocapture`
#[tokio::test]
#[ignore]
async fn print_the_names_of_the_human_test() {
    use common::validation::{library, Truth};
    let truth = Truth::load();
    let tier: usize = std::env::var("ACAI_TIER")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);
    let lib = library(&truth, tier, &["health-fr", "health-ch"]).await;
    let connection = lib.lab.side();
    let mut statement = connection
        .prepare(
            "SELECT e.type_id, e.status, e.canonical_name,
                    (SELECT COUNT(DISTINCT s.relative_path) FROM kb_mentions m
                       JOIN kb_sources s ON s.source_id = m.source_id
                      WHERE m.entity_id = e.entity_id AND s.domain = 'documents'),
                    (SELECT group_concat(a.display, ' | ') FROM kb_aliases a WHERE a.entity_id = e.entity_id)
             FROM kb_entities e WHERE e.status IN ('active', 'candidate')
             ORDER BY e.type_id, e.canonical_name",
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
    let methods: Vec<(String, String)> = connection
        .prepare(
            "SELECT e.canonical_name, group_concat(DISTINCT m.method)
             FROM kb_entities e JOIN kb_mentions m ON m.entity_id = e.entity_id
             WHERE e.canonical_name IN ('Centre hospitalier universitaire vaudois', 'Giuseppe D''Amico')
             GROUP BY e.entity_id",
        )
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(|row| row.unwrap())
        .collect();
    println!("methods: {methods:?}");
    let pairs: Vec<(String, String, String, String)> = connection
        .prepare(
            "SELECT a.canonical_name, b.canonical_name, p.reason, p.state
             FROM kb_possible_matches p
             JOIN kb_entities a ON a.entity_id = p.entity_a
             JOIN kb_entities b ON b.entity_id = p.entity_b
             WHERE p.reason <> 'initial'",
        )
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .map(|row| row.unwrap())
        .collect();
    println!("possible matches: {pairs:?}");
    let wanted = std::env::var("ACAI_TYPES").unwrap_or_else(|_| "organization".to_string());
    for (type_id, status, name, documents, aliases) in rows {
        if wanted.split(',').any(|t| t == type_id) {
            println!(
                "{type_id} / {status} / {name} / {documents} document(s) / {}",
                aliases.unwrap_or_default()
            );
        }
    }
}

/// The walk of the restore step of the human test: delete a name a workbook wrote, restore it, analyse the Data
/// folder with no change of the file. `cargo test --release --test knowledge_fix_names print_the_restore -- --ignored --nocapture`
#[tokio::test]
#[ignore]
async fn print_the_restore_walk_of_the_human_test() {
    use common::validation::{library, Truth};
    let truth = Truth::load();
    let mut lib = library(&truth, 100, &["health-fr", "health-ch"]).await;
    let name = std::env::var("ACAI_NAME").unwrap_or_else(|_| "Lucas Roy".to_string());
    let id = named(&lib.lab, &name).expect("the name").entity_id;
    let show = |lab: &Lab, step: &str| {
        let cells = cells_mentions_of(&lab.side(), id);
        let places: Vec<String> = cells
            .iter()
            .map(|m| format!("{} / {}", m.path, m.column.clone().unwrap_or_default()))
            .collect();
        let status = named(lab, &name).map(|r| r.status).unwrap_or_default();
        println!(
            "{step}: status {status}, {} table mention(s) {places:?}",
            cells.len()
        );
    };
    show(&lib.lab, "before");
    let outcome = manage::delete_entity(&lib.lab.side(), id).expect("deletes");
    println!("deleted: documents due {}", outcome.documents_due);
    show(&lib.lab, "after the deletion");
    let outcome = manage::restore_entity(&lib.lab.side(), id).expect("restores");
    println!("restored: documents due {}", outcome.documents_due);
    show(&lib.lab, "after the restore, before the Data Analyse");
    lib.lab.analyse_data();
    show(&lib.lab, "after the Data Analyse");
}
