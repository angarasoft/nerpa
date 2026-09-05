//! Checks on the written documentation, so that it decays visibly rather than quietly.
//!
//! The decision records are the part worth guarding. They are immutable once
//! accepted and superseded rather than edited, which only works if the numbering
//! is sound and every supersession points both ways — otherwise the trail that
//! explains a reversal breaks, and the reversal looks like an accident.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use crate::shell;

/// One Markdown page: a repository-relative path, and its text.
type Page = (String, String);

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

/// The three words a section of the architecture may use about itself.
///
/// Ordered longest-first nowhere on purpose: none is a prefix of another after
/// the separator, so a plain `contains` cannot confuse them.
const BUILD_MARKERS: [&str; 3] = ["· built", "· partly built", "· planned"];

/// Every section of the architecture says whether it exists yet.
///
/// The document is written in the present tense throughout, because that is how
/// a contract reads. Without a marker there is nothing on the page telling a
/// reader that a section describes something nobody has built, and the file
/// becomes indistinguishable from a description of a working program.
fn architecture(text: &str) -> Vec<String> {
    let mut complaints = Vec::new();
    let mut sections = 0usize;

    for (index, line) in text.lines().enumerate() {
        let Some(heading) = line.strip_prefix("## ") else {
            continue;
        };
        sections += 1;
        if !BUILD_MARKERS.iter().any(|marker| line.contains(marker)) {
            complaints.push(format!(
                "docs/ARCHITECTURE.md:{}: {heading:?} does not end in `· built`, \
                 `· partly built` or `· planned`",
                index + 1
            ));
        }
    }

    if sections == 0 {
        complaints.push("docs/ARCHITECTURE.md: no sections to judge".to_owned());
    }
    complaints
}

/// Where a link in `page` points, as a repository-relative path.
///
/// `None` for anything that does not name a file in this repository: an external
/// address, or a bare fragment pointing within the page itself.
fn resolve(page: &str, target: &str) -> Option<String> {
    let target = target.split('#').next().unwrap_or_default();
    if target.is_empty()
        || target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("mailto:")
    {
        return None;
    }

    let mut parts: Vec<&str> = page.split('/').collect();
    parts.pop();
    for step in target.split('/') {
        match step {
            "." | "" => {}
            ".." => {
                parts.pop()?;
            }
            name => parts.push(name),
        }
    }
    Some(parts.join("/"))
}

/// Every link between written documents points at something that is there.
///
/// The failure this exists to prevent is specific rather than theoretical: a
/// route that goes on looking plausible after the file it names has moved, so a
/// reader follows it and believes whatever they land on. A broken link is
/// noticed; a link into the wrong place is not.
fn routes(pages: &[Page], present: &BTreeSet<String>) -> Vec<String> {
    let mut complaints = Vec::new();

    for (page, text) in pages {
        for (number, line) in text.lines().enumerate() {
            let mut rest = line;
            while let Some((_, tail)) = rest.split_once("](") {
                let Some((target, after)) = tail.split_once(')') else {
                    break;
                };
                rest = after;

                let Some(path) = resolve(page, target) else {
                    continue;
                };
                if !present.contains(&path) {
                    complaints.push(format!(
                        "{page}:{}: links to {target:?}, which is not there",
                        number + 1
                    ));
                }
            }
        }
    }

    complaints
}

/// What the written documentation is made of, as far as a link is concerned.
#[derive(Debug)]
struct Written {
    /// Every Markdown page, as a repository-relative path and its text.
    pages: Vec<Page>,
    /// Every path in the repository, which is what a link may point at.
    present: BTreeSet<String>,
}

fn written() -> Result<Written, String> {
    let root = shell::root();
    let mut pages = Vec::new();
    let mut present = BTreeSet::new();
    let mut pending = vec![root.clone()];

    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?;
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(relative) = path.strip_prefix(&root) else {
                continue;
            };
            let shown = relative.to_string_lossy().replace('\\', "/");
            // Build output and version control are not documentation, and
            // walking them is seconds rather than milliseconds.
            if matches!(
                shown.as_str(),
                "target" | ".git" | "mutants.out" | ".cargo" | "node_modules"
            ) {
                continue;
            }

            present.insert(shown.clone());
            if path.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
                && let Ok(text) = fs::read_to_string(&path)
            {
                pages.push((shown, text));
            }
        }
    }

    Ok(Written { pages, present })
}

/// Verifies the decision records and the written documentation, then that the
/// API documentation builds clean.
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

    let architecture_path = shell::root().join("docs/ARCHITECTURE.md");
    let text = fs::read_to_string(&architecture_path)
        .map_err(|error| format!("{}: {error}", architecture_path.display()))?;
    let complaints = architecture(&text);
    if !complaints.is_empty() {
        return Err(complaints.join("\n"));
    }

    let Written { pages, present } = written()?;
    let complaints = routes(&pages, &present);
    if !complaints.is_empty() {
        return Err(complaints.join("\n"));
    }
    eprintln!(
        "  written: {} pages, every section placed and every route resolving",
        pages.len()
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
    use super::{Record, architecture, audit, resolve, routes};
    use std::collections::BTreeSet;

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

    // Every rule above is shown a fixture that breaks it. A check nobody has
    // watched go red is a check nobody has any grounds to trust.

    #[test]
    fn a_section_that_does_not_say_whether_it_is_built_is_reported() {
        let complaints = architecture("# Architecture\n\n## Secrets — [0027](x.md)\n\nProse.\n");
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(complaints[0].contains("does not end in"), "{complaints:?}");
    }

    #[test]
    fn each_of_the_three_words_is_accepted() {
        for marker in ["built", "partly built", "planned"] {
            let text = format!("## A section — [0001](x.md) · {marker}\n");
            assert_eq!(architecture(&text), Vec::<String>::new(), "{marker}");
        }
    }

    #[test]
    fn partly_built_is_not_read_as_built() {
        // The two markers share a word, and a careless `contains` would let a
        // section claiming half of itself pass as a section claiming all of it.
        let complaints = architecture("## A section — [0001](x.md) · partly\n");
        assert_eq!(complaints.len(), 1, "{complaints:?}");
    }

    #[test]
    fn an_architecture_with_no_sections_is_itself_a_complaint() {
        assert_eq!(architecture("# Architecture\n\nProse only.\n").len(), 1);
    }

    #[test]
    fn a_link_is_resolved_against_the_page_that_holds_it() {
        assert_eq!(
            resolve("docs/MAP.md", "decisions/0005-a.md").as_deref(),
            Some("docs/decisions/0005-a.md")
        );
        assert_eq!(
            resolve("docs/decisions/0005-a.md", "../ARCHITECTURE.md").as_deref(),
            Some("docs/ARCHITECTURE.md")
        );
        assert_eq!(
            resolve("README.md", "ROADMAP.md").as_deref(),
            Some("ROADMAP.md")
        );
    }

    #[test]
    fn an_anchor_names_the_page_it_sits_on() {
        assert_eq!(
            resolve("docs/MAP.md", "ARCHITECTURE.md#addresses").as_deref(),
            Some("docs/ARCHITECTURE.md")
        );
    }

    #[test]
    fn what_is_not_a_file_in_this_repository_is_left_alone() {
        assert_eq!(resolve("docs/MAP.md", "https://example.invalid/a"), None);
        assert_eq!(resolve("docs/MAP.md", "#a-heading-here"), None);
        assert_eq!(
            resolve("docs/MAP.md", "mailto:someone@example.invalid"),
            None
        );
    }

    #[test]
    fn a_route_to_something_that_moved_is_reported_with_its_line() {
        let pages = vec![(
            "docs/MAP.md".to_owned(),
            "One line.\nSee [the plan](decisions/0099-gone.md).\n".to_owned(),
        )];
        let present = BTreeSet::from(["docs/MAP.md".to_owned()]);

        let complaints = routes(&pages, &present);
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(complaints[0].contains("docs/MAP.md:2"), "{complaints:?}");
    }

    #[test]
    fn several_routes_on_one_line_are_each_checked() {
        let pages = vec![(
            "docs/MAP.md".to_owned(),
            "[a](a.md) and [b](b.md) and [c](c.md)\n".to_owned(),
        )];
        let present = BTreeSet::from(["docs/b.md".to_owned()]);

        let complaints = routes(&pages, &present);
        assert_eq!(complaints.len(), 2, "{complaints:?}");
    }

    #[test]
    fn a_route_that_resolves_is_quiet() {
        let pages = vec![(
            "docs/MAP.md".to_owned(),
            "See [the record](decisions/0005-a.md).\n".to_owned(),
        )];
        let present = BTreeSet::from(["docs/decisions/0005-a.md".to_owned()]);

        assert_eq!(routes(&pages, &present), Vec::<String>::new());
    }
}
