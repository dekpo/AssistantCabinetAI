//! Lot 4: the knowledge base learns names from the documents a pass analyses.
//!
//! These tests drive the real pass (`indexing::run_with_knowledge`) over real files in a temporary
//! folder, with the gateway faked, and then look at the `kb_*` tables from a second connection. What
//! they check is what only shows end to end: provenance down to the chunk, what a second pass does
//! (nothing), what a changed or a deleted file leaves behind, that a document is still indexed when
//! the knowledge step breaks, that an index from before the knowledge base is filled without one
//! request to the gateway, and that a personal identifier is never written as itself.
//!
//! Every name, address and number in this file is invented. The documents are French because the
//! pilot is; the code under test holds no French word (the packs do).

mod common;

use assistant_cabinet_ai_lib::index_store::RefreshWrite;
use assistant_cabinet_ai_lib::knowledge::backfill;
use assistant_cabinet_ai_lib::knowledge::secret::IdentifierKey;
use assistant_cabinet_ai_lib::knowledge::store as kb;
use assistant_cabinet_ai_lib::knowledge::{
    AliasKind, Domain, EntityStatus, IntegrityReport, KnowledgeDelta, KnowledgeMode, NewAlias,
    NewEntity, Origin, SourceRef, CURRENT_KB_VERSION,
};
use assistant_cabinet_ai_lib::ocr::fake::FakeOcrProvider;
use assistant_cabinet_ai_lib::ocr::OcrPage;

use common::fake_gateway::{FakeGateway, Reply};
use common::knowledge::{
    count, entities, entity_named, every_text_value, knowledge_rows, mentions_of, snapshot, Lab,
};

/// A letter that names a doctor, a patient, a practice, an e-mail address and an invoice.
const LETTER: &str = "Docteur Jean Dupont\n\
Cabinet Médical du Parc\n\n\
Objet : renouvellement\n\n\
Madame Claire Martin a été reçue le 12 mars 2026. Dr Dupont recommande un contrôle.\n\
Contact : claire.martin@exemple.fr\n\
Facture FAC-2026-0042 à régler.\n";

fn clean(lab: &Lab) {
    assert_eq!(
        kb::integrity_check(&lab.side()).expect("checks"),
        IntegrityReport::default(),
        "the knowledge base must be consistent"
    );
}

fn live(lab: &Lab) -> Vec<(String, String, String)> {
    entities(&lab.side())
        .into_iter()
        .map(|row| (row.type_id, row.canonical, row.status))
        .collect()
}

fn entity(lab: &Lab, canonical: &str) -> common::knowledge::EntityRow {
    entity_named(&lab.side(), canonical)
        .unwrap_or_else(|| panic!("no entity named {canonical:?}: {:?}", live(lab)))
}

// ------------------------------------------------------------------------------------ reading

#[tokio::test]
async fn analysing_a_document_records_its_entities_with_chunk_provenance() {
    let mut lab = Lab::new();
    lab.write("courrier-dupont.txt", LETTER);

    let summary = lab.analyse().await;

    assert_eq!(summary.indexed_files, 1);
    let jean = entity(&lab, "Jean Dupont");
    assert_eq!(
        (jean.type_id.as_str(), jean.status.as_str()),
        ("person", "active")
    );
    let claire = entity(&lab, "Claire Martin");
    assert_eq!(claire.status, "active");
    let practice = entity(&lab, "Cabinet Médical du Parc");
    assert_eq!(
        (practice.type_id.as_str(), practice.status.as_str()),
        ("organization", "candidate"),
        "a word that usually opens an organisation's name is a hint, not proof"
    );
    let invoice = entity(&lab, "FAC-2026-0042");
    assert_eq!(invoice.type_id, "identifier");
    assert_eq!(
        invoice.normalized, "FAC20260042",
        "an ordinary identifier is stored as itself"
    );

    // Provenance: the chunk the name was found in, and how many times it stands there.
    let mentions = mentions_of(&lab.side(), jean.entity_id);
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0].path, "courrier-dupont.txt");
    assert_eq!(mentions[0].locator_kind, "chunk");
    assert_eq!(
        mentions[0].chunk_id.as_deref(),
        Some("courrier-dupont.txt#p1#s1")
    );
    assert_eq!(
        mentions[0].occurrences, 2,
        "the full name, and the surname alone that the same file also writes"
    );
    clean(&lab);
}

#[tokio::test]
async fn the_summary_reports_counts_and_no_name() {
    let mut lab = Lab::new();
    lab.write("courrier-dupont.txt", LETTER);

    let summary = lab.analyse().await.knowledge.expect("a knowledge summary");

    assert_eq!(
        summary.entities_detected, 3,
        "the doctor, the patient, the practice"
    );
    assert_eq!(summary.created_new, 3);
    assert_eq!(summary.matched_existing, 0);
    assert_eq!(summary.candidates, 1, "the practice is only a candidate");
    assert_eq!(
        summary.identifiers, 2,
        "the e-mail address and the invoice number"
    );
    assert_eq!(summary.errors, 0);
    assert_eq!(summary.truncated_sources, 0);
    assert_eq!(
        summary.refreshed_sources, 0,
        "a file this very pass analysed is not an earlier document read again"
    );
    let serialised = serde_json::to_string(&summary).expect("serialises");
    for name in ["Dupont", "Martin", "exemple", "FAC-2026"] {
        assert!(
            !serialised.contains(name),
            "the summary must hold counts only"
        );
    }
}

#[tokio::test]
async fn the_same_person_in_two_documents_is_one_entity_with_two_sources() {
    let mut lab = Lab::new();
    lab.write(
        "a-rappel.txt",
        "Pour mémoire : Madame Claire Martin a appelé.\n",
    );
    lab.write(
        "b-suivi.txt",
        "Suivi : Madame Claire Martin viendra demain.\n",
    );

    let summary = lab.analyse().await.knowledge.expect("summary");

    let people: Vec<_> = entities(&lab.side())
        .into_iter()
        .filter(|row| row.type_id == "person")
        .collect();
    assert_eq!(people.len(), 1, "{people:?}");
    let mentions = mentions_of(&lab.side(), people[0].entity_id);
    let paths: Vec<_> = mentions.iter().map(|m| m.path.as_str()).collect();
    assert_eq!(paths, ["a-rappel.txt", "b-suivi.txt"]);
    assert_eq!(summary.entities_detected, 1);
    assert_eq!(summary.created_new, 1);
    clean(&lab);
}

#[tokio::test]
async fn a_full_name_an_initial_and_a_title_with_a_surname_resolve_by_evidence() {
    let mut lab = Lab::new();
    lab.write(
        "evidence.txt",
        "Docteur Jean Dupont a reçu la patiente. Plus tard, J. Dupont a signé. Enfin M. Dupont est parti.\n",
    );

    lab.analyse().await;

    let people: Vec<_> = entities(&lab.side())
        .into_iter()
        .filter(|row| row.type_id == "person")
        .collect();
    assert_eq!(
        people.len(),
        1,
        "one person, three ways of writing him: {people:?}"
    );
    let mentions = mentions_of(&lab.side(), people[0].entity_id);
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0].occurrences, 3);
}

#[tokio::test]
async fn an_initial_is_not_linked_when_another_person_of_the_file_shares_the_surname() {
    let mut lab = Lab::new();
    lab.write(
        "two-dupont.txt",
        "Docteur Jean Dupont et Madame Marie Dupont ont répondu. Plus tard, P. Dupont a signé. Dr Dupont a conclu.\n",
    );

    lab.analyse().await;

    let people = entities(&lab.side())
        .into_iter()
        .filter(|row| row.type_id == "person")
        .map(|row| row.canonical)
        .collect::<Vec<_>>();
    assert!(people.contains(&"Jean Dupont".to_string()), "{people:?}");
    assert!(people.contains(&"Marie Dupont".to_string()), "{people:?}");
    // "Dr Dupont" could be either: it links to neither and creates nobody.
    assert!(!people.contains(&"Dupont".to_string()), "{people:?}");
    // "P. Dupont" is compatible with neither full name: a separate guess, never a link.
    let guess = entity(&lab, "P. Dupont");
    assert_eq!(guess.status, "candidate");
}

#[tokio::test]
async fn dr_martin_alone_does_not_create_a_person() {
    let mut lab = Lab::new();
    lab.write("alone.txt", "Le Docteur Martin a téléphoné ce matin.\n");

    lab.analyse().await;

    assert!(
        entities(&lab.side())
            .iter()
            .all(|row| row.type_id != "person"),
        "{:?}",
        live(&lab)
    );
    assert_eq!(count(&lab.side(), "kb_mentions"), 0);
}

#[tokio::test]
async fn a_surname_alone_is_not_linked_to_a_person_known_only_from_another_file() {
    let mut lab = Lab::new();
    lab.write(
        "a-known.txt",
        "Contrôle : Docteur Pierre Martin a répondu.\n",
    );
    lab.write("b-alone.txt", "Le Docteur Martin a téléphoné ce matin.\n");

    lab.analyse().await;

    let pierre = entity(&lab, "Pierre Martin");
    let sources: Vec<_> = mentions_of(&lab.side(), pierre.entity_id)
        .into_iter()
        .map(|m| m.path)
        .collect();
    assert_eq!(
        sources,
        ["a-known.txt"],
        "the owner accepted the recall cost: a file that only says Dr Martin is not Pierre Martin's"
    );
}

#[tokio::test]
async fn a_name_seen_once_without_support_stays_a_candidate_and_is_promoted_by_a_second_source() {
    let mut lab = Lab::new();
    lab.write(
        "a-marche.txt",
        "Hier, nous avons croisé Paul Verdier au marché.\n",
    );

    lab.analyse().await;
    let guess = entity(&lab, "Paul Verdier");
    assert_eq!(guess.status, "candidate");
    let mention = &mentions_of(&lab.side(), guess.entity_id)[0];
    assert_eq!(mention.method, "capitalised_name");
    assert!((mention.confidence - 0.40).abs() < 1e-6);

    lab.write(
        "b-retour.txt",
        "Ensuite, Paul Verdier est revenu avec un ami.\n",
    );
    lab.analyse().await;

    let promoted = entity(&lab, "Paul Verdier");
    assert_eq!(
        promoted.entity_id, guess.entity_id,
        "the same entity, not a second one"
    );
    assert_eq!(promoted.status, "active");
    assert_eq!(mentions_of(&lab.side(), promoted.entity_id).len(), 2);
    clean(&lab);
}

#[tokio::test]
async fn an_organisation_with_a_legal_form_is_trusted() {
    let mut lab = Lab::new();
    lab.write(
        "contrat.txt",
        "Le contrat avec Martin Dubois SARL a été signé.\n",
    );

    lab.analyse().await;

    let company = entity(&lab, "Martin Dubois SARL");
    assert_eq!(
        (company.type_id.as_str(), company.status.as_str()),
        ("organization", "active")
    );
}

#[tokio::test]
async fn a_hyphenated_spelling_finds_the_name_the_base_already_knows() {
    let mut lab = Lab::new();
    lab.write("a-source.txt", "Docteur Jean Dupont a signé.\n");
    lab.write("b-autre.txt", "Voir Jean-Dupont à la réunion.\n");

    lab.analyse().await;

    let jean = entity(&lab, "Jean Dupont");
    let sources: Vec<_> = mentions_of(&lab.side(), jean.entity_id)
        .into_iter()
        .map(|m| m.path)
        .collect();
    assert_eq!(sources, ["a-source.txt", "b-autre.txt"]);
}

#[tokio::test]
async fn months_weekdays_and_document_words_never_become_names() {
    let mut lab = Lab::new();
    lab.write(
        "dates.txt",
        "Rendez-vous le Mardi 3 Mars. Voir Lundi Janvier puis Vendredi Juin. Facture Devis Annexe.\n",
    );

    lab.analyse().await;

    assert_eq!(knowledge_rows_without_sources(&lab), 0, "{:?}", live(&lab));
}

fn knowledge_rows_without_sources(lab: &Lab) -> usize {
    knowledge_rows(&lab.side()) - count(&lab.side(), "kb_sources") as usize
}

#[tokio::test]
async fn a_file_name_matches_a_known_name_and_never_creates_one() {
    let mut lab = Lab::new();
    lab.write("a-source.txt", "Madame Claire Martin a appelé.\n");
    lab.write("Martin_Claire_2026-03.txt", "Rien de particulier ici.\n");
    lab.write(
        "Verdier_Paul_2026-03.txt",
        "Rien de particulier ici non plus.\n",
    );

    lab.analyse().await;

    let claire = entity(&lab, "Claire Martin");
    let kinds: Vec<_> = mentions_of(&lab.side(), claire.entity_id)
        .into_iter()
        .map(|m| (m.path, m.locator_kind, m.method))
        .collect();
    assert!(
        kinds.contains(&(
            "Martin_Claire_2026-03.txt".to_string(),
            "filename".to_string(),
            "filename_token".to_string()
        )),
        "{kinds:?}"
    );
    assert!(
        entity_named(&lab.side(), "Paul Verdier").is_none(),
        "names are never created from file names"
    );
}

// ------------------------------------------------------------------------------- identifiers

const WITH_IDENTIFIERS: &str = "Pour le dossier :\n\
Contact : claire.martin@exemple.fr\n\
Règlement sur IBAN FR76 3000 6000 0112 3456 7890 189 avant la fin du mois.\n\
Facture FAC-2026-0042 en attente.\n";

#[tokio::test]
async fn a_personal_identifier_is_stored_as_a_keyed_hash_and_never_as_itself() {
    let mut lab = Lab::new();
    lab.write("compta.txt", WITH_IDENTIFIERS);

    lab.analyse().await;

    let texts = every_text_value(&lab.side());
    for secret in [
        "claire.martin@exemple.fr",
        "CLAIREMARTIN",
        "exemple.fr",
        "FR7630006000011234567890189",
        "FR76 3000",
        "3000 6000",
    ] {
        assert!(
            !texts
                .iter()
                .any(|text| text.to_lowercase().contains(&secret.to_lowercase())),
            "{secret:?} must not be written anywhere in the knowledge base"
        );
    }

    let side = lab.side();
    let email = entities(&side)
        .into_iter()
        .find(|row| row.subtype.as_deref() == Some("email"))
        .expect("an e-mail identifier entity");
    assert_eq!(email.normalized.len(), 64, "the hash is the stored name");
    assert!(email.canonical.starts_with("email "), "{}", email.canonical);
    assert!(!email.canonical.contains('@'));
    let attribute: String = side
        .query_row(
            "SELECT value FROM kb_attributes WHERE entity_id = ?1 AND key = 'identifier:email'",
            [email.entity_id],
            |row| row.get(0),
        )
        .expect("the identifier attribute");
    assert_eq!(
        attribute, email.normalized,
        "the attribute holds the same hash"
    );
    let iban = entities(&side)
        .into_iter()
        .find(|row| row.subtype.as_deref() == Some("iban"))
        .expect("an IBAN identifier entity");
    assert_eq!(iban.normalized.len(), 64);

    // The invoice number is not personal: it stays a value, as before.
    let invoice = entity(&lab, "FAC-2026-0042");
    assert_eq!(invoice.normalized, "FAC20260042");
}

#[tokio::test]
async fn the_same_personal_identifier_in_two_documents_is_one_entity() {
    let mut lab = Lab::new();
    lab.write("a.txt", "Adresse : claire.martin@exemple.fr\n");
    lab.write("b.txt", "Mon adresse : CLAIRE.MARTIN@EXEMPLE.FR\n");

    lab.analyse().await;

    let emails: Vec<_> = entities(&lab.side())
        .into_iter()
        .filter(|row| row.subtype.as_deref() == Some("email"))
        .collect();
    assert_eq!(
        emails.len(),
        1,
        "equality is all matching needs: {emails:?}"
    );
    let sources: Vec<_> = mentions_of(&lab.side(), emails[0].entity_id)
        .into_iter()
        .map(|m| m.path)
        .collect();
    assert_eq!(sources, ["a.txt", "b.txt"]);
}

#[tokio::test]
async fn the_key_lives_beside_the_index_and_never_inside_it() {
    let directory = tempfile::tempdir().expect("temp app-data folder");
    let key = IdentifierKey::load_or_create(directory.path()).expect("makes the key");
    let digest = key.digest("email", "CLAIREMARTINEXEMPLEFR");
    let key_file = directory
        .path()
        .join(assistant_cabinet_ai_lib::knowledge::secret::KEY_FILE_NAME);
    let stored = std::fs::read_to_string(&key_file).expect("the key file");

    let mut lab = Lab::new();
    lab.write("a.txt", "Adresse : claire.martin@exemple.fr\n");
    lab.key = hex_to_bytes(stored.trim());
    lab.analyse().await;

    let texts = every_text_value(&lab.side());
    assert!(
        texts.iter().any(|text| *text == digest),
        "hashed with that very key"
    );
    assert!(
        !texts.iter().any(|text| text.contains(stored.trim())),
        "the key is not in the index"
    );
    let index_bytes = std::fs::read(lab.index_path()).expect("reads the index file");
    let needle = stored.trim().as_bytes();
    assert!(
        !index_bytes
            .windows(needle.len())
            .any(|window| window == needle),
        "the key is not in the index file either"
    );
}

fn hex_to_bytes(text: &str) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    for (position, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[position * 2..position * 2 + 2], 16).expect("hex");
    }
    bytes
}

#[tokio::test]
async fn a_new_key_hashes_the_identifiers_again_and_leaves_no_old_hash_behind() {
    let mut lab = Lab::new();
    lab.write("a.txt", "Adresse : claire.martin@exemple.fr\n");
    lab.analyse().await;
    let before = entities(&lab.side())
        .into_iter()
        .find(|row| row.subtype.as_deref() == Some("email"))
        .expect("the e-mail entity");

    lab.key = [9; 32];
    lab.analyse().await;

    let after: Vec<_> = entities(&lab.side())
        .into_iter()
        .filter(|row| row.subtype.as_deref() == Some("email"))
        .collect();
    assert_eq!(after.len(), 1, "{after:?}");
    assert_ne!(after[0].normalized, before.normalized);
    clean(&lab);
}

#[tokio::test]
async fn an_identifier_split_by_a_line_break_is_still_one_identifier() {
    // The case of defect KBD-05: a number written over two lines.
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write(
        "a.txt",
        "Numéro : 1 85 03 69\n123 456 78 pour la patiente.\n",
    );
    lab.write("b.txt", "Rappel : 185036912345678 est le numéro.\n");

    lab.analyse().await;

    let numbers: Vec<_> = entities(&lab.side())
        .into_iter()
        .filter(|row| row.subtype.as_deref() == Some("health_insurance_number"))
        .collect();
    assert_eq!(numbers.len(), 1, "{numbers:?}");
    assert_eq!(mentions_of(&lab.side(), numbers[0].entity_id).len(), 2);
    let texts = every_text_value(&lab.side());
    assert!(!texts.iter().any(|text| text.contains("185036912345678")));
}

const SWISS_LETTER: &str = "Patiente : Madame Rose Marchand
N° AVS 756.1234.5678.97
Règlement : IBAN CH93 0076 2011 6238 5295 7
";

#[tokio::test]
async fn a_swiss_avs_number_is_read_only_with_the_swiss_module_and_is_never_stored_as_itself() {
    // France only: the number is just digits in a sentence.
    let mut france = Lab::new();
    france.packs = vec!["health-fr"];
    france.write("suisse.txt", SWISS_LETTER);
    france.analyse().await;
    assert!(
        entities(&france.side())
            .iter()
            .all(|row| row.subtype.as_deref() != Some("health_insurance_number")),
        "the Swiss number is not read without the Swiss module"
    );

    // With the Swiss module: the AVS number and the Swiss IBAN, both as keyed hashes.
    let mut switzerland = Lab::new();
    switzerland.packs = vec!["health-ch"];
    switzerland.write("suisse.txt", SWISS_LETTER);
    let summary = switzerland.analyse().await.knowledge.expect("summary");

    assert_eq!(summary.identifiers, 2, "{summary:?}");
    let side = switzerland.side();
    let avs = entities(&side)
        .into_iter()
        .find(|row| row.subtype.as_deref() == Some("health_insurance_number"))
        .expect("an AVS entity");
    assert_eq!(avs.normalized.len(), 64, "a hash, not the number");
    let iban = entities(&side)
        .into_iter()
        .find(|row| row.subtype.as_deref() == Some("iban"))
        .expect("an IBAN entity");
    assert_eq!(iban.normalized.len(), 64);
    for text in every_text_value(&side) {
        let squashed: String = text.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        assert!(
            !squashed.contains("7561234567897")
                && !squashed.to_uppercase().contains("CH9300762011"),
            "a Swiss identifier was written as itself: {text:?}"
        );
    }
    clean(&switzerland);
}

#[tokio::test]
async fn the_same_avs_number_written_two_ways_is_one_entity() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-ch"];
    lab.write(
        "a.txt",
        "Patiente : AVS 756.1234.5678.97 ouvert.
",
    );
    lab.write(
        "b.txt",
        "Rappel : 7561234567897 est son numéro.
",
    );

    lab.analyse().await;

    let numbers: Vec<_> = entities(&lab.side())
        .into_iter()
        .filter(|row| row.subtype.as_deref() == Some("health_insurance_number"))
        .collect();
    assert_eq!(numbers.len(), 1, "{numbers:?}");
    assert_eq!(mentions_of(&lab.side(), numbers[0].entity_id).len(), 2);
}

#[tokio::test]
async fn a_marker_word_followed_by_a_small_word_makes_an_organisation_and_a_heading_makes_nobody() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write(
        "caisse.txt",
        "CAISSE PRIMAIRE D'ASSURANCE MALADIE DU RHONE

Objet : radiation

Le service des affiliations, CPAM du Rhone (fictif).
",
    );

    lab.analyse().await;

    let rows = entities(&lab.side());
    let people: Vec<_> = rows.iter().filter(|row| row.type_id == "person").collect();
    assert!(
        people.is_empty(),
        "a heading in capitals is not a person: {people:?}"
    );
    let organisation = entity(&lab, "CPAM du Rhone");
    assert_eq!(organisation.type_id, "organization");
}

// ------------------------------------------------------------------------------------ lifecycle

#[tokio::test]
async fn a_second_pass_over_unchanged_files_writes_nothing() {
    let mut lab = Lab::new();
    lab.write("a.txt", LETTER);
    lab.write("b.txt", "Pour mémoire : Madame Claire Martin a appelé.\n");
    lab.write("c.txt", "Hier, nous avons croisé Paul Verdier au marché.\n");
    lab.write("d.txt", "Ensuite, Paul Verdier est revenu avec un ami.\n");
    lab.analyse().await;
    let before = snapshot(&lab.side());
    assert!(!before["kb_entities"].is_empty());

    let second = lab.analyse().await;

    assert_eq!(second.unchanged_files, 4);
    assert_eq!(
        snapshot(&lab.side()),
        before,
        "I14: nothing moved, ids and timestamps included"
    );
    let summary = second
        .knowledge
        .expect("a summary even when nothing changed");
    assert_eq!(summary.entities_detected, 0);
    assert_eq!(summary.refreshed_sources, 0);
    clean(&lab);
}

#[tokio::test]
async fn a_modified_file_loses_its_stale_associations_and_gains_the_current_ones() {
    let mut lab = Lab::new();
    lab.write("note.txt", "Rappel : Madame Claire Martin a appelé.\n");
    lab.write("other.txt", "Suivi : Madame Anne Bernard viendra.\n");
    lab.analyse().await;
    assert!(entity_named(&lab.side(), "Claire Martin").is_some());

    lab.write("note.txt", "Rappel : Monsieur Luc Perrin a appelé.\n");
    let summary = lab.analyse().await;

    assert_eq!(summary.indexed_files, 1);
    assert!(
        entity_named(&lab.side(), "Claire Martin").is_none(),
        "nothing mentions her any more, and an automatic entity nothing holds on to is collected"
    );
    let luc = entity(&lab, "Luc Perrin");
    assert_eq!(mentions_of(&lab.side(), luc.entity_id)[0].path, "note.txt");
    assert!(
        entity_named(&lab.side(), "Anne Bernard").is_some(),
        "the other file is untouched"
    );
    clean(&lab);
}

#[tokio::test]
async fn a_deleted_file_takes_its_mentions_and_keeps_an_entity_another_file_still_names() {
    let mut lab = Lab::new();
    lab.write("a.txt", "Rappel : Madame Claire Martin a appelé.\n");
    lab.write(
        "b.txt",
        "Suivi : Madame Claire Martin viendra. Madame Anne Bernard aussi.\n",
    );
    lab.analyse().await;
    assert_eq!(
        mentions_of(&lab.side(), entity(&lab, "Claire Martin").entity_id).len(),
        2
    );

    lab.remove("b.txt");
    lab.analyse().await;

    let claire = entity(&lab, "Claire Martin");
    let sources: Vec<_> = mentions_of(&lab.side(), claire.entity_id)
        .into_iter()
        .map(|m| m.path)
        .collect();
    assert_eq!(sources, ["a.txt"]);
    assert!(entity_named(&lab.side(), "Anne Bernard").is_none());
    clean(&lab);
}

#[tokio::test]
async fn the_same_relative_path_in_both_folders_stays_two_sources() {
    let mut lab = Lab::new();
    lab.write("shared.txt", "Rappel : Madame Claire Martin a appelé.\n");
    lab.analyse().await;
    let side = lab.side();
    // A workbook of the Data Folder with the same relative path (lot 5 will fill these in).
    side.execute(
        "INSERT INTO kb_sources (domain, relative_path, content_id, kb_version, gazetteer_epoch)
         VALUES ('data', 'shared.txt', 'sha-data', 1, 0)",
        [],
    )
    .expect("registers the workbook");

    let sources = kb::sources(&side, None).expect("lists");
    assert_eq!(sources.len(), 2);
    assert_eq!(
        sources
            .iter()
            .filter(|row| row.source.relative_path == "shared.txt")
            .count(),
        2
    );
    let documents = kb::sources(&side, Some(Domain::Documents)).expect("lists");
    assert_eq!(documents.len(), 1);
    // A second pass does not touch the workbook's row, nor mix it with the document's.
    lab.analyse().await;
    assert_eq!(kb::sources(&lab.side(), None).expect("lists").len(), 2);
}

#[tokio::test]
async fn knowledge_mode_off_writes_no_knowledge_row_and_indexes_as_before() {
    let mut lab = Lab::new();
    lab.mode = KnowledgeMode::Off;
    lab.write("a.txt", LETTER);
    let before = snapshot(&lab.side());

    let summary = lab.analyse().await;

    assert_eq!(summary.indexed_files, 1);
    assert!(
        summary.knowledge.is_none(),
        "the knowledge base took no part"
    );
    assert_eq!(lab.index.chunk_count().unwrap(), 1);
    assert_eq!(snapshot(&lab.side()), before, "no kb_ row was written");
    assert!(
        !lab.app_data
            .path()
            .join(assistant_cabinet_ai_lib::knowledge::secret::KEY_FILE_NAME)
            .exists(),
        "no key is made when nothing is read"
    );
}

#[tokio::test]
async fn switching_the_knowledge_base_off_forgets_what_a_changed_file_taught_it() {
    let mut lab = Lab::new();
    lab.write("a.txt", "Rappel : Madame Claire Martin a appelé.\n");
    lab.analyse().await;
    assert_eq!(count(&lab.side(), "kb_sources"), 1);

    lab.mode = KnowledgeMode::Off;
    lab.write("a.txt", "Rappel : Monsieur Luc Perrin a appelé.\n");
    lab.analyse().await;

    assert_eq!(
        count(&lab.side(), "kb_sources"),
        0,
        "knowledge never outlives the text it described"
    );
    assert_eq!(count(&lab.side(), "kb_mentions"), 0);
    clean(&lab);
}

// --------------------------------------------------------------------- atomicity and degradation

#[tokio::test]
async fn an_embedding_failure_leaves_no_knowledge_rows_for_that_file() {
    let mut lab = Lab::new();
    lab.write("a-fine.txt", "Rappel : Madame Claire Martin a appelé.\n");
    lab.write("b-stalls.txt", "POISONED Madame Anne Bernard attend.\n");
    lab.gateway = FakeGateway::start(|seen| {
        if seen.inputs.iter().any(|text| text.contains("POISONED")) {
            Reply::slow(std::time::Duration::from_millis(900))
        } else {
            Reply::vectors()
        }
    });

    let summary = lab.analyse().await;

    assert_eq!(summary.failed_files.len(), 1);
    assert!(entity_named(&lab.side(), "Claire Martin").is_some());
    assert!(
        entity_named(&lab.side(), "Anne Bernard").is_none(),
        "a file that never reached the index teaches the knowledge base nothing"
    );
    assert_eq!(count(&lab.side(), "kb_sources"), 1);
    clean(&lab);
}

#[tokio::test]
async fn a_broken_knowledge_table_does_not_stop_a_document_being_indexed() {
    let mut lab = Lab::new();
    lab.write("a.txt", LETTER);
    // The corruption: a table the write needs is gone.
    lab.side()
        .execute_batch("DROP TABLE kb_attributes")
        .expect("breaks the table");

    let summary = lab.analyse().await;

    assert_eq!(summary.indexed_files, 1, "the document is indexed");
    assert_eq!(lab.index.chunk_count().unwrap(), 1);
    let knowledge = summary.knowledge.expect("the partial failure is reported");
    assert_eq!(
        knowledge.errors, 1,
        "one file whose names could not be recorded"
    );
    assert_eq!(count(&lab.side(), "kb_mentions"), 0, "nothing half written");

    // Repair the table (it is what opening the index does) and analyse again: the document is
    // unchanged, yet it is read for names because it was left due.
    kb::ensure_schema(&lab.side()).expect("recreates the table");
    let again = lab.analyse().await;
    assert_eq!(again.unchanged_files, 1);
    assert_eq!(again.knowledge.expect("summary").errors, 0);
    assert!(entity_named(&lab.side(), "Jean Dupont").is_some());
    clean(&lab);
}

// ------------------------------------------------------------------------------------- backfill

#[tokio::test]
async fn an_index_built_with_the_knowledge_base_off_is_filled_without_one_request_to_the_gateway() {
    let mut lab = Lab::new();
    lab.mode = KnowledgeMode::Off;
    lab.write("a.txt", LETTER);
    lab.write("b.txt", "Pour mémoire : Madame Claire Martin a appelé.\n");
    lab.analyse().await;
    assert_eq!(count(&lab.side(), "kb_entities"), 0);
    let embeddings_so_far = lab.gateway.requests().len();
    assert!(embeddings_so_far > 0);

    // Switch it on, and point the pass at a gateway that fails on any request.
    lab.mode = KnowledgeMode::Suggest;
    let hostile = FakeGateway::start(|_| Reply::refusal(500, "must_not_be_called"));
    let mut context = lab.context();
    let summary = lab
        .analyse_through(&mut context, &hostile.url, None, None)
        .await;

    assert_eq!(summary.unchanged_files, 2);
    assert!(
        hostile.requests().is_empty(),
        "the backfill never contacts the gateway"
    );
    let knowledge = summary.knowledge.expect("summary");
    assert_eq!(knowledge.refreshed_sources, 2);
    assert_eq!(knowledge.entities_detected, 3, "{knowledge:?}");
    assert_eq!(knowledge.identifiers, 2, "{knowledge:?}");
    assert!(entity_named(&lab.side(), "Claire Martin").is_some());
    assert_eq!(lab.gateway.requests().len(), embeddings_so_far);
    assert_eq!(lab.index.chunk_count().unwrap(), 2, "no chunk was touched");
    clean(&lab);
}

#[tokio::test]
async fn a_new_extractor_version_reads_the_documents_again_without_embedding_them() {
    let mut lab = Lab::new();
    lab.write("a.txt", LETTER);
    lab.analyse().await;
    let versions = |lab: &Lab| -> Vec<u32> {
        kb::sources(&lab.side(), Some(Domain::Documents))
            .unwrap()
            .into_iter()
            .map(|row| row.kb_version)
            .collect()
    };
    assert_eq!(versions(&lab), [CURRENT_KB_VERSION]);

    let hostile = FakeGateway::start(|_| Reply::refusal(500, "must_not_be_called"));
    let mut context = lab.context().with_kb_version(CURRENT_KB_VERSION + 1);
    let summary = lab
        .analyse_through(&mut context, &hostile.url, None, None)
        .await;

    assert_eq!(versions(&lab), [CURRENT_KB_VERSION + 1]);
    assert_eq!(summary.knowledge.expect("summary").refreshed_sources, 1);
    assert!(hostile.requests().is_empty());
}

#[tokio::test]
async fn documents_analysed_before_a_name_existed_get_their_mentions_when_the_name_is_added() {
    let mut lab = Lab::new();
    // A capital that only opens a sentence proves nothing, so nobody is recorded.
    lab.write("a.txt", "Zoé Lambert a appelé ce matin.\n");
    lab.analyse().await;
    assert!(entity_named(&lab.side(), "Zoé Lambert").is_none());

    // The user (or a later pass over the Data Folder) adds the name to the knowledge base.
    let side = lab.side();
    let id = kb::create_entity(
        &side,
        &NewEntity {
            type_id: "person".to_string(),
            subtype: None,
            canonical_name: "Zoé Lambert".to_string(),
            normalized_name: "zoe lambert".to_string(),
            disambiguator: String::new(),
            status: EntityStatus::Active,
            origin: Origin::Manual,
        },
    )
    .expect("creates the entity");
    kb::add_alias(
        &side,
        id,
        &NewAlias {
            display: "Zoé Lambert".to_string(),
            normalized: "zoe lambert".to_string(),
            phonetic_key: String::new(),
            kind: AliasKind::Manual,
            confidence: 1.0,
        },
        Origin::Manual,
    )
    .expect("adds the alias");

    let summary = lab.analyse().await;

    assert_eq!(summary.unchanged_files, 1, "the file itself did not change");
    assert_eq!(summary.knowledge.expect("summary").refreshed_sources, 1);
    let mentions = mentions_of(&lab.side(), id);
    assert_eq!(
        mentions.len(),
        1,
        "the document is read again against the longer list"
    );
    assert_eq!(mentions[0].path, "a.txt");
    clean(&lab);
}

#[tokio::test]
async fn a_name_the_user_deleted_is_not_recreated_by_the_next_analysis() {
    let mut lab = Lab::new();
    lab.write("a.txt", "Rappel : Madame Claire Martin a appelé.\n");
    lab.analyse().await;
    let claire = entity(&lab, "Claire Martin");
    {
        let mut side = lab.side();
        let tx = side.transaction().expect("transaction");
        kb::delete_entity(&tx, claire.entity_id).expect("deletes");
        tx.commit().expect("commits");
    }

    lab.write("b.txt", "Suivi : Madame Claire Martin viendra.\n");
    lab.analyse().await;

    let rows = entities(&lab.side());
    let claires: Vec<_> = rows
        .iter()
        .filter(|row| row.canonical == "Claire Martin")
        .collect();
    assert_eq!(claires.len(), 1, "only the tombstone");
    assert_eq!(claires[0].status, "deleted");
    assert!(mentions_of(&lab.side(), claire.entity_id).is_empty());
    clean(&lab);
}

#[tokio::test]
async fn a_manual_alias_is_found_by_the_next_scan() {
    let mut lab = Lab::new();
    lab.write("a.txt", "Madame Claire Martin a appelé.\n");
    lab.write("b.txt", "Voir Mme Clairette en priorité.\n");
    lab.analyse().await;
    let claire = entity(&lab, "Claire Martin");
    assert_eq!(mentions_of(&lab.side(), claire.entity_id).len(), 1);

    kb::add_alias(
        &lab.side(),
        claire.entity_id,
        &NewAlias {
            display: "Clairette".to_string(),
            normalized: "clairette".to_string(),
            phonetic_key: String::new(),
            kind: AliasKind::Manual,
            confidence: 1.0,
        },
        Origin::Manual,
    )
    .expect("adds the alias");
    lab.analyse().await;

    let sources: Vec<_> = mentions_of(&lab.side(), claire.entity_id)
        .into_iter()
        .map(|m| m.path)
        .collect();
    assert_eq!(sources, ["a.txt", "b.txt"]);
}

#[tokio::test]
async fn a_spelling_that_sounds_alike_is_kept_apart_and_offered_as_a_possible_match() {
    let mut lab = Lab::new();
    lab.write(
        "a.txt",
        "Pour info : Docteur Jean Dupont a signé.
",
    );
    lab.write(
        "b.txt",
        "Voir aussi : Docteur Jean Dupond pour la suite.
",
    );

    lab.analyse().await;

    let side = lab.side();
    let one = entity(&lab, "Jean Dupont");
    let other = entity(&lab, "Jean Dupond");
    assert_ne!(
        one.entity_id, other.entity_id,
        "sounding alike is never a merge"
    );
    let pairs: Vec<(i64, i64, String, String)> = side
        .prepare("SELECT entity_a, entity_b, reason, state FROM kb_possible_matches")
        .expect("prepares")
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .expect("queries")
        .collect::<Result<_, _>>()
        .expect("reads");
    assert_eq!(pairs.len(), 1, "{pairs:?}");
    assert_eq!(
        (pairs[0].0, pairs[0].1),
        (
            one.entity_id.min(other.entity_id),
            one.entity_id.max(other.entity_id)
        )
    );
    assert_eq!(pairs[0].3, "open");
    clean(&lab);
}

#[tokio::test]
async fn a_refresh_stops_between_two_documents_when_asked_and_resumes_later() {
    let mut lab = Lab::new();
    lab.mode = KnowledgeMode::Off;
    lab.write(
        "a.txt",
        "Rappel : Madame Claire Martin a appelé.
",
    );
    lab.write(
        "b.txt",
        "Suivi : Madame Anne Bernard viendra.
",
    );
    lab.write(
        "c.txt",
        "Note : Madame Rose Marchand écrira.
",
    );
    lab.analyse().await;
    lab.mode = KnowledgeMode::Suggest;
    let mut context = lab.context();

    let asked = std::cell::Cell::new(0);
    let stop_at_the_second = || {
        asked.set(asked.get() + 1);
        asked.get() >= 2
    };
    let first =
        backfill::refresh_documents(&mut lab.index, &mut context, &stop_at_the_second, &|_| {})
            .expect("refreshes");
    assert!(first.stopped);
    assert_eq!(first.refreshed, 1);
    assert!(backfill::anything_due(&lab.index, &context).expect("asks"));

    let mut context = lab.context();
    let second = backfill::refresh_documents(&mut lab.index, &mut context, &|| false, &|_| {})
        .expect("refreshes");
    assert!(!second.stopped);
    // The two that were left, and the first again: reading it taught the list a name, so it is
    // behind the list it helped to write.
    assert_eq!(second.refreshed, 3);
    assert!(!backfill::anything_due(&lab.index, &context).expect("asks"));
    for name in ["Claire Martin", "Anne Bernard", "Rose Marchand"] {
        assert!(entity_named(&lab.side(), name).is_some(), "{name}");
    }
    clean(&lab);
}

#[tokio::test]
async fn a_refresh_never_overwrites_a_document_that_changed_under_it() {
    let mut lab = Lab::new();
    lab.write(
        "a.txt",
        "Rappel : Madame Claire Martin a appelé.
",
    );
    lab.analyse().await;
    let before = snapshot(&lab.side());

    // A delta read from a version of the file that is no longer the stored one.
    let old = KnowledgeDelta::empty(SourceRef {
        domain: Domain::Documents,
        relative_path: "a.txt".to_string(),
        content_id: "sha-of-an-older-version".to_string(),
    });
    assert_eq!(
        lab.index.apply_knowledge_refresh(&old).unwrap(),
        RefreshWrite::Stale
    );
    // And one for a document that is gone.
    let gone = KnowledgeDelta::empty(SourceRef {
        domain: Domain::Documents,
        relative_path: "gone.txt".to_string(),
        content_id: "sha".to_string(),
    });
    assert_eq!(
        lab.index.apply_knowledge_refresh(&gone).unwrap(),
        RefreshWrite::Stale
    );

    assert_eq!(
        snapshot(&lab.side()),
        before,
        "a stale refresh writes nothing"
    );
}

// ------------------------------------------------------------------------------------ the scans

fn page(path: &str, text: &str, confidence: f32) -> OcrPage {
    OcrPage {
        confidence: Some(confidence),
        ..FakeOcrProvider::recognised_page(path, 1, text)
    }
}

const PHOTO: &str = "2026-03-26_ordonnance-scan.png";

fn copy_photo(lab: &Lab) {
    let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("fixtures")
        .join("gp-sandbox")
        .join("inbox")
        .join(PHOTO);
    std::fs::copy(source, lab.work_path().join(PHOTO)).expect("copies the fixture");
}

#[tokio::test]
async fn a_scan_read_by_ocr_feeds_the_knowledge_base_and_a_poor_scan_creates_nothing() {
    let text = "Ordonnance du Docteur Anne Lefèvre pour Madame Rose Marchand, IBAN FR76 3000 6000 0112 3456 7890 189.";

    let mut lab = Lab::new();
    copy_photo(&lab);
    let engine = FakeOcrProvider::new();
    engine.set_response(PHOTO, 1, Ok(page(PHOTO, text, 0.92)));
    let mut context = lab.context();
    let url = lab.gateway.url.clone();
    lab.analyse_through(&mut context, &url, Some(&engine), None)
        .await;
    assert!(entity_named(&lab.side(), "Anne Lefèvre").is_some());
    assert!(entity_named(&lab.side(), "Rose Marchand").is_some());
    assert!(entities(&lab.side())
        .iter()
        .any(|row| row.subtype.as_deref() == Some("iban")));

    // The same scan, read badly: it may repeat a name already known, and nothing else.
    let mut poor = Lab::new();
    copy_photo(&poor);
    let engine = FakeOcrProvider::new();
    engine.set_response(PHOTO, 1, Ok(page(PHOTO, text, 0.65)));
    let mut context = poor.context();
    let url = poor.gateway.url.clone();
    poor.analyse_through(&mut context, &url, Some(&engine), None)
        .await;
    assert_eq!(poor.index.chunk_count().unwrap(), 1, "the scan is indexed");
    assert_eq!(
        entities(&poor.side()).len(),
        0,
        "a poor reading cannot create a person or an identifier: {:?}",
        live(&poor)
    );
}

#[tokio::test]
async fn a_poor_scan_may_repeat_a_name_the_base_already_knows() {
    let mut lab = Lab::new();
    lab.write("a.txt", "Madame Rose Marchand a appelé.\n");
    lab.analyse().await;
    copy_photo(&lab);
    let engine = FakeOcrProvider::new();
    engine.set_response(
        PHOTO,
        1,
        Ok(page(
            PHOTO,
            "Ordonnance pour Rose Marchand et Luc Perrin.",
            0.65,
        )),
    );
    let mut context = lab.context();
    let url = lab.gateway.url.clone();
    lab.analyse_through(&mut context, &url, Some(&engine), None)
        .await;

    let rose = entity(&lab, "Rose Marchand");
    let scan = mentions_of(&lab.side(), rose.entity_id)
        .into_iter()
        .find(|m| m.path == PHOTO)
        .expect("a mention in the scan");
    assert!(
        scan.confidence < 0.9 * 0.7,
        "scaled by the page's confidence: {scan:?}"
    );
    assert!(entity_named(&lab.side(), "Luc Perrin").is_none());
}

// ----------------------------------------------------------------------------------- robustness

#[tokio::test]
async fn odd_documents_never_stop_the_pass() {
    let mut lab = Lab::new();
    lab.write("empty.txt", "");
    lab.write("digits.txt", "12345 67890 2026 0042 99\n");
    lab.write(
        "scripts.txt",
        "Zhang Wei 中文文字 Дмитрий Иванов 東京 مرحبا\n",
    );
    lab.write_bytes("broken.pdf", b"%PDF-1.4 this is not a real pdf at all");
    let repeated = "Contrôle : Docteur Jean Dupont a signé.\n".repeat(2_000);
    lab.write("huge.txt", &repeated);
    lab.write("normal.txt", "Madame Claire Martin a appelé.\n");

    let summary = lab.analyse().await;

    assert!(summary.scanned_files >= 6);
    assert!(entity_named(&lab.side(), "Claire Martin").is_some());
    assert!(entity_named(&lab.side(), "Jean Dupont").is_some());
    let knowledge = summary.knowledge.expect("summary");
    assert_eq!(knowledge.errors, 0);
    clean(&lab);
}

#[tokio::test]
async fn a_document_with_far_too_many_names_is_capped_and_reported() {
    let mut lab = Lab::new();
    let mut text = String::new();
    for number in 0..700 {
        text.push_str(&format!("vu {} {}, ", word(number), word(number + 5_000)));
    }
    lab.write("directory.txt", &text);

    let summary = lab.analyse().await.knowledge.expect("summary");

    assert_eq!(summary.truncated_sources, 1);
    let people = entities(&lab.side()).len();
    assert!(people <= 500, "{people}");
    assert!(people >= 400, "{people}");
    clean(&lab);
}

/// A pronounceable word that is different for every number.
fn word(number: usize) -> String {
    let consonants = ['b', 'd', 'f', 'g', 'k', 'l', 'm', 'n', 'p', 'r', 's', 't'];
    let vowels = ['a', 'e', 'i', 'o', 'u'];
    let mut n = number;
    let mut out = String::new();
    for round in 0..4 {
        out.push(consonants[n % consonants.len()]);
        n /= consonants.len();
        out.push(vowels[(n + round) % vowels.len()]);
        n /= vowels.len();
    }
    let mut chars = out.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

// ------------------------------------------------------------------------ the product's promises

#[tokio::test]
async fn the_knowledge_tables_hold_names_and_never_a_passage() {
    let mut lab = Lab::new();
    lab.write(
        "letter.txt",
        "Madame Claire Martin a déclaré que la livraison attendue depuis longtemps n'est jamais arrivée au cabinet.\n",
    );

    lab.analyse().await;

    for text in every_text_value(&lab.side()) {
        assert!(
            !text.contains("livraison") && !text.contains("déclaré") && text.chars().count() < 80,
            "a knowledge table holds a name, a code or a count, never prose: {text:?}"
        );
    }
}

#[tokio::test]
async fn an_unchanged_pass_leaves_the_index_free_of_other_work() {
    // The pass keeps the chunks exactly as before the knowledge base: same count, same text.
    let mut with = Lab::new();
    with.write("a.txt", LETTER);
    with.analyse().await;

    let mut without = Lab::new();
    without.mode = KnowledgeMode::Off;
    without.write("a.txt", LETTER);
    without.analyse().await;

    let chunks = |lab: &Lab| -> Vec<String> {
        lab.index
            .all_chunks()
            .unwrap()
            .into_iter()
            .map(|chunk| format!("{}|{}", chunk.chunk_id, chunk.text))
            .collect()
    };
    assert_eq!(chunks(&with), chunks(&without));
}

#[test]
fn the_fixture_documents_of_the_first_human_pass_are_named_by_the_extractor() {
    // The pilot's own fictional documents (they already contain Martin and Dupont). Read through the
    // real extraction, so a regression in either shows here before it shows to the owner.
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("test-reports")
        .join("human-acceptance-pass-1")
        .join("fixtures")
        .join("documents");
    if !fixtures.exists() {
        return;
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let mut lab = Lab::new();
        copy_tree(&fixtures, lab.work_path());
        let summary = lab.analyse().await.knowledge.expect("summary");
        assert!(summary.entities_detected > 0, "{summary:?}");
        assert_eq!(summary.errors, 0);
        clean(&lab);
    });
}

fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    for entry in std::fs::read_dir(from).expect("reads the folder") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            std::fs::create_dir_all(&target).expect("creates");
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("copies");
        }
    }
}

// ------------------------------------------------------------------------------------ the table

/// Prints what a pass learns from the pilot's fictional documents, one line per entity. For the
/// owner's human test: `cargo test --test knowledge_documents print_what_a_pass_learns -- --ignored
/// --nocapture`.
#[test]
#[ignore]
fn print_what_a_pass_learns_from_the_first_human_pass_documents() {
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("test-reports")
        .join("human-acceptance-pass-1")
        .join("fixtures")
        .join("documents");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let mut lab = Lab::new();
        // The application's default: the French health module. `ACAI_PACKS=health-ch` shows what the
        // Swiss module reads instead (one health module at a time, never both).
        lab.packs = match std::env::var("ACAI_PACKS") {
            Ok(list) => list
                .split(',')
                .map(|id| &*Box::leak(id.trim().to_string().into_boxed_str()))
                .collect(),
            Err(_) => vec!["health-fr"],
        };
        copy_tree(&fixtures, lab.work_path());
        let summary = lab.analyse().await.knowledge.expect("summary");
        println!("summary: {summary:?}");
        let side = lab.side();
        for row in entities(&side) {
            let mentions = mentions_of(&side, row.entity_id);
            let files: Vec<_> = mentions.iter().map(|m| m.path.as_str()).collect();
            println!(
                "{:<12} {:<10} {:<34} {}",
                row.type_id,
                row.status,
                row.canonical,
                files.join(", ")
            );
        }
    });
}

// --------------------------------------------------------------------------------- the benchmark

/// Extraction cost per 1 000 chunks, to put beside the embedding cost of one chunk (0.7 s on the
/// reference CPU). Prints; does not assert. `cargo test --release --test knowledge_documents
/// extraction_cost -- --ignored --nocapture`.
#[test]
#[ignore]
fn extraction_cost_per_thousand_chunks() {
    let mut lab = Lab::new();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let mut chunks = Vec::new();
    for number in 0..1_000 {
        chunks.push(
            assistant_cabinet_ai_lib::knowledge::extract::ChunkInput::native(
                &format!("bench.txt#p1#s{}", number + 1),
                &format!(
                    "Contrôle du dossier. Docteur {} {} a reçu Madame {} {} le 12 mars 2026, \
                 voir {}@exemple.fr et facture FAC-2026-{:04}. Association {} {} présente. \
                 Cabinet Médical du Parc. Plus tard, {} a signé.",
                    word(number),
                    word(number + 1),
                    word(number + 2),
                    word(number + 3),
                    word(number + 4),
                    number,
                    word(number % 50),
                    word(number % 70),
                    word(number + 1)
                ),
            ),
        );
    }
    runtime.block_on(async {
        lab.write("seed.txt", "Madame Claire Martin a appelé.\n");
        lab.analyse().await;
    });
    let context = lab.context();
    let side = lab.side();
    let started = std::time::Instant::now();
    let built = context
        .build(
            &side,
            assistant_cabinet_ai_lib::knowledge::SourceRef {
                domain: Domain::Documents,
                relative_path: "bench.txt".to_string(),
                content_id: "sha-bench".to_string(),
            },
            chunks,
        )
        .expect("builds");
    println!(
        "extraction + resolution of 1000 chunks: {:?} ({} mentions, {} entities)",
        started.elapsed(),
        built.delta.mentions.len(),
        built.delta.entities.len()
    );
}

/// Writes an index with the same shape as the human test's (the pilot's fictional documents), then
/// lets the owner's script read it. Kept as a test so the script cannot drift from the schema:
/// `python show_kb_entities.py <path printed here>`.
#[test]
#[ignore]
fn write_an_index_for_the_inspection_script() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let mut lab = Lab::new();
        lab.write("courrier-dupont.txt", LETTER);
        lab.write("compta.txt", WITH_IDENTIFIERS);
        lab.analyse().await;
        let kept = std::env::temp_dir().join("acai-lot4-inspection.sqlite3");
        std::fs::copy(lab.index_path(), &kept).expect("copies the index");
        println!("index copied to {}", kept.display());
    });
}
