//! What a machine turns out to be.
//!
//! A fact is a reading like any other, gathered in the same journey as
//! everything else on that machine — decision 0029 needed no new machinery for
//! collecting them, only for what they may decide.
//!
//! # Why the set is closed
//!
//! Ansible gathers hundreds and pays for it on every run. Here each fact is a
//! question somebody wrote down, with what to ask and how to read the answer, so
//! the cost of gathering is the length of this list rather than the length of
//! what a machine could be asked. Growing it is a change to this file, which is
//! the point: an unbounded set is one nobody prices.
//!
//! What is not here is an extension mechanism. A provider that wants to
//! contribute facts is a decision nobody has taken, and inventing one before
//! anything needs it is what `0030` asks this project to stop doing.

use nerpa_proto::{Observation, Seen};

/// One thing a machine can be asked about itself.
pub(crate) struct Question {
    /// What the fact is called in a configuration.
    pub(crate) name: &'static str,
    /// What to ask.
    pub(crate) ask: fn() -> Observation,
    /// What the answer means, or `None` if it means nothing usable.
    pub(crate) read: fn(&Seen) -> Option<String>,
}

/// Everything a machine is asked about itself.
pub(crate) const ASKED: [Question; 2] = [
    Question {
        name: "arch",
        ask: || Observation::Output {
            program: "uname".to_owned(),
            arguments: vec!["-m".to_owned()],
        },
        read: |seen| said(seen).map(|text| text.trim().to_owned()),
    },
    Question {
        name: "os_family",
        ask: || Observation::Contents {
            path: "/etc/os-release".to_owned(),
        },
        read: |seen| family(seen),
    },
];

/// What a program printed, if it ran and printed text.
fn said(seen: &Seen) -> Option<String> {
    match seen {
        Seen::Said { status: 0, out, .. } => String::from_utf8(out.clone()).ok(),
        _ => None,
    }
}

/// The distribution family, from `/etc/os-release`.
///
/// `ID_LIKE` first and `ID` after it: a derivative says what it is like, and what
/// a configuration wants to branch on is almost always the family rather than
/// the particular spin. A file that says neither yields nothing, which reads as
/// a machine that was not asked rather than as a machine of no family.
fn family(seen: &Seen) -> Option<String> {
    let Seen::Entry(nerpa_proto::Entry::File { content, .. }) = seen else {
        return None;
    };
    let text = String::from_utf8(content.clone()).ok()?;

    let field = |key: &str| {
        text.lines()
            .filter_map(|line| line.strip_prefix(key))
            .map(|value| value.trim_matches('"').trim().to_owned())
            .next()
    };

    field("ID_LIKE=")
        .and_then(|likes| likes.split_whitespace().next().map(ToOwned::to_owned))
        .or_else(|| field("ID="))
        .filter(|found| !found.is_empty())
}
