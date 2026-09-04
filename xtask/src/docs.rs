//! Checks on the written documentation, so that it decays visibly rather than quietly.
//!
//! The decision records are the part worth guarding. They are immutable once
//! accepted and superseded rather than edited, which only works if the numbering
//! is sound and every supersession points both ways — otherwise the trail that
//! explains a reversal breaks, and the reversal looks like an accident.

use std::collections::BTreeMap;
use std::fs;

use crate::shell;

const REQUIRED_SECTIONS: [&str; 3] = ["## Context", "## Decision", "## Consequences"];

/// One decision record, as read from disk.
#[derive(Debug)]
struct Record {
    number: u16,
    file: String,
    text: String,
}

impl Record {
    fn status(&self) -> Option<&str> {
        self.text
            .lines()
            .find_map(|line| line.strip_prefix("- Status: "))
            .map(str::trim)
    }
}

fn read_records() -> Result<Vec<Record>, String> {
    let directory = shell::root().join("docs/decisions");
    let entries =
        fs::read_dir(&directory).map_err(|error| format!("{}: {error}", directory.display()))?;

    let mut records = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let is_markdown = std::path::Path::new(file)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("md"));
        if !is_markdown || file == "README.md" || file.starts_with("0000-") {
            continue;
        }

        let Some((digits, _)) = file.split_once('-') else {
            return Err(format!("docs/decisions/{file}: name must start with NNNN-"));
        };
        let number: u16 = digits
            .parse()
            .map_err(|_| format!("docs/decisions/{file}: {digits:?} is not a number"))?;

        let text = fs::read_to_string(&path).map_err(|error| format!("{file}: {error}"))?;
        records.push(Record {
            number,
            file: file.to_owned(),
            text,
        });
    }

    records.sort_by_key(|record| record.number);
    Ok(records)
}

/// Everything that can be judged from the records alone, with no filesystem.
///
/// Returns one line per complaint. Empty means the records are sound.
fn audit(records: &[Record]) -> Vec<String> {
    let mut complaints = Vec::new();
    let mut seen: BTreeMap<u16, String> = BTreeMap::new();

    for record in records {
        let file = &record.file;

        if let Some(other) = seen.insert(record.number, file.clone()) {
            complaints.push(format!(
                "{file}: number {} is also used by {other}",
                record.number
            ));
        }

        let heading = format!("# {:04}.", record.number);
        if !record.text.starts_with(&heading) {
            complaints.push(format!("{file}: should open with `{heading} Title`"));
        }
        for section in REQUIRED_SECTIONS {
            if !record.text.contains(section) {
                complaints.push(format!("{file}: has no `{section}` section"));
            }
        }
        // The template asks for the downside by name, and a record without one
        // reads as advocacy rather than a decision.
        if !record.text.to_lowercase().contains("cost") {
            complaints.push(format!("{file}: consequences never name a cost"));
        }

        match record.status() {
            None => complaints.push(format!("{file}: no `- Status:` line")),
            Some("accepted" | "proposed") => {}
            Some(status) => match status.strip_prefix("superseded by ") {
                None => complaints.push(format!("{file}: unknown status {status:?}")),
                Some(successor) => {
                    complaints.extend(check_supersession(record, successor, records));
                }
            },
        }
    }

    for (position, number) in records.iter().map(|record| record.number).enumerate() {
        let expected = u16::try_from(position + 1).unwrap_or(u16::MAX);
        if number != expected {
            complaints.push(format!("decision numbers skip from {expected} to {number}"));
            break;
        }
    }

    complaints
}

/// Verifies the decision records, then that the API documentation builds clean.
pub(crate) fn check() -> Result<(), String> {
    let records = read_records()?;
    let complaints = audit(&records);
    if !complaints.is_empty() {
        return Err(complaints.join("\n"));
    }

    eprintln!(
        "  decisions: {} records, numbering and supersessions sound",
        records.len()
    );
    shell::cargo_with_doc_warnings_denied(&["doc", "--no-deps", "--all-features"])
}

/// A supersession has to point both ways, or the trail explaining a reversal breaks.
fn check_supersession(record: &Record, successor: &str, records: &[Record]) -> Vec<String> {
    let Ok(number) = successor.parse::<u16>() else {
        return vec![format!(
            "{}: {successor:?} is not a decision number",
            record.file
        )];
    };

    let Some(later) = records.iter().find(|candidate| candidate.number == number) else {
        return vec![format!(
            "{}: superseded by {number:04}, which does not exist",
            record.file
        )];
    };

    if later.text.contains(&format!("{:04}", record.number)) {
        Vec::new()
    } else {
        vec![format!(
            "{}: says it supersedes {:04}, but never mentions it",
            later.file, record.number
        )]
    }
}

// The gate is only worth what its failures are worth, so each rule is shown a
// record that breaks it.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests may be blunt: an unwrap is an expectation and an index is a claim about shape"
)]
mod tests {
    use super::{Record, audit};

    fn record(number: u16, status: &str, extra: &str) -> Record {
        Record {
            number,
            file: format!("{number:04}-a-decision.md"),
            text: format!(
                "# {number:04}. A decision\n\n- Status: {status}\n- Date: 2026-09-04\n\n\
                 ## Context\n\nSomething was true.\n\n\
                 ## Decision\n\nSomething was decided.\n\n\
                 ## Consequences\n\nIt buys one thing. The cost is another.\n{extra}"
            ),
        }
    }

    #[test]
    fn sound_records_are_quiet() {
        let records = vec![record(1, "accepted", ""), record(2, "accepted", "")];
        assert_eq!(audit(&records), Vec::<String>::new());
    }

    #[test]
    fn a_supersession_pointing_nowhere_is_reported() {
        let records = vec![
            record(1, "superseded by 0099", ""),
            record(2, "accepted", ""),
        ];
        let complaints = audit(&records);
        assert!(
            complaints
                .iter()
                .any(|line| line.contains("does not exist")),
            "{complaints:?}"
        );
    }

    #[test]
    fn a_supersession_the_successor_never_mentions_is_reported() {
        let records = vec![
            record(1, "superseded by 0002", ""),
            record(2, "accepted", ""),
        ];
        let complaints = audit(&records);
        assert!(
            complaints
                .iter()
                .any(|line| line.contains("never mentions")),
            "{complaints:?}"
        );
    }

    #[test]
    fn a_supersession_named_at_both_ends_is_quiet() {
        let records = vec![
            record(1, "superseded by 0002", ""),
            record(2, "accepted", "\nReplaces 0001.\n"),
        ];
        assert_eq!(audit(&records), Vec::<String>::new());
    }

    #[test]
    fn a_missing_section_is_reported() {
        let mut records = vec![record(1, "accepted", "")];
        records[0].text = records[0].text.replace("## Decision", "## Something Else");
        let complaints = audit(&records);
        assert!(
            complaints.iter().any(|line| line.contains("## Decision")),
            "{complaints:?}"
        );
    }

    #[test]
    fn consequences_that_never_name_a_cost_are_reported() {
        let mut records = vec![record(1, "accepted", "")];
        records[0].text = records[0]
            .text
            .replace("The cost is another.", "It is all upside.");
        let complaints = audit(&records);
        assert!(
            complaints.iter().any(|line| line.contains("name a cost")),
            "{complaints:?}"
        );
    }

    #[test]
    fn a_gap_in_the_numbering_is_reported() {
        let records = vec![record(1, "accepted", ""), record(3, "accepted", "")];
        let complaints = audit(&records);
        assert!(
            complaints.iter().any(|line| line.contains("skip from")),
            "{complaints:?}"
        );
    }

    #[test]
    fn an_unknown_status_is_reported() {
        let records = vec![record(1, "mostly agreed", "")];
        let complaints = audit(&records);
        assert!(
            complaints
                .iter()
                .any(|line| line.contains("unknown status")),
            "{complaints:?}"
        );
    }
}
