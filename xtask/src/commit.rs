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

/// Strips the wrappers git puts round a subject it did not invent, leaving the
/// subject underneath to be judged like any other.
///
/// An earlier version of this took the wrappers as proof that git had written
/// the line, and exempted the whole message. It was wrong, and an independent
/// review defeated the entire check with `fixup! anything at all.` — five
/// literal strings that nothing stopped a person from typing. Nothing eats a
/// `fixup!` automatically either: `rebase --autosquash` is a command somebody
/// runs, not a law of nature.
///
/// So the wrapper is removed rather than believed. `git commit --fixup` produces
/// `fixup! <the target's own subject>`, and `git revert` produces
/// `Revert "<the original subject>"` — in both cases what is left is a subject
/// this repository already accepted, so checking it is exactly right and a typed
/// wrapper round nonsense is not.
///
/// A merge is the one that cannot be checked this way, because git invents the
/// text. It is settled by [`Situation`] instead: the repository either is in the
/// middle of a merge or it is not, and that is not a matter of what was typed.
fn unwrapped(subject: &str) -> &str {
    let mut subject = subject;
    // Bounded: `fixup! fixup! …` is legitimate, an unbounded loop over attacker
    // controlled text is not.
    for _ in 0..8 {
        let stripped = ["fixup! ", "squash! ", "amend! "]
            .iter()
            .find_map(|prefix| subject.strip_prefix(prefix))
            .or_else(|| {
                subject
                    .strip_prefix("Revert \"")
                    .and_then(|rest| rest.strip_suffix('"'))
            });
        match stripped {
            Some(rest) => subject = rest.trim_start(),
            None => return subject,
        }
    }
    subject
}

/// What the repository is doing, which decides whether a merge subject is one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Situation {
    /// An ordinary commit. `Merge …` is then just words somebody typed.
    Ordinary,
    /// A merge is in progress, so git wrote the subject and its shape is not
    /// ours to insist on.
    Merging,
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
fn judge(message: &str, situation: Situation) -> Review {
    let mut review = Review::default();

    let Some((index, written)) = subject_of(message) else {
        review.refuse("the commit message is empty");
        return review;
    };

    // Git writes a merge subject itself, and only when a merge is actually under
    // way. Typing the word proves nothing.
    if situation == Situation::Merging {
        return review;
    }
    if written.starts_with("Merge ") {
        review.refuse(
            "this looks like a merge subject, and no merge is in progress. \
             Git writes those itself; type an ordinary subject instead",
        );
        return review;
    }

    let subject = unwrapped(written);

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

    // Measured on what actually lands in the log, wrapper and all.
    let width = written.chars().count();
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

/// Whether a merge is under way, asked of the repository rather than of the text.
///
/// `MERGE_HEAD` exists for exactly as long as a merge is in progress, which is
/// the only moment a merge subject is one.
fn situation() -> Situation {
    let git_dir = shell::git(&["rev-parse", "--git-dir"])
        .map(|found| found.trim().to_owned())
        .unwrap_or_default();
    if git_dir.is_empty() {
        return Situation::Ordinary;
    }
    if std::path::Path::new(&git_dir).join("MERGE_HEAD").exists() {
        Situation::Merging
    } else {
        Situation::Ordinary
    }
}

/// The work items and slices of this tree, by the names their records carry.
fn plan_ids(root: &std::path::Path) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    for dir in ["work", "slice"] {
        if let Ok(entries) = fs::read_dir(root.join("doc").join(dir)) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if let Some(stem) = name.to_string_lossy().strip_suffix(".rs") {
                    out.insert(stem.to_owned());
                }
            }
        }
    }
    out
}

/// The basis: the message names a work item of this tree (INV-PILOT-003).
///
/// The trailer is read from the message and the record from the tree, so the
/// two have to agree: a message without its basis is refused, and so is one
/// whose basis names a record this tree does not hold.
#[dacc_derive::doc_anchor(id = "the-commit-gate-checks-the-basis")]
pub(crate) fn basis(message: &str, records: &std::collections::BTreeSet<String>) -> Vec<String> {
    let named = message.lines().find_map(|line| {
        line.strip_prefix("Dacc-Work: ")
            .or_else(|| line.strip_prefix("Dacc-Slice: "))
            .map(str::trim)
            .filter(|id| !id.is_empty())
    });
    match named {
        None => {
            vec![
                "the message carries no Dacc-Work or Dacc-Slice trailer naming its basis"
                    .to_owned(),
            ]
        }
        Some(id) if records.contains(id) => Vec::new(),
        Some(id) => vec![format!("{id} names no work item or slice of this tree")],
    }
}

/// Checks a message file, or `HEAD` when given none.
pub(crate) fn check(path: Option<&str>) -> Result<(), String> {
    let message = match path {
        Some(path) => fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?,
        None => shell::git(&["log", "-1", "--pretty=%B"])?,
    };

    let situation = situation();
    let review = judge(&message, situation);
    for warning in &review.warnings {
        eprintln!("  commit-msg: {warning}");
    }
    let mut refusals = review.refusals;
    // A merge's message is git's own, and the tree it names is the merge that
    // is under way: the basis rule is for messages somebody wrote.
    if matches!(situation, Situation::Ordinary) {
        refusals.extend(basis(&message, &plan_ids(&shell::root())));
    }
    if refusals.is_empty() {
        eprintln!("  commit-msg: subject well formed");
        return Ok(());
    }

    let mut report = refusals.join("\n");
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
    use super::{Situation, basis, judge};

    /// A message without its basis is refused, and one naming a work item this
    /// tree does not hold is refused by name (INV-PILOT-003).
    #[dacc_derive::doc_anchor(id = "a-basis-less-message-is-refused")]
    #[test]
    fn a_basis_less_message_is_refused() {
        let records: std::collections::BTreeSet<String> =
            ["w-obligations".to_owned()].into_iter().collect();
        let complaints = basis("[FIX] os: a thing\n", &records);
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(complaints[0].contains("no Dacc-Work"), "{complaints:?}");
    }

    #[dacc_derive::doc_anchor(id = "a-basis-naming-no-work-item-is-refused")]
    #[test]
    fn a_basis_naming_no_work_item_is_refused() {
        let records: std::collections::BTreeSet<String> =
            ["w-obligations".to_owned()].into_iter().collect();
        let complaints = basis("[FIX] os: a thing\n\nDacc-Work: w-nothing-here\n", &records);
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(complaints[0].contains("w-nothing-here"), "{complaints:?}");
    }

    /// The two trailers are read alike, and a record of this tree settles it.
    #[test]
    fn a_named_basis_of_this_tree_is_accepted() {
        let records: std::collections::BTreeSet<String> =
            ["w-obligations".to_owned(), "s-dacc-050".to_owned()]
                .into_iter()
                .collect();
        for message in [
            "[FIX] os: a thing\n\nDacc-Work: w-obligations\n",
            "[FIX] os: a thing\n\nDacc-Slice: s-dacc-050\n",
        ] {
            assert_eq!(basis(message, &records), Vec::<String>::new(), "{message}");
        }
    }

    fn refusals(message: &str) -> Vec<String> {
        judge(message, Situation::Ordinary).refusals
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
        let review = judge(&long, Situation::Ordinary);
        assert!(review.refusals.is_empty(), "{review:?}");
        assert_eq!(review.warnings.len(), 1, "{review:?}");

        let longer = format!("[DOCS] core: {}", "a".repeat(90));
        assert!(!judge(&longer, Situation::Ordinary).refusals.is_empty());
    }

    #[test]
    fn what_git_actually_writes_is_left_alone() {
        // `git revert` and `git commit --fixup` wrap a subject this repository
        // already accepted, so the wrapper comes off and the rest is judged.
        for subject in [
            "Revert \"[FEAT] core: something\"",
            "fixup! [FEAT] core: something",
            "squash! [FIX] os: something",
            "fixup! fixup! [FEAT] core: something",
        ] {
            assert_eq!(refusals(subject), Vec::<String>::new(), "{subject}");
        }
    }

    #[test]
    fn a_wrapper_typed_round_nonsense_is_not_a_way_past_the_check() {
        // The hole an independent review found: five literal prefixes used to
        // exempt the whole message, and nothing stopped a person typing one.
        for subject in [
            "fixup! anything at all.",
            "Revert \"nothing, just testing the gate\"",
            "squash! no type here either",
        ] {
            assert!(!refusals(subject).is_empty(), "{subject} slipped through");
        }
    }

    #[test]
    fn a_merge_subject_counts_only_while_a_merge_is_happening() {
        let subject = "Merge branch 'develop' into main";
        assert_eq!(
            judge(subject, Situation::Merging).refusals,
            Vec::<String>::new()
        );

        let complaints = refusals(subject);
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(
            complaints[0].contains("no merge is in progress"),
            "{complaints:?}"
        );
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
