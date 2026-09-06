//! The book, assembled from what is already written and already checked.
//!
//! Tooling rather than a chapter: this module has no section in the
//! architecture because it is not part of what the product is.
//! Reasoning: decision record 0036.
//!
//! # Why there is only one parser
//!
//! Sphinx renders the book, and Sphinx is Python. The obvious arrangement — a
//! Python extension that reads the Rust sources itself — would put a second
//! parser of `//!` blocks in the repository, and two parsers of one thing
//! disagree about it eventually. So the parsing happens here, once, and the
//! result is written out as JSON that the extension reads. One source, one
//! reader of it, two consumers of the reading.
//!
//! # How a chapter is made
//!
//! `docs/ARCHITECTURE.md` is already a curated sequence: nineteen sections,
//! each naming the decision record that settled it and each marked `built`,
//! `partly built` or `planned`. That is a table of contents somebody wrote by
//! hand, so the book does not invent a second one.
//!
//! A module joins a chapter by naming the same record:
//!
//! ```text
//!   //! Turning desired and observed state into a plan, in one pass.
//!   //!
//!   //! Expands: 0006
//! ```
//!
//! The record number is the join key because it is the only identifier here that
//! cannot change: rule 1 makes an accepted record immutable, while a section
//! heading is prose somebody may improve. The check runs both ways, in the
//! spirit of `// ENFORCES:` — a module naming a record that has no section is
//! reported, and so is a record that does not exist at all.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;

use crate::{lint, shell};

/// One section of `docs/ARCHITECTURE.md`.
#[derive(Debug, Clone)]
struct Chapter {
    /// The decision record it names, as four digits.
    record: String,
    /// The heading, without the record link or the marker.
    title: String,
    /// `built`, `partly built` or `planned`.
    status: String,
    /// The prose under the heading.
    summary: String,
}

/// One module that asked to expand a chapter.
#[derive(Debug, Clone)]
struct Module {
    /// Repository-relative path, so a reader can find it.
    path: String,
    /// The record it names.
    record: String,
    /// Its `//!` block, with the marker line removed.
    doc: String,
}

/// Reads the sections of the architecture, in the order they are written.
fn chapters(text: &str) -> Vec<Chapter> {
    let mut found: Vec<Chapter> = Vec::new();

    for line in text.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            let record = heading
                .split_once("](")
                .and_then(|(before, _)| before.rsplit('[').next())
                .filter(|digits| digits.len() == 4 && digits.chars().all(|ch| ch.is_ascii_digit()))
                .unwrap_or_default()
                .to_owned();

            let status = ["partly built", "built", "planned"]
                .iter()
                .find(|marker| heading.ends_with(*marker))
                .map(|marker| (*marker).to_owned())
                .unwrap_or_default();

            let title = heading
                .split(" — ")
                .next()
                .unwrap_or(heading)
                .trim_end_matches(&status)
                .trim_end_matches(['·', ' '])
                .trim()
                .to_owned();

            found.push(Chapter {
                record,
                title,
                status,
                summary: String::new(),
            });
            continue;
        }

        // Everything until the next heading is the section's own prose.
        if let Some(current) = found.last_mut() {
            let _ = writeln!(current.summary, "{line}");
        }
    }

    found
}

/// The `//!` block at the top of a source file, and the record it expands.
///
/// Returns `None` for a module that did not ask to be a chapter. Most modules
/// do not: the book is a curated thing, and a file that is only plumbing has
/// nothing to say to a reader who does not already have the source open.
fn module(path: &str, text: &str) -> Option<Module> {
    let mut record = None;
    let mut doc = String::new();

    for line in text.lines() {
        let Some(body) = line.strip_prefix("//!") else {
            // The block ends at the first line that is not part of it.
            if line.trim().is_empty() {
                continue;
            }
            break;
        };
        let body = body.strip_prefix(' ').unwrap_or(body);

        if let Some(named) = body.trim().strip_prefix("Expands: ") {
            record = Some(named.trim().to_owned());
            continue;
        }
        let _ = writeln!(doc, "{body}");
    }

    record.map(|record| Module {
        path: path.to_owned(),
        record,
        doc: demoted(&delinked(doc.trim())),
    })
}

/// Rewrites the links rustdoc understands into links a book can follow.
///
/// A `//!` block is written for `cargo doc`, where `[`Address`]` resolves
/// against the crate's item tree and `../../../docs/decisions` walks out of
/// `target/doc`. Neither means anything to a renderer that has never heard of
/// Rust, and leaving them produces a page full of links that go nowhere — worse
/// than no link, because a reader follows one before finding out.
///
/// Both become plain text for now, which is the honest rendering while the API
/// reference is not shipped beside the book. When it is, this is the one place
/// that has to learn the mapping.
fn delinked(doc: &str) -> String {
    let mut out = String::with_capacity(doc.len());
    let mut rest = doc;

    while let Some(open) = rest.find('[') {
        let (before, tail) = rest.split_at(open);
        out.push_str(before);

        let Some(close) = tail.find(']') else {
            out.push_str(tail);
            return out;
        };
        let label = tail.get(1..close).unwrap_or_default();
        let after = tail.get(close + 1..).unwrap_or_default();

        // `[label](target)` — keep it only if the target is a real address.
        if let Some(rest_of_target) = after.strip_prefix('(')
            && let Some(end) = rest_of_target.find(')')
        {
            let target = rest_of_target.get(..end).unwrap_or_default();
            let onward = rest_of_target.get(end + 1..).unwrap_or_default();
            if target.starts_with("http://") || target.starts_with("https://") {
                let _ = write!(out, "[{label}]({target})");
            } else {
                out.push_str(label);
            }
            rest = onward;
            continue;
        }

        // `[`Address`]` — a rustdoc intra-doc link, which is a bare label.
        out.push_str(label);
        rest = after;
    }

    out.push_str(rest);
    out
}

/// Pushes a module's own headings under the chapter's.
///
/// A `//!` block is the top of its own document, so its sections start at `#`.
/// In a chapter that is already `#`, and the module's path is `##`, those become
/// a second title and the page has three of them. Two levels down puts them
/// where a reader expects.
fn demoted(doc: &str) -> String {
    let mut out = String::with_capacity(doc.len());
    let mut fenced = false;

    for line in doc.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        // A `#` inside a fence is a shell prompt or a comment, not a heading.
        if !fenced && line.starts_with('#') {
            out.push_str("##");
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Both directions of the join between chapters and the modules that expand them.
///
/// Returns one line per complaint. Empty means the book can be assembled.
fn audit(chapters: &[Chapter], modules: &[Module], records: &[String]) -> Vec<String> {
    let mut complaints = Vec::new();

    for chapter in chapters {
        if chapter.status.is_empty() {
            complaints.push(format!(
                "docs/ARCHITECTURE.md: {:?} carries no build marker",
                chapter.title
            ));
        }
    }

    for module in modules {
        if !records.contains(&module.record) {
            complaints.push(format!(
                "{}: expands {:?}, which is not a decision record",
                module.path, module.record
            ));
            continue;
        }
        if !chapters
            .iter()
            .any(|chapter| chapter.record == module.record)
        {
            complaints.push(format!(
                "{}: expands {}, which has no section in docs/ARCHITECTURE.md",
                module.path, module.record
            ));
        }
    }

    complaints
}

/// The book as one document, for something that is not this program to render.
fn index(chapters: &[Chapter], modules: &[Module]) -> String {
    let mut by_record: BTreeMap<&str, Vec<&Module>> = BTreeMap::new();
    for module in modules {
        by_record.entry(&module.record).or_default().push(module);
    }

    let mut out = String::from("{\n  \"chapters\": [\n");
    for (position, chapter) in chapters.iter().enumerate() {
        if position > 0 {
            out.push_str(",\n");
        }
        let expanded = by_record.get(chapter.record.as_str());
        let _ = write!(
            out,
            "    {{\n      \"record\": {},\n      \"title\": {},\n      \
             \"status\": {},\n      \"summary\": {},\n      \"modules\": [",
            quoted(&chapter.record),
            quoted(&chapter.title),
            quoted(&chapter.status),
            quoted(&delinked(chapter.summary.trim())),
        );
        for (index, module) in expanded.into_iter().flatten().enumerate() {
            if index > 0 {
                out.push(',');
            }
            let _ = write!(
                out,
                "\n        {{\"path\": {}, \"doc\": {}}}",
                quoted(&module.path),
                quoted(&module.doc)
            );
        }
        out.push_str("\n      ]\n    }");
    }
    out.push_str("\n  ]\n}\n");
    out
}

/// A JSON string, written out rather than pulled in.
///
/// `serde_json` is not a dependency of this crate and does not become one for a
/// hundred lines of output: the alternative is a build tool that has to compile
/// half of serde before it can tell anybody their commit message is wrong.
fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if u32::from(ch) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(ch));
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// Assembles the book's index, and refuses to if the join does not hold.
pub(crate) fn check() -> Result<(), String> {
    let root = shell::root();

    let architecture = fs::read_to_string(root.join("docs/ARCHITECTURE.md"))
        .map_err(|error| format!("docs/ARCHITECTURE.md: {error}"))?;
    let chapters = chapters(&architecture);

    let records: Vec<String> = fs::read_dir(root.join("docs/decisions"))
        .map_err(|error| format!("docs/decisions: {error}"))?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.split_once('-')
                .map(|(digits, _)| digits.to_owned())
                .filter(|digits| digits.len() == 4)
        })
        .collect();

    let modules: Vec<Module> = lint::library_sources()
        .into_iter()
        .filter_map(|(path, text)| module(&lint::display(&path), &text))
        .collect();

    let complaints = audit(&chapters, &modules, &records);
    if !complaints.is_empty() {
        return Err(complaints.join("\n"));
    }

    let destination = root.join("target/book");
    fs::create_dir_all(&destination).map_err(|error| format!("target/book: {error}"))?;
    let written = destination.join("index.json");
    fs::write(&written, index(&chapters, &modules))
        .map_err(|error| format!("{}: {error}", written.display()))?;

    let placed = chapters
        .iter()
        .filter(|chapter| modules.iter().any(|module| module.record == chapter.record))
        .count();
    eprintln!(
        "  book: {} chapters, {placed} with modules, {} modules placed",
        chapters.len(),
        modules.len()
    );
    Ok(())
}

// Every rule is shown something that breaks it. A check nobody has watched fail
// is not a check.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests may be blunt: an unwrap is an expectation and an index is a claim about shape"
)]
mod tests {
    use super::{Chapter, Module, audit, chapters, module, quoted};

    const ARCHITECTURE: &str = "\
# Architecture

Some preamble.

## A single planning pass — [0006](decisions/0006-a.md) · built

Planning happens once.

## Renaming — [0028](decisions/0028-b.md) · planned

Not built.
";

    #[test]
    fn a_section_gives_up_its_record_title_status_and_prose() {
        let found = chapters(ARCHITECTURE);
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].record, "0006");
        assert_eq!(found[0].title, "A single planning pass");
        assert_eq!(found[0].status, "built");
        assert!(found[0].summary.contains("Planning happens once"));
        assert_eq!(found[1].status, "planned");
    }

    #[test]
    fn partly_built_is_not_shortened_to_built() {
        let found = chapters("## A thing — [0011](decisions/0011-a.md) · partly built\n");
        assert_eq!(found[0].status, "partly built");
        assert_eq!(found[0].title, "A thing");
    }

    #[test]
    fn a_module_that_asks_for_a_chapter_hands_over_its_documentation() {
        let found = module(
            "crates/nerpa-core/src/planner.rs",
            "//! Turning state into a plan.\n//!\n//! Expands: 0006\n//!\n//! # Why\n\nuse std::fs;\n",
        )
        .unwrap();
        assert_eq!(found.record, "0006");
        assert!(found.doc.starts_with("Turning state into a plan."));
        assert!(found.doc.contains("# Why"));
        assert!(!found.doc.contains("Expands"), "{:?}", found.doc);
    }

    #[test]
    fn a_module_that_did_not_ask_is_not_in_the_book() {
        assert!(module("a.rs", "//! Plumbing, and no more.\n\nuse std::fs;\n").is_none());
    }

    #[test]
    fn a_module_naming_a_record_that_does_not_exist_is_reported() {
        let modules = vec![Module {
            path: "a.rs".to_owned(),
            record: "9999".to_owned(),
            doc: String::new(),
        }];
        let complaints = audit(&chapters(ARCHITECTURE), &modules, &["0006".to_owned()]);
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(
            complaints[0].contains("not a decision record"),
            "{complaints:?}"
        );
    }

    #[test]
    fn a_record_with_no_section_cannot_be_expanded() {
        let modules = vec![Module {
            path: "a.rs".to_owned(),
            record: "0013".to_owned(),
            doc: String::new(),
        }];
        let records = vec!["0006".to_owned(), "0013".to_owned(), "0028".to_owned()];
        let complaints = audit(&chapters(ARCHITECTURE), &modules, &records);
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(complaints[0].contains("no section"), "{complaints:?}");
    }

    #[test]
    fn a_section_without_a_marker_is_reported() {
        let unmarked = vec![Chapter {
            record: "0006".to_owned(),
            title: "A thing".to_owned(),
            status: String::new(),
            summary: String::new(),
        }];
        let complaints = audit(&unmarked, &[], &["0006".to_owned()]);
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(complaints[0].contains("no build marker"), "{complaints:?}");
    }

    #[test]
    fn a_join_that_holds_says_nothing() {
        let modules = vec![Module {
            path: "a.rs".to_owned(),
            record: "0006".to_owned(),
            doc: String::new(),
        }];
        let records = vec!["0006".to_owned(), "0028".to_owned()];
        assert_eq!(
            audit(&chapters(ARCHITECTURE), &modules, &records),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_rustdoc_link_becomes_the_words_it_was_wrapped_around() {
        assert_eq!(
            super::delinked("see [`Address`] for it"),
            "see `Address` for it"
        );
        assert_eq!(
            super::delinked("see [0011](../../../docs/decisions) now"),
            "see 0011 now"
        );
        assert_eq!(super::delinked("[`crate::contract`]"), "`crate::contract`");
    }

    #[test]
    fn a_modules_own_headings_go_under_the_chapters() {
        let out = super::demoted("Summary.\n\n# Why one pass\n\nBecause.\n");
        assert!(out.contains("### Why one pass"), "{out}");
        assert!(out.starts_with("Summary."), "{out}");
    }

    #[test]
    fn a_hash_inside_a_fence_is_not_a_heading() {
        let out = super::demoted("```console\n# apt install nerpa\n```\n");
        assert!(out.contains("\n# apt install"), "{out}");
    }

    #[test]
    fn a_real_address_survives() {
        assert_eq!(
            super::delinked("[docs](https://example.invalid/a) here"),
            "[docs](https://example.invalid/a) here"
        );
    }

    #[test]
    fn text_with_no_links_is_returned_whole() {
        assert_eq!(
            super::delinked("plain prose, no brackets"),
            "plain prose, no brackets"
        );
        assert_eq!(
            super::delinked("an unclosed [bracket"),
            "an unclosed [bracket"
        );
    }

    #[test]
    fn what_json_cannot_hold_raw_is_escaped() {
        assert_eq!(quoted("a \"b\" \\ c\nd"), "\"a \\\"b\\\" \\\\ c\\nd\"");
        assert_eq!(quoted("tab\there"), "\"tab\\there\"");
    }
}
