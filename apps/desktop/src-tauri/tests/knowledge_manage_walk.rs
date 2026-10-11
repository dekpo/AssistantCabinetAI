//! Walks the lot 8 human test with the real Rust code, on the same thirty-one invented documents, and
//! prints what each step shows, so the protocol's figures come from the code and not from a guess.
//!
//! Not a regression test (it asserts almost nothing): run it with
//!
//! ```text
//! python docs\test-reports\knowledge-base-pass-1\human-tests\lot08_helper.py write-fixtures <dir>
//! set ACAI_LOT08_DIR=<dir>
//! cargo test --test knowledge_manage_walk -- --ignored --nocapture
//! ```

mod common;

use assistant_cabinet_ai_lib::knowledge::manage::{
    self, EntityDetail, ListQuery, ListView, NameTools, SplitRequest,
};
use assistant_cabinet_ai_lib::knowledge::packs::PackSet;
use common::knowledge::Lab;

fn tools(lab: &Lab) -> NameTools {
    let packs = PackSet::load_with_type_vocabulary(lab.locale, &lab.packs).expect("loads");
    NameTools::from_packs(&packs, lab.locale)
}

fn list(lab: &Lab, text: &str, view: ListView, problems: bool) -> Vec<String> {
    manage::list_entities(
        &lab.side(),
        &ListQuery {
            text: text.to_string(),
            view,
            problems_only: problems,
            ..ListQuery::default()
        },
    )
    .unwrap()
    .rows
    .iter()
    .map(|row| {
        format!(
            "{} [{}/{}/{}] {} mentions, {} files, {} matches",
            row.name,
            row.type_id,
            row.status,
            row.origin,
            row.mentions,
            row.sources,
            row.open_matches
        )
    })
    .collect()
}

fn id(lab: &Lab, name: &str) -> i64 {
    common::knowledge::entity_named(&lab.side(), name)
        .unwrap_or_else(|| panic!("no entity {name}"))
        .entity_id
}

fn strip(lab: &Lab) -> String {
    let o = manage::overview(&lab.side()).unwrap();
    format!(
        "live {} candidates {} manual {} merged {} deleted {} mentions {} docs {} due {} consistent {} last {:?}",
        o.entities_live, o.entities_candidate, o.entities_manual, o.entities_merged,
        o.entities_deleted, o.mentions, o.documents_read, o.documents_due, o.consistent,
        o.last_operation.map(|l| (l.kind, l.undoable))
    )
}

fn sheet(lab: &Lab, name: &str) {
    let detail: EntityDetail = manage::entity_detail(&lab.side(), id(lab, name)).unwrap();
    println!(
        "  SHEET {name}: status {} origin {} mentions {} files {}",
        detail.entity.status, detail.entity.origin, detail.mention_count, detail.entity.sources
    );
    println!(
        "    names: {:?}",
        detail
            .aliases
            .iter()
            .map(|a| format!("{} ({}/{})", a.display, a.kind, a.origin))
            .collect::<Vec<_>>()
    );
    println!(
        "    merged in: {:?}",
        detail
            .merged_in
            .iter()
            .map(|b| b.name.clone())
            .collect::<Vec<_>>()
    );
    for source in &detail.sources {
        for m in &source.mentions {
            println!(
                "    {} | page {:?} section {:?} | {} {:.0}% | alias {:?} | origin {} split_from {:?} | {:?}",
                source.relative_path, m.locator.page, m.locator.section, m.method,
                m.confidence * 100.0, m.alias, m.origin, m.split_from,
                m.excerpt.as_ref().map(|e| e.text.chars().take(60).collect::<String>())
            );
        }
    }
}

#[tokio::test]
#[ignore]
async fn print_the_walk_of_the_human_test() {
    let dir = std::env::var("ACAI_LOT08_DIR").expect("set ACAI_LOT08_DIR");
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        lab.write(
            path.file_name().unwrap().to_str().unwrap(),
            &std::fs::read_to_string(&path).unwrap(),
        );
    }

    println!("== Preparation: Analyse");
    let summary = lab.analyse().await;
    println!(
        "  indexed {} ; knowledge {:?}",
        summary.indexed_files, summary.knowledge
    );
    println!("  STRIP {}", strip(&lab));

    println!("== A: read");
    for row in list(&lab, "", ListView::Live, false) {
        println!("  ROW {row}");
    }
    println!(
        "  search dupont -> {:?}",
        list(&lab, "dupont", ListView::Live, false)
    );
    sheet(&lab, "Jean Dupont");
    println!(
        "  problems only -> {:?}",
        list(&lab, "", ListView::Live, true)
    );

    println!("== B: add a name, analyse");
    let outcome = manage::add_alias(
        &lab.side(),
        &tools(&lab),
        id(&lab, "Jean Dupont"),
        "Jeannot",
    )
    .unwrap();
    println!("  outcome {outcome:?}");
    println!("  STRIP {}", strip(&lab));
    let summary = lab.analyse().await;
    println!("  knowledge {:?}", summary.knowledge);
    println!("  STRIP {}", strip(&lab));
    sheet(&lab, "Jean Dupont");

    println!("== C: merge Paul and Pierre Martin, unmerge");
    println!(
        "  search martin -> {:?}",
        list(&lab, "martin", ListView::Live, false)
    );
    let (paul, pierre) = (id(&lab, "Paul Martin"), id(&lab, "Pierre Martin"));
    println!("  ids paul {paul} pierre {pierre} (equal mentions: the smaller id survives)");
    let survivor = paul.min(pierre);
    let absorbed = paul.max(pierre);
    manage::merge(&lab.side(), survivor, absorbed).unwrap();
    println!("  STRIP {}", strip(&lab));
    sheet(
        &lab,
        if survivor == paul {
            "Paul Martin"
        } else {
            "Pierre Martin"
        },
    );
    println!(
        "  merged view -> {:?}",
        list(&lab, "", ListView::Merged, false)
    );
    let absorbed_name = if absorbed == paul {
        "Paul Martin"
    } else {
        "Pierre Martin"
    };
    let detail = manage::entity_detail(&lab.side(), absorbed).unwrap();
    println!(
        "  absorbed sheet survivor: {:?}",
        detail.survivor.map(|b| b.name)
    );
    manage::unmerge(&lab.side(), absorbed).unwrap();
    println!(
        "  after unmerge {absorbed_name}: {:?}",
        list(&lab, absorbed_name, ListView::Live, false)
    );
    println!("  STRIP {}", strip(&lab));

    println!("== D: split Claire Martin");
    sheet(&lab, "Claire Martin");
    let claire = id(&lab, "Claire Martin");
    let detail = manage::entity_detail(&lab.side(), claire).unwrap();
    let ordonnance = detail
        .sources
        .iter()
        .find(|s| s.relative_path == "martin-ordonnance.txt")
        .expect("ordonnance source");
    let outcome = manage::split(
        &lab.side(),
        &tools(&lab),
        &SplitRequest {
            entity_id: claire,
            mention_ids: vec![ordonnance.mentions[0].mention_id],
            source_ids: vec![],
            name: "Claire Martin 2".to_string(),
        },
    )
    .unwrap();
    println!("  outcome {outcome:?}");
    sheet(&lab, "Claire Martin 2");
    sheet(&lab, "Claire Martin");
    println!("  STRIP {}", strip(&lab));
    let summary = lab.analyse().await;
    println!("  analyse again: knowledge {:?}", summary.knowledge);
    sheet(&lab, "Claire Martin 2");
    sheet(&lab, "Claire Martin");
    manage::undo_last_operation(&lab.side()).unwrap();
    println!(
        "  after undo: {:?}",
        list(&lab, "claire", ListView::Live, false)
    );
    println!("  STRIP {}", strip(&lab));

    println!("== E: delete cabinet des Lilas, analyse, restore, analyse");
    println!(
        "  search lilas -> {:?}",
        list(&lab, "lilas", ListView::Live, false)
    );
    let lilas = id(&lab, "cabinet des Lilas");
    manage::delete_entity(&lab.side(), lilas).unwrap();
    println!(
        "  deleted view -> {:?}",
        list(&lab, "", ListView::Deleted, false)
    );
    println!("  STRIP {}", strip(&lab));
    let summary = lab.analyse().await;
    println!("  analyse: knowledge {:?}", summary.knowledge);
    println!(
        "  live search lilas -> {:?}",
        list(&lab, "lilas", ListView::Live, false)
    );
    println!(
        "  deleted view -> {:?}",
        list(&lab, "", ListView::Deleted, false)
    );
    manage::restore_entity(&lab.side(), lilas).unwrap();
    println!(
        "  live search lilas (before analyse) -> {:?}",
        list(&lab, "lilas", ListView::Live, false)
    );
    lab.analyse().await;
    println!(
        "  live search lilas (after analyse) -> {:?}",
        list(&lab, "lilas", ListView::Live, false)
    );
    println!("  STRIP {}", strip(&lab));

    println!("== F: to check, types");
    let health = manage::health_report(&lab.side()).unwrap();
    println!(
        "  health: once {} candidates {} docs without {} books without {} duplicates {} not in tables {}",
        health.seen_once.total, health.candidates, health.documents_without_entity.total,
        health.workbooks_without_usable_column.total, health.probable_duplicates,
        health.in_documents_not_in_tables.total
    );
    println!(
        "  pairs {}",
        manage::possible_matches(&lab.side(), 0).unwrap().total
    );
    let packs = PackSet::load_with_type_vocabulary(lab.locale, &lab.packs).unwrap();
    let types = manage::types_and_roles(&lab.side(), &packs).unwrap();
    println!(
        "  types {:?}",
        types
            .types
            .iter()
            .map(|t| (t.id.clone(), t.entities))
            .collect::<Vec<_>>()
    );
    println!(
        "  roles {} packs {:?} available {:?}",
        types.roles.len(),
        types.packs.iter().map(|p| p.id.clone()).collect::<Vec<_>>(),
        types.available_packs
    );

    println!("== G: reset keeping mine");
    let outcome = manage::reset(&lab.side(), false).unwrap();
    println!("  outcome {outcome:?}");
    for row in list(&lab, "", ListView::Live, false) {
        println!("  ROW {row}");
    }
    println!(
        "  deleted view -> {:?}",
        list(&lab, "", ListView::Deleted, false)
    );
    println!("  STRIP {}", strip(&lab));
    let summary = lab.analyse().await;
    println!("  analyse: knowledge {:?}", summary.knowledge);
    for row in list(&lab, "", ListView::Live, false) {
        println!("  ROW {row}");
    }
    println!("  STRIP {}", strip(&lab));

    println!("== G2: reset everything");
    manage::reset(&lab.side(), true).unwrap();
    println!("  rows -> {:?}", list(&lab, "", ListView::Live, false));
    println!("  STRIP {}", strip(&lab));
    let summary = lab.analyse().await;
    println!("  analyse: knowledge {:?}", summary.knowledge);
    for row in list(&lab, "", ListView::Live, false) {
        println!("  ROW {row}");
    }
    sheet(&lab, "Jean Dupont");
    println!("  STRIP {}", strip(&lab));
}
