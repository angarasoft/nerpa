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

    // Either quote, because `os-release` permits both and distributions differ:
    // Rocky writes `ID="rocky"` and Gentoo writes `ID='gentoo'`. Stripping only
    // one of them yielded a family called `'gentoo'`, quotes and all, which
    // matched nothing and refused a machine that was perfectly ordinary. Found
    // by reading a second distribution, which is the whole reason there are two.
    let field = |key: &str| {
        text.lines()
            .filter_map(|line| line.strip_prefix(key))
            .map(|value| value.trim().trim_matches(['"', '\'']).trim().to_owned())
            .next()
    };

    field("ID_LIKE=")
        .and_then(|likes| likes.split_whitespace().next().map(ToOwned::to_owned))
        .or_else(|| field("ID="))
        .filter(|found| !found.is_empty())
}

// The one thing here that parses somebody else's file, and the only place a
// second distribution has already caught a mistake.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "tests may be blunt: see docs/CODING_STANDARDS.md"
)]
mod tests {
    use super::family;
    use nerpa_proto::{Entry, Seen};

    fn os_release(text: &str) -> Seen {
        Seen::Entry(Entry::File {
            mode: 0o644,
            content: text.as_bytes().to_vec(),
        })
    }

    /// `os-release` permits either quote and distributions use both: Rocky
    /// writes `ID="rocky"`, Gentoo writes `ID='gentoo'`. Stripping one of them
    /// gave a family called `'gentoo'`, quotes and all, which matched nothing
    /// and refused a machine that was perfectly ordinary.
    #[test]
    fn either_quote_is_stripped_because_distributions_use_both() {
        assert_eq!(
            family(&os_release("ID='gentoo'\n")).as_deref(),
            Some("gentoo")
        );
        assert_eq!(
            family(&os_release("ID=\"rocky\"\n")).as_deref(),
            Some("rocky")
        );
        assert_eq!(
            family(&os_release("ID=debian\n")).as_deref(),
            Some("debian")
        );
    }

    /// What a machine is *like* comes first, because a provider cares about the
    /// family rather than the badge: Rocky says `ID="rocky"` and
    /// `ID_LIKE="rhel centos fedora"`, and it is the first of those that says
    /// how packages arrive.
    #[test]
    fn what_a_machine_is_like_wins_over_what_it_calls_itself() {
        let said = os_release("ID=\"rocky\"\nID_LIKE=\"rhel centos fedora\"\n");
        assert_eq!(family(&said).as_deref(), Some("rhel"));
    }

    #[test]
    fn a_file_that_says_nothing_useful_answers_nothing() {
        assert_eq!(family(&os_release("PRETTY_NAME=\"something\"\n")), None);
        assert_eq!(family(&os_release("ID=\"\"\n")), None);
        assert_eq!(family(&Seen::Nothing), None);
    }
}
