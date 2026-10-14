//! Tab 1 of the long-documents chain, item 6: the user's own business types. Manual rows in the local index that
//! no Analyse writes, read by the profile's type step; re-evaluating them reads the stored profile and no file.
//! Every document is invented.

mod common;

use assistant_cabinet_ai_lib::file_profile::store::{index_status, FolderProfileStatus};
use assistant_cabinet_ai_lib::file_profile::user_types::{
    delete, list, reevaluate, save, with_stored_types, UserType,
};
use assistant_cabinet_ai_lib::file_profile::vocabulary::ProfileVocabulary;
use common::knowledge::{count, Lab};
use rusqlite::Connection;

const DELIVERY: &str =
    "Bon de livraison n 5\nMedSupply SA\nLivraison du 4 mars 2026 : dix boites de compresses.\n";
const INVOICE: &str = "Facture FAC-2026-014\nMedSupply SA\nFacture n FAC-2026-014\nDate : 14 avril 2026\nClient : Cabinet medical des Voirons\nTotal HT : CHF 326.30\n";
const NOTE: &str = "Note interne\nLe planning des vacances a ete adopte.\n";

fn delivery_note() -> UserType {
    UserType {
        code: "user_delivery_note".to_string(),
        scope: "documents".to_string(),
        label: "Bon de livraison".to_string(),
        name: vec!["livraison".to_string()],
        title: vec!["bon de livraison".to_string()],
        header: vec!["bon de livraison".to_string()],
        body: Vec::new(),
        sheet: Vec::new(),
    }
}

fn business_type(connection: &Connection, path: &str) -> Option<String> {
    connection
        .query_row(
            "SELECT business_type FROM file_profiles WHERE relative_path = ?1",
            [path],
            |row| row.get::<_, Option<String>>(0),
        )
        .expect("the profile exists")
}

async fn lab() -> Lab {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    lab.write("2026-03-04_bon-de-livraison.txt", DELIVERY);
    lab.write("2026-04-14_facture-medsupply.txt", INVOICE);
    lab.write("2026-05-02_note.txt", NOTE);
    lab.analyse().await;
    lab
}

#[test]
fn a_definition_that_cannot_be_a_rule_is_refused_and_kept_nowhere() {
    let lab = Lab::new();
    let connection = lab.side();
    let mut bad = delivery_note();
    bad.code = "invoice".to_string();
    assert!(
        save(&connection, &bad).is_err(),
        "a shipped code is not a user code"
    );
    bad = delivery_note();
    bad.code = "user_Delivery Note".to_string();
    assert!(save(&connection, &bad).is_err());
    bad = delivery_note();
    bad.label = "  ".to_string();
    assert!(save(&connection, &bad).is_err());
    bad = delivery_note();
    bad.scope = "everything".to_string();
    assert!(save(&connection, &bad).is_err());
    bad = delivery_note();
    bad.name.clear();
    bad.title.clear();
    bad.header.clear();
    assert!(
        save(&connection, &bad).is_err(),
        "a rule with no phrase is no evidence"
    );
    bad = delivery_note();
    bad.name = vec!["...".to_string()];
    assert!(
        save(&connection, &bad).is_err(),
        "a phrase with no word is refused"
    );
    assert!(list(&connection).expect("lists").is_empty());

    save(&connection, &delivery_note()).expect("a good definition is kept");
    let mut renamed = delivery_note();
    renamed.label = "Livraison".to_string();
    save(&connection, &renamed).expect("the same code replaces it");
    let kept = list(&connection).expect("lists");
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].label, "Livraison");
    assert!(delete(&connection, "user_delivery_note").expect("deletes"));
    assert!(list(&connection).expect("lists").is_empty());
}

#[tokio::test]
async fn re_evaluating_reads_the_stored_profile_and_no_file_and_keeps_what_was_typed() {
    let lab = lab().await;
    let connection = lab.side();
    assert_eq!(
        business_type(&connection, "2026-03-04_bon-de-livraison.txt"),
        None
    );
    assert_eq!(
        business_type(&connection, "2026-04-14_facture-medsupply.txt").as_deref(),
        Some("invoice")
    );

    save(&connection, &delivery_note()).expect("kept");
    // No file is read: the folder is gone when the types are weighed.
    std::fs::remove_dir_all(lab.work_path()).expect("removes the folder");
    let vocabulary = with_stored_types(
        ProfileVocabulary::load(&["health-fr", "health-ch"]).expect("loads"),
        &connection,
    );
    let summary = reevaluate(&connection, &vocabulary).expect("re-evaluates");
    assert_eq!((summary.examined, summary.changed), (3, 1), "{summary:?}");
    assert_eq!(
        business_type(&connection, "2026-03-04_bon-de-livraison.txt").as_deref(),
        Some("user_delivery_note")
    );
    assert_eq!(
        business_type(&connection, "2026-04-14_facture-medsupply.txt").as_deref(),
        Some("invoice"),
        "a shipped type is not displaced by a type that has no evidence"
    );
    assert_eq!(business_type(&connection, "2026-05-02_note.txt"), None);

    // Weighing again changes nothing.
    let again = reevaluate(&connection, &vocabulary).expect("re-evaluates");
    assert_eq!((again.examined, again.changed), (3, 0));

    // A type that is gone leaves its files with none the next time they are weighed.
    delete(&connection, "user_delivery_note").expect("deletes");
    let vocabulary = with_stored_types(
        ProfileVocabulary::load(&["health-fr", "health-ch"]).expect("loads"),
        &connection,
    );
    let gone = reevaluate(&connection, &vocabulary).expect("re-evaluates");
    assert_eq!(gone.changed, 1);
    assert_eq!(
        business_type(&connection, "2026-03-04_bon-de-livraison.txt"),
        None
    );
}

#[tokio::test]
async fn an_analyse_leaves_the_types_of_the_user_intact_and_reads_them() {
    let mut lab = lab().await;
    let connection = lab.side();
    save(&connection, &delivery_note()).expect("kept");
    let rows_before = list(&connection).expect("lists");

    // The next Analyse builds the profiles again, because the vocabulary changed, and reads the full evidence.
    let summary = lab.analyse().await.profile.expect("the profile pass ran");
    assert_eq!(summary.built, 3, "{summary:?}");
    assert_eq!(list(&lab.side()).expect("lists"), rows_before);
    assert_eq!(
        business_type(&lab.side(), "2026-03-04_bon-de-livraison.txt").as_deref(),
        Some("user_delivery_note")
    );
    assert_eq!(
        business_type(&lab.side(), "2026-04-14_facture-medsupply.txt").as_deref(),
        Some("invoice")
    );

    // Then nothing is built again while the types stay the same, and the rows are still the user's.
    let next = lab.analyse().await.profile.expect("the profile pass ran");
    assert_eq!((next.built, next.unchanged), (0, 3), "{next:?}");
    assert_eq!(list(&lab.side()).expect("lists"), rows_before);
    assert_eq!(count(&lab.side(), "file_profile_user_types"), 1);
}

#[tokio::test]
async fn the_quick_pass_agrees_with_the_full_one_on_what_the_stored_profile_holds() {
    let mut lab = lab().await;
    save(&lab.side(), &delivery_note()).expect("kept");
    let vocabulary = with_stored_types(
        ProfileVocabulary::load(&["health-fr", "health-ch"]).expect("loads"),
        &lab.side(),
    );
    reevaluate(&lab.side(), &vocabulary).expect("re-evaluates");
    let quick: Vec<(String, Option<String>)> = all_types(&lab.side());
    lab.analyse().await;
    assert_eq!(
        all_types(&lab.side()),
        quick,
        "the Analyse that follows changes no type the quick pass settled"
    );
}

fn all_types(connection: &Connection) -> Vec<(String, Option<String>)> {
    let mut statement = connection
        .prepare("SELECT relative_path, business_type FROM file_profiles ORDER BY relative_path")
        .expect("prepares");
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("queries")
        .collect::<Result<_, _>>()
        .expect("reads")
}

#[test]
fn an_old_index_gains_the_table_empty_and_nothing_else_changes() {
    let lab = Lab::new();
    let connection = lab.side();
    assert!(list(&connection).expect("lists").is_empty());
    assert_eq!(
        index_status(&connection).documents,
        FolderProfileStatus::Empty
    );
}
