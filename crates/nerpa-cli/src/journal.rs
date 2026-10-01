//! A journal of each run, beside state, for a person.
//!
//! RFC 0001 names it among what that proposal does not build: a record of each
//! run for somebody reading afterwards, outside any plan. One appended line per
//! run — who acted, and what the run was — and never a second copy of what a
//! plan or the state already says. Append-only by opening for appending, and a
//! line is written when a run has an outcome, so a run killed outright leaves
//! nothing rather than a line claiming more than happened. When the log of
//! `0009` exists, what has to be believed rather than merely read moves there.

use std::io::Write as _;
use std::path::{Path, PathBuf};

/// Where the journal lives: beside state and named after it, so one state
/// directory holds one record of the runs that wrote it.
fn beside(state: &Path) -> PathBuf {
    let mut named = state.as_os_str().to_owned();
    named.push(".journal");
    PathBuf::from(named)
}

/// Appends one line to the run journal.
///
/// # Errors
///
/// Returns the problem if the journal could not be opened or written: a
/// journal that fails quietly is worse than none, because the next reader
/// believes the record is complete.
pub(crate) fn record(state: &Path, line: &str) -> Result<(), String> {
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("the clock is before 1970: {error}"))?;
    let where_to = beside(state);
    let mut into = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&where_to)
        .map_err(|error| format!("{}: {error}", where_to.display()))?;
    writeln!(into, "{} {line}", since.as_secs())
        .map_err(|error| format!("{}: {error}", where_to.display()))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests may be blunt: see docs/CODING_STANDARDS.md"
)]
mod tests {
    use super::{beside, record};
    use std::path::Path;

    #[test]
    fn a_run_journal_says_who_acted() {
        let scratch = tempfile::tempdir().unwrap();
        let state = scratch.path().join("nerpa.state.json");

        record(&state, "plan 00 as alice over run until further notice").unwrap();
        record(
            &state,
            "apply 00 · 2 changes as alice over run until further notice",
        )
        .unwrap();

        let written = std::fs::read_to_string(beside(&state)).unwrap();
        let lines: Vec<&str> = written.lines().collect();
        assert_eq!(lines.len(), 2, "{written}");
        assert!(
            lines[0].ends_with("plan 00 as alice over run until further notice"),
            "{written}"
        );
        assert!(
            lines[1].ends_with("apply 00 · 2 changes as alice over run until further notice"),
            "{written}"
        );
        // Seconds since the epoch, the same clock the state file reads.
        let at: u64 = lines[0].split(' ').next().unwrap().parse().unwrap();
        assert!(at > 1_700_000_000, "{written}");
    }

    #[test]
    fn the_journal_lives_beside_state_and_not_inside_it() {
        let beside_state = beside(Path::new("/somewhere/nerpa.state.json"));
        assert_eq!(
            beside_state,
            Path::new("/somewhere/nerpa.state.json.journal")
        );
    }
}
