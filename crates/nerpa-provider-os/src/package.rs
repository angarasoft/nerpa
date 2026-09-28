//! What a package is, per family, and nothing else.
//!
//! Every difference between distributions that this provider knows about lives
//! here. That is on purpose: `W-010` says the control machine holds each
//! distribution's semantics and that being wrong about one is discovered
//! remotely, so the least this can do is keep them in one file where they can be
//! read side by side.
//!
//! Two families, chosen to stand as far apart as the space allows, and a refusal
//! by name for anything else — see
//! `0038`.
//!
//! # Measured, not assumed
//!
//! Every command here was run against a real machine before it was written down.
//!
//! * `rpm -q --queryformat '%{VERSION}-%{RELEASE}\n' bash` answers `5.1.8-9.el9`
//!   and exits 0; for a package that is not there it exits 1 and says so.
//! * `portageq match / dev-lang/python` answers with one line per installed
//!   slot and exits 0. For a package that is not there it answers **nothing and
//!   still exits 0** — so the two families do not agree even about how absence
//!   is reported, and the code below does not pretend they do.
//! * `qlist -ICv dev-lang/python` matches by substring and answers
//!   `dev-lang/python-exec` as well. It is the wrong instrument and is not used.

use nerpa_core::value::Value;
use nerpa_proto::{Observation, Operation, Seen};

/// A family of package management, as far as this provider knows one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    /// `rpm` to look, `dnf` to act. Prebuilt packages inside a transaction.
    Rhel,
    /// `portageq` to look, `emerge` to act. Built from source, and a name is
    /// `category/name`.
    Portage,
}

impl Family {
    /// What a machine calling itself this is managed by, if anything here knows.
    ///
    /// `None` is the answer for every family this provider has not been taught,
    /// and the caller turns it into a refusal naming the machine and what it
    /// said rather than a command sent on the chance that it works.
    pub(crate) fn of(said: &str) -> Option<Self> {
        match said {
            "rhel" | "fedora" | "centos" => Some(Self::Rhel),
            "gentoo" => Some(Self::Portage),
            _ => None,
        }
    }

    /// What to look at to find out whether a package is installed.
    ///
    /// One observation per package, which the engine sends in the same journey
    /// as everything else it is asking this machine.
    pub(crate) fn query(self, atom: &str) -> Observation {
        let (program, arguments) = match self {
            Self::Rhel => (
                "rpm",
                vec![
                    "-q".to_owned(),
                    "--queryformat".to_owned(),
                    "%{VERSION}-%{RELEASE}\n".to_owned(),
                    atom.to_owned(),
                ],
            ),
            // `/` is the root to ask about, which portageq takes as an argument
            // rather than assuming.
            Self::Portage => (
                "portageq",
                vec!["match".to_owned(), "/".to_owned(), atom.to_owned()],
            ),
        };
        Observation::Output {
            program: program.to_owned(),
            arguments,
        }
    }

    /// What the answer means: the installed version, or `None` for not there.
    ///
    /// The two families disagree about how to say "not installed" — `rpm` exits
    /// 1, `portageq` exits 0 and says nothing — so each is read on its own
    /// terms. Anything neither shape covers is an error rather than a guess.
    pub(crate) fn installed(self, atom: &str, seen: &Seen) -> Result<Option<String>, String> {
        let Seen::Said { status, out, .. } = seen else {
            return Err(format!("{atom}: the machine did not run the query"));
        };
        let said = String::from_utf8_lossy(out);
        match self {
            Self::Rhel => match status {
                0 => Ok(Some(said.trim().to_owned())),
                1 => Ok(None),
                other => Err(format!("{atom}: rpm exited {other}")),
            },
            Self::Portage => {
                if *status != 0 {
                    return Err(format!("{atom}: portageq exited {status}"));
                }
                // One line per installed slot, each `category/name-version`.
                // The first is enough here: a declaration that means one slot
                // says which slot, and the atom carries it.
                let Some(line) = said
                    .lines()
                    .next()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                else {
                    return Ok(None);
                };
                let stem = atom.split(':').next().unwrap_or(atom);
                line.strip_prefix(stem)
                    .and_then(|rest| rest.strip_prefix('-'))
                    .map(|version| Some(version.to_owned()))
                    .ok_or_else(|| {
                        format!("{atom}: portageq answered {line:?}, which does not name it")
                    })
            }
        }
    }

    /// What to run to make the package present.
    ///
    /// A provider's word, not a configuration author's: nothing in a
    /// declaration reaches this, and the arguments are separate words that never
    /// meet a shell. See decision 0033.
    pub(crate) fn install(self, atom: &str, version: &str) -> Operation {
        // A version is written down or the declaration does not have one, and
        // "the newest there is" is refused by 0038 — so pinning is the ordinary
        // case rather than the exception, and each family spells it its own way.
        //
        // `--oneshot` on purpose: it keeps the package out of portage's `world`
        // set, so the declaration stays the only record of what was asked for.
        // Two records of the same thing drift, and the one nerpa does not write
        // would drift silently.
        let (program, arguments) = match self {
            Self::Rhel => (
                "dnf",
                vec![
                    "--assumeyes".to_owned(),
                    "--quiet".to_owned(),
                    "install".to_owned(),
                    format!("{atom}-{version}"),
                ],
            ),
            // `=` is portage's exact-version operator, and the slot the atom may
            // already carry goes after the version rather than before it.
            Self::Portage => {
                let (stem, slot) = match atom.split_once(':') {
                    Some((stem, slot)) => (stem, format!(":{slot}")),
                    None => (atom, String::new()),
                };
                (
                    "emerge",
                    vec![
                        "--quiet".to_owned(),
                        "--oneshot".to_owned(),
                        format!("={stem}-{version}{slot}"),
                    ],
                )
            }
        };
        Operation::Process {
            program: program.to_owned(),
            arguments,
            stdin: Vec::new(),
            private: false,
            expect: vec![0],
        }
    }

    /// What to run to make it absent.
    ///
    /// Portage gets `--depclean`, which refuses to remove a package something
    /// else still needs, and refuses one recorded in the `world` set. Neither
    /// refusal is worked around: a package nerpa installed is never in `world`
    /// (see `install`), so the only thing this cannot remove is one somebody
    /// put there by hand — and `--deselect`ing it would edit a record nerpa does
    /// not own. The read-back then reports that it is still installed, which is
    /// the truth.
    ///
    /// `dnf remove` is the opposite and worse: it takes everything that depends
    /// on the package with it, so a plan saying "remove one" can remove twelve.
    /// That is `W-019` and is not solved here.
    pub(crate) fn remove(self, atom: &str) -> Operation {
        let (program, arguments) = match self {
            Self::Rhel => (
                "dnf",
                vec![
                    "--assumeyes".to_owned(),
                    "--quiet".to_owned(),
                    "remove".to_owned(),
                    atom.to_owned(),
                ],
            ),
            Self::Portage => (
                "emerge",
                vec![
                    "--quiet".to_owned(),
                    "--depclean".to_owned(),
                    atom.to_owned(),
                ],
            ),
        };
        Operation::Process {
            program: program.to_owned(),
            arguments,
            stdin: Vec::new(),
            private: false,
            expect: vec![0],
        }
    }
}

/// What the provider reports about a package that is there.
pub(crate) fn as_attributes(version: &str) -> Vec<(String, Value)> {
    vec![("version".to_owned(), Value::Text(version.to_owned()))]
}
