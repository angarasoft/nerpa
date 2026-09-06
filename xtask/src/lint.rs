//! The rules `clippy` does not know about, because they are this project's own.
//!
//! Each one is written down in `docs/CODING_STANDARDS.md` and enforced here. A
//! standard that is only written down is a suggestion; the point of putting the
//! tool first is that the two cannot drift apart.

use std::fs;
use std::path::{Path, PathBuf};

use crate::shell;

/// Every `.rs` file in the workspace, tests included.
pub(crate) fn all_sources() -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    for crate_root in ["crates", "xtask"] {
        walk(&shell::root().join(crate_root), &mut found);
    }
    found
}

/// Every `.rs` file that is part of a library or binary, excluding tests.
///
/// Tests get more latitude: a test that panics is reporting a failure, and a
/// test that unwraps is stating an expectation.
pub(crate) fn library_sources() -> Vec<(PathBuf, String)> {
    let mut found = all_sources();
    found.retain(|(path, _)| {
        !path
            .components()
            .any(|component| component.as_os_str() == "tests")
    });
    found
}

fn walk(directory: &Path, found: &mut Vec<(PathBuf, String)>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            walk(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && let Ok(text) = fs::read_to_string(&path)
        {
            found.push((path, text));
        }
    }
}

/// A panic must unwind, so that the guard holding the state lock and the one
/// holding an open transport get to run their destructors.
///
/// This is rule 2, and until now it was held by a comment in the manifest asking
/// nobody to undo it. An independent review pointed out what this project's own
/// thesis already says: an obligation with nothing behind it is skipped. It is
/// one line to check, and the consequence of missing it is a lock file left on a
/// machine nobody is watching and a half-applied change nobody recorded.
fn unwinds_on_panic(manifest: &str) -> Vec<String> {
    manifest
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let line = line.split('#').next().unwrap_or_default();
            let squashed: String = line.chars().filter(|ch| !ch.is_whitespace()).collect();
            squashed.starts_with("panic=") && squashed.contains("abort")
        })
        .map(|(index, _)| {
            format!(
                "Cargo.toml:{}: aborting on panic leaves the state lock behind — rule 2",
                index + 1
            )
        })
        .collect()
}

/// Module documentation is written impersonally, and carries nothing unfinished.
///
/// The voice was already the convention — one first person plural in seven
/// hundred lines, and that one inside a quoted phrase — so this writes down what
/// was already true rather than imposing a taste. It looks at `//!` blocks only.
/// Decision records are immutable by rule 1, so a gate over them could never be
/// anything but a list of things nobody is allowed to fix.
///
/// Quoted text is left alone. `no field means "we will work this out during
/// apply"` is the document reporting somebody else's voice, not speaking in it,
/// and a check that cannot tell the difference would be wrong on the one example
/// this repository actually contains.
fn header_prose(path: &str, text: &str) -> Vec<String> {
    let mut complaints = Vec::new();

    for (index, line) in text.lines().enumerate() {
        let Some(body) = line.strip_prefix("//!") else {
            if line.trim().is_empty() {
                continue;
            }
            break;
        };

        let outside_quotes: String = {
            let mut kept = String::with_capacity(body.len());
            let mut quoted = false;
            for ch in body.chars() {
                if ch == '"' {
                    quoted = !quoted;
                    continue;
                }
                kept.push(if quoted { ' ' } else { ch });
            }
            kept
        };

        let words = outside_quotes
            .to_lowercase()
            .split(|ch: char| !ch.is_ascii_alphabetic())
            .map(str::to_owned)
            .collect::<Vec<_>>();

        for pronoun in ["we", "our", "ours"] {
            if words.iter().any(|word| word == pronoun) {
                complaints.push(format!(
                    "{path}:{}: module documentation is impersonal — {pronoun:?} \
                     names a speaker the reader cannot see",
                    index + 1
                ));
            }
        }
        for unfinished in ["TODO", "TBD", "FIXME"] {
            if body.contains(unfinished) {
                complaints.push(format!(
                    "{path}:{}: {unfinished} in a module header. A header is a \
                     chapter of the book; leave the note in the code or the watchlist",
                    index + 1
                ));
            }
        }
    }

    complaints
}

pub(crate) fn repository_rules() -> Result<(), String> {
    let sources = library_sources();
    let mut complaints = Vec::new();

    let manifest = shell::root().join("Cargo.toml");
    let text = fs::read_to_string(&manifest).map_err(|error| format!("Cargo.toml: {error}"))?;
    complaints.extend(unwinds_on_panic(&text));

    for (path, text) in &sources {
        let shown = display(path);
        complaints.extend(bare_unwrap(&shown, text));
        complaints.extend(missing_module_docs(&shown, text));
        complaints.extend(header_prose(&shown, text));
    }

    if complaints.is_empty() {
        eprintln!("  repository rules: {} files, clean", sources.len());
        return Ok(());
    }
    Err(complaints.join("\n"))
}

/// `unwrap()` says "this cannot fail" and gives nobody a reason to believe it.
/// `expect("...")` says the same thing and leaves the reason in the panic
/// message, where whoever hits it at three in the morning will read it.
fn bare_unwrap(path: &str, text: &str) -> Vec<String> {
    // Assembled rather than written out, so that this line does not report
    // itself. The rule still applies to this file like any other.
    let needle = concat!(".unwrap", "()");

    code_lines(text)
        .filter(|(_, line)| line.contains(needle) && !line.trim_start().starts_with("//"))
        .map(|(index, line)| {
            format!(
                "{path}:{}: bare unwrap() — use expect(\"why this cannot fail\")\n    {}",
                index + 1,
                line.trim()
            )
        })
        .collect()
}

/// Everything up to an inline test module.
///
/// Tests get the same latitude here as tests in a `tests/` directory do: an
/// unwrap is an expectation and a fixture may hold anything at all. This assumes
/// the convention that `#[cfg(test)] mod tests` comes last in a file, which is
/// where Rust puts it and where `cargo fmt` leaves it.
fn code_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines()
        .enumerate()
        .take_while(|(_, line)| line.trim() != "#[cfg(test)]")
}

/// A module without a `//!` header is a module whose purpose lives only in
/// somebody's head.
fn missing_module_docs(path: &str, text: &str) -> Vec<String> {
    let has_header = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .is_some_and(|line| line.trim_start().starts_with("//!"));

    if has_header {
        Vec::new()
    } else {
        vec![format!("{path}:1: no `//!` module documentation")]
    }
}

pub(crate) fn display(path: &Path) -> String {
    path.strip_prefix(shell::root())
        .unwrap_or(path)
        .display()
        .to_string()
}

// A rule nobody has watched fire is not a rule.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests may be blunt: an unwrap is an expectation and an index is a claim about shape"
)]
mod tests {
    use super::{bare_unwrap, header_prose, missing_module_docs, unwinds_on_panic};

    #[test]
    fn a_bare_unwrap_is_reported_with_its_line() {
        let source = "//! A module.\nfn main() {\n    let x = thing().unwrap();\n}\n";

        let complaints = bare_unwrap("src/thing.rs", source);
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(
            complaints[0].starts_with("src/thing.rs:3:"),
            "{complaints:?}"
        );
    }

    #[test]
    fn expect_and_unwrap_or_are_left_alone() {
        let source = "//! A module.\nlet a = x.expect(\"why\");\nlet b = y.unwrap_or_default();\n";

        assert!(bare_unwrap("src/thing.rs", source).is_empty());
    }

    #[test]
    fn an_unwrap_inside_a_comment_is_not_code() {
        let source = "//! A module.\n// never write x.unwrap() here\n";

        assert!(bare_unwrap("src/thing.rs", source).is_empty());
    }

    #[test]
    fn code_after_an_inline_test_module_is_not_code() {
        let source = concat!(
            "//! A module.\n",
            "fn real() { thing().unwrap(); }\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    fn a_test() { thing().unwrap(); }\n",
            "}\n"
        );

        let complaints = bare_unwrap("src/thing.rs", source);
        assert_eq!(
            complaints.len(),
            1,
            "only the line above the test module counts"
        );
        assert!(
            complaints[0].starts_with("src/thing.rs:2:"),
            "{complaints:?}"
        );
    }

    #[test]
    fn a_module_without_a_header_is_reported() {
        let complaints = missing_module_docs("src/thing.rs", "use std::fs;\n");
        assert_eq!(complaints.len(), 1, "{complaints:?}");
    }

    #[test]
    fn a_module_header_after_blank_lines_still_counts() {
        assert!(missing_module_docs("src/thing.rs", "\n\n//! A module.\n").is_empty());
    }

    #[test]
    fn aborting_on_panic_is_refused_wherever_it_is_written() {
        for manifest in [
            "[profile.release]\npanic = \"abort\"\n",
            "[profile.release]\npanic=\"abort\"\n",
            "[profile.release]\n  panic = 'abort'\n",
        ] {
            let complaints = unwinds_on_panic(manifest);
            assert_eq!(complaints.len(), 1, "{manifest:?} -> {complaints:?}");
            assert!(complaints[0].contains("rule 2"), "{complaints:?}");
        }
    }

    #[test]
    fn the_comment_asking_nobody_to_do_it_is_not_doing_it() {
        // The real manifest carries exactly this line, and it must stay quiet.
        let manifest = "# Do not set panic = \"abort\".\n";
        assert_eq!(unwinds_on_panic(manifest), Vec::<String>::new());
    }

    #[test]
    fn unwinding_says_nothing() {
        assert_eq!(
            unwinds_on_panic("[profile.release]\nlto = true\npanic = \"unwind\"\n"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_module_header_that_names_a_speaker_is_reported() {
        let complaints = header_prose("a.rs", "//! We decided this together.\n");
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(complaints[0].contains("impersonal"), "{complaints:?}");
    }

    #[test]
    fn a_quoted_voice_is_not_the_document_speaking() {
        // The one real example: a field that would say this, if it existed.
        let complaints = header_prose(
            "a.rs",
            "//! no field means \"we will work this out during apply\".\n",
        );
        assert_eq!(complaints, Vec::<String>::new(), "{complaints:?}");
    }

    #[test]
    fn a_word_that_merely_contains_a_pronoun_is_left_alone() {
        // "weight", "ourselves" as substrings, and "Owner" capitalised.
        let complaints = header_prose("a.rs", "//! The weight of the answer is ours to bear.\n");
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(complaints[0].contains("ours"), "{complaints:?}");
        assert_eq!(
            header_prose("a.rs", "//! The weight of it.\n"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn something_unfinished_in_a_header_is_reported() {
        let complaints = header_prose("a.rs", "//! Reads a plan. TODO: the rest.\n");
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(
            complaints[0].contains("chapter of the book"),
            "{complaints:?}"
        );
    }

    #[test]
    fn code_below_the_header_is_not_prose() {
        let complaints = header_prose("a.rs", "//! Impersonal.\n\nfn we_do_things() {}\n");
        assert_eq!(complaints, Vec::<String>::new(), "{complaints:?}");
    }
}
