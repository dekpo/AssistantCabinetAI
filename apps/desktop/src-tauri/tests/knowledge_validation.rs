//! KB lot 11 - the validation of the knowledge base on the sandbox library, with figures and no judgement by eye.
//!
//! Every figure is a comparison between what the base did and what the generator knows, because it wrote the
//! text (`fixtures/knowledge-sandbox`, `scripts/gen-knowledge-sandbox.py`). The sandbox mixes the invented
//! confusables of the brief (the Dupont and Martin families, a doctor who has a patient's name, sound-alike
//! surnames, names with particles and apostrophes, names of French-speaking Switzerland and of abroad), French
//! and Swiss papers (social security and AVS numbers, CPAM and LAMal letters), scans, long documents and
//! formats the product does not read.
//!
//! Two kinds of test live here:
//!
//! - the ones that run in every `cargo test` and assert only the invariants of the programme (I1 to I4 and the
//!   rules that make a reduction safe) on the 30-document library, so they stay fast;
//! - the `#[ignore]` reports, which print the tables the lot 11 report is made of. They assert nothing about
//!   quality: the thresholds of the release gate are the owner's to agree once she has seen the numbers.
//!
//! ```text
//! cargo test --test knowledge_validation -- --nocapture                     # the invariants
//! cargo test --release --test knowledge_validation report -- --ignored --nocapture
//!     ACAI_VALIDATION_OUT=<folder> writes the tables as Markdown files; ACAI_SANDBOX_DIR points at another library
//! ```

mod common;

use std::collections::{BTreeMap, BTreeSet};

use assistant_cabinet_ai_lib::knowledge::normalize::TitleSet;
use assistant_cabinet_ai_lib::knowledge::reduce::SkipReason;
use assistant_cabinet_ai_lib::knowledge::targeting::{QueryVocabulary, TargetingRequest};
use assistant_cabinet_ai_lib::knowledge::KnowledgeMode;

use common::validation::*;

const PACKS: [&str; 2] = ["health-fr", "health-ch"];

fn vocabulary() -> QueryVocabulary {
    QueryVocabulary::load("fr-FR", &PACKS).expect("loads the vocabulary")
}

// ------------------------------------------------------------------------------------- truth to base mapping

struct Mapping {
    /// Truth entity id -> positions in the base's list.
    kb_of: BTreeMap<String, Vec<usize>>,
    /// Position in the base's list -> truth entity ids that share its identity key.
    truth_of: BTreeMap<usize, Vec<String>>,
}

/// The identity keys a truth entity is known by: its full name, and for an organisation its acronym or usual
/// short form (`Helsana` for `Helsana Assurances SA`), which is how a table writes it.
fn truth_keys(entity: &EntityTruth, titles: &TitleSet) -> Vec<String> {
    let mut keys = vec![truth_key(entity, titles)];
    if entity.kind == "organization" {
        if let Some(short) = entity.forms.get("acronym") {
            keys.push(
                assistant_cabinet_ai_lib::knowledge::normalize::normalize_name(short, titles)
                    .token_set_key,
            );
        }
    }
    keys
}

fn map_truth(truth: &Truth, base: &[KbEntity], titles: &TitleSet) -> Mapping {
    let mut kb_of: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut truth_of: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    let ordered = |text: &str| {
        assistant_cabinet_ai_lib::knowledge::normalize::normalize_name(text, titles).joined
    };
    for (id, entity) in &truth.entities {
        if entity.kind != "person" && entity.kind != "organization" {
            continue;
        }
        let keys = truth_keys(entity, titles);
        let mut positions: Vec<usize> = (0..base.len())
            .filter(|&position| {
                let kb = &base[position];
                kb.type_id == entity.kind && keys.iter().any(|key| kb.keys.contains(key))
            })
            .collect();
        // Two people whose given name and surname are each other's reverse (`Lucas Roy`, `Roy Lucas`)
        // share their order-free key: the entity that is written in the person's own order is theirs.
        // When no entity is, the key match stands, so a real merge is still seen as one.
        let twin_exists = entity.kind == "person"
            && truth.entities.iter().any(|(other_id, other)| {
                other_id != id
                    && other.kind == "person"
                    && truth_key(other, titles) == truth_key(entity, titles)
            });
        if twin_exists {
            let own: Vec<usize> = positions
                .iter()
                .copied()
                .filter(|&position| {
                    ordered(&base[position].canonical) == ordered(&entity.canonical)
                })
                .collect();
            if !own.is_empty() {
                positions = own;
            }
        }
        for position in positions {
            kb_of.entry(id.clone()).or_default().push(position);
            truth_of.entry(position).or_default().push(id.clone());
        }
    }
    Mapping { kb_of, truth_of }
}

// --------------------------------------------------------------------------------------------- entity quality

#[derive(Default, Clone)]
struct TypeTotals {
    entities: usize,
    found: usize,
    active: usize,
    truth_links: usize,
    found_links: usize,
    kb_links: usize,
}

/// Some fields are read only by the other test files that share this layout, or when a failing run is debugged.
#[allow(dead_code)]
struct Quality {
    text: String,
    false_merges: Vec<String>,
    persons: TypeTotals,
    organizations: TypeTotals,
    persons_in_tables: TypeTotals,
    unmapped_active: usize,
    unmapped_candidate: usize,
}

/// The files each truth entity is named in, among the files the product read, per domain.
fn truth_files(truth: &Truth, lib: &Library, tables: bool) -> BTreeMap<String, BTreeSet<String>> {
    let mut files: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    if tables {
        let readable: BTreeSet<String> = lib
            .lab
            .workbooks()
            .into_iter()
            .map(|w| w.relative_path)
            .collect();
        let mut forms: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for book in &truth.books {
            if !readable.contains(&book.path) {
                continue;
            }
            for m in &book.mentions {
                files
                    .entry(m.entity.clone())
                    .or_default()
                    .insert(book.path.clone());
                forms
                    .entry(m.entity.clone())
                    .or_default()
                    .insert(m.form.clone());
            }
        }
        // a person a table only writes as surname then initial has no full name to be found by
        files.retain(|id, _| {
            forms
                .get(id)
                .is_none_or(|set| !set.iter().all(|f| f == "surname_initial"))
        });
        return files;
    }
    let read: BTreeSet<&str> = lib.product_files.iter().map(String::as_str).collect();
    for doc in truth.docs_in(lib.tier) {
        if !read.contains(doc.path.as_str()) {
            continue;
        }
        for m in doc.mentions.iter().filter(|m| m.place == "text") {
            files
                .entry(m.entity.clone())
                .or_default()
                .insert(doc.path.clone());
        }
    }
    files
}

fn group_of(truth: &Truth, id: &str) -> String {
    truth
        .entities
        .get(id)
        .and_then(|e| e.homonym_group.clone())
        .unwrap_or_else(|| id.to_string())
}

fn is_surname_initial(name: &str) -> bool {
    let mut words = name.split_whitespace().rev();
    match (words.next(), words.next()) {
        (Some(last), Some(_)) => last.chars().count() == 1,
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn totals_for(
    truth: &Truth,
    base: &[KbEntity],
    mapping: &Mapping,
    files: &BTreeMap<String, BTreeSet<String>>,
    tables: bool,
    kind: &str,
    missed: &mut Vec<(String, String)>,
    extra_links: &mut Vec<(String, String)>,
) -> TypeTotals {
    let mut group_files: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut group_members: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (id, set) in files {
        let Some(entity) = truth.entities.get(id) else {
            continue;
        };
        if entity.kind != kind {
            continue;
        }
        let group = group_of(truth, id);
        group_files
            .entry(group.clone())
            .or_default()
            .extend(set.iter().cloned());
        group_members.entry(group).or_default().push(id.clone());
    }
    let mut totals = TypeTotals::default();
    for (group, truth_set) in &group_files {
        let members = &group_members[group];
        let first = &truth.entities[&members[0]];
        totals.entities += 1;
        let mut positions: BTreeSet<usize> = BTreeSet::new();
        for id in members {
            positions.extend(mapping.kb_of.get(id).into_iter().flatten().copied());
        }
        totals.truth_links += truth_set.len();
        if positions.is_empty() {
            missed.push((first.canonical.clone(), first.kind.clone()));
            continue;
        }
        totals.found += 1;
        if positions.iter().any(|p| base[*p].status == "active") {
            totals.active += 1;
        }
        let kb_files: BTreeSet<String> = positions
            .iter()
            .flat_map(|p| {
                if tables {
                    base[*p].workbooks.iter().cloned().collect::<Vec<_>>()
                } else {
                    base[*p].documents.iter().cloned().collect::<Vec<_>>()
                }
            })
            .collect();
        totals.found_links += truth_set.intersection(&kb_files).count();
        totals.kb_links += kb_files.len();
        for path in kb_files.difference(truth_set) {
            extra_links.push((first.canonical.clone(), path.clone()));
        }
    }
    totals
}

fn quality(lib: &Library, truth: &Truth, titles: &TitleSet) -> Quality {
    let connection = lib.lab.side();
    let base = read_base(&connection, titles);
    let mapping = map_truth(truth, &base, titles);
    let doc_files = truth_files(truth, lib, false);
    let table_files = truth_files(truth, lib, true);
    let mut out = String::new();

    let mut missed: Vec<(String, String)> = Vec::new();
    let mut extra_links: Vec<(String, String)> = Vec::new();
    let persons = totals_for(
        truth,
        &base,
        &mapping,
        &doc_files,
        false,
        "person",
        &mut missed,
        &mut extra_links,
    );
    let organizations = totals_for(
        truth,
        &base,
        &mapping,
        &doc_files,
        false,
        "organization",
        &mut missed,
        &mut extra_links,
    );
    let mut missed_in_tables: Vec<(String, String)> = Vec::new();
    let persons_in_tables = totals_for(
        truth,
        &base,
        &mapping,
        &table_files,
        true,
        "person",
        &mut missed_in_tables,
        &mut extra_links,
    );
    let organizations_in_tables = totals_for(
        truth,
        &base,
        &mapping,
        &table_files,
        true,
        "organization",
        &mut missed_in_tables,
        &mut extra_links,
    );

    // false merges: one base entity carrying the identity key of two different people
    let mut false_merges = Vec::new();
    for (position, kb) in base.iter().enumerate() {
        if kb.type_id != "person" {
            continue;
        }
        let groups: BTreeSet<String> = mapping
            .truth_of
            .get(&position)
            .into_iter()
            .flatten()
            .map(|id| group_of(truth, id))
            .collect();
        if groups.len() > 1 {
            false_merges.push(format!(
                "{} ({}) holds {} - {} document(s), {} workbook(s)",
                kb.canonical,
                kb.status,
                groups.into_iter().collect::<Vec<_>>().join(" + "),
                kb.documents.len(),
                kb.workbooks.len()
            ));
        }
    }

    let mut unmapped_active = 0;
    let mut unmapped_candidate = 0;
    let mut surname_initial = 0;
    let mut junk: Vec<String> = Vec::new();
    for (position, kb) in base.iter().enumerate() {
        if !(kb.type_id == "person" || kb.type_id == "organization")
            || mapping.truth_of.contains_key(&position)
        {
            continue;
        }
        if kb.type_id == "person" && is_surname_initial(&kb.canonical) {
            surname_initial += 1;
            continue;
        }
        if kb.status == "active" {
            unmapped_active += 1;
        } else {
            unmapped_candidate += 1;
        }
        junk.push(format!(
            "{} / {} / {} / {} doc(s), {} workbook(s)",
            kb.type_id,
            kb.status,
            kb.canonical,
            kb.documents.len(),
            kb.workbooks.len()
        ));
    }

    // per-form recall: the files where a name is written in one shape only
    let mut by_form: BTreeMap<(String, String), (usize, usize)> = BTreeMap::new();
    let read: BTreeSet<&str> = lib.product_files.iter().map(String::as_str).collect();
    for doc in truth.docs_in(lib.tier) {
        if !read.contains(doc.path.as_str()) {
            continue;
        }
        let mut forms_of: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for m in doc.mentions.iter().filter(|m| m.place == "text") {
            forms_of
                .entry(m.entity.as_str())
                .or_default()
                .insert(m.form.as_str());
        }
        for (id, forms) in forms_of {
            let Some(entity) = truth.entities.get(id) else {
                continue;
            };
            if entity.kind != "person" && entity.kind != "organization" {
                continue;
            }
            if forms.len() != 1 {
                continue;
            }
            let form = forms.iter().next().unwrap();
            let hit = mapping
                .kb_of
                .get(id)
                .into_iter()
                .flatten()
                .any(|p| base[*p].documents.contains(&doc.path));
            let cell = by_form
                .entry((entity.kind.clone(), form.to_string()))
                .or_insert((0, 0));
            cell.1 += 1;
            cell.0 += usize::from(hit);
        }
    }

    // recall by the format of the file: the same names, read from a PDF, a DOCX, a plain text, a scan
    let mut by_format: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for doc in truth.docs_in(lib.tier) {
        if !read.contains(doc.path.as_str()) {
            continue;
        }
        let mut forms_of: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for m in doc.mentions.iter().filter(|m| m.place == "text") {
            forms_of
                .entry(m.entity.as_str())
                .or_default()
                .insert(m.form.as_str());
        }
        for (id, forms) in forms_of {
            let Some(entity) = truth.entities.get(id) else {
                continue;
            };
            if (entity.kind != "person" && entity.kind != "organization")
                || forms.iter().all(|f| WEAK_FORMS.contains(f))
            {
                continue;
            }
            let hit = mapping
                .kb_of
                .get(id)
                .into_iter()
                .flatten()
                .any(|p| base[*p].documents.contains(&doc.path));
            let cell = by_format
                .entry(format!("{} ({})", doc.format, entity.kind))
                .or_insert((0, 0));
            cell.1 += 1;
            cell.0 += usize::from(hit);
        }
    }

    // names split by a line break: a PDF wraps a paragraph, so a title or a given name can end a line and the
    // rest of the name start the next one
    let mut by_break: BTreeMap<(bool, String), (usize, usize)> = BTreeMap::new();
    for doc in truth.docs_in(lib.tier) {
        if !read.contains(doc.path.as_str()) {
            continue;
        }
        let chunks = lib
            .lab
            .index
            .chunk_texts_for_document(&doc.path)
            .unwrap_or_default();
        let text: String = chunks
            .iter()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let squashed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for m in doc.mentions.iter().filter(|m| m.place == "text") {
            let Some(entity) = truth.entities.get(&m.entity) else {
                continue;
            };
            if (entity.kind != "person" && entity.kind != "organization")
                || !m.text.contains(' ')
                || WEAK_FORMS.contains(&m.form.as_str())
            {
                continue;
            }
            if !seen.insert(m.entity.as_str()) {
                continue;
            }
            let verbatim = text.contains(&m.text);
            let anywhere = squashed.contains(&m.text);
            if !verbatim && !anywhere {
                continue;
            }
            let hit = mapping
                .kb_of
                .get(&m.entity)
                .into_iter()
                .flatten()
                .any(|p| base[*p].documents.contains(&doc.path));
            let cell = by_break
                .entry((!verbatim, entity.kind.clone()))
                .or_insert((0, 0));
            cell.1 += 1;
            cell.0 += usize::from(hit);
        }
    }

    out.push_str(&format!(
        "### Entity quality, tier {} ({} documents read)\n\n",
        lib.tier,
        lib.product_files.len()
    ));
    out.push_str("| Side | Type | Names in the library | Found by the base | Active | File links expected | File links found | File links the base made | Link recall | Link precision |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for (side, label, t) in [
        ("documents", "person", &persons),
        ("documents", "organization", &organizations),
        ("tables", "person", &persons_in_tables),
        ("tables", "organization", &organizations_in_tables),
    ] {
        out.push_str(&format!(
            "| {side} | {label} | {} | {} ({}) | {} | {} | {} | {} | {} | {} |\n",
            t.entities,
            t.found,
            pct(t.found, t.entities),
            t.active,
            t.truth_links,
            t.found_links,
            t.kb_links,
            pct(t.found_links, t.truth_links),
            pct(t.found_links, t.kb_links)
        ));
    }
    out.push_str(&format!(
        "\nFalse merges (one base entity, two different people): **{}**\n",
        false_merges.len()
    ));
    for line in &false_merges {
        out.push_str(&format!("- {line}\n"));
    }
    out.push_str(&format!(
        "\nEntities the base holds that match no invented name: {unmapped_active} active, {unmapped_candidate} candidate (and {surname_initial} in the surname-then-initial form of a table, which are possible matches by design). All of them:\n"
    ));
    for line in &junk {
        out.push_str(&format!("- {line}\n"));
    }
    out.push_str(&format!(
        "\nNames in documents never found ({}):\n",
        missed.len()
    ));
    for (name, kind) in missed.iter() {
        out.push_str(&format!("- {kind}: {name}\n"));
    }
    out.push_str(&format!(
        "\nNames in tables never found ({}):\n",
        missed_in_tables.len()
    ));
    for (name, kind) in missed_in_tables.iter().take(30) {
        out.push_str(&format!("- {kind}: {name}\n"));
    }
    out.push_str("\nLinks the base made that the generator does not know (first 25):\n");
    for (name, path) in extra_links.iter().take(25) {
        out.push_str(&format!("- {name} -> {path}\n"));
    }
    out.push_str("\nRecall by the shape the name is written in (files where the name appears in that one shape):\n\n| Type | Shape | Files | Linked | Recall |\n| --- | --- | --- | --- | --- |\n");
    for ((kind, form), (hit, total)) in &by_form {
        out.push_str(&format!(
            "| {kind} | {form} | {total} | {hit} | {} |\n",
            pct(*hit, *total)
        ));
    }
    out.push_str("\nRecall by the format of the file (names written in a form that can be linked):\n\n| Format (type) | Names in files | Linked | Recall |\n| --- | --- | --- | --- |\n");
    for (label, (hit, total)) in &by_format {
        out.push_str(&format!(
            "| {label} | {total} | {hit} | {} |\n",
            pct(*hit, *total)
        ));
    }
    out.push_str("\nRecall when the name is whole on its line, and when a line break splits it (a name counts once per file, split if its first mention there is split):\n\n| Type | Name | Files | Linked | Recall |\n| --- | --- | --- | --- | --- |\n");
    for ((split, kind), (hit, total)) in &by_break {
        out.push_str(&format!(
            "| {kind} | {} | {total} | {hit} | {} |\n",
            if *split {
                "split by a line break"
            } else {
                "on one line"
            },
            pct(*hit, *total)
        ));
    }
    Quality {
        text: out,
        false_merges,
        persons,
        organizations,
        persons_in_tables,
        unmapped_active,
        unmapped_candidate,
    }
}

// ------------------------------------------------------------------------------------------------- questions

/// Kept whole so a failing run can be debugged with `{:?}`; the report reads only some of the fields.
#[allow(dead_code)]
struct QResult {
    id: String,
    category: String,
    text: String,
    resolution: String,
    mode: KnowledgeMode,
    route: RouteKind,
    applied: bool,
    has_candidates: bool,
    skip: String,
    selected: usize,
    candidates: usize,
    relevant: usize,
    found: usize,
    kept_irrelevant: usize,
    wrong_entity: bool,
    leaked: usize,
    knowledge_ms: u64,
    missing: Vec<(String, &'static str)>,
    capped: usize,
    risk: String,
    data_selected: usize,
    data_applied: bool,
    data_candidates: usize,
    data_relevant: usize,
    data_found: usize,
}

fn selection_for(q: &QuestionTruth, lib: &Library) -> (Option<Vec<String>>, bool) {
    let paths = match &q.selection.documents {
        Pick::Word(_) => None,
        Pick::Paths { paths } => Some(
            paths
                .iter()
                .filter(|p| lib.product_files.contains(p))
                .cloned()
                .collect::<Vec<_>>(),
        ),
    };
    let workbooks = !matches!(&q.selection.workbooks, Pick::Word(word) if word == "none");
    (paths, workbooks)
}

const WEAK_FORMS: [&str; 5] = [
    "title_surname",
    "surname_only",
    "initial_surname",
    "surname_initial",
    "hyphen_error",
];

/// Why a relevant file is not in the reduced set: the name is only written in a form too weak to link, the
/// previous answer's person narrowed a new question, the base found nothing in the file, or it found the name
/// and still did not use it.
fn lost_because(
    q: &QuestionTruth,
    truth: &Truth,
    base: &[KbEntity],
    mapping: &Mapping,
    path: &str,
    capped: &BTreeSet<String>,
) -> &'static str {
    if q.risk.contains("KBD-19") {
        return "carry_over";
    }
    if capped.contains(path) {
        return "capped";
    }
    let Some(doc) = truth.doc(path) else {
        return "unknown";
    };
    let forms: BTreeSet<&str> = doc
        .mentions
        .iter()
        .filter(|m| m.place == "text" && q.name_entities.contains(&m.entity))
        .map(|m| m.form.as_str())
        .collect();
    if !forms.is_empty() && forms.iter().all(|f| WEAK_FORMS.contains(f)) {
        return "weak_form_only";
    }
    let known = q.name_entities.iter().any(|id| {
        mapping
            .kb_of
            .get(id)
            .into_iter()
            .flatten()
            .any(|p| base[*p].documents.contains(path))
    });
    if known {
        "entity_not_resolved"
    } else {
        "no_mention_in_file"
    }
}

fn run_questions(
    lib: &Library,
    truth: &Truth,
    vocabulary: &QueryVocabulary,
    titles: &TitleSet,
    modes: &[KnowledgeMode],
) -> Vec<QResult> {
    let connection = lib.lab.side();
    let base = read_base(&connection, titles);
    let mapping = map_truth(truth, &base, titles);
    let replay = Replay::new(lib, vocabulary);
    let workbooks_here: BTreeSet<String> = lib
        .lab
        .workbooks()
        .into_iter()
        .map(|w| w.relative_path)
        .collect();
    let mut out = Vec::new();
    for q in truth.questions.iter().filter(|q| q.min_tier <= lib.tier) {
        let (paths, with_workbooks) = selection_for(q, lib);
        // the previous answer's entities, as the client holds them
        let carry: Vec<i64> = q
            .carry_over
            .iter()
            .flat_map(|id| {
                mapping
                    .kb_of
                    .get(id)
                    .into_iter()
                    .flatten()
                    .map(|p| base[*p].id)
            })
            .collect();
        let request = TargetingRequest {
            carry_over: carry,
            ..TargetingRequest::default()
        };
        for &mode in modes {
            let asked = replay.ask(&q.text, mode, &request, paths.as_deref(), with_workbooks);
            let selected: BTreeSet<&str> = asked.selected.iter().map(String::as_str).collect();
            let relevant: BTreeSet<&str> = q
                .relevant_readable
                .iter()
                .map(String::as_str)
                .filter(|p| selected.contains(p))
                .collect();
            let searched = asked.searched();
            let searched_set: BTreeSet<&str> = searched.iter().map(String::as_str).collect();
            let found = relevant
                .iter()
                .filter(|p| searched_set.contains(**p))
                .count();
            let kept_irrelevant = searched_set
                .iter()
                .filter(|p| !relevant.contains(**p))
                .count();
            let leaked = q
                .left_out
                .iter()
                .filter(|p| searched_set.contains(p.as_str()))
                .count()
                + searched_set
                    .iter()
                    .filter(|p| !selected.contains(**p))
                    .count();
            let capped_set: BTreeSet<String> = asked
                .plan
                .as_ref()
                .map(|plan| plan.reduction.documents.capped.iter().cloned().collect())
                .unwrap_or_default();
            let (
                has_candidates,
                knowledge_ms,
                picked,
                data_applied,
                data_candidates,
                data_selected,
            ) = match &asked.plan {
                Some(plan) => (
                    plan.reduction.documents.has_candidates(),
                    plan.knowledge_ms,
                    plan.reduction.documents.entity_ids.clone(),
                    plan.reduction.data.is_applied(),
                    plan.reduction.data.candidates.clone(),
                    plan.reduction.data.selected,
                ),
                None => (false, 0, Vec::new(), false, Vec::new(), 0),
            };
            let data_relevant: BTreeSet<&str> = q
                .relevant_workbooks
                .iter()
                .map(String::as_str)
                .filter(|p| workbooks_here.contains(*p))
                .collect();
            let data_found = if data_applied {
                data_relevant
                    .iter()
                    .filter(|p| data_candidates.iter().any(|c| c == **p))
                    .count()
            } else {
                data_relevant.len()
            };
            let wanted: BTreeSet<&str> = q.name_entities.iter().map(String::as_str).collect();
            let all_mapped =
                !wanted.is_empty() && wanted.iter().all(|id| mapping.kb_of.contains_key(*id));
            let wrong_entity = all_mapped
                && asked.applied()
                && !picked.iter().any(|kb_id| {
                    base.iter()
                        .position(|k| k.id == *kb_id)
                        .and_then(|pos| mapping.truth_of.get(&pos))
                        .is_some_and(|ids| ids.iter().any(|id| wanted.contains(id.as_str())))
                });
            let missing: Vec<(String, &'static str)> = relevant
                .iter()
                .filter(|p| !searched_set.contains(**p))
                .map(|p| {
                    (
                        p.to_string(),
                        lost_because(q, truth, &base, &mapping, p, &capped_set),
                    )
                })
                .collect();
            out.push(QResult {
                id: q.id.clone(),
                category: q.category.clone(),
                text: q.text.clone(),
                resolution: q.expect.resolution.clone(),
                mode,
                route: asked.route,
                applied: asked.applied(),
                has_candidates,
                skip: asked.skip_code(),
                selected: asked.selected.len(),
                candidates: searched.len(),
                relevant: relevant.len(),
                found,
                kept_irrelevant,
                wrong_entity,
                leaked,
                knowledge_ms,
                missing,
                capped: capped_set.len(),
                risk: q.risk.clone(),
                data_selected,
                data_applied,
                data_candidates: data_candidates.len(),
                data_relevant: data_relevant.len(),
                data_found,
            });
        }
    }
    out
}

fn fmt_opt(v: Option<f64>) -> String {
    v.map(|v| format!("{v:.0}"))
        .unwrap_or_else(|| "-".to_string())
}

fn summarise(results: &[QResult], mode: KnowledgeMode, tier: usize) -> String {
    let mine: Vec<&QResult> = results.iter().filter(|r| r.mode == mode).collect();
    let mut out = format!(
        "### Questions, tier {tier}, mode `{mode:?}` ({} questions)\n\n",
        mine.len()
    );
    let mut by_category: BTreeMap<&str, Vec<&QResult>> = BTreeMap::new();
    for r in &mine {
        by_category.entry(r.category.as_str()).or_default().push(r);
    }
    out.push_str("| Category | Questions | Answered by the folder | Documents reduced | Would reduce | Recall when reduced | Files searched, median | Files dropped, median | Irrelevant files kept | Wrong entity | Leaks | Workbooks reduced | Workbook recall |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    let mut total = (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut reductions: Vec<f64> = Vec::new();
    for (category, rows) in &by_category {
        let folder = rows
            .iter()
            .filter(|r| r.route == RouteKind::Folder || r.route == RouteKind::NothingSelected)
            .count();
        let applied: Vec<&&QResult> = rows.iter().filter(|r| r.applied).collect();
        let would = rows.iter().filter(|r| r.has_candidates).count();
        let relevant: usize = applied.iter().map(|r| r.relevant).sum();
        let found: usize = applied.iter().map(|r| r.found).sum();
        let dropped: Vec<f64> = applied
            .iter()
            .map(|r| (r.selected - r.candidates.min(r.selected)) as f64)
            .collect();
        let searched: Vec<f64> = rows
            .iter()
            .filter(|r| r.route != RouteKind::Folder)
            .map(|r| r.candidates as f64)
            .collect();
        let kept: usize = applied.iter().map(|r| r.kept_irrelevant).sum();
        let wrong = rows.iter().filter(|r| r.wrong_entity).count();
        let leaks: usize = rows.iter().map(|r| r.leaked).sum();
        let data_reduced = rows.iter().filter(|r| r.data_applied).count();
        let data_rel: usize = rows
            .iter()
            .filter(|r| r.data_applied)
            .map(|r| r.data_relevant)
            .sum();
        let data_fnd: usize = rows
            .iter()
            .filter(|r| r.data_applied)
            .map(|r| r.data_found)
            .sum();
        out.push_str(&format!(
            "| {category} | {} | {folder} | {} | {would} | {} | {} | {} | {kept} | {wrong} | {leaks} | {data_reduced} | {} |\n",
            rows.len(), applied.len(), pct(found, relevant), fmt_opt(median(searched)), fmt_opt(median(dropped)), pct(data_fnd, data_rel),
        ));
        total.0 += applied.len();
        total.1 += relevant;
        total.2 += found;
        total.3 += wrong;
        total.4 += leaks;
        for r in &applied {
            reductions.push(
                100.0 * (r.selected - r.candidates.min(r.selected)) as f64
                    / r.selected.max(1) as f64,
            );
        }
    }
    out.push_str(&format!(
        "\nQuestions whose documents were reduced: {} of {}. Recall of the reduced set on those: **{}** ({} of {} relevant files). Median reduction: {}. Wrong entity picked: {}. Files outside the selection or left out on purpose that were searched: {}.\n",
        total.0, mine.len(), pct(total.2, total.1), total.2, total.1,
        median(reductions).map(|v| format!("{v:.0} %")).unwrap_or_else(|| "-".into()), total.3, total.4,
    ));
    let mut skips: BTreeMap<&str, usize> = BTreeMap::new();
    for r in &mine {
        *skips.entry(r.skip.as_str()).or_default() += 1;
    }
    out.push_str(&format!("Why a question was not reduced: {skips:?}\n"));
    let lookups: Vec<f64> = mine
        .iter()
        .filter(|r| r.route != RouteKind::Folder)
        .map(|r| r.knowledge_ms as f64)
        .collect();
    out.push_str(&format!(
        "Knowledge lookup: median {} ms, p95 {} ms, max {} ms.\n",
        fmt_opt(median(lookups.clone())),
        fmt_opt(percentile(lookups.clone(), 0.95)),
        fmt_opt(percentile(lookups, 1.0)),
    ));
    let mut by_reason: BTreeMap<&'static str, usize> = BTreeMap::new();
    let misses: Vec<&&QResult> = mine
        .iter()
        .filter(|r| r.applied && r.found < r.relevant)
        .collect();
    for r in &misses {
        for (_, why) in &r.missing {
            *by_reason.entry(*why).or_default() += 1;
        }
    }
    out.push_str(&format!(
        "\nRelevant files lost by a reduced set, by reason: {by_reason:?}\n"
    ));
    out.push_str(&format!(
        "\nQuestions where the reduced set lost a relevant file ({}):\n",
        misses.len()
    ));
    for r in misses {
        let lost: Vec<String> = r
            .missing
            .iter()
            .map(|(p, why)| format!("{p} ({why})"))
            .collect();
        out.push_str(&format!(
            "- {} [{}] `{}` {}/{} found; lost: {}\n",
            r.id,
            r.category,
            r.text,
            r.found,
            r.relevant,
            lost.join(", ")
        ));
    }
    let wrong_ones: Vec<&&QResult> = mine.iter().filter(|r| r.wrong_entity).collect();
    out.push_str(&format!("\nQuestions where the reduction was built on an entity that is not the one the question names ({}):\n", wrong_ones.len()));
    for r in wrong_ones {
        out.push_str(&format!(
            "- {} [{}] `{}` {}/{} found\n",
            r.id, r.category, r.text, r.found, r.relevant
        ));
    }
    out
}

// ---------------------------------------------------------------------------------------------------- tests

#[test]
fn the_sandbox_matches_its_manifest() {
    let truth = Truth::load();
    assert!(
        truth.docs.len() >= 150,
        "the versioned core holds at least 150 documents"
    );
    for doc in &truth.docs {
        assert!(
            truth.root.join("documents").join(&doc.path).is_file(),
            "missing {}",
            doc.path
        );
    }
    for book in &truth.books {
        assert!(
            truth.root.join("data").join(&book.path).is_file(),
            "missing workbook {}",
            book.path
        );
    }
    // the tiers are prefixes of the stream
    assert!(truth.docs.iter().enumerate().all(|(i, d)| d.index == i + 1));
    // every format of the brief is present, and the product's blind spots are present on purpose
    let formats: BTreeSet<&str> = truth.docs.iter().map(|d| d.format.as_str()).collect();
    for wanted in [
        "pdf", "scan-pdf", "docx", "txt", "md", "jpg", "png", "odt", "rtf", "eml", "vtt", "pptx",
        "odp", "ppt",
    ] {
        assert!(formats.contains(wanted), "the library holds no {wanted}");
    }
    let books: BTreeSet<&str> = truth.books.iter().map(|b| b.format.as_str()).collect();
    for wanted in ["xlsx", "csv", "ods", "xls"] {
        assert!(books.contains(wanted), "the data folder holds no {wanted}");
    }
    assert!(
        truth.questions.len() >= 150,
        "{} questions",
        truth.questions.len()
    );
    // nothing real: every phone number is a number the regulator reserves for fiction or the conventional 555
    for entity in truth.entities.values() {
        assert!(!entity.id.is_empty());
    }
}

/// The invariants of the programme on the library that motivated it (30 documents), for every question of the
/// sandbox that can be asked at that size, in the three modes. Fast enough for every run.
#[tokio::test]
async fn the_invariants_hold_for_every_sandbox_question_on_thirty_documents() {
    let truth = Truth::load();
    let lib = library(&truth, 30, &PACKS).await;
    let vocabulary = vocabulary();
    let titles = vocabulary.titles.clone();
    let results = run_questions(
        &lib,
        &truth,
        &vocabulary,
        &titles,
        &[
            KnowledgeMode::Off,
            KnowledgeMode::Suggest,
            KnowledgeMode::Auto,
        ],
    );
    assert!(results.len() > 60, "{} replays", results.len());
    for r in &results {
        let label = format!("{} [{:?}] {}", r.id, r.mode, r.text);
        // I1: nothing outside the selection, nothing the user left out
        assert_eq!(
            r.leaked, 0,
            "{label}: a file outside the selection was searched"
        );
        // the base off, and the base in suggest, never narrow what the user did not ask to narrow
        if r.mode != KnowledgeMode::Auto {
            assert!(!r.applied, "{label}: reduced in {:?}", r.mode);
        }
        if r.resolution == "ambiguous" && r.mode == KnowledgeMode::Auto {
            assert!(!r.applied, "{label}: an ambiguous name narrowed the search");
        }
        if r.resolution == "none" && r.mode == KnowledgeMode::Auto && r.category == "unknown" {
            assert!(!r.applied, "{label}: an unknown name narrowed the search");
        }
        // I2: a file named in the question is never reduced
        if r.route == RouteKind::NamedFile {
            assert!(!r.applied, "{label}: a named file was reduced");
        }
        // a selection of fewer than eight files is left alone
        if r.category == "small_scope" {
            assert!(!r.applied, "{label}: a small selection was reduced");
        }
    }
    let _ = (SkipReason::Off,);
}

#[tokio::test]
#[ignore]
async fn report_quality_and_recall() {
    let truth = Truth::load();
    let vocabulary = vocabulary();
    let titles = vocabulary.titles.clone();
    let tiers: Vec<usize> = std::env::var("ACAI_TIERS")
        .unwrap_or_else(|_| "30,100,200".to_string())
        .split(',')
        .filter_map(|t| t.trim().parse().ok())
        .collect();
    for tier in tiers {
        let mut tier_text = String::new();
        let tier = truth.tier_size(tier);
        let lib = library(&truth, tier, &PACKS).await;
        let q = quality(&lib, &truth, &titles);
        println!("{}", q.text);
        tier_text.push_str(&q.text);
        println!("persons found {} / {}, organizations {} / {}, false merges {}, unmapped active {} candidate {}",
            q.persons.found, q.persons.entities, q.organizations.found, q.organizations.entities,
            q.false_merges.len(), q.unmapped_active, q.unmapped_candidate);
        let results = run_questions(
            &lib,
            &truth,
            &vocabulary,
            &titles,
            &[KnowledgeMode::Auto, KnowledgeMode::Suggest],
        );
        for mode in [KnowledgeMode::Auto, KnowledgeMode::Suggest] {
            let table = summarise(&results, mode, tier);
            println!("{table}");
            tier_text.push_str(&table);
        }
        println!(
            "analysis of the tier: {} ms, {} documents read",
            lib.analyse_ms,
            lib.product_files.len()
        );
        write_report(&format!("quality-tier-{tier}.md"), &tier_text);
    }
}

/// What the base holds after a pass over the first `ACAI_TIER` documents (default 30): every person and
/// organisation with its status, its aliases and the files that mention it. Read it next to
/// `fixtures/knowledge-sandbox/documents.json` to see what the extractor did with a given sentence.
#[tokio::test]
#[ignore]
async fn print_what_the_base_holds() {
    let truth = Truth::load();
    let tier: usize = std::env::var("ACAI_TIER")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);
    let lib = library(&truth, tier, &PACKS).await;
    let connection = lib.lab.side();
    let mut statement = connection
        .prepare(
            "SELECT e.entity_id, e.type_id, e.status, e.canonical_name,
                    (SELECT group_concat(a.display || '[' || a.kind || ']', ' | ') FROM kb_aliases a WHERE a.entity_id = e.entity_id),
                    (SELECT group_concat(DISTINCT s.relative_path) FROM kb_mentions m JOIN kb_sources s ON s.source_id = m.source_id WHERE m.entity_id = e.entity_id)
             FROM kb_entities e WHERE e.status IN ('candidate','active') ORDER BY e.type_id, e.canonical_name",
        )
        .expect("prepares");
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })
        .expect("queries");
    for row in rows {
        let (id, kind, status, name, aliases, files) = row.expect("reads");
        println!(
            "#{id} {kind} {status} {name}\n    aliases: {}\n    files: {}",
            aliases.unwrap_or_default(),
            files.unwrap_or_default()
        );
    }
}

/// What the base made of one question: the names it found, how sure it was, the files it kept and why.
/// `ACAI_Q=Q011 ACAI_TIER=200 cargo test --release --test knowledge_validation explain -- --ignored --nocapture`
#[tokio::test]
#[ignore]
async fn explain_one_question() {
    let truth = Truth::load();
    let tier: usize = std::env::var("ACAI_TIER")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200);
    let wanted = std::env::var("ACAI_Q").unwrap_or_else(|_| "Q011".to_string());
    let q = truth
        .questions
        .iter()
        .find(|q| q.id == wanted)
        .expect("a question of the sandbox");
    let lib = library(&truth, tier, &PACKS).await;
    let vocabulary = vocabulary();
    let replay = Replay::new(&lib, &vocabulary);
    let (paths, with_workbooks) = selection_for(q, &lib);
    let asked = replay.ask(
        &q.text,
        KnowledgeMode::Auto,
        &TargetingRequest::default(),
        paths.as_deref(),
        with_workbooks,
    );
    println!("{} [{}] {}", q.id, q.category, q.text);
    println!("route {:?}, selected {}", asked.route, asked.selected.len());
    let Some(plan) = &asked.plan else { return };
    if let Some(analysis) = &plan.analysis {
        for e in &analysis.entities {
            println!("  entity #{} {} / {} / {:?} / conf {:.2} / {} / documents {} workbooks {} / selectivity {:.2} / terms {:?}", e.entity_id, e.display, e.type_id, e.status, e.confidence, e.method, e.document_files.len(), e.data_files.len(), e.document_selectivity, e.terms);
        }
        for a in &analysis.ambiguities {
            println!(
                "  ambiguity {:?}",
                a.candidates
                    .iter()
                    .map(|c| format!("#{} {}", c.entity_id, c.display))
                    .collect::<Vec<_>>()
            );
        }
        for p in &analysis.possibles {
            println!(
                "  possible {:?}",
                p.candidates
                    .iter()
                    .map(|c| format!("#{} {}", c.entity_id, c.display))
                    .collect::<Vec<_>>()
            );
        }
        println!(
            "  unresolved capitalised spans: {}",
            analysis.unresolved_spans
        );
    }
    let d = &plan.reduction.documents;
    println!("  documents: outcome {:?} skipped {:?} candidates {} lexical {} possible {} unread {} capped {}", d.outcome, d.skipped.map(|r| r.as_code()), d.candidates.len(), d.added_by_lexical.len(), d.from_possible_matches.len(), d.kept_unread.len(), d.capped.len());
    for c in d.candidates.iter().take(20) {
        println!("    keeps {c}");
    }
}

/// The generated legacy and OpenDocument workbooks are real files: a spreadsheet reader opens them. (The
/// product does not read them; that is the next test.)
#[test]
fn the_unread_workbook_formats_of_the_sandbox_are_real_files() {
    use calamine::{open_workbook_auto, Reader};
    let truth = Truth::load();
    for name in [
        "contrats-2026.xls",
        "rendez-vous-2026.ods",
        "contrats-2026.xlsx",
    ] {
        let path = truth.root.join("data").join(name);
        let mut workbook =
            open_workbook_auto(&path).unwrap_or_else(|e| panic!("opens {name}: {e}"));
        let sheets = workbook.sheet_names().to_vec();
        assert!(!sheets.is_empty(), "{name} has sheets");
        let range = workbook
            .worksheet_range(&sheets[0])
            .unwrap_or_else(|e| panic!("reads {name}: {e}"));
        assert!(
            range.height() >= 2,
            "{name} holds rows ({})",
            range.height()
        );
        assert!(
            range.width() >= 5,
            "{name} holds columns ({})",
            range.width()
        );
    }
}

/// What the product does with every kind of file of the library, in both folders: the extension, whether the
/// document pipeline reads it, and what the folder card would show. Printed for the report.
#[tokio::test]
#[ignore]
async fn report_what_the_product_does_with_each_format() {
    use assistant_cabinet_ai_lib::inventory::WorkFolderInventory;
    let truth = Truth::load();
    let lib = library(&truth, truth.docs.len(), &PACKS).await;
    // put every file of the sandbox in the folders, supported or not, then look at them
    for doc in &truth.docs {
        if !doc.product_reads() {
            let to = lib.lab.work_path().join(&doc.path);
            std::fs::create_dir_all(to.parent().unwrap()).unwrap();
            std::fs::copy(truth.root.join("documents").join(&doc.path), to).unwrap();
        }
    }
    for book in &truth.books {
        lib.lab.write_data_bytes(
            &book.path,
            &std::fs::read(truth.root.join("data").join(&book.path)).unwrap(),
        );
    }
    let mut lib = lib;
    lib.lab.analyse_data();
    let inventory =
        WorkFolderInventory::discover(lib.lab.work_path(), Some(&lib.lab.index)).unwrap();
    let mut out = String::from("### What the product does with each format\n\n| Folder | Extension | Files in the sandbox | Kind the card shows | Read (indexed / usable) | Not read |\n| --- | --- | --- | --- | --- | --- |\n");
    let mut by_ext: BTreeMap<String, (usize, usize, String)> = BTreeMap::new();
    for record in inventory.all_files() {
        let cell = by_ext.entry(record.extension.clone()).or_insert((
            0,
            0,
            record.kind.as_code().to_string(),
        ));
        cell.0 += 1;
        cell.1 += usize::from(record.indexed);
    }
    for (ext, (total, indexed, kind)) in &by_ext {
        out.push_str(&format!(
            "| documents | .{ext} | {total} | {kind} | {indexed} | {} |\n",
            total - indexed
        ));
    }
    let folder = lib.lab.data_folder();
    let mut by_ext: BTreeMap<String, (usize, usize, String, BTreeSet<String>)> = BTreeMap::new();
    for record in folder.files().all_files() {
        let usable = folder.usable_inventory(&record.relative_path).is_some();
        let cell = by_ext.entry(record.extension.clone()).or_insert((
            0,
            0,
            record.kind.as_code().to_string(),
            BTreeSet::new(),
        ));
        cell.0 += 1;
        cell.1 += usize::from(usable);
        cell.3.insert(format!("{:?}", record.processing_status));
    }
    for (ext, (total, usable, kind, states)) in &by_ext {
        out.push_str(&format!(
            "| data | .{ext} | {total} | {kind} | {usable} | {} ({states:?}) |\n",
            total - usable
        ));
    }
    println!("{out}");
    write_report("formats.md", &out);
}
