//! The Stop of an Analyse, one flag per folder.
//!
//! The Documents pass and the Data pass can run at the same time, and each has its own Stop: pressing
//! it on one family must not touch the other, and starting one pass must not cancel a Stop pressed on
//! the other (`docs/DECISIONS.md`, "Progress of an Analyse"). A flag is raised by `cancel_analysis`,
//! read between two files (or two batches) by the pass of its own folder, and lowered when that pass
//! starts. Plain atomics: nothing here waits, so it is tested on the object without a socket.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::file_profile::build::Domain;

#[derive(Debug, Default)]
pub struct AnalysisStops {
    documents: AtomicBool,
    data: AtomicBool,
}

impl AnalysisStops {
    fn flag(&self, domain: Domain) -> &AtomicBool {
        match domain {
            Domain::Documents => &self.documents,
            Domain::Data => &self.data,
        }
    }

    /// A pass of this folder is starting: forget a Stop pressed before it. The other folder's flag is
    /// left exactly as it is.
    pub fn lower(&self, domain: Domain) {
        self.flag(domain).store(false, Ordering::SeqCst);
    }

    /// Ask the pass of this folder to stop. Nothing happens when none is running: the next pass
    /// lowers the flag before it reads anything.
    pub fn raise(&self, domain: Domain) {
        self.flag(domain).store(true, Ordering::SeqCst);
    }

    /// Whether the pass of this folder has been asked to stop.
    pub fn is_raised(&self, domain: Domain) -> bool {
        self.flag(domain).load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stop_on_one_folder_leaves_the_other_running() {
        let stops = AnalysisStops::default();

        stops.raise(Domain::Data);

        assert!(stops.is_raised(Domain::Data));
        assert!(!stops.is_raised(Domain::Documents));
    }

    #[test]
    fn starting_a_pass_lowers_only_its_own_flag() {
        let stops = AnalysisStops::default();
        stops.raise(Domain::Documents);
        stops.raise(Domain::Data);

        stops.lower(Domain::Documents);

        assert!(!stops.is_raised(Domain::Documents));
        assert!(
            stops.is_raised(Domain::Data),
            "a Stop pressed on the other folder survives the start of this pass"
        );
    }

    #[test]
    fn a_stop_pressed_with_no_pass_running_is_forgotten_by_the_next_pass() {
        let stops = AnalysisStops::default();
        stops.raise(Domain::Documents);

        stops.lower(Domain::Documents);

        assert!(!stops.is_raised(Domain::Documents));
    }
}
