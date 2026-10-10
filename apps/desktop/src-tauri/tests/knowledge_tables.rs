//! Lot 5: the knowledge base learns what the columns of a workbook mean, and the names, organisations
//! and identifiers their values hold.
//!
//! The first half drives the table extractor on columns (what a header is read as, and when the
//! values refuse it). The second half drives the real Data Analyse (`data_folder::analyse_with_knowledge`)
//! over real files in a temporary folder and then looks at the `kb_*` tables from a second connection:
//! provenance down to the sheet, column and rows; what a second pass does (nothing); what a changed,
//! a deleted or an unreadable workbook leaves behind; that a personal identifier is never written as
//! itself; that the engine's inventory is the one it has always been.
//!
//! Every name, address and number in this file is invented. The headers are French because the pilot
//! is; the code under test holds no French word (the packs do).

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use assistant_cabinet_ai_lib::knowledge::extract::table::{
    SemanticReason, TableEntityExtractor, MAX_DISTINCT_VALUES_PER_COLUMN,
};
use assistant_cabinet_ai_lib::knowledge::extract::{
    ColumnInput, ColumnKind, ExtractContext, SheetInput, WorkbookInput,
};
use assistant_cabinet_ai_lib::knowledge::gazetteer::Gazetteer;
use assistant_cabinet_ai_lib::knowledge::packs::PackSet;
use assistant_cabinet_ai_lib::knowledge::phonetic::{encoder_for_locale, PhoneticEncoder};
use assistant_cabinet_ai_lib::knowledge::store as kb;
use assistant_cabinet_ai_lib::knowledge::{
    Domain, IntegrityReport, KnowledgeMode, NewEntity, Origin, SourceRef,
};

use common::knowledge::{
    attributes_of, cells_mentions_of, column_answers, column_reading, count, entities,
    entity_named, every_text_value, mentions_of, relations, snapshot, Lab,
};
use common::tabular_fixtures::{write_xlsx, Cell};

// ------------------------------------------------------------------------------------- helpers

fn clean(lab: &Lab) {
    assert_eq!(
        kb::integrity_check(&lab.side()).expect("checks"),
        IntegrityReport::default(),
        "the knowledge base must be consistent"
    );
}

fn entity(lab: &Lab, canonical: &str) -> common::knowledge::EntityRow {
    entity_named(&lab.side(), canonical).unwrap_or_else(|| {
        let known: Vec<_> = entities(&lab.side())
            .into_iter()
            .map(|row| (row.type_id, row.canonical, row.status))
            .collect();
        panic!("no entity named {canonical:?}: {known:?}")
    })
}

fn names_of_type(lab: &Lab, type_id: &str) -> Vec<String> {
    entities(&lab.side())
        .into_iter()
        .filter(|row| row.type_id == type_id)
        .map(|row| row.canonical)
        .collect()
}

fn lines(rows: &[&str]) -> String {
    let mut text = rows.join("\n");
    text.push('\n');
    text
}

/// A CP1252 file, the way a French spreadsheet exports a CSV.
fn cp1252(text: &str) -> Vec<u8> {
    let (bytes, _, unmappable) = encoding_rs::WINDOWS_1252.encode(text);
    assert!(!unmappable);
    bytes.into_owned()
}

fn context_for(packs: &[&str]) -> (PackSet, Box<dyn PhoneticEncoder>) {
    (
        PackSet::load_with_type_vocabulary("fr-FR", packs).expect("loads"),
        encoder_for_locale("fr-FR"),
    )
}

fn column(header: &str, values: &[&str]) -> ColumnInput {
    ColumnInput::text(header, values)
}

fn sheet(name: &str, columns: Vec<ColumnInput>) -> SheetInput {
    SheetInput {
        name: name.to_string(),
        has_header: true,
        row_numbers: Vec::new(),
        columns,
    }
}

fn workbook(sheets: Vec<SheetInput>) -> WorkbookInput {
    WorkbookInput {
        source: SourceRef {
            domain: Domain::Data,
            relative_path: "classeur.csv".to_string(),
            content_id: "sha".to_string(),
        },
        sheets,
    }
}

/// How the extractor reads one column under a set of active modules.
fn read_as(
    locale: &str,
    modules: &[&str],
    header: &str,
    values: &[&str],
) -> Option<(String, Option<String>, Option<String>, SemanticReason)> {
    let packs = PackSet::load_with_type_vocabulary(locale, modules).expect("loads");
    let extractor = TableEntityExtractor::for_locale(locale).expect("loads the vocabulary");
    extractor
        .classify(&packs, "Feuille", 0, &column(header, values))
        .map(|found| {
            (
                found.entity_type.as_str().to_string(),
                found.subtype,
                found.role.map(|role| role.as_str().to_string()),
                found.reason,
            )
        })
}

const PEOPLE: [&str; 6] = [
    "Alice Archer",
    "Bruno Baker",
    "Carole Clark",
    "Denis Dumas",
    "Emma Evans",
    "Felix Fabre",
];

/// A mail-merge export, the shape of the pilot's own: a given name and a family name in two columns,
/// an e-mail address, a client number, an order number and an article.
fn mailing_csv() -> String {
    lines(&[
        "ID_Client,Civilite,Prenom,Nom,Email,No_Commande,Article,Quantite,Prix_Unitaire,Total_Ligne",
        "101,M.,Jean,Dupont,jean.dupont@exemple.fr,CMD-2026-001,Ordinateur Portable,1,899.0,899.0",
        "101,M.,Jean,Dupont,jean.dupont@exemple.fr,CMD-2026-001,Souris Sans Fil,1,25.5,25.5",
        "102,Mme,Sophie,Martin,sophie.martin@exemple.fr,CMD-2026-002,Chaise Ergonomique,1,189.0,189.0",
        "103,M.,Thomas,Rousseau,thomas.rousseau@exemple.fr,CMD-2026-003,Ecran 27 pouces,2,199.99,399.98",
    ])
}

/// Appointments and invoices: the two workbooks of the pilot's data folder, in miniature.
fn appointments_csv() -> String {
    lines(&[
        "date;patient;medecin traitant;duree_min",
        "02/03/2026;Alice Archer;Dr Bruno Baker;20",
        "03/03/2026;Carole Clark;Dr Bruno Baker;25",
        "04/03/2026;Alice Archer;Dr Denis Dumas;20",
        "05/03/2026;Emma Evans;Dr Bruno Baker;45",
    ])
}

fn invoices_csv() -> String {
    lines(&[
        "date;fournisseur;montant",
        "02/03/2026;MedSupply;450",
        "03/03/2026;PapeterieDuParc;180",
        "04/03/2026;MedSupply;620",
        "05/03/2026;Dupont Freres SARL;75",
    ])
}

// ------------------------------------------------------------------- headers, per language

#[test]
fn a_header_is_read_the_same_whatever_its_spelling() {
    // French, accented or not, snake case, camel case, spaced, upper case.
    let modules = ["health-fr"];
    for header in [
        "Patient",
        "patient",
        "PATIENT",
        "Patiente",
        "nom_patient",
        "NomPatient",
        "nomPatient",
        "Nom du patient",
        "nom patient",
    ] {
        let found = read_as("fr-FR", &modules, header, &PEOPLE);
        assert_eq!(
            found.as_ref().map(|f| (f.0.as_str(), f.2.as_deref())),
            Some(("person", Some("client"))),
            "{header}"
        );
    }
    for header in [
        "Medecin traitant",
        "Médecin traitant",
        "medecin_traitant",
        "MedecinTraitant",
        "praticien",
        "Docteur",
    ] {
        let found = read_as("fr-FR", &modules, header, &PEOPLE);
        assert_eq!(
            found.as_ref().map(|f| (f.0.as_str(), f.2.as_deref())),
            Some(("person", Some("provider"))),
            "{header}"
        );
    }
    // English headers, read in an English session.
    for header in [
        "Patient",
        "patient_name",
        "patient_name_full",
        "PatientName",
    ] {
        let found = read_as("en-US", &modules, header, &PEOPLE);
        assert_eq!(
            found.as_ref().map(|f| (f.0.as_str(), f.2.as_deref())),
            Some(("person", Some("client"))),
            "{header}"
        );
    }
}

#[test]
fn the_base_reads_clients_suppliers_companies_and_the_parts_of_a_name_with_no_module() {
    let organisations = ["MedSupply", "Dupont Freres SARL", "Papeterie du Parc"];
    for (header, expected) in [
        ("client", ("person", None, Some("client"))),
        ("Cliente", ("person", None, Some("client"))),
        ("destinataire", ("person", None, Some("recipient"))),
        ("auteur", ("person", None, Some("author"))),
        ("Nom", ("person", Some("family_name"), None)),
        ("Prénom", ("person", Some("given_name"), None)),
        ("Prenom", ("person", Some("given_name"), None)),
    ] {
        let found = read_as("fr-FR", &[], header, &PEOPLE);
        assert_eq!(
            found
                .as_ref()
                .map(|f| (f.0.as_str(), f.1.as_deref(), f.2.as_deref())),
            Some(expected),
            "{header}"
        );
    }
    for (header, role) in [
        ("fournisseur", Some("supplier")),
        ("Fournisseur", Some("supplier")),
        ("Nom du fournisseur", Some("supplier")),
        ("entreprise", None),
        ("société", None),
        ("Raison sociale", None),
    ] {
        let found = read_as("fr-FR", &[], header, &organisations);
        assert_eq!(
            found.as_ref().map(|f| (f.0.as_str(), f.2.as_deref())),
            Some(("organization", role)),
            "{header}"
        );
    }
    // English.
    assert_eq!(
        read_as("en-US", &[], "supplier", &organisations).map(|f| (f.0, f.2)),
        Some(("organization".to_string(), Some("supplier".to_string())))
    );
    assert_eq!(
        read_as("en-US", &[], "Customer Name", &PEOPLE).map(|f| (f.0, f.2)),
        Some(("person".to_string(), Some("client".to_string())))
    );
}

#[test]
fn a_header_only_a_module_reads_carries_less_meaning_when_the_module_is_off_and_is_never_retyped() {
    // `patient` is the health modules' word. With none of them on, the column is not classified; it is
    // not read as a client, a name or anything else.
    assert!(read_as("fr-FR", &[], "patient", &PEOPLE).is_none());
    assert!(read_as("fr-FR", &["legal-fr"], "patient", &PEOPLE).is_none());
    // "nom" is a word of the base, and "patient" a word of person headers: the column is still a
    // column of people, with less meaning (no role) and never another type.
    let without = read_as("fr-FR", &["accounting-fr"], "nom_patient", &PEOPLE).expect("read");
    assert_eq!((without.0.as_str(), without.2.as_deref()), ("person", None));
    let with = read_as("fr-FR", &["health-fr"], "nom_patient", &PEOPLE).expect("read");
    assert_eq!(
        (with.0.as_str(), with.2.as_deref()),
        ("person", Some("client"))
    );
    assert!(read_as("fr-FR", &["health-fr"], "patient", &PEOPLE).is_some());
    assert!(read_as("fr-FR", &["health-ch"], "patient", &PEOPLE).is_some());
    // An insurer is an organisation in the Swiss module and in the French one; with neither on, the
    // header is left alone. It is never read as a person because the word "nom" is a person's.
    let funds = ["Caisse Atlas", "Assurance du Lac", "Mutuelle des Alpes"];
    assert!(read_as("fr-FR", &[], "nom de la caisse", &funds).is_none());
    assert!(read_as("fr-FR", &[], "caisse maladie", &funds).is_none());
    let found = read_as("fr-FR", &["health-ch"], "caisse maladie", &funds).expect("read");
    assert_eq!(found.0, "organization");
    assert_eq!(found.1.as_deref(), Some("insurer"));
    let found = read_as("fr-FR", &["health-fr"], "nom de la caisse", &funds).expect("read");
    assert_eq!(found.0, "organization");
}

#[test]
fn a_module_never_changes_the_type_of_a_column() {
    // The table twin of `a_module_never_changes_the_type_of_a_name`: for every header any shipped pack
    // knows, in both languages, under every combination of the optional modules, the column is either
    // left alone or read as the type the header has. Never another one.
    let available = assistant_cabinet_ai_lib::knowledge::packs::available_pack_ids();
    let combinations: Vec<Vec<&str>> = (0..(1u32 << available.len()))
        .map(|mask| {
            available
                .iter()
                .enumerate()
                .filter(|(position, _)| mask & (1 << position) != 0)
                .map(|(_, id)| *id)
                .collect()
        })
        .collect();
    for locale in ["fr-FR", "en-US"] {
        let everything = PackSet::load(locale, &available).expect("loads");
        let extractor = TableEntityExtractor::for_locale(locale).expect("loads");
        let sets: Vec<(&Vec<&str>, PackSet)> = combinations
            .iter()
            .map(|modules| {
                (
                    modules,
                    PackSet::load_with_type_vocabulary(locale, modules).expect("loads"),
                )
            })
            .collect();
        let mut headers: Vec<(String, String)> = Vec::new();
        for rule in everything.column_rules() {
            for entry in rule.headers.iter().map(|words| words.join(" ")) {
                headers.push((entry, rule.entity_type.as_str().to_string()));
            }
        }
        assert!(headers.len() > 40, "{locale}: the vocabulary is read");
        // Values that fit any type of column: a short text which is also a name.
        let values: Vec<&str> = vec!["Alice Archer", "Bruno Baker", "Carole Clark", "Denis Dumas"];
        let mut read = 0;
        for (header, expected) in &headers {
            for (modules, packs) in &sets {
                if let Some(found) = extractor.classify(packs, "S", 0, &column(header, &values)) {
                    read += 1;
                    assert_eq!(
                        found.entity_type.as_str(),
                        expected,
                        "{locale}: {header:?} with {modules:?} was read as {}",
                        found.entity_type.as_str()
                    );
                }
            }
        }
        assert!(
            read > headers.len(),
            "{locale}: headers are read at all ({read})"
        );
    }
}

// ------------------------------------------------------------------- what the values refuse

#[test]
fn the_whole_header_is_compared_before_it_is_split_into_words() {
    // The lesson of defect KBD-01: a column named with a word of some vocabulary is not that
    // vocabulary. An entry the header equals wins over a shorter entry it merely contains, and a
    // header made of words no entry holds is nothing, whatever the words are elsewhere.
    let exact = read_as("fr-FR", &[], "Nom du client", &PEOPLE).expect("read");
    assert_eq!(
        (exact.0.as_str(), exact.1, exact.2.as_deref()),
        ("person", None, Some("client"))
    );
    assert_eq!(exact.3, SemanticReason::HeaderExact);
    let partial = read_as("fr-FR", &[], "Nom client", &PEOPLE).expect("read");
    assert_eq!(partial.3, SemanticReason::HeaderPartial);
    assert!(read_as("fr-FR", &[], "Total_Ligne", &["899.0", "25.5", "189.0"]).is_none());
    assert!(read_as("fr-FR", &[], "Ligne client depuis", &PEOPLE).is_none());
}

#[test]
fn a_column_called_name_that_holds_codes_is_not_a_column_of_names() {
    let codes: Vec<String> = (0..20).map(|i| format!("A{i}7-{i}")).collect();
    let refs: Vec<&str> = codes.iter().map(String::as_str).collect();
    assert!(read_as("fr-FR", &[], "Nom", &refs).is_none());
}

#[test]
fn a_patient_column_of_free_text_notes_is_rejected() {
    let notes: Vec<String> = (0..12)
        .map(|i| {
            format!("controle de routine numero {i} apres la visite du matin, rien a signaler pour ce dossier")
        })
        .collect();
    let refs: Vec<&str> = notes.iter().map(String::as_str).collect();
    assert!(read_as("fr-FR", &["health-fr"], "Patient", &refs).is_none());
}

#[test]
fn an_amount_and_a_date_never_become_entities() {
    let lab_free = read_as(
        "fr-FR",
        &["health-fr"],
        "Montant",
        &["12,50", "30,00", "45,00"],
    );
    assert!(lab_free.is_none(), "an amount has no header meaning");
    // A column the inventory typed as a date or a number is never a column of names, even under a
    // header a pack knows.
    let packs = PackSet::load_with_type_vocabulary("fr-FR", &["health-fr"]).unwrap();
    let extractor = TableEntityExtractor::for_locale("fr-FR").unwrap();
    for kind in [ColumnKind::Date, ColumnKind::Numeric] {
        let mut dated = column("patient", &["2026-03-02", "2026-03-03", "2026-03-04"]);
        dated.kind = kind;
        assert!(
            extractor.classify(&packs, "S", 0, &dated).is_none(),
            "{kind:?}"
        );
    }
}

#[test]
fn a_column_of_social_security_numbers_is_an_identifier_column_only_where_the_module_knows_the_number(
) {
    let numbers = [
        "1 85 03 69 123 456 78",
        "2 90 07 13 055 123 45",
        "1 77 11 75 108 222 31",
        "2 01 02 33 063 011 12",
        "1 68 05 44 109 333 90",
    ];
    let with = read_as("fr-FR", &["health-fr"], "NIR", &numbers).expect("read");
    assert_eq!(with.0, "identifier");
    assert_eq!(with.1.as_deref(), Some("health_insurance_number"));
    assert!(
        read_as("fr-FR", &[], "NIR", &numbers).is_none(),
        "less meaning, not another meaning"
    );
    assert!(read_as("fr-FR", &["legal-fr"], "NIR", &numbers).is_none());
}

#[test]
fn a_column_with_no_usable_header_but_all_identifiers_is_read_by_its_values() {
    let mails = [
        "a.archer@exemple.fr",
        "b.baker@exemple.fr",
        "c.clark@exemple.fr",
        "d.dumas@exemple.fr",
        "e.evans@exemple.fr",
    ];
    let found = read_as("fr-FR", &[], "Contact", &mails).expect("read by its values");
    assert_eq!(found.0, "identifier");
    assert_eq!(found.3, SemanticReason::ValueShape);
    // Too few values to trust the shape.
    assert!(read_as("fr-FR", &[], "Contact", &mails[..3]).is_none());
    // A header another module reads is left alone rather than read by its values.
    assert!(read_as("fr-FR", &[], "patient", &mails).is_none());
}

// ----------------------------------------------------------------------- the extractor on cells

fn read_workbook(
    modules: &[&str],
    input: &WorkbookInput,
) -> assistant_cabinet_ai_lib::knowledge::extract::table::TableReading {
    let (packs, encoder) = context_for(modules);
    let gazetteer = Gazetteer::new();
    TableEntityExtractor::for_locale("fr-FR")
        .expect("loads")
        .read(
            input,
            &ExtractContext {
                packs: &packs,
                locale: "fr-FR",
                gazetteer: &gazetteer,
                encoder: encoder.as_ref(),
            },
        )
}

#[test]
fn empty_null_like_numeric_and_long_values_are_not_names_and_repeats_count_once() {
    let long_text = "Alice Archer a telephone puis elle a rappele le lendemain pour confirmer";
    let input = workbook(vec![sheet(
        "Feuille",
        vec![column(
            "Patient",
            &[
                "Alice Archer",
                "",
                "N/A",
                "-",
                "?",
                "unknown",
                "NULL",
                "12345",
                "Bruno Baker",
                long_text,
                "alice  archer",
                "ALICE ARCHER",
                "Mars",
            ],
        )],
    )]);

    let reading = read_workbook(&["health-fr"], &input);

    let surfaces: Vec<(&str, u32)> = reading
        .knowledge
        .candidates
        .iter()
        .map(|c| (c.surface.as_str(), c.occurrence_count))
        .collect();
    assert_eq!(surfaces, [("Alice Archer", 3), ("Bruno Baker", 1)]);
}

#[test]
fn a_formula_cell_is_counted_and_never_read() {
    let mut patients = column("Patient", &["Alice Archer", "", "Bruno Baker"]);
    patients.formula_cells = 1;
    let input = workbook(vec![sheet("Feuille", vec![patients])]);

    let reading = read_workbook(&["health-fr"], &input);

    assert_eq!(reading.stats.formula_cells, 1);
    assert_eq!(reading.knowledge.candidates.len(), 2);
}

#[test]
fn a_sheet_with_no_header_row_classifies_nothing() {
    let mut headerless = sheet("Feuille", vec![column("Patient", &PEOPLE)]);
    headerless.has_header = false;
    let reading = read_workbook(&["health-fr"], &workbook(vec![headerless]));
    assert!(reading.columns.is_empty());
    assert!(reading.knowledge.candidates.is_empty());
    assert_eq!((reading.stats.sheets, reading.stats.tables), (1, 0));
}

#[test]
fn a_column_with_more_distinct_values_than_the_cap_keeps_the_cap_and_says_so() {
    let values: Vec<String> = (0..(MAX_DISTINCT_VALUES_PER_COLUMN + 500))
        .map(|i| format!("Personne Numero{}", letters(i)))
        .collect();
    let refs: Vec<&str> = values.iter().map(String::as_str).collect();
    let reading = read_workbook(
        &[],
        &workbook(vec![sheet("Feuille", vec![column("client", &refs)])]),
    );
    assert_eq!(
        reading.knowledge.candidates.len(),
        MAX_DISTINCT_VALUES_PER_COLUMN
    );
    assert!(reading.knowledge.truncated);
}

/// A name without digits that is different for every `i`.
fn letters(mut i: usize) -> String {
    let mut out = String::new();
    loop {
        out.push((b'a' + (i % 26) as u8) as char);
        i /= 26;
        if i == 0 {
            return out;
        }
    }
}

#[test]
fn hundreds_of_columns_are_read_without_trouble() {
    let columns: Vec<ColumnInput> = (0..400)
        .map(|i| column(&format!("colonne_{i}"), &["un", "deux", "trois"]))
        .chain([column("client", &PEOPLE)])
        .collect();
    let reading = read_workbook(&[], &workbook(vec![sheet("Feuille", columns)]));
    assert_eq!(reading.columns.len(), 401, "every column gets an answer");
    assert_eq!(
        reading.columns.iter().filter(|c| c.is_read()).count(),
        1,
        "and only one is read"
    );
    assert_eq!(reading.knowledge.candidates.len(), PEOPLE.len());
}

#[test]
fn relations_come_only_from_two_role_columns_of_one_sheet() {
    let one_sheet = sheet(
        "Feuille",
        vec![
            column("patient", &["Alice Archer", "Alice Archer", "Carole Clark"]),
            column("medecin", &["Bruno Baker", "Bruno Baker", "Denis Dumas"]),
        ],
    );
    let reading = read_workbook(&["health-fr"], &workbook(vec![one_sheet]));
    let pairs: Vec<(String, String, u32)> = reading
        .knowledge
        .relations
        .iter()
        .map(|r| {
            (
                reading.knowledge.candidates[r.subject].surface.clone(),
                format!(
                    "{}>{}",
                    r.predicate, reading.knowledge.candidates[r.object].surface
                ),
                r.occurrence_count,
            )
        })
        .collect();
    assert_eq!(
        pairs,
        [
            (
                "Alice Archer".to_string(),
                "served_by>Bruno Baker".to_string(),
                2
            ),
            (
                "Carole Clark".to_string(),
                "served_by>Denis Dumas".to_string(),
                1
            ),
        ]
    );

    // The same two columns on two different sheets state nothing.
    let split = workbook(vec![
        sheet(
            "Un",
            vec![column("patient", &["Alice Archer"]), column("note", &["x"])],
        ),
        sheet(
            "Deux",
            vec![column("medecin", &["Bruno Baker"]), column("note", &["y"])],
        ),
    ]);
    assert!(read_workbook(&["health-fr"], &split)
        .knowledge
        .relations
        .is_empty());

    // With the module off the two columns carry no role, so they state no relation.
    assert!(read_workbook(
        &[],
        &workbook(vec![sheet(
            "Feuille",
            vec![
                column("patient", &["Alice Archer"]),
                column("medecin", &["Bruno Baker"]),
            ],
        )])
    )
    .knowledge
    .relations
    .is_empty());
}

#[test]
fn a_given_name_and_a_family_name_are_one_person_per_row() {
    let input = workbook(vec![sheet(
        "Feuille",
        vec![
            column("Prenom", &["Jean", "Sophie", "", "Jean"]),
            column("Nom", &["Dupont", "MARTIN", "Solo", "Dupont"]),
        ],
    )]);

    let reading = read_workbook(&[], &input);

    let people: Vec<(&str, u32)> = reading
        .knowledge
        .candidates
        .iter()
        .map(|c| (c.surface.as_str(), c.occurrence_count))
        .collect();
    assert_eq!(people, [("Jean Dupont", 2), ("Sophie MARTIN", 1)]);
    // Both columns are still described, so the management dialog can say why.
    assert_eq!(reading.columns.len(), 2);
}

// ------------------------------------------------------------------------- the real Data pass

#[test]
fn a_data_pass_reads_the_columns_and_records_the_people_with_cell_provenance() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write_data("rdv-mars-2026.csv", &appointments_csv());

    let summary = lab.analyse_data();

    assert_eq!(summary.indexed_files, 1);
    let knowledge = summary.knowledge.expect("the knowledge base took part");
    assert_eq!(knowledge.workbooks_analysed, 1);
    assert_eq!(
        knowledge.semantic_columns, 2,
        "patient and medecin traitant; a date and a duration are not entities"
    );
    assert_eq!(
        knowledge.entities_detected, 5,
        "Alice, Carole, Emma, Bruno, Denis"
    );
    assert_eq!(
        knowledge.relations, 4,
        "one per distinct pair: A-B, C-B, A-D, E-B"
    );
    assert_eq!(knowledge.errors, 0);

    let reading = column_reading(&lab.side(), "rdv-mars-2026.csv");
    assert_eq!(reading.len(), 2);
    assert_eq!(
        (
            reading[0].1.as_str(),
            reading[0].2.as_str(),
            reading[0].5.as_str()
        ),
        ("medecin traitant", "person", "header_exact")
    );
    assert_eq!(reading[0].4.as_deref(), Some("provider"));
    assert_eq!(reading[1].1, "patient");
    assert_eq!(reading[1].4.as_deref(), Some("client"));

    let alice = entity(&lab, "Alice Archer");
    assert_eq!(
        (alice.type_id.as_str(), alice.status.as_str()),
        ("person", "active")
    );
    let mentions = cells_mentions_of(&lab.side(), alice.entity_id);
    assert_eq!(
        mentions.len(),
        1,
        "one mention per entity and column, never one per cell"
    );
    let mention = &mentions[0];
    assert_eq!(mention.path, "rdv-mars-2026.csv");
    assert_eq!(mention.column.as_deref(), Some("patient"));
    assert_eq!(mention.occurrences, 2);
    // The rows of the file, as the spreadsheet program numbers them: row 1 is the header.
    assert_eq!(mention.first_row, Some(2));
    assert_eq!(mention.sample_rows.as_deref(), Some("[2,4]"));
    assert_eq!(mention.method, "column_value");
    assert_eq!(mention.role.as_deref(), Some("client"));

    // The provider is one person whose titles were read, not part of the name.
    let bruno = entity(&lab, "Bruno Baker");
    let bruno_cells = cells_mentions_of(&lab.side(), bruno.entity_id);
    assert_eq!(bruno_cells[0].column.as_deref(), Some("medecin traitant"));
    assert_eq!(bruno_cells[0].occurrences, 3);
    assert_eq!(bruno_cells[0].role.as_deref(), Some("provider"));
    assert!(entity_named(&lab.side(), "Dr Bruno Baker").is_none());

    // The relations are the table's own: a patient is served by a doctor.
    assert_eq!(
        relations(&lab.side()),
        [
            (
                "Alice Archer".to_string(),
                "served_by".to_string(),
                "Bruno Baker".to_string(),
                1
            ),
            (
                "Alice Archer".to_string(),
                "served_by".to_string(),
                "Denis Dumas".to_string(),
                1
            ),
            (
                "Carole Clark".to_string(),
                "served_by".to_string(),
                "Bruno Baker".to_string(),
                1
            ),
            (
                "Emma Evans".to_string(),
                "served_by".to_string(),
                "Bruno Baker".to_string(),
                1
            ),
        ]
    );
    clean(&lab);
}

#[test]
fn a_supplier_column_gives_organisations_with_no_module_and_nothing_is_retyped() {
    let mut lab = Lab::new();
    lab.packs = Vec::new();
    lab.write_data("factures-fournisseurs-2026.csv", &invoices_csv());

    let summary = lab.analyse_data();

    let knowledge = summary.knowledge.expect("took part");
    assert_eq!(knowledge.semantic_columns, 1, "only the supplier column");
    assert_eq!(
        names_of_type(&lab, "organization"),
        ["Dupont Freres SARL", "MedSupply", "PapeterieDuParc"]
    );
    assert!(names_of_type(&lab, "person").is_empty());
    let med = entity(&lab, "MedSupply");
    let cells = cells_mentions_of(&lab.side(), med.entity_id);
    assert_eq!(cells[0].role.as_deref(), Some("supplier"));
    assert_eq!(cells[0].occurrences, 2);
    clean(&lab);
}

#[test]
fn a_module_never_changes_the_type_of_a_name_in_a_table() {
    // The same two workbooks under every module combination: what a module adds is meaning (a role,
    // more columns), never a different type. A name found in two runs has the same type in both.
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for modules in [
        vec![],
        vec!["health-fr"],
        vec!["health-ch"],
        vec!["health-fr", "health-ch"],
        vec!["legal-fr"],
        vec!["accounting-fr"],
    ] {
        let mut lab = Lab::new();
        lab.packs = modules.clone();
        lab.write_data("rdv.csv", &appointments_csv());
        lab.write_data("factures.csv", &invoices_csv());
        lab.analyse_data();

        for row in entities(&lab.side())
            .into_iter()
            .filter(|row| row.type_id != "identifier")
        {
            if let Some(previous) = seen.insert(row.canonical.clone(), row.type_id.clone()) {
                assert_eq!(
                    previous, row.type_id,
                    "{modules:?} retyped {:?} from {previous} to {}",
                    row.canonical, row.type_id
                );
            }
        }
        // An organisation column never produces a person, and a person column never an organisation.
        for supplier in ["MedSupply", "Dupont Freres SARL", "PapeterieDuParc"] {
            if let Some(row) = entity_named(&lab.side(), supplier) {
                assert_eq!(row.type_id, "organization", "{modules:?}: {supplier}");
            }
        }
        for patient in ["Alice Archer", "Carole Clark", "Emma Evans"] {
            if let Some(row) = entity_named(&lab.side(), patient) {
                assert_eq!(row.type_id, "person", "{modules:?}: {patient}");
            }
        }
        // The patients are only known where a module reads the word.
        let knows_patients = modules.iter().any(|m| m.starts_with("health"));
        assert_eq!(
            entity_named(&lab.side(), "Alice Archer").is_some(),
            knows_patients,
            "{modules:?}"
        );
    }
}

#[test]
fn the_parts_of_a_name_become_one_person_and_the_e_mail_is_a_hashed_identifier_of_that_person() {
    let mut lab = Lab::new();
    lab.packs = Vec::new();
    lab.write_data("publipostage.csv", &mailing_csv());

    let summary = lab.analyse_data();

    let knowledge = summary.knowledge.expect("took part");
    assert_eq!(knowledge.workbooks_analysed, 1);
    let jean = entity(&lab, "Jean Dupont");
    assert_eq!(jean.type_id, "person");
    assert!(
        entity_named(&lab.side(), "Dupont").is_none(),
        "a family name alone is not a person"
    );
    assert!(entity_named(&lab.side(), "Jean").is_none());
    let cells = cells_mentions_of(&lab.side(), jean.entity_id);
    assert_eq!(cells.len(), 1);
    assert_eq!(
        cells[0].column.as_deref(),
        Some("Nom"),
        "the mention sits on the family name column"
    );
    assert_eq!(cells[0].occurrences, 2);
    assert_eq!(cells[0].sample_rows.as_deref(), Some("[2,3]"));
    assert_eq!(
        names_of_type(&lab, "person"),
        ["Jean Dupont", "Sophie Martin", "Thomas Rousseau"]
    );

    // The e-mail column is an identifier column; the address itself is stored nowhere.
    let readings = column_reading(&lab.side(), "publipostage.csv");
    let email = readings
        .iter()
        .find(|r| r.1 == "Email")
        .expect("classified");
    assert_eq!(
        (email.2.as_str(), email.3.as_deref()),
        ("identifier", Some("email"))
    );
    let identifiers = names_of_type(&lab, "identifier");
    let emails = identifiers
        .iter()
        .filter(|label| label.starts_with("email "))
        .count();
    assert_eq!(emails, 3, "{identifiers:?}");
    // The order numbers are identifiers too, read by the header `No_Commande` and kept as written.
    let orders: Vec<&String> = identifiers
        .iter()
        .filter(|label| label.starts_with("CMD-"))
        .collect();
    assert_eq!(orders.len(), 3, "{identifiers:?}");
    assert_eq!(identifiers.len(), 6);
    let everything = every_text_value(&lab.side()).join("\n");
    assert!(
        !everything.contains("exemple.fr"),
        "an address was written as itself"
    );
    assert!(!everything.contains("jean.dupont"));

    // The person carries the hash of the address on their row, never the address.
    let attributes = attributes_of(&lab.side(), jean.entity_id);
    assert_eq!(attributes.len(), 1);
    assert_eq!(attributes[0].0, "identifier:email");
    assert_eq!(attributes[0].1.len(), 64, "a keyed hash, in hex");
    clean(&lab);
}

#[test]
fn a_personal_identifier_column_is_never_written_as_itself() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    lab.write_data(
        "patients.csv",
        &lines(&[
            "patient;NIR;AVS;Telephone",
            "Alice Archer;1 85 03 69 123 456 78;756.1234.5678.97;",
            "Bruno Baker;2 90 07 13 055 123 45;756.9876.5432.10;",
            "Carole Clark;1 77 11 75 108 222 31;756.1111.2222.33;",
        ]),
    );

    lab.analyse_data();

    let text = every_text_value(&lab.side()).join("\n");
    for secret in [
        "185036912345678",
        "1 85 03 69",
        "756.1234",
        "7561234567897",
        "290071305512345",
    ] {
        assert!(!text.contains(secret), "{secret} was written");
    }
    let labels = names_of_type(&lab, "identifier");
    assert_eq!(labels.len(), 6);
    assert!(
        labels
            .iter()
            .all(|label| label.starts_with("health_insurance_number ")),
        "{labels:?}"
    );
    let alice = entity(&lab, "Alice Archer");
    let keys: Vec<String> = attributes_of(&lab.side(), alice.entity_id)
        .into_iter()
        .map(|a| a.0)
        .collect();
    assert_eq!(
        keys.len(),
        2,
        "her French number and her Swiss one, both on her row: {keys:?}"
    );
    clean(&lab);
}

#[test]
fn the_same_person_in_two_sheets_and_in_two_workbooks_is_one_entity_with_several_sources() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    write_xlsx(
        &lab.data.path().join("planning.xlsx"),
        &[
            (
                "Mars".to_string(),
                vec![
                    vec![
                        Cell::text("date"),
                        Cell::text("patient"),
                        Cell::text("duree_min"),
                    ],
                    vec![
                        Cell::text("02/03/2026"),
                        Cell::text("Alice Archer"),
                        Cell::Number(20.0),
                    ],
                    vec![
                        Cell::text("03/03/2026"),
                        Cell::text("Bruno Baker"),
                        Cell::Number(25.0),
                    ],
                ],
            ),
            (
                "Avril".to_string(),
                vec![
                    vec![
                        Cell::text("date"),
                        Cell::text("patient"),
                        Cell::text("duree_min"),
                    ],
                    vec![
                        Cell::text("02/04/2026"),
                        Cell::text("Carole Clark"),
                        Cell::Number(20.0),
                    ],
                    vec![
                        Cell::text("03/04/2026"),
                        Cell::text("Alice Archer"),
                        Cell::Number(30.0),
                    ],
                ],
            ),
        ],
    );
    lab.write_data("rdv.csv", &appointments_csv());

    lab.analyse_data();

    let alice = entity(&lab, "Alice Archer");
    let cells = cells_mentions_of(&lab.side(), alice.entity_id);
    let places: Vec<(String, Option<String>, Option<i64>)> = cells
        .iter()
        .map(|m| (m.path.clone(), m.sheet.clone(), m.first_row))
        .collect();
    assert_eq!(
        places,
        [
            (
                "planning.xlsx".to_string(),
                Some("Avril".to_string()),
                Some(3)
            ),
            (
                "planning.xlsx".to_string(),
                Some("Mars".to_string()),
                Some(2)
            ),
            ("rdv.csv".to_string(), Some("rdv".to_string()), Some(2)),
        ]
    );
    let alices: Vec<_> = entities(&lab.side())
        .into_iter()
        .filter(|e| e.canonical == "Alice Archer")
        .collect();
    assert_eq!(alices.len(), 1, "one entity");
    clean(&lab);
}

#[test]
fn a_person_a_document_names_and_a_workbook_lists_is_one_entity() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write(
        "courrier.txt",
        "Objet : suivi\n\nNous avons recu Jean Dupont ce matin pour son bilan.\n",
    );
    let first = futures_block(lab.analyse());
    assert_eq!(first.indexed_files, 1);
    let before = entities(&lab.side());
    assert!(before
        .iter()
        .any(|e| e.canonical == "Jean Dupont" && e.status == "candidate"));

    lab.write_data(
        "clients.csv",
        &lines(&[
            "Prenom;Nom;Email",
            "Jean;Dupont;jean.dupont@exemple.fr",
            "Sophie;Martin;sophie@exemple.fr",
        ]),
    );
    let summary = lab.analyse_data();

    let jeans: Vec<_> = entities(&lab.side())
        .into_iter()
        .filter(|e| e.canonical == "Jean Dupont")
        .collect();
    assert_eq!(
        jeans.len(),
        1,
        "the document's Jean Dupont and the table's are one entity"
    );
    assert_eq!(jeans[0].status, "active", "two sources promote a guess");
    let paths: Vec<String> = mentions_of(&lab.side(), jeans[0].entity_id)
        .into_iter()
        .map(|m| m.path)
        .collect();
    assert!(paths.contains(&"courrier.txt".to_string()));
    assert!(paths.contains(&"clients.csv".to_string()));
    // The names the table taught are looked for in the documents read earlier, locally.
    assert_eq!(summary.knowledge.expect("took part").refreshed_sources, 1);
    // Pressing the Data Analyse again changes nothing, documents included: the documents were read
    // against the new names, and the names did not grow.
    let after_first = snapshot(&lab.side());
    let again = lab.analyse_data();
    let known = again.knowledge.expect("took part");
    assert_eq!((known.refreshed_sources, known.workbooks_analysed), (0, 0));
    assert_eq!(
        after_first,
        snapshot(&lab.side()),
        "a second Data pass writes nothing"
    );
    // Defect KBD-13 must not be copied: the document was read again against the names the table
    // taught, and the table's own mention still says it was found by its column.
    let table_mention = cells_mentions_of(&lab.side(), jeans[0].entity_id);
    assert_eq!(table_mention.len(), 1);
    assert_eq!(table_mention[0].method, "column_value");
    clean(&lab);
}

#[test]
fn an_e_mail_in_a_letter_and_in_a_table_is_one_identifier() {
    // The same key, the same scheme and the same reduction: the hash is the same wherever the
    // address is written, so the two files lead to one entity (and the base still holds no address).
    let mut lab = Lab::new();
    lab.packs = Vec::new();
    lab.write(
        "courrier.txt",
        "Merci d'ecrire a jean.dupont@exemple.fr pour toute question.
",
    );
    futures_block(lab.analyse());
    lab.write_data(
        "clients.csv",
        &lines(&[
            "Prenom;Nom;Email",
            "Jean;Dupont;Jean.Dupont@Exemple.fr",
            "Sophie;Martin;sophie@exemple.fr",
        ]),
    );

    lab.analyse_data();

    let labels = names_of_type(&lab, "identifier");
    assert_eq!(labels.len(), 2, "{labels:?}");
    let side = lab.side();
    let jean_mail = side
        .query_row(
            "SELECT COUNT(DISTINCT s.domain) FROM kb_mentions m
             JOIN kb_sources s ON s.source_id = m.source_id
             JOIN kb_entities e ON e.entity_id = m.entity_id
             WHERE e.type_id = 'identifier'
             GROUP BY e.entity_id ORDER BY COUNT(DISTINCT s.domain) DESC LIMIT 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("reads");
    assert_eq!(
        jean_mail, 2,
        "one address, found in a document and in a table"
    );
    let text = every_text_value(&side).join(
        "
",
    );
    assert!(!text.to_lowercase().contains("exemple.fr"));
    clean(&lab);
}

/// Run an async block from a synchronous test that also drives the data pass.
fn futures_block<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime")
        .block_on(future)
}

#[test]
fn two_people_in_a_document_make_no_relation() {
    // Co-occurrence is not a relation: only a table's own role columns state one.
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write(
        "compte-rendu.txt",
        "Dr Bruno Baker a recu Madame Alice Archer et Madame Carole Clark ce matin.\n",
    );
    futures_block(lab.analyse());
    assert!(relations(&lab.side()).is_empty());
    assert_eq!(count(&lab.side(), "kb_relations"), 0);
}

#[test]
fn rows_and_columns_are_right_on_an_accented_cp1252_file_and_on_a_workbook_with_a_title_row() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    // CP1252, `;`, a title row above the header, accents in headers and in values, decimal commas in
    // the amounts. The delimiter is settled by the adapter from the rows themselves (a file with a
    // title row and no comma anywhere is read as one column: defect KBD-14, not this lot's), so the
    // amounts carry their commas as an export's do.
    let mut rows = vec![
        "Export du 12/03/2026".to_string(),
        "date;Patiente;Médecin traitant;montant".to_string(),
        "02/03/2026;Éloïse Grégoire;Dr Zoé Lefèvre;20,00".to_string(),
        "03/03/2026;Cécile Bérard;Dr Zoé Lefèvre;25,50".to_string(),
        "04/03/2026;Éloïse Grégoire;Dr Zoé Lefèvre;20,00".to_string(),
    ];
    for day in 5..=16 {
        rows.push(format!(
            "{day:02}/03/2026;Cécile Bérard;Dr Zoé Lefèvre;30,00"
        ));
    }
    lab.write_data_bytes(
        "consultations.csv",
        &cp1252(
            &(rows.join(
                "
",
            ) + "
"),
        ),
    );

    lab.analyse_data();

    let eloise = entity(&lab, "Éloïse Grégoire");
    let cells = cells_mentions_of(&lab.side(), eloise.entity_id);
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].column.as_deref(), Some("Patiente"));
    assert_eq!(
        cells[0].first_row,
        Some(3),
        "the title row is line 1, the header line 2, the first data row line 3"
    );
    assert_eq!(cells[0].sample_rows.as_deref(), Some("[3,5]"));
    assert_eq!(cells[0].occurrences, 2);
    let zoe = entity(&lab, "Zoé Lefèvre");
    assert_eq!(
        cells_mentions_of(&lab.side(), zoe.entity_id)[0].occurrences,
        15
    );
    // The accent-free spelling a question would carry reaches the same entity.
    let side = lab.side();
    let by_name = kb::find_entities_by_normalized(&side, "eloise gregoire").expect("reads");
    assert_eq!(by_name.len(), 1);
    clean(&lab);
}

// ------------------------------------------------------------------------------- the lifecycle

#[test]
fn only_the_sheets_the_engine_would_answer_from_are_read() {
    // `mixte.xlsx` has a clean sheet and one made of formulas: the second is red for the engine and
    // is not read for knowledge either, whatever its headers say.
    let mut lab = Lab::new();
    lab.packs = Vec::new();
    write_xlsx(
        &lab.data.path().join("mixte.xlsx"),
        &common::tabular_fixtures::mixte_sheets(),
    );

    lab.analyse_data();

    let sheets: Vec<String> = column_reading(&lab.side(), "mixte.xlsx")
        .into_iter()
        .map(|row| row.0)
        .collect();
    assert_eq!(
        sheets,
        ["Donnees"],
        "the formula sheet is not read: {sheets:?}"
    );
    clean(&lab);
}

#[test]
fn a_second_pass_over_unchanged_workbooks_writes_nothing_and_reads_nothing() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write_data("rdv.csv", &appointments_csv());
    lab.write_data("factures.csv", &invoices_csv());
    lab.write_data("publipostage.csv", &mailing_csv());
    let first = lab.analyse_data();
    assert_eq!(first.knowledge.expect("took part").workbooks_analysed, 3);
    let before = snapshot(&lab.side());

    let second = lab.analyse_data();

    assert_eq!(
        second.indexed_files, 3,
        "the inventories are rebuilt as always"
    );
    let knowledge = second.knowledge.expect("took part");
    assert_eq!(knowledge.workbooks_analysed, 0, "nothing was read again");
    assert_eq!(knowledge.entities_detected, 0);
    assert_eq!(
        before,
        snapshot(&lab.side()),
        "a pass over unchanged files changes nothing"
    );
}

#[test]
fn a_changed_workbook_replaces_its_knowledge_and_a_deleted_one_removes_it() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write_data("rdv.csv", &appointments_csv());
    lab.analyse_data();
    assert!(entity_named(&lab.side(), "Emma Evans").is_some());

    // Emma goes, Felix comes; Alice stays.
    lab.write_data(
        "rdv.csv",
        &lines(&[
            "date;patient;medecin traitant;duree_min",
            "02/03/2026;Alice Archer;Dr Bruno Baker;20",
            "06/03/2026;Felix Fabre;Dr Bruno Baker;30",
        ]),
    );
    let summary = lab.analyse_data();

    assert_eq!(summary.knowledge.expect("took part").workbooks_analysed, 1);
    assert!(
        entity_named(&lab.side(), "Emma Evans").is_none(),
        "stale knowledge went with the old content"
    );
    assert!(entity_named(&lab.side(), "Felix Fabre").is_some());
    assert!(entity_named(&lab.side(), "Alice Archer").is_some());
    assert_eq!(count(&lab.side(), "kb_sources"), 1);
    clean(&lab);

    lab.remove_data("rdv.csv");
    let summary = lab.analyse_data();

    assert_eq!(summary.removed_files, vec!["rdv.csv"]);
    assert_eq!(count(&lab.side(), "kb_sources"), 0);
    assert_eq!(
        count(&lab.side(), "kb_entities"),
        0,
        "an entity nothing supports is collected"
    );
    assert_eq!(count(&lab.side(), "kb_column_semantics"), 0);
    clean(&lab);
}

#[test]
fn the_data_reset_removes_the_data_knowledge_and_only_that() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    futures_block(async {});
    lab.write(
        "courrier.txt",
        "Merci a Docteur Gaston Gilbert pour son aide.\n",
    );
    futures_block(lab.analyse());
    lab.write_data("rdv.csv", &appointments_csv());
    lab.analyse_data();
    assert!(entity_named(&lab.side(), "Gaston Gilbert").is_some());
    assert!(entity_named(&lab.side(), "Alice Archer").is_some());

    lab.index
        .clear_tabular_inventories()
        .expect("resets the data folder");

    assert!(entity_named(&lab.side(), "Alice Archer").is_none());
    assert!(
        entity_named(&lab.side(), "Gaston Gilbert").is_some(),
        "the documents keep theirs"
    );
    let domains: Vec<String> = kb::sources(&lab.side(), None)
        .unwrap()
        .into_iter()
        .map(|s| s.source.domain.as_code().to_string())
        .collect();
    assert_eq!(domains, ["documents"]);
    clean(&lab);

    // And analysing again brings the table's knowledge back.
    lab.analyse_data();
    assert!(entity_named(&lab.side(), "Alice Archer").is_some());
}

#[test]
fn the_same_relative_path_in_both_folders_stays_two_sources() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    // A document source that happens to have the path of a workbook.
    {
        let side = lab.side();
        kb::upsert_source(
            &side,
            &SourceRef {
                domain: Domain::Documents,
                relative_path: "rdv.csv".into(),
                content_id: "sha-doc".into(),
            },
            1,
            0,
        )
        .expect("registers");
    }
    lab.write_data("rdv.csv", &appointments_csv());

    lab.analyse_data();

    let domains: Vec<(String, String)> = kb::sources(&lab.side(), None)
        .unwrap()
        .into_iter()
        .map(|s| (s.source.domain.as_code().to_string(), s.source.content_id))
        .collect();
    assert_eq!(domains.len(), 2);
    assert!(domains.contains(&("documents".to_string(), "sha-doc".to_string())));
    // The column readings belong to the workbook only.
    assert_eq!(column_reading(&lab.side(), "rdv.csv").len(), 2);
    clean(&lab);
}

#[test]
fn a_change_of_modules_reads_an_unchanged_workbook_again() {
    let mut lab = Lab::new();
    lab.packs = Vec::new();
    lab.write_data("rdv.csv", &appointments_csv());
    let first = lab.analyse_data();
    assert_eq!(
        first.knowledge.expect("took part").semantic_columns,
        0,
        "nothing to read without the module"
    );
    assert!(entity_named(&lab.side(), "Alice Archer").is_none());

    lab.packs = vec!["health-fr"];
    let second = lab.analyse_data();

    let knowledge = second.knowledge.expect("took part");
    assert_eq!(
        knowledge.workbooks_analysed, 1,
        "the vocabulary changed, so the workbook is read again"
    );
    assert_eq!(knowledge.semantic_columns, 2);
    assert!(entity_named(&lab.side(), "Alice Archer").is_some());

    // Switching it off again changes the vocabulary once more, so the workbook is read again with
    // less: the patients are no longer known. They are not turned into anything else.
    lab.packs = Vec::new();
    let third = lab.analyse_data();
    assert_eq!(third.knowledge.expect("took part").workbooks_analysed, 1);
    assert!(entities(&lab.side())
        .iter()
        .all(|e| e.canonical != "Alice Archer"));
    clean(&lab);
}

#[test]
fn the_knowledge_mode_off_writes_no_row_and_drops_what_was_learned() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write_data("rdv.csv", &appointments_csv());
    lab.analyse_data();
    assert!(count(&lab.side(), "kb_entities") > 0);

    lab.mode = KnowledgeMode::Off;
    let summary = lab.analyse_data();

    assert!(
        summary.knowledge.is_none(),
        "the switch off leaves the summary as it was"
    );
    assert_eq!(
        summary.indexed_files, 1,
        "the workbook is analysed as always"
    );
    assert_eq!(count(&lab.side(), "kb_sources"), 0);
    assert_eq!(count(&lab.side(), "kb_mentions"), 0);
    assert_eq!(count(&lab.side(), "kb_column_semantics"), 0);
    clean(&lab);
}

#[test]
fn a_deleted_name_is_not_recreated_by_a_table() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write_data("rdv.csv", &appointments_csv());
    lab.analyse_data();
    let carole = entity(&lab, "Carole Clark");
    {
        let mut side = lab.side();
        let tx = side.transaction().expect("opens");
        kb::delete_entity(&tx, carole.entity_id).expect("deletes");
        tx.commit().expect("commits");
    }
    // The workbook changes, which makes it due again.
    lab.write_data(
        "rdv.csv",
        &lines(&[
            "date;patient;medecin traitant;duree_min",
            "02/03/2026;Alice Archer;Dr Bruno Baker;20",
            "03/03/2026;Carole Clark;Dr Bruno Baker;25",
        ]),
    );

    let summary = lab.analyse_data();

    let live: Vec<_> = entities(&lab.side())
        .into_iter()
        .filter(|e| e.canonical == "Carole Clark" && e.status != "deleted")
        .collect();
    assert!(live.is_empty(), "the tombstone suppressed her: {live:?}");
    assert_eq!(summary.knowledge.expect("took part").errors, 0);
    clean(&lab);
}

#[test]
fn a_manual_entity_is_picked_up_by_a_table() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    let manual = {
        let side = lab.side();
        kb::create_entity(
            &side,
            &NewEntity {
                type_id: "person".into(),
                subtype: None,
                canonical_name: "Alice Archer".into(),
                normalized_name: "alice archer".into(),
                disambiguator: String::new(),
                status: assistant_cabinet_ai_lib::knowledge::EntityStatus::Active,
                origin: Origin::Manual,
            },
        )
        .expect("creates")
    };
    lab.write_data("rdv.csv", &appointments_csv());

    lab.analyse_data();

    let alices: Vec<_> = entities(&lab.side())
        .into_iter()
        .filter(|e| e.canonical == "Alice Archer")
        .collect();
    assert_eq!(alices.len(), 1);
    assert_eq!(
        alices[0].entity_id, manual,
        "the table's Alice is the one she typed"
    );
    assert!(!cells_mentions_of(&lab.side(), manual).is_empty());
}

// ----------------------------------------------------------------------------- robustness

#[test]
fn a_broken_workbook_an_empty_one_and_a_title_only_one_do_not_stop_the_pass() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write_data("faux.xlsx", "not a zip archive at all");
    lab.write_data("vide.csv", "");
    lab.write_data("titre.csv", "Juste une note\n");
    lab.write_data("rdv.csv", &appointments_csv());

    let summary = lab.analyse_data();

    assert_eq!(summary.indexed_files, 1, "only the real table is green");
    assert!(summary.empty_files.contains(&"faux.xlsx".to_string()));
    let knowledge = summary.knowledge.expect("took part");
    assert_eq!(knowledge.workbooks_analysed, 1);
    assert!(
        entity_named(&lab.side(), "Alice Archer").is_some(),
        "the other workbooks are unaffected"
    );
    assert_eq!(
        count(&lab.side(), "kb_sources"),
        1,
        "a workbook with no usable sheet has no knowledge"
    );
    clean(&lab);
}

#[test]
fn a_hundred_thousand_rows_are_read_in_a_bounded_time() {
    let mut text = String::from("date;client;fournisseur;montant\n");
    for i in 0..100_000usize {
        text.push_str(&format!(
            "{:02}/03/2026;{};{};{},50\n",
            i % 28 + 1,
            PEOPLE[i % PEOPLE.len()],
            ["MedSupply", "Papeterie du Parc"][i % 2],
            i % 900
        ));
    }
    let mut lab = Lab::new();
    lab.packs = Vec::new();
    lab.write_data("gros.csv", &text);

    let started = std::time::Instant::now();
    let summary = lab.analyse_data();
    let elapsed = started.elapsed();

    let knowledge = summary.knowledge.expect("took part");
    assert_eq!(knowledge.semantic_columns, 2);
    assert_eq!(knowledge.entities_detected, 8);
    let alice = entity(&lab, "Alice Archer");
    assert_eq!(
        cells_mentions_of(&lab.side(), alice.entity_id)[0].occurrences,
        16_667
    );
    assert!(
        elapsed < std::time::Duration::from_secs(120),
        "100 000 rows took {elapsed:?}"
    );
    clean(&lab);
}

#[test]
fn a_failing_knowledge_step_never_costs_the_workbook() {
    // Owner rule of 9 October 2026: the knowledge base never blocks an analysis.
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write_data("rdv.csv", &appointments_csv());
    lab.write_data("factures.csv", &invoices_csv());
    {
        let side = lab.side();
        side.execute_batch("DROP TABLE kb_column_semantics")
            .expect("breaks a knowledge table");
    }

    let summary = lab.analyse_data();

    assert_eq!(summary.indexed_files, 2, "both workbooks are analysed");
    let knowledge = summary.knowledge.expect("took part");
    assert_eq!(
        knowledge.errors, 2,
        "and the pass says that part of the analysis failed"
    );
    assert_eq!(lab.index.all_tabular_inventories().expect("lists").len(), 2);
    assert_eq!(
        count(&lab.side(), "kb_mentions"),
        0,
        "no half-written knowledge"
    );
}

// ------------------------------------------------------------------------- the engine is untouched

#[test]
fn the_inventory_is_the_same_with_the_knowledge_base_on_and_off() {
    let inventories = |mode: KnowledgeMode| {
        let mut lab = Lab::new();
        lab.packs = vec!["health-fr"];
        lab.mode = mode;
        lab.write_data("rdv.csv", &appointments_csv());
        lab.write_data("factures.csv", &invoices_csv());
        lab.write_data("publipostage.csv", &mailing_csv());
        lab.analyse_data();
        let side = lab.side();
        let mut statement = side
            .prepare("SELECT relative_path, workbook_id, inventory_json FROM tabular_inventories ORDER BY relative_path")
            .expect("prepares");
        let rows: Vec<(String, String, String)> = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .expect("queries")
            .collect::<Result<_, _>>()
            .expect("reads");
        rows
    };
    assert_eq!(
        inventories(KnowledgeMode::Suggest),
        inventories(KnowledgeMode::Off)
    );
}

#[test]
fn the_knowledge_layer_never_writes_sheet_restrictions() {
    // Grep level: no source of the knowledge layer so much as names the engine's scope restriction.
    fn sources(directory: &std::path::Path, found: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(directory).expect("reads the folder") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                sources(&path, found);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                found.push(path);
            }
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("knowledge");
    let mut files = Vec::new();
    sources(&root, &mut files);
    assert!(files.len() > 10);
    for file in files {
        let text = std::fs::read_to_string(&file).expect("reads");
        assert!(
            !text.contains("sheet_restrictions"),
            "{} mentions the engine's sheet restrictions",
            file.display()
        );
    }
    // And the data pass itself writes none either.
    let data_folder = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("data_folder.rs"),
    )
    .expect("reads");
    assert!(!data_folder.contains("sheet_restrictions"));
}

// ------------------------------------------------------------------------------ the summary

#[test]
fn the_summary_reports_counts_and_no_name() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write_data("rdv.csv", &appointments_csv());
    lab.write_data("publipostage.csv", &mailing_csv());

    let summary = lab.analyse_data();

    let knowledge = summary.knowledge.expect("took part");
    let json = serde_json::to_string(&knowledge).expect("serialises");
    for name in ["Alice", "Archer", "Dupont", "exemple"] {
        assert!(!json.contains(name), "the summary carries a name: {json}");
    }
    assert_eq!(knowledge.workbooks_analysed, 2);
    assert!(knowledge.semantic_columns >= 4);
    assert!(json.contains("semanticColumns") && json.contains("workbooksAnalysed"));
}

// ------------------------------------------------------------------------------ benchmarks

#[test]
#[ignore = "benchmark: run in release with --nocapture"]
fn extraction_cost_on_fifty_thousand_rows_of_eight_columns() {
    let mut text =
        String::from("date;client;fournisseur;email;reference;article;quantite;montant\n");
    for i in 0..50_000usize {
        text.push_str(&format!(
            "{:02}/03/2026;{} {};{};client{}@exemple.fr;FA-2026-{:04};Article {};{};{},50\n",
            i % 28 + 1,
            PEOPLE[i % PEOPLE.len()],
            letters(i % 400),
            ["MedSupply", "Papeterie du Parc", "Dupont Freres SARL"][i % 3],
            i % 2_000,
            i % 5_000,
            i % 300,
            i % 9 + 1,
            i % 900
        ));
    }
    let mut lab = Lab::new();
    lab.packs = Vec::new();
    lab.write_data("bench.csv", &text);

    let started = std::time::Instant::now();
    let plain = {
        let mut off = Lab::new();
        off.mode = KnowledgeMode::Off;
        off.write_data("bench.csv", &text);
        let started = std::time::Instant::now();
        off.analyse_data();
        started.elapsed()
    };
    let summary = lab.analyse_data();
    let with_knowledge = started.elapsed();

    let knowledge = summary.knowledge.expect("took part");
    println!(
        "50 000 rows x 8 columns: pass with the knowledge base {:?}, without {:?}; {} entities, {} columns read, {} ms reading names",
        with_knowledge, plain, knowledge.entities_detected, knowledge.semantic_columns, knowledge.elapsed_ms
    );
}

/// What a pass learns from the pilot's fictional workbooks, printed for a human to read. The
/// owner's human test quotes these numbers.
#[test]
#[ignore = "prints what a pass learns from the pilot's fictional workbooks"]
fn print_what_a_pass_learns_from_the_pilot_workbooks() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs/test-reports/human-acceptance-pass-1/fixtures");
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    let files = [
        "data/rdv-mars-2026.xlsx",
        "data/factures-fournisseurs-2026.xlsx",
        "publipostage/donnees_publipostage.csv",
    ];
    for file in files {
        let name = file.rsplit('/').next().expect("a name");
        std::fs::copy(fixtures.join(file), lab.data.path().join(name)).expect("copies");
    }
    let summary = lab.analyse_data();
    println!("summary: {:?}", summary.knowledge);
    for file in files {
        let name = file.rsplit('/').next().expect("a name");
        println!("{name}");
        for row in column_reading(&lab.side(), name) {
            println!("  {row:?}");
        }
    }
    for row in entities(&lab.side()) {
        println!("{} | {} | {}", row.type_id, row.canonical, row.status);
    }
    for row in relations(&lab.side()) {
        println!("relation {row:?}");
    }
}

#[test]
fn a_french_session_reads_english_headers_and_an_english_one_french_headers() {
    // Owner decision of 11 October 2026: an export from an English program is read in a French
    // session, and the other way round. The type is the same in both, and a module that is off still
    // carries less meaning, never another one.
    let organisations = ["MedSupply", "Dupont Freres SARL", "Papeterie du Parc"];
    let found = |locale: &str, modules: &[&str], header: &str, values: &[&str]| {
        read_as(locale, modules, header, values).map(|f| (f.0, f.2))
    };
    let person_client = Some(("person".to_string(), Some("client".to_string())));
    let supplier = Some(("organization".to_string(), Some("supplier".to_string())));

    assert_eq!(found("fr-FR", &[], "Customer", &PEOPLE), person_client);
    assert_eq!(
        found("fr-FR", &["health-fr"], "patient_name", &PEOPLE),
        person_client
    );
    assert_eq!(found("fr-FR", &[], "supplier", &organisations), supplier);
    assert_eq!(found("en-US", &[], "fournisseur", &organisations), supplier);
    assert_eq!(found("en-US", &[], "Nom du client", &PEOPLE), person_client);
    let provider = Some(("person".to_string(), Some("provider".to_string())));
    assert_eq!(
        found("en-US", &["health-fr"], "medecin traitant", &PEOPLE),
        provider
    );

    // With no module the health words of either language carry nothing of their own: the column is
    // still a column of people (the base knows "name"), without the role, never another type.
    assert_eq!(
        found("fr-FR", &[], "patient_name", &PEOPLE),
        Some(("person".to_string(), None))
    );
    assert_eq!(found("en-US", &[], "medecin traitant", &PEOPLE), None);
}

#[test]
fn a_column_that_is_not_read_is_recorded_with_the_reason() {
    let mut lab = Lab::new();
    lab.packs = Vec::new();
    let mut text = String::from(
        "date;patient;salle;montant;Nom;client
",
    );
    for day in 1..=12 {
        text.push_str(&format!(
            "{day:02}/03/2026;{};Salle {};{},50;A{day}7-{day};{}
",
            PEOPLE[day % PEOPLE.len()],
            day % 3 + 1,
            day * 10,
            100 + day
        ));
    }
    lab.write_data("diagnostic.csv", &text);

    let summary = lab.analyse_data();

    let answers = column_answers(&lab.side(), "diagnostic.csv");
    let reason = |column: &str| {
        answers
            .iter()
            .find(|a| a.0 == column)
            .map(|a| (a.1.clone(), a.2.clone(), a.3.clone(), a.4))
            .unwrap_or_else(|| panic!("no answer for {column}: {answers:?}"))
    };
    assert_eq!(answers.len(), 6, "every column has an answer: {answers:?}");
    // No word of any pack, in any language.
    assert_eq!(reason("date").3, 0.0);
    assert_eq!(reason("date").2, "no_header_match");
    assert_eq!(reason("salle").2, "no_header_match");
    assert_eq!(reason("montant").2, "no_header_match");
    // A module that is off would read it: a hint with no confidence, never a reading.
    let patient = reason("patient");
    assert_eq!(
        (
            patient.0.as_str(),
            patient.1.as_deref(),
            patient.2.as_str(),
            patient.3
        ),
        ("person", Some("client"), "module_off", 0.0)
    );
    // The header means something and the values do not look like it.
    let name = reason("Nom");
    assert_eq!(
        (name.0.as_str(), name.2.as_str()),
        ("person", "values_mismatch")
    );
    assert!(name.3 < 0.6);
    // The header means a person and the column holds numbers.
    assert_eq!(reason("client").2, "number_or_date");

    // None of them is read: no entity, no semantic column counted, nothing in the readings.
    assert!(column_reading(&lab.side(), "diagnostic.csv").is_empty());
    let knowledge = summary.knowledge.expect("took part");
    assert_eq!(
        (knowledge.semantic_columns, knowledge.entities_detected),
        (0, 0)
    );
    assert_eq!(count(&lab.side(), "kb_entities"), 0);
    assert_eq!(count(&lab.side(), "kb_column_semantics"), 6);
    clean(&lab);

    // Switching the French module on reads the one column it knows; the others keep their reason.
    lab.packs = vec!["health-fr"];
    lab.analyse_data();
    let after = column_answers(&lab.side(), "diagnostic.csv");
    let patient = after.iter().find(|a| a.0 == "patient").expect("answered");
    assert_eq!(patient.3, "header_exact");
    assert!(after
        .iter()
        .find(|a| a.0 == "salle")
        .is_some_and(|a| a.3 == "no_header_match"));
    assert_eq!(column_reading(&lab.side(), "diagnostic.csv").len(), 1);
}

#[test]
fn rows_are_the_rows_of_the_file_whatever_sits_above_the_table_or_between_its_lines() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    // A sheet whose first two rows are empty and whose header is on row 3: the engine reads it from
    // the header, the knowledge base says "row 4" for the first patient, as Excel does.
    let mut rows: Vec<Vec<Cell>> = vec![vec![], vec![]];
    rows.push(vec![Cell::text("patient"), Cell::text("duree_min")]);
    for (offset, name) in [
        "Alice Archer",
        "Bruno Baker",
        "Alice Archer",
        "Carole Clark",
    ]
    .into_iter()
    .enumerate()
    {
        rows.push(vec![Cell::text(name), Cell::Number(20.0 + offset as f64)]);
    }
    write_xlsx(
        &lab.data.path().join("decale.xlsx"),
        &[("Planning".to_string(), rows)],
    );
    // A CSV with a blank line inside: it has no row and keeps its number.
    let mut text = String::from(
        "date;patient;montant
01/03/2026;Alice Archer;10,50

",
    );
    for day in 3..=12 {
        text.push_str(&format!(
            "{day:02}/03/2026;Bruno Baker;{day},50
"
        ));
    }
    lab.write_data("ligne-vide.csv", &text);

    lab.analyse_data();

    let alice = entity(&lab, "Alice Archer");
    let cells = cells_mentions_of(&lab.side(), alice.entity_id);
    let xlsx = cells
        .iter()
        .find(|m| m.path == "decale.xlsx")
        .expect("in the workbook");
    assert_eq!(xlsx.first_row, Some(4));
    assert_eq!(xlsx.sample_rows.as_deref(), Some("[4,6]"));
    let csv = cells
        .iter()
        .find(|m| m.path == "ligne-vide.csv")
        .expect("in the csv");
    assert_eq!(csv.first_row, Some(2));
    let bruno = entity(&lab, "Bruno Baker");
    let bruno_csv = cells_mentions_of(&lab.side(), bruno.entity_id)
        .into_iter()
        .find(|m| m.path == "ligne-vide.csv")
        .expect("in the csv");
    assert_eq!(bruno_csv.first_row, Some(4), "the blank line is line 3");
    clean(&lab);
}

#[test]
fn an_order_number_column_is_read_by_its_header_and_its_numbers_are_orders() {
    assert_eq!(
        read_as(
            "fr-FR",
            &[],
            "No_Commande",
            &["CMD-2026-001", "CMD-2026-002", "CMD-2026-003"]
        )
        .map(|f| (f.0, f.1, f.3)),
        Some((
            "identifier".to_string(),
            Some("order".to_string()),
            SemanticReason::HeaderExact
        ))
    );
    assert_eq!(
        read_as(
            "en-US",
            &[],
            "Order number",
            &["PO-20260001", "PO-20260002"]
        )
        .map(|f| f.1),
        Some(Some("order".to_string()))
    );
    let mut lab = Lab::new();
    lab.packs = Vec::new();
    lab.write_data("publipostage.csv", &mailing_csv());
    lab.analyse_data();
    let order = entity(&lab, "CMD-2026-001");
    assert_eq!(order.subtype.as_deref(), Some("order"));
}

/// The pilot's three fictional workbooks, copied into the lab's Data folder.
fn copy_pilot_workbooks(lab: &Lab) {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs/test-reports/human-acceptance-pass-1/fixtures");
    for file in [
        "data/rdv-mars-2026.xlsx",
        "data/factures-fournisseurs-2026.xlsx",
        "publipostage/donnees_publipostage.csv",
    ] {
        let name = file.rsplit('/').next().expect("a name");
        std::fs::copy(fixtures.join(file), lab.data.path().join(name)).expect("copies");
    }
}

/// What `lot05_helper.py edit-workbook` does: the same zip, one name changed in the sheet.
fn rename_a_patient_in(workbook: &std::path::Path, from: &str, to: &str) {
    use std::io::{Read, Write};
    let mut archive = zip::ZipArchive::new(std::fs::File::open(workbook).expect("opens")).unwrap();
    let mut parts: Vec<(String, Vec<u8>)> = Vec::new();
    for index in 0..archive.len() {
        let mut part = archive.by_index(index).expect("reads a part");
        let mut bytes = Vec::new();
        part.read_to_end(&mut bytes).expect("reads");
        let name = part.name().to_string();
        if name == "xl/worksheets/sheet1.xml" {
            let text = String::from_utf8(bytes).expect("utf-8");
            assert!(text.contains(&format!("<t>{from}</t>")));
            bytes = text
                .replace(&format!("<t>{from}</t>"), &format!("<t>{to}</t>"))
                .into_bytes();
        }
        parts.push((name, bytes));
    }
    let mut writer = zip::ZipWriter::new(std::fs::File::create(workbook).expect("creates"));
    let options: zip::write::FileOptions<()> = zip::write::FileOptions::default();
    for (name, bytes) in parts {
        writer.start_file(name, options).expect("starts");
        writer.write_all(&bytes).expect("writes");
    }
    writer.finish().expect("finishes");
}

#[test]
fn the_numbers_the_human_test_quotes_hold() {
    // The sequence of `human-tests/lot-05-table-semantics.md`, on the pilot's fictional workbooks:
    // first pass, second pass, the French module off and on again, one workbook changed, one removed.
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    copy_pilot_workbooks(&lab);
    let counts = |summary: assistant_cabinet_ai_lib::indexing::IndexSummary| {
        let k = summary.knowledge.expect("took part");
        (
            k.entities_detected,
            k.matched_existing,
            k.created_new,
            k.identifiers,
            k.semantic_columns,
            k.workbooks_analysed,
            k.relations,
            k.errors,
        )
    };

    // (people and organisations, already known, new, identifiers, columns, workbooks, links, errors)
    assert_eq!(
        counts(lab.analyse_data()),
        (15, 0, 15, 6, 7, 3, 0, 0),
        "first pass"
    );
    assert_eq!(
        counts(lab.analyse_data()),
        (0, 0, 0, 0, 0, 0, 0, 0),
        "second pass"
    );

    lab.packs = Vec::new();
    assert_eq!(
        counts(lab.analyse_data()),
        (6, 6, 0, 6, 6, 3, 0, 0),
        "French module off"
    );
    assert!(entity_named(&lab.side(), "Dupont J").is_none());
    assert_eq!(entity(&lab, "MedSupply").type_id, "organization");
    assert_eq!(entity(&lab, "Jean Dupont").type_id, "person");

    lab.packs = vec!["health-fr"];
    assert_eq!(
        counts(lab.analyse_data()),
        (15, 6, 9, 6, 7, 3, 0, 0),
        "French module on again"
    );

    rename_a_patient_in(
        &lab.data.path().join("rdv-mars-2026.xlsx"),
        "Simon V",
        "Garnier C",
    );
    assert_eq!(
        counts(lab.analyse_data()),
        (9, 8, 1, 0, 1, 1, 0, 0),
        "one workbook changed"
    );
    assert!(entity_named(&lab.side(), "Simon V").is_none());
    assert!(entity_named(&lab.side(), "Garnier C").is_some());

    lab.remove_data("factures-fournisseurs-2026.xlsx");
    let summary = lab.analyse_data();
    assert_eq!(
        summary.removed_files,
        vec!["factures-fournisseurs-2026.xlsx"]
    );
    assert!(entity_named(&lab.side(), "MedSupply").is_none());
    assert!(
        entity_named(&lab.side(), "Dupont J").is_some(),
        "the other workbooks keep theirs"
    );
    clean(&lab);
}

/// Writes an index with the same shape as the human test's (the pilot's fictional workbooks and
/// documents) and keeps a copy, so that the owner's read-only script can be run against it without
/// the application: `python show_kb_entities.py <path printed here>`. Kept as a test so the script
/// cannot drift from the schema.
#[test]
#[ignore = "writes an index for the inspection script"]
fn write_an_index_for_the_inspection_script() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs/test-reports/human-acceptance-pass-1/fixtures");
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write(
        "courrier.txt",
        "Objet : suivi

Nous avons recu Jean Dupont ce matin pour son bilan.
",
    );
    futures_block(lab.analyse());
    for file in [
        "data/rdv-mars-2026.xlsx",
        "data/factures-fournisseurs-2026.xlsx",
        "publipostage/donnees_publipostage.csv",
    ] {
        let name = file.rsplit('/').next().expect("a name");
        std::fs::copy(fixtures.join(file), lab.data.path().join(name)).expect("copies");
    }
    lab.write_data("rdv-sante.csv", &appointments_csv());
    lab.analyse_data();
    let kept = std::env::temp_dir().join("acai-lot5-inspection.sqlite3");
    std::fs::copy(lab.index_path(), &kept).expect("copies the index");
    println!("index copied to {}", kept.display());
}
