//! Stopping a question that is still being worked on.
//!
//! A flag flipped in the interface would be a trap: `invoke` returns an ordinary promise, and
//! abandoning it does not stop this side. The answer would be hidden while the practice machine
//! kept generating at full load, and the orphaned stream would keep pushing text at an entry that
//! no longer exists. So the stop has to reach the request itself.
//!
//! One question is in flight at a time - the composer refuses a second while one is running - so a
//! single signal is enough. A run announces itself with [`Cancellation::begin`], and
//! [`Cancellation::cancel`] asks it to stop. The run's future is then dropped rather than asked to
//! wind down: dropping the request closes the connection, and that is what actually stops the
//! model. The gateway copes - its streaming generator records its metadata in a `finally`, and the
//! runtime call sits inside an `async with`, so the disconnection propagates all the way down.

use std::future::Future;

use tokio::sync::watch;

use crate::error::AppError;

pub struct Cancellation {
    sender: watch::Sender<bool>,
}

impl Default for Cancellation {
    fn default() -> Self {
        Self {
            sender: watch::channel(false).0,
        }
    }
}

impl Cancellation {
    /// Announce a run and receive its stop signal. A stop asked for while nothing was running is
    /// forgotten here, so a click that landed between two questions cannot cancel the next one.
    pub fn begin(&self) -> watch::Receiver<bool> {
        self.sender.send_replace(false);
        self.sender.subscribe()
    }

    /// Ask the run in flight to stop. Nothing happens when there is none.
    pub fn cancel(&self) {
        self.sender.send_replace(true);
    }
}

/// Run `work` until it finishes or the run is stopped, whichever comes first.
///
/// A stop wins even when it arrives in the same instant as the answer: she asked for nothing to be
/// kept, so the result is dropped rather than raced for.
pub async fn until_stopped<T>(
    stopped: &mut watch::Receiver<bool>,
    work: impl Future<Output = Result<T, AppError>>,
) -> Result<T, AppError> {
    tokio::pin!(work);
    loop {
        tokio::select! {
            result = &mut work => {
                return if *stopped.borrow() { Err(AppError::ChatCancelled) } else { result };
            }
            changed = stopped.changed() => {
                // A reset to `false` is another run announcing itself, not a stop. A closed
                // sender means the state is going away, which ends this run too.
                if changed.is_err() || *stopped.borrow() {
                    return Err(AppError::ChatCancelled);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    async fn never_finishes() -> Result<&'static str, AppError> {
        std::future::pending().await
    }

    #[tokio::test]
    async fn a_stop_ends_a_run_that_is_still_waiting() {
        let cancellation = Cancellation::default();
        let mut stopped = cancellation.begin();
        cancellation.cancel();

        let outcome = until_stopped(&mut stopped, never_finishes()).await;

        assert_eq!(outcome.expect_err("stopped").code(), "chat_cancelled");
    }

    #[tokio::test]
    async fn work_that_finishes_on_its_own_keeps_its_answer() {
        let cancellation = Cancellation::default();
        let mut stopped = cancellation.begin();

        let outcome = until_stopped(&mut stopped, async { Ok("an answer") }).await;

        assert_eq!(outcome.expect("finished"), "an answer");
    }

    #[tokio::test]
    async fn a_stop_asked_for_between_two_questions_does_not_cancel_the_next_one() {
        let cancellation = Cancellation::default();
        cancellation.cancel();

        let mut stopped = cancellation.begin();
        let outcome = until_stopped(&mut stopped, async { Ok("an answer") }).await;

        assert_eq!(outcome.expect("finished"), "an answer");
    }

    #[tokio::test]
    async fn a_stop_that_arrives_with_the_answer_still_discards_it() {
        let cancellation = Cancellation::default();
        let mut stopped = cancellation.begin();
        cancellation.cancel();

        // Both branches are ready at the first poll, so without the check the outcome would
        // depend on which one `select!` happened to pick.
        let outcome = until_stopped(&mut stopped, async { Ok("an answer") }).await;

        assert_eq!(outcome.expect_err("stopped").code(), "chat_cancelled");
    }

    #[tokio::test]
    async fn work_still_in_progress_is_not_interrupted_by_anything_else() {
        let cancellation = Cancellation::default();
        let mut stopped = cancellation.begin();

        let outcome = until_stopped(&mut stopped, async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            Ok("an answer")
        })
        .await;

        assert_eq!(outcome.expect("finished"), "an answer");
    }
}
