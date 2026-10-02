//! A journal of each run, beside state, for a person.
//!
//! RFC 0001 names it among what that proposal does not build: a record of each
//! run for somebody reading afterwards, outside any plan. One appended line per
//! run — who acted, and what the run was — and never a second copy of what a
//! plan or the state already says. Append-only by opening for appending, and a
//! line is written when a run has an outcome, so a run killed outright leaves
//! nothing rather than a line claiming more than happened.
//!
//! # An append-only hash chain
//!
//! What has to be believed rather than merely read lives here, so `0009` makes
//! the log an append-only hash chain. Every entry's hash is taken over its own
//! line and the hash of the entry before it, from the first entry on, so an
//! entry rewritten, reordered or removed breaks the chain at that point and
//! [`verify`] names it (`INV-LOG-001`).
//!
//! What a chain over itself cannot prove, said rather than left implied:
//! truncating the tail leaves a chain that still verifies, and rewriting the
//! whole log with recomputed hashes verifies too. The chain makes an edit to a
//! standing record visible; that the first entry is the first is what an anchor
//! outside the log has to say, and who wrote any of it is what the detached
//! signature of `0009` is for.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// The bytes every entry's hash starts with, so an entry hash can never collide
/// with a hash of anything else, and changing how an entry is encoded changes
/// every hash rather than some of them.
const ENTRY_DOMAIN: &[u8] = b"nerpa-journal-entry-1";

/// What the first entry chains from: nothing came before it.
const GENESIS: [u8; 32] = [0; 32];

/// Where the journal lives: beside state and named after it, so one state
/// directory holds one record of the runs that wrote it.
fn beside(state: &Path) -> PathBuf {
    let mut named = state.as_os_str().to_owned();
    named.push(".journal");
    PathBuf::from(named)
}

/// One line of the journal, read back.
#[derive(Debug, PartialEq, Eq)]
struct Entry {
    /// Seconds since the epoch, the same clock the state file reads.
    at: u64,
    /// What the run was. Last on the line, because a person reads it.
    text: String,
    /// What this entry hashes to: its own line and the entry before it.
    hash: [u8; 32],
}

/// The hash an entry carries — over its own line and the hash of the entry
/// before it, which is what makes the entries a chain rather than a list.
fn chained(previous: &[u8; 32], at: u64, text: &str) -> [u8; 32] {
    let line = format!("{at} {text}");
    let mut over = Vec::from(ENTRY_DOMAIN);
    over.extend_from_slice(previous);
    over.extend_from_slice(line.as_bytes());
    *blake3::hash(&over).as_bytes()
}

/// Reads one line back as an entry. Anything that is not `seconds hash text`
/// is not an entry of this chain — a line of a journal written before the
/// chain, or one that has been cut.
fn read_entry(line: &str) -> Option<Entry> {
    let (at, rest) = line.split_once(' ')?;
    let (hash, text) = rest.split_once(' ')?;
    Some(Entry {
        at: at.parse().ok()?,
        text: text.to_owned(),
        hash: unhex(hash)?,
    })
}

/// The hash the last entry carries, or [`GENESIS`] for a journal with nothing
/// in it. A journal whose last line is not an entry is refused rather than
/// chained onto: extending a record that cannot be verified is how a log stops
/// meaning anything.
fn previous(where_to: &Path) -> Result<[u8; 32], String> {
    let written = match std::fs::read_to_string(where_to) {
        Ok(written) => written,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(GENESIS),
        Err(error) => return Err(format!("{}: {error}", where_to.display())),
    };
    match written.lines().next_back().map(read_entry) {
        None => Ok(GENESIS),
        Some(Some(entry)) => Ok(entry.hash),
        Some(None) => Err(format!(
            "{}: the journal is not a hash chain — its last line carries no \
             entry hash, so nothing appended to it could be verified from the \
             first entry",
            where_to.display()
        )),
    }
}

/// Appends one line to the run journal.
///
/// # Errors
///
/// Returns the problem if the journal could not be opened or written, if the
/// line holds a newline — which would forge an entry boundary — or if the
/// journal it appends to is not a chain to append to. A journal that fails
/// quietly is worse than none, because the next reader believes the record is
/// complete.
#[dacc_derive::doc_anchor(id = "inv-log-001")]
pub(crate) fn record(state: &Path, line: &str) -> Result<(), String> {
    if line.contains('\n') {
        return Err("a journal line holds no newline: one would forge an entry \
                    this chain cannot cover"
            .to_owned());
    }
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("the clock is before 1970: {error}"))?;
    let where_to = beside(state);
    let previous = previous(&where_to)?;
    let at = since.as_secs();
    let mut into = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&where_to)
        .map_err(|error| format!("{}: {error}", where_to.display()))?;
    writeln!(into, "{at} {} {line}", hex(&chained(&previous, at, line)))
        .map_err(|error| format!("{}: {error}", where_to.display()))
}

/// Walks the log from the first entry and says how many entries chain from it.
///
/// # Errors
///
/// Returns where the chain breaks — the entry that does not chain from the one
/// before it, and what that means — or that there is no journal at all. The
/// answer names the entry rather than saying only "broken", because a person
/// reading at three in the morning needs to know how much of the record stands.
pub(crate) fn verify(state: &Path) -> Result<usize, String> {
    let where_to = beside(state);
    let written = match std::fs::read_to_string(&where_to) {
        Ok(written) => written,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!("there is no journal beside {}", state.display()));
        }
        Err(error) => return Err(format!("{}: {error}", where_to.display())),
    };

    let mut previous = GENESIS;
    let mut entries = 0;
    for (at, line) in written.lines().enumerate() {
        let number = at + 1;
        let Some(entry) = read_entry(line) else {
            return Err(format!(
                "the log does not verify: entry {number} carries no entry hash, \
                 so it is not one this chain covers — a journal written before \
                 the chain, or one that has been cut"
            ));
        };
        if chained(&previous, entry.at, &entry.text) != entry.hash {
            return Err(format!(
                "the log does not verify: entry {number} does not chain from the \
                 entry before it — rewritten, reordered or removed. The entries \
                 before it stand; nothing from here on can be believed"
            ));
        }
        previous = entry.hash;
        entries += 1;
    }
    Ok(entries)
}

/// Thirty-two bytes as hexadecimal, the form a line carries.
fn hex(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        // Infallible on a String, and refused rather than ignored were that to
        // stop being true.
        write!(out, "{byte:02x}").expect("a String takes whatever is written to it");
    }
    out
}

/// Hexadecimal back, exactly sixty-four characters of it. Anything else is
/// [`None`]: a hash field of another shape is not a hash of this chain.
fn unhex(text: &str) -> Option<[u8; 32]> {
    let written = text.as_bytes();
    if written.len() != 64 {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (at, byte) in bytes.iter_mut().enumerate() {
        let high = digit(*written.get(at * 2)?)?;
        let low = digit(*written.get(at * 2 + 1)?)?;
        *byte = high * 16 + low;
    }
    Some(bytes)
}

/// One hexadecimal digit.
fn digit(character: u8) -> Option<u8> {
    match character {
        b'0'..=b'9' => Some(character - b'0'),
        b'a'..=b'f' => Some(character - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests may be blunt: see docs/CODING_STANDARDS.md"
)]
mod tests {
    use super::{beside, chained, read_entry, record, verify};
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

    /// The point of the whole shape: what is written verifies, and every entry
    /// carries the one before it rather than standing alone.
    #[test]
    fn what_is_written_verifies_from_the_first_entry() {
        let scratch = tempfile::tempdir().unwrap();
        let state = scratch.path().join("nerpa.state.json");

        record(&state, "plan 00 as alice over run until further notice").unwrap();
        record(&state, "plan 01 as alice over run until further notice").unwrap();
        record(&state, "release as alice over run until further notice").unwrap();

        assert_eq!(verify(&state).unwrap(), 3, "three entries, all chained");
    }

    /// An entry is chained to the one before it and to nothing else: taking the
    /// hash of an entry out of another log's entry chain does not work.
    #[test]
    fn an_entry_chains_to_the_one_before_it() {
        let at = 1_700_000_000;
        let first = chained(&super::GENESIS, at, "plan 00 as alice over run");
        let same = chained(&super::GENESIS, at, "plan 00 as alice over run");
        assert_eq!(first, same, "the same entry hashes the same every time");
        assert_ne!(
            first,
            chained(&first, at, "plan 00 as alice over run"),
            "and an entry after another is a different entry"
        );
    }

    /// The line reads back as what was written: `seconds hash text`, with the
    /// text last because a person reads it.
    #[test]
    fn a_line_reads_back_as_an_entry() {
        let at = 1_700_000_000;
        let hash = chained(&super::GENESIS, at, "plan 00 as alice");
        let line = format!("{at} {} plan 00 as alice", super::hex(&hash));
        let entry = read_entry(&line).unwrap();
        assert_eq!(entry.at, at);
        assert_eq!(entry.text, "plan 00 as alice");
        assert_eq!(entry.hash, hash);
    }

    /// A line that is not `seconds hash text` is not an entry, whatever else it
    /// looks like — a journal of another shape is not silently read as this one.
    #[test]
    fn a_line_of_another_shape_is_not_an_entry() {
        assert!(read_entry("1700000000 plan 00 as alice").is_none());
        assert!(read_entry("1700000000 nothex plan 00 as alice").is_none());
        assert!(read_entry("").is_none());
    }

    /// A line holding a newline would forge an entry boundary inside one entry.
    #[test]
    fn a_line_holding_a_newline_is_refused_rather_than_written() {
        let scratch = tempfile::tempdir().unwrap();
        let state = scratch.path().join("nerpa.state.json");

        let refused = record(&state, "plan 00\napply 00 · everything as mallory");
        assert!(refused.is_err(), "one line is one entry");
        assert!(
            std::fs::read_to_string(beside(&state)).is_err(),
            "and nothing at all is written"
        );
    }

    /// A journal that cannot be verified is not one to extend: appending to a
    /// log of another shape would make a record nothing can walk from the start.
    #[test]
    fn appending_to_a_journal_of_another_shape_is_refused() {
        let scratch = tempfile::tempdir().unwrap();
        let state = scratch.path().join("nerpa.state.json");
        std::fs::write(beside(&state), "1700000000 plan 00 as alice\n").unwrap();

        let refused = record(&state, "apply 00 as alice over run");
        assert!(refused.is_err(), "there is no chain to continue");
    }
}
