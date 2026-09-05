//! The shape of a commit subject, enforced where it is written.
//!
//! # Why a gate rather than a paragraph
//!
//! Twenty-five commits into this repository the history already spelled one type
//! two ways — `[DOC]` twice and `[docs]` three times — for the same act. Nobody
//! decided that; it is what happens when the convention lives in prose and the
//! writer, human or otherwise, is going from memory. The list of types is worth
//! having only if it is closed, and a list is only closed if something closes it.
//!
//! Reachable as `cargo xtask commit-msg`, and wired into `.githooks/commit-msg`
//! by `cargo xtask hooks`. Like every other check here it is a pure function
//! over text, so it can be handed a fixture and asked to go red.
//!
//! # What is refused, and what is only mentioned
//!
//! Refused: the `[TYPE] ` shape, a type from the closed list, a scope that looks
//! like one, a subject that is not empty and does not end in a full stop, and a
//! blank line before any body.
//!
//! Mentioned: a subject over 72 characters. Six of the first twenty-five are
//! over it and the longest is 84, so refusing at 72 would reject a quarter of
//! established practice — that is a wall, not a gate, and people climb walls. The
//! hard limit sits at 90, above everything written so far, so it caps the drift
//! without rejecting the past. Bring the practice down before tightening it.

use std::fs;

use crate::shell;

/// Every type a commit may have. Closed on purpose: an open list is a list of
/// synonyms waiting to happen.
///
/// | Type | What it marks |
/// | --- | --- |
/// | `FEAT` | Something the tool can do that it could not do before |
/// | `FIX` | Behaviour that was wrong and is not now |
/// | `PERF` | The same behaviour, measurably cheaper |
/// | `CHANGE` | Different on purpose: a policy, a default, a name, a boundary |
/// | `DOCS` | Writing only. No behaviour moves |
/// | `TEST` | Tests only, including fixtures that prove a gate can fail |
/// | `CI` | The gate itself, the pipelines, the hooks |
/// | `CHORE` | Dependencies, versions, housekeeping nobody looks up later |
const TYPES: [&str; 8] = [
    "FEAT", "FIX", "PERF", "CHANGE", "DOCS", "TEST", "CI", "CHORE",
];

/// What somebody wrote, and what they meant by it.
///
/// The mapping is the useful half of this check: an author who guessed is told
/// which token to use, not merely that they were wrong.
const INSTEAD: [(&str, &str); 8] = [
    ("DOC", "DOCS"),
    ("DOCUMENTATION", "DOCS"),
    ("FEATURE", "FEAT"),
    ("BUGFIX", "FIX"),
    ("REFACTOR", "CHANGE"),
    ("STYLE", "CHANGE"),
    ("BUILD", "CHORE"),
    ("DEPS", "CHORE"),
];

/// Where a subject stops being comfortable, and where it stops being allowed.
const COMFORTABLE: usize = 72;
const REFUSED: usize = 90;

/// What the check found: what it will not accept, and what it merely mentions.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Review {
    refusals: Vec<String>,
    warnings: Vec<String>,
}

impl Review {
    fn refuse(&mut self, complaint: impl Into<String>) {
        self.refusals.push(complaint.into());
    }
}

/// Whether git itself will consume this message, so its shape is not ours.
///
/// A merge or a revert is written by git, and `fixup!` and friends are eaten by
/// `rebase --autosquash` before anybody reads them.
fn git_writes_it(subject: &str) -> bool {
    ["Merge ", "Revert ", "fixup!", "squash!", "amend!"]
        .iter()
        .any(|prefix| subject.starts_with(prefix))
}

/// The first line that is neither blank nor one of git's own comments.
///
/// `COMMIT_EDITMSG` arrives full of `#` lines, and the subject is whatever
/// survives them.
fn subject_of(message: &str) -> Option<(usize, &str)> {
    message
        .lines()
        .enumerate()
        .find(|(_, line)| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|(index, line)| (index, line.trim_end()))
}

/// Judges one commit message, with no filesystem and no git in sight.
fn judge(message: &str) -> Review {
    let mut review = Review::default();

    let Some((index, subject)) = subject_of(message) else {
        review.refuse("the commit message is empty");
        return review;
    };
    if git_writes_it(subject) {
        return review;
    }

    // The body has to be separated from the subject by a blank line, because
    // every tool that shows a log assumes it and runs them together otherwise.
    if let Some(next) = message.lines().nth(index + 1)
        && !next.trim().is_empty()
        && !next.starts_with('#')
    {
        review.refuse("the line after the subject must be blank, then the body");
    }

    let Some(rest) = subject.strip_prefix('[') else {
        review.refuse(format!(
            "{subject:?} does not start with a type: write `[TYPE] subject`, \
             one of {}",
            TYPES.join(", ")
        ));
        return review;
    };
    let Some((kind, rest)) = rest.split_once(']') else {
        review.refuse(format!(
            "{subject:?} opens a type with `[` and never closes it"
        ));
        return review;
    };

    if !TYPES.contains(&kind) {
        let suggestion = INSTEAD
            .iter()
            .find(|(wrong, _)| wrong.eq_ignore_ascii_case(kind))
            .map(|(_, right)| (*right).to_owned())
            .or_else(|| {
                // The commonest mistake by far, and worth its own sentence: the
                // right word in the wrong case.
                TYPES
                    .iter()
                    .find(|known| known.eq_ignore_ascii_case(kind))
                    .map(|known| (*known).to_owned())
            });
        match suggestion {
            Some(right) => review.refuse(format!("[{kind}] is not a type here — use [{right}]")),
            None => review.refuse(format!(
                "[{kind}] is not a type here. The list is closed: {}",
                TYPES.join(", ")
            )),
        }
    }

    let Some(after) = rest.strip_prefix(' ') else {
        review.refuse("there must be exactly one space after `]`");
        return review;
    };
    if after.starts_with(' ') {
        review.refuse("there must be exactly one space after `]`");
    }

    // A scope is optional, and it is whatever sits before the first `: `.
    let described = match after.split_once(": ") {
        Some((scope, described)) if scope_shaped(scope) => {
            if scope.is_empty() {
                review.refuse("the scope before `:` is empty");
            }
            described
        }
        _ => after,
    };

    if described.trim().is_empty() {
        review.refuse("the subject says nothing after the type and scope");
        return review;
    }
    if described.ends_with('.') {
        review.refuse("the subject does not end in a full stop");
    }

    let width = subject.chars().count();
    if width > REFUSED {
        review.refuse(format!(
            "the subject is {width} characters, and {REFUSED} is the most that is read \
             before a log truncates it"
        ));
    } else if width > COMFORTABLE {
        review.warnings.push(format!(
            "the subject is {width} characters; {COMFORTABLE} reads better"
        ));
    }

    review
}

/// Whether this looks like a scope rather than the first words of a sentence.
///
/// `os`, `core`, `nerpa-cli`, `0023`. Anything with a space in it is prose that
/// happened to contain a colon, and taking it for a scope would then complain
/// about the rest of the sentence.
fn scope_shaped(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '.')
}

/// Checks a message file, or `HEAD` when given none.
pub(crate) fn check(path: Option<&str>) -> Result<(), String> {
    let message = match path {
        Some(path) => fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?,
        None => shell::git(&["log", "-1", "--pretty=%B"])?,
    };

    let review = judge(&message);
    for warning in &review.warnings {
        eprintln!("  commit-msg: {warning}");
    }
    if review.refusals.is_empty() {
        eprintln!("  commit-msg: subject well formed");
        return Ok(());
    }

    let mut report = review.refusals.join("\n");
    report.push_str(
        "\n\nShape: [TYPE] scope: subject — the scope is optional.\nSee docs/COMMITS.md.",
    );
    Err(report)
}

// Every rule above is shown a message that breaks it. A gate nobody has watched
// refuse anything is a gate nobody has grounds to trust.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests may be blunt: an unwrap is an expectation and an index is a claim about shape"
)]
mod tests {
    use super::judge;

    fn refusals(message: &str) -> Vec<String> {
        judge(message).refusals
    }

    #[test]
    fn the_shapes_this_repository_already_uses_are_accepted() {
        // Taken from the log rather than invented, so the gate is measured
        // against the practice it is meant to keep rather than against itself.
        for subject in [
            "[FIX] os: a file it may not read is not a file that is not there",
            "[FEAT] secrets: a value the model can carry and never write down",
            "[CHANGE] rename to nerpa",
            "[DOCS] 0023: choose the renderer by measurement, not reputation",
        ] {
            assert_eq!(refusals(subject), Vec::<String>::new(), "{subject}");
        }
    }

    #[test]
    fn the_two_spellings_of_one_type_are_told_which_one_to_use() {
        let complaints = refusals("[DOC] record the architecture");
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(complaints[0].contains("[DOCS]"), "{complaints:?}");
    }

    #[test]
    fn the_right_word_in_the_wrong_case_is_named_rather_than_listed() {
        let complaints = refusals("[docs] record the architecture");
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(complaints[0].contains("use [DOCS]"), "{complaints:?}");
    }

    #[test]
    fn a_type_nobody_has_a_use_for_gets_the_whole_list() {
        let complaints = refusals("[WIP] halfway through something");
        assert!(complaints[0].contains("list is closed"), "{complaints:?}");
        assert!(complaints[0].contains("CHORE"), "{complaints:?}");
    }

    #[test]
    fn a_subject_with_no_type_at_all_is_refused() {
        let complaints = refusals("fix the thing");
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(
            complaints[0].contains("does not start with a type"),
            "{complaints:?}"
        );
    }

    #[test]
    fn a_full_stop_at_the_end_is_refused() {
        let complaints = refusals("[FIX] os: stop reading through a symlink.");
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(complaints[0].contains("full stop"), "{complaints:?}");
    }

    #[test]
    fn a_body_running_straight_on_from_the_subject_is_refused() {
        let complaints = refusals("[FIX] os: a thing\nand the reason for it");
        assert!(
            complaints.iter().any(|line| line.contains("blank")),
            "{complaints:?}"
        );
    }

    #[test]
    fn a_body_after_a_blank_line_is_what_was_asked_for() {
        assert_eq!(
            refusals("[FIX] os: a thing\n\nand the reason for it\n"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_long_subject_is_mentioned_and_a_much_longer_one_is_refused() {
        let long = format!("[DOCS] core: {}", "a".repeat(70));
        let review = judge(&long);
        assert!(review.refusals.is_empty(), "{review:?}");
        assert_eq!(review.warnings.len(), 1, "{review:?}");

        let longer = format!("[DOCS] core: {}", "a".repeat(90));
        assert!(!judge(&longer).refusals.is_empty());
    }

    #[test]
    fn git_writes_its_own_merges_and_they_are_left_alone() {
        for subject in [
            "Merge branch 'develop' into main",
            "Revert \"[FEAT] core: something\"",
            "fixup! [FEAT] core: something",
        ] {
            assert_eq!(refusals(subject), Vec::<String>::new(), "{subject}");
        }
    }

    #[test]
    fn a_colon_in_prose_is_not_read_as_a_scope() {
        // Without this the words before the colon would be taken for a scope,
        // and the check would then complain about a scope with spaces in it.
        assert_eq!(
            refusals("[CHANGE] one thing changed: and here is what it was"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn the_comment_lines_git_fills_the_editor_with_are_skipped() {
        let message = "\n# Please enter the commit message.\n[FEAT] core: a thing\n";
        assert_eq!(refusals(message), Vec::<String>::new());
    }

    #[test]
    fn an_empty_message_is_refused_rather_than_ignored() {
        let complaints = refusals("\n# only comments\n");
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(complaints[0].contains("empty"), "{complaints:?}");
    }

    #[test]
    fn two_spaces_after_the_type_are_refused() {
        let complaints = refusals("[FEAT]  core: a thing");
        assert!(
            complaints.iter().any(|line| line.contains("one space")),
            "{complaints:?}"
        );
    }
}
