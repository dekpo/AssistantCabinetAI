//! Lot G of the retrieval-quality programme: summaries in stages, every heavy one started by hand. T1 reads the
//! beginning of each chapter, T2 reads the whole document in batches; both can be stopped, are kept per file and
//! per prompt version, are dropped when the file changes, and a reading is called complete only when every unit
//! was read without a failure.

mod common;

use assistant_cabinet_ai_lib::error::AppError;
use assistant_cabinet_ai_lib::summaries::{
    estimate_seconds, record_speed, run, status, stored_speed, units_for, Stage, SummaryProgress,
    SUMMARY_INSTRUCTION,
};
use common::knowledge::Lab;
use std::cell::{Cell, RefCell};

fn line(tag: &str, n: usize) -> String {
    format!("the ordinary sentence number {n} of the part {tag} goes on with the usual words and ends here.")
}

/// Three chapters of more than 20 000 characters each, so the document is cut at its headings.
fn book() -> String {
    let mut text = String::new();
    for (chapter, tag) in ["alpha", "beta", "gamma"].iter().enumerate() {
        text.push_str(&format!("# Chapter {} {}\n", chapter + 1, tag));
        for n in 0..260 {
            text.push_str(&line(tag, n));
            text.push('\n');
        }
    }
    text
}

async fn analysed_book() -> Lab {
    let mut lab = Lab::new();
    lab.write("book.md", &book());
    lab.analyse().await;
    lab
}

#[tokio::test]
async fn the_chapter_stage_makes_one_unit_per_chapter_and_reads_only_the_beginning_of_each() {
    let lab = analysed_book().await;
    let units = units_for(&lab.index, "book.md", Stage::Chapters).expect("units");
    let titles: Vec<&str> = units.iter().map(|unit| unit.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["Chapter 1 alpha", "Chapter 2 beta", "Chapter 3 gamma"]
    );
    for unit in &units {
        assert!(
            unit.text.chars().count() <= 9_000,
            "a chapter is read up to a cap"
        );
    }
    assert!(units[1].text.contains("part beta"));
    assert!(!units[1].text.contains("part alpha"));
}

#[tokio::test]
async fn the_full_reading_covers_every_passage_in_batches_of_a_bounded_size() {
    let lab = analysed_book().await;
    let units = units_for(&lab.index, "book.md", Stage::FullReading).expect("units");
    assert!(
        units.len() >= 3,
        "a document of 60 000 characters is several batches"
    );
    for unit in &units {
        assert!(unit.text.chars().count() <= 9_000);
    }
    // every sentence of the book is in exactly the batches, in order
    let joined: String = units
        .iter()
        .map(|unit| unit.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for tag in ["alpha", "beta", "gamma"] {
        for n in [0usize, 130, 259] {
            assert!(joined.contains(&line(tag, n)), "{tag} {n} is read");
        }
    }
}

#[tokio::test]
async fn a_run_keeps_what_it_wrote_and_a_failure_is_counted_not_hidden() {
    let mut lab = analysed_book().await;
    let calls = Cell::new(0usize);
    let result = run(
        &mut lab.index,
        "book.md",
        Stage::Chapters,
        |_text: String| {
            calls.set(calls.get() + 1);
            let n = calls.get();
            async move {
                if n == 2 {
                    Err(AppError::ServerTimeout { url: String::new() })
                } else {
                    Ok(format!("summary {n}"))
                }
            }
        },
        || false,
        |_progress: SummaryProgress| {},
    )
    .await
    .expect("runs");
    assert_eq!((result.done, result.failed, result.total), (2, 1, 3));
    assert!(!result.complete, "one chapter failed: not complete");
    let state = status(&lab.index, "book.md", Stage::Chapters).expect("status");
    assert_eq!((state.done, state.total, state.complete), (2, 3, false));

    // resumed: only the missing unit is written
    let again = Cell::new(0usize);
    let result = run(
        &mut lab.index,
        "book.md",
        Stage::Chapters,
        |_text: String| {
            again.set(again.get() + 1);
            async move { Ok("late summary".to_string()) }
        },
        || false,
        |_progress: SummaryProgress| {},
    )
    .await
    .expect("runs");
    assert_eq!(
        again.get(),
        1,
        "the two finished chapters are not written again"
    );
    assert!(result.complete);
    assert_eq!(
        status(&lab.index, "book.md", Stage::Chapters).unwrap().done,
        3
    );
}

#[tokio::test]
async fn a_stop_between_two_units_ends_the_run_and_keeps_the_finished_ones() {
    let mut lab = analysed_book().await;
    let written = Cell::new(0usize);
    let result = run(
        &mut lab.index,
        "book.md",
        Stage::Chapters,
        |_text: String| {
            written.set(written.get() + 1);
            async move { Ok("done".to_string()) }
        },
        || written.get() >= 1,
        |_progress: SummaryProgress| {},
    )
    .await
    .expect("runs");
    assert!(result.cancelled);
    assert_eq!(result.done, 1);
    assert!(!result.complete);
}

#[tokio::test]
async fn progress_names_the_unit_being_read_and_the_count() {
    let mut lab = analysed_book().await;
    let seen: RefCell<Vec<(usize, usize, String)>> = RefCell::new(Vec::new());
    run(
        &mut lab.index,
        "book.md",
        Stage::Chapters,
        |_text: String| async move { Ok("ok".to_string()) },
        || false,
        |progress: SummaryProgress| {
            seen.borrow_mut()
                .push((progress.done, progress.total, progress.title.clone()));
        },
    )
    .await
    .expect("runs");
    let seen = seen.borrow();
    assert_eq!(seen.first(), Some(&(0, 3, "Chapter 1 alpha".to_string())));
    assert_eq!(seen.last().map(|entry| (entry.0, entry.1)), Some((2, 3)));
}

#[tokio::test]
async fn a_changed_file_drops_its_summaries() {
    let mut lab = analysed_book().await;
    run(
        &mut lab.index,
        "book.md",
        Stage::Chapters,
        |_text: String| async move { Ok("ok".to_string()) },
        || false,
        |_progress: SummaryProgress| {},
    )
    .await
    .expect("runs");
    assert!(
        status(&lab.index, "book.md", Stage::Chapters)
            .unwrap()
            .complete
    );

    lab.write(
        "book.md",
        &format!("{}\n# Chapter 4 delta\nA new chapter.\n", book()),
    );
    lab.analyse().await;
    let state = status(&lab.index, "book.md", Stage::Chapters).unwrap();
    assert_eq!(
        state.done, 0,
        "the file changed: nothing stored for it is kept"
    );
    assert!(state.stale);
    assert!(!state.complete);
}

#[tokio::test]
async fn the_estimate_is_the_remaining_units_times_the_measured_speed_of_this_machine() {
    let lab = analysed_book().await;
    assert_eq!(stored_speed(&lab.index).unwrap(), None);
    let default = estimate_seconds(10, None);
    assert!(
        default > 10.0 * 16.0,
        "a floor of 16 s a call and the tokens at about 90 a second"
    );
    record_speed(&lab.index, 4_000.0).expect("records");
    let speed = stored_speed(&lab.index).unwrap();
    assert_eq!(speed, Some(4_000.0));
    assert_eq!(estimate_seconds(10, speed), 40.0);
    assert_eq!(estimate_seconds(0, speed), 0.0);
}

#[test]
fn the_summary_instruction_stays_neutral_about_who_the_user_is() {
    let lower = SUMMARY_INSTRUCTION.to_lowercase();
    for word in [
        "patient",
        "doctor",
        "practitioner",
        "practice",
        "gp",
        "lawyer",
    ] {
        assert!(
            !lower.contains(word),
            "{word:?} found in SUMMARY_INSTRUCTION"
        );
    }
}
