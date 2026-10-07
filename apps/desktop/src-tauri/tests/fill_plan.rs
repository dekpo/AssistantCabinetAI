//! Lot E, steps E4 and E5, as the interface drives them, on the owner's own publipostage files
//! (`docs/test-reports/human-acceptance-pass-1/fixtures/publipostage/`): find the workbook and the
//! row a question names, find the template, build the plan, preview it, then write the letters.
//! Nothing here calls a model, and nothing is written until `generate`.

use std::path::PathBuf;

use assistant_cabinet_ai_lib::analysis_scope::ScopeMode;
use assistant_cabinet_ai_lib::data_folder::{self, DataFolder};
use assistant_cabinet_ai_lib::fill_plan::{
    build_plan, candidate_templates, generate, preview, read_template, FillRequest, RequestMapping,
};
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::inventory::{FileHashCache, WorkFolderInventory};
use assistant_cabinet_ai_lib::tabular_answer::{self, FillSource};
use assistant_cabinet_ai_lib::template_fill::docx_text;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("test-reports")
        .join("human-acceptance-pass-1")
        .join("fixtures")
        .join("publipostage")
        .join(name)
}

struct Folders {
    work: tempfile::TempDir,
    data: tempfile::TempDir,
    _app: tempfile::TempDir,
    index: IndexStore,
}

impl Folders {
    fn build() -> Self {
        let work = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let app = tempfile::tempdir().unwrap();
        std::fs::copy(fixture("modele_lettre.docx"), work.path().join("modele_lettre.docx")).unwrap();
        std::fs::copy(fixture("modele_lettre.txt"), work.path().join("modele_lettre.txt")).unwrap();
        std::fs::copy(fixture("donnees_publipostage.csv"), data.path().join("donnees_publipostage.csv")).unwrap();
        let mut index = IndexStore::open_at(&app.path().join("index.sqlite3")).unwrap();
        data_folder::analyse(data.path(), &mut index, "fr-FR", &|_| {}).unwrap();
        Self { work, data, _app: app, index }
    }

    fn data(&self) -> DataFolder {
        DataFolder::discover(self.data.path(), Some(&self.index), &FileHashCache::new()).unwrap()
    }

    fn inventory(&self) -> WorkFolderInventory {
        WorkFolderInventory::discover(self.work.path(), None).unwrap()
    }

    fn source(&self, question: &str, every_row: bool) -> Option<FillSource> {
        tabular_answer::locate_fill_source(
            question,
            &self.data(),
            &ScopeMode::WholeFolder,
            "fr-FR",
            &self.index,
            every_row,
        )
        .unwrap()
    }
}

const OWNERS_QUESTION: &str = "G\u{e9}n\u{e8}re le courrier de la commande CMD-2026-002 en reprenant les donn\u{e9}es du client et en les ins\u{e9}rant dans la lettre correspondante.";

fn request_from(plan: &assistant_cabinet_ai_lib::fill_plan::FillPlan, all: bool, key_column: Option<&str>) -> FillRequest {
    FillRequest {
        template: plan.template.clone(),
        data_file: plan.data_file.clone(),
        key_column: key_column.map(str::to_string).or_else(|| plan.key_column.clone()),
        key_value: plan.key_value.clone(),
        all,
        mapping: plan
            .placeholders
            .iter()
            .map(|placeholder| RequestMapping {
                placeholder: placeholder.name.clone(),
                // The user confirmed every proposal.
                column: placeholder.column.clone(),
            })
            .collect(),
    }
}

#[test]
fn the_owners_question_finds_the_order_the_template_and_a_plan_without_writing_anything() {
    let folders = Folders::build();
    let source = folders.source(OWNERS_QUESTION, false).expect("the order is found in the workbook");
    assert_eq!(source.file, "donnees_publipostage.csv");
    assert_eq!(source.key, Some(("No_Commande".to_string(), "CMD-2026-002".to_string())));

    let templates = candidate_templates(&folders.inventory(), &source.table.columns);
    let paths: Vec<&str> = templates.iter().map(|template| template.relative_path.as_str()).collect();
    assert_eq!(paths[0], "modele_lettre.docx", "the Word letter matches best: {paths:?}");
    assert!(paths.contains(&"modele_lettre.txt"));

    let plan = build_plan(templates, &source.file, &source.sheet, &source.table, source.key).unwrap();
    assert_eq!(plan.template, "modele_lettre.docx");
    assert_eq!(plan.preview.letters, 1);
    assert_eq!(plan.preview.lines, 1);
    assert!(plan.preview.text.contains("Sophie Martin"), "{}", plan.preview.text);
    assert!(plan.preview.text.contains("Chaise Ergonomique"), "{}", plan.preview.text);
    assert!(plan.preview.unresolved.is_empty(), "{:?}", plan.preview.unresolved);
    assert_eq!(plan.preview.file_name, "modele_lettre-CMD-2026-002.docx");
    assert!(!folders.work.path().join("Generated").exists(), "a plan writes nothing");
}

#[test]
fn approving_writes_one_new_word_letter_and_touches_neither_the_template_nor_the_data() {
    let folders = Folders::build();
    let source = folders.source(OWNERS_QUESTION, false).unwrap();
    let templates = candidate_templates(&folders.inventory(), &source.table.columns);
    let plan = build_plan(templates, &source.file, &source.sheet, &source.table, source.key).unwrap();
    let template_before = std::fs::read(folders.work.path().join("modele_lettre.docx")).unwrap();
    let data_before = std::fs::read(folders.data.path().join("donnees_publipostage.csv")).unwrap();

    let request = request_from(&plan, false, None);
    let template = read_template(&folders.inventory(), &request.template).unwrap();
    let report = generate(
        folders.work.path(),
        "Generated",
        &folders.work.path().join("generated.jsonl.outside-the-folder"),
        1,
        &template,
        &source.table,
        &request,
    )
    .unwrap();

    assert_eq!(report.written.len(), 1);
    assert_eq!(report.written[0].relative_path, "Generated/modele_lettre-CMD-2026-002.docx");
    let letter = std::fs::read(folders.work.path().join("Generated").join("modele_lettre-CMD-2026-002.docx")).unwrap();
    let text = docx_text(&letter).unwrap();
    assert!(text.contains("Mme Sophie Martin") && text.contains("Chaise Ergonomique"), "{text}");
    assert!(!text.contains("Dupont"), "{text}");
    assert_eq!(std::fs::read(folders.work.path().join("modele_lettre.docx")).unwrap(), template_before);
    assert_eq!(std::fs::read(folders.data.path().join("donnees_publipostage.csv")).unwrap(), data_before);
}

#[test]
fn for_each_order_one_letter_per_order_with_its_own_lines() {
    let folders = Folders::build();
    let source = folders
        .source("G\u{e9}n\u{e8}re un courrier pour chaque commande", true)
        .expect("a question about every row needs no identifier");
    assert_eq!(source.key, None);
    let templates = candidate_templates(&folders.inventory(), &source.table.columns);
    let plan = build_plan(templates, &source.file, &source.sheet, &source.table, source.key).unwrap();
    // With no key, one letter per line (5): she picks the order number on the card to get one per order.
    assert_eq!(plan.preview.letters, 5);

    let request = request_from(&plan, true, Some("No_Commande"));
    let template = read_template(&folders.inventory(), &request.template).unwrap();
    assert_eq!(preview(&template, &source.table, &request).unwrap().letters, 3);
    let report = generate(
        folders.work.path(),
        "Generated",
        &folders.work.path().join("generated.jsonl.outside-the-folder"),
        1,
        &template,
        &source.table,
        &request,
    )
    .unwrap();
    let names: Vec<&str> = report.written.iter().map(|written| written.relative_path.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "Generated/modele_lettre-CMD-2026-001.docx",
            "Generated/modele_lettre-CMD-2026-002.docx",
            "Generated/modele_lettre-CMD-2026-003.docx"
        ]
    );
    let first = docx_text(&std::fs::read(folders.work.path().join(&report.written[0].relative_path)).unwrap()).unwrap();
    assert!(first.contains("Ordinateur Portable") && first.contains("Souris Sans Fil"), "{first}");
    let third = docx_text(&std::fs::read(folders.work.path().join(&report.written[2].relative_path)).unwrap()).unwrap();
    assert!(third.contains("Rousseau") && third.contains("Clavier"), "{third}");
}

#[test]
fn a_question_that_names_no_order_and_no_every_makes_no_plan() {
    let folders = Folders::build();
    assert!(folders.source("R\u{e9}dige un courrier de confirmation", false).is_none());
}

#[test]
fn an_unconfirmed_proposal_is_left_as_written_in_the_letter() {
    let folders = Folders::build();
    let source = folders.source(OWNERS_QUESTION, false).unwrap();
    let templates = candidate_templates(&folders.inventory(), &source.table.columns);
    // Choose the text template: its `[Prix_U]` and `[Total]` are abbreviations, only proposed.
    let text_first: Vec<_> = templates
        .into_iter()
        .filter(|template| template.relative_path.ends_with(".txt"))
        .collect();
    let plan = build_plan(text_first, &source.file, &source.sheet, &source.table, source.key).unwrap();
    let status = |name: &str| plan.placeholders.iter().find(|p| p.name == name).map(|p| p.status);
    assert_eq!(status("Prix_U"), Some("proposed"));
    assert_eq!(status("Total"), Some("proposed"));
    assert!(plan.preview.text.contains("[Prix_U]"), "{}", plan.preview.text);
    assert!(plan.preview.unresolved.contains(&"Prix_U".to_string()));
    assert!(plan.preview.text.contains("Chaise Ergonomique"), "{}", plan.preview.text);
}
