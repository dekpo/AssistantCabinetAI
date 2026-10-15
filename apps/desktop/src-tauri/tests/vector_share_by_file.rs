//! Lot H: the share of passages that have their meaning vector, per file. Only the files below 100 % are
//! listed, so the interface can say « Recherche par sens : 62 % » on exactly those.

mod common;

use common::knowledge::Lab;

fn text(tag: &str) -> String {
    (0..40)
        .map(|n| format!("sentence {n} of {tag} goes on with the usual words and ends here."))
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn only_the_files_with_passages_lacking_a_vector_are_listed() {
    let mut lab = Lab::new();
    lab.write("whole.txt", &text("whole"));
    lab.write("partial.txt", &text("partial"));
    lab.analyse().await;
    assert!(lab
        .index
        .incomplete_vector_shares()
        .expect("shares")
        .is_empty());

    let side = lab.side();
    side.execute(
        "UPDATE chunks SET embedding = NULL WHERE relative_path = 'partial.txt'",
        [],
    )
    .expect("clears the vectors");
    let shares = lab.index.incomplete_vector_shares().expect("shares");
    assert_eq!(shares.len(), 1);
    assert_eq!(shares[0].relative_path, "partial.txt");
    assert!(shares[0].chunks >= 1);
    assert_eq!(shares[0].vectors, 0);
}
