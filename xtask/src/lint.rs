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

pub(crate) fn repository_rules() -> Result<(), String> {
    let sources = library_sources();
    let mut complaints = Vec::new();

    for (path, text) in &sources {
        let shown = display(path);
        complaints.extend(bare_unwrap(&shown, text));
        complaints.extend(missing_module_docs(&shown, text));
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
    use super::{bare_unwrap, missing_module_docs};

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
}
