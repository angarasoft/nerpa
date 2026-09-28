//! Which machines there are, and what is written down about them.
//!
//! A document rather than part of the program, because the fleet changes without
//! the configuration changing and one configuration runs against several fleets.
//! It is read before evaluation, so what it says is data by the time the program
//! sees it — which is what makes `for host in group("web")` legal where
//! iterating a fact is not. See decision 0025.
//!
//! ```toml
//! [nodes.web-01]
//! groups = ["web", "production"]
//! values = { workers = 16 }
//!
//! [groups.web]
//! port = 443
//! workers = 4
//! ```
//!
//! A machine reached over `ssh` says so, and may say which configuration file
//! holds the port, the jump host and the key — a generated one is not the file
//! `ssh` reads on its own:
//!
//! ```toml
//! [nodes.web-01]
//! ssh = "web-01.example.com"
//! ssh_config = "/home/somebody/.tbot/nerpa/ssh_config"
//! become = "sudo -n"
//! ```
//!
//! # Two groups, one key
//!
//! An error naming both, not a precedence resolution. Ansible answers "where did
//! this value come from" with a table of twenty-two levels; here the question has
//! one answer per value and a collision is a thing somebody has to decide, not a
//! thing a rule decides for them. A node may override its groups, because that is
//! a statement rather than a rank.

use std::collections::{BTreeMap, BTreeSet};

use nerpa_core::{Target, Value as CoreValue};

/// What is written down about the machines.
#[derive(Debug, Default, Clone)]
pub struct Fleet {
    nodes: BTreeMap<Target, Machine>,
    groups: BTreeMap<String, BTreeSet<Target>>,
}

/// One machine, and what belongs to it.
#[derive(Debug, Default, Clone)]
struct Machine {
    groups: Vec<String>,
    values: BTreeMap<String, CoreValue>,
    route: Route,
}

/// How a machine is reached.
///
/// A property of the node rather than of the protocol, which is decision 0035:
/// the executor is a filter that knows nothing about how it was reached, so
/// where it runs and what it runs under are settled before the first byte of
/// wire. Escalation is an argument here and appears nowhere else.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum Route {
    /// It is the machine the engine is running on.
    #[default]
    Here,
    /// Reached by running `ssh`.
    Ssh {
        /// What to give `ssh` — a host, or an alias its own configuration knows.
        host: String,
        /// Which `ssh` configuration says how to get there, if not the usual one.
        ///
        /// The port, the jump host and the key belong to `ssh` and are not
        /// repeated here; this says which file holds them. It exists because a
        /// generated one — Teleport's `tbot` writes an `ssh_config` carrying a
        /// `ProxyCommand` and a short-lived certificate — is not the file `ssh`
        /// would read on its own, and pointing at it is the whole of what an
        /// identity-aware proxy needs from this document.
        config: Option<String>,
        /// Where the executor is over there, if somebody wrote it down.
        ///
        /// `None` means put one there: a path in the document is a statement
        /// about that machine which this has no business overriding.
        executor: Option<String>,
        /// What to run it under, one word to an entry: `["sudo", "-n"]`.
        escalate: Vec<String>,
    },
}

impl Fleet {
    /// A fleet with nothing in it, for a configuration that names its machines
    /// itself.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Reads a fleet from the text of a document.
    ///
    /// # Errors
    ///
    /// Returns a message if the document is not the shape above, if a value is
    /// of a kind the model cannot carry, or if two groups give one node the same
    /// key.
    #[dacc_derive::doc_anchor(id = "inv-fleet-001")]
    pub fn read(text: &str) -> Result<Self, String> {
        let document: toml::Value =
            toml::from_str(text).map_err(|error| format!("the fleet is not TOML: {error}"))?;

        let mut groups: BTreeMap<String, BTreeMap<String, CoreValue>> = BTreeMap::new();
        if let Some(table) = document.get("groups").and_then(toml::Value::as_table) {
            for (name, values) in table {
                groups.insert(name.clone(), read_values(name, values)?);
            }
        }

        let mut fleet = Self::default();
        let Some(nodes) = document.get("nodes").and_then(toml::Value::as_table) else {
            return Ok(fleet);
        };

        for (name, described) in nodes {
            let node = Target::new(name).map_err(|error| format!("{name:?}: {error}"))?;
            let belongs: Vec<String> = described
                .get("groups")
                .and_then(toml::Value::as_array)
                .map(|listed| {
                    listed
                        .iter()
                        .filter_map(|entry| entry.as_str().map(ToOwned::to_owned))
                        .collect()
                })
                .unwrap_or_default();

            for group in &belongs {
                if !groups.contains_key(group) {
                    return Err(format!(
                        "{name} is in group {group:?}, which nothing describes"
                    ));
                }
                fleet
                    .groups
                    .entry(group.clone())
                    .or_default()
                    .insert(node.clone());
            }

            let mut values: BTreeMap<String, CoreValue> = BTreeMap::new();
            let mut whence: BTreeMap<String, String> = BTreeMap::new();
            for group in &belongs {
                for (key, value) in groups.get(group).into_iter().flatten() {
                    if let Some(first) = whence.get(key) {
                        return Err(format!(
                            "{name} is in {first:?} and {group:?}, and both give it {key:?}; \
                             one of them has to stop, or the node has to say which"
                        ));
                    }
                    whence.insert(key.clone(), group.clone());
                    values.insert(key.clone(), value.clone());
                }
            }

            // Last, and silently: a node saying what it is is a statement, not a
            // rank in a table nobody can remember.
            if let Some(own) = described.get("values") {
                values.extend(read_values(name, own)?);
            }

            fleet.nodes.insert(
                node,
                Machine {
                    groups: belongs,
                    values,
                    route: read_route(name, described)?,
                },
            );
        }
        Ok(fleet)
    }

    /// Every machine in a group, in a stable order.
    ///
    /// `None` where nothing describes the group, which is a different answer to
    /// a group nobody is in.
    #[must_use]
    pub(crate) fn group(&self, name: &str) -> Option<Vec<Target>> {
        if !self.groups.contains_key(name)
            && !self
                .nodes
                .values()
                .any(|machine| machine.groups.iter().any(|held| held == name))
        {
            return None;
        }
        Some(
            self.groups
                .get(name)
                .map(|members| members.iter().cloned().collect())
                .unwrap_or_default(),
        )
    }

    /// How to reach a machine.
    ///
    /// A machine the document does not describe is this one, which is what a
    /// configuration naming its own nodes means by `node("localhost")`.
    #[must_use]
    pub fn route(&self, node: &Target) -> Route {
        self.nodes
            .get(node)
            .map(|machine| machine.route.clone())
            .unwrap_or_default()
    }

    /// Everything written down about a machine.
    #[must_use]
    pub(crate) fn values_of(&self, node: &Target) -> BTreeMap<String, CoreValue> {
        self.nodes
            .get(node)
            .map(|machine| machine.values.clone())
            .unwrap_or_default()
    }

    /// How many machines it describes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether it describes none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// How a machine says it is reached.
fn read_route(name: &str, described: &toml::Value) -> Result<Route, String> {
    let Some(host) = described.get("ssh") else {
        return Ok(Route::Here);
    };
    let host = host
        .as_str()
        .ok_or_else(|| format!("{name}: `ssh` has to be a host or an alias"))?
        .to_owned();
    // `ssh` reads a leading `-` as another option rather than as a host, and
    // `-oProxyCommand=…` runs a command on *this* machine before any other is
    // contacted. A fleet is a document that gets swapped, shared, and one day
    // generated from a cloud inventory, so a name out of one must not be able
    // to reach the machine doing the planning.
    if host.starts_with('-') {
        return Err(format!(
            "{name}: `ssh` is {host:?}, which `ssh` would read as an option \
             rather than as a host"
        ));
    }

    let config = described
        .get("ssh_config")
        .map(|given| {
            given
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| format!("{name}: `ssh_config` has to be a path"))
        })
        .transpose()?;

    let executor = described
        .get("executor")
        .map(|given| {
            let path = given
                .as_str()
                .ok_or_else(|| format!("{name}: `executor` has to be a path"))?;
            plainly(name, "executor", path)?;
            // A remote shell resolves anything but an absolute path from where the
            // connection starts, which is the connecting user's home, and `~` is
            // that home by name. Either is a path that user writes, and with
            // `become` a binary that user writes is root for them (0039). The
            // characters were checked above; what they spell is checked here.
            if !path.starts_with('/')
                || path.contains('~')
                || path.split('/').any(|part| part == "." || part == "..")
            {
                return Err(format!(
                    "{name}: `executor` is {path:?}, and it has to be an absolute path \
                     with no `~`, `.` or `..` in it: a remote shell resolves anything \
                     else from a directory the connecting user writes"
                ));
            }
            Ok::<String, String>(path.to_owned())
        })
        .transpose()?;

    // Split rather than passed as one string, because a shell string is where
    // somebody else's quoting becomes somebody else's command.
    let escalate = described.get("become").map_or_else(
        || Ok::<Vec<String>, String>(Vec::new()),
        |given| {
            let written = given.as_str().ok_or_else(|| {
                format!("{name}: `become` has to be a command such as \"sudo -n\"")
            })?;
            let words: Vec<String> = written.split_whitespace().map(ToOwned::to_owned).collect();
            for word in &words {
                plainly(name, "become", word)?;
            }
            Ok(words)
        },
    )?;

    // An executor this delivers goes to a path the connecting user writes, and
    // an executor that user can write is one they can run as root — so
    // `NOPASSWD` on a delivered executor is full root wearing a narrow rule,
    // and pretending otherwise is worse than saying so. Refused here rather
    // than discovered by somebody reading a sudoers file two years from now.
    if !escalate.is_empty() && executor.is_none() {
        return Err(format!(
            "{name}: `become` needs an `executor` that root put there. Nerpa \
             delivers one into the connecting user's own directory, and letting \
             that user run it under sudo gives them root by another name — the \
             rule would say one path and mean everything. Install it once \
             (`/usr/local/sbin/nerpa-executor`, owned by root) and name it \
             here"
        ));
    }

    Ok(Route::Ssh {
        host,
        config,
        executor,
        escalate,
    })
}

/// Refuses anything a remote shell would read as more than one word.
///
/// `ssh` joins whatever trails the host into one string and hands it to a shell
/// on the far side — its own doc comment in the transport says so — so a path
/// holding a `;` is two commands there, and quoting here would only look like
/// protection. Rather than quote, refuse: everything these fields legitimately
/// hold is a path or a program name, and neither needs a character a shell
/// treats as punctuation.
fn plainly(whose: &str, field: &str, word: &str) -> Result<(), String> {
    const PLAIN: fn(char) -> bool = |character: char| {
        character.is_ascii_alphanumeric() || matches!(character, '/' | '.' | '-' | '_' | '~' | '=')
    };
    if word.is_empty() || !word.chars().all(PLAIN) {
        return Err(format!(
            "{whose}: `{field}` is {word:?}, and a remote shell would read that \
             as more than the one word it is meant to be"
        ));
    }
    Ok(())
}

/// A table of values, in the vocabulary a resource can carry.
fn read_values(whose: &str, table: &toml::Value) -> Result<BTreeMap<String, CoreValue>, String> {
    let Some(table) = table.as_table() else {
        return Err(format!("{whose}: values have to be a table"));
    };
    table
        .iter()
        .map(|(key, value)| Ok((key.clone(), as_core(whose, key, value)?)))
        .collect()
}

fn as_core(whose: &str, key: &str, value: &toml::Value) -> Result<CoreValue, String> {
    Ok(match value {
        toml::Value::String(text) => CoreValue::Text(text.clone()),
        toml::Value::Integer(number) => CoreValue::Integer(*number),
        toml::Value::Boolean(flag) => CoreValue::Boolean(*flag),
        toml::Value::Array(items) => CoreValue::List(
            items
                .iter()
                .map(|item| as_core(whose, key, item))
                .collect::<Result<_, _>>()?,
        ),
        toml::Value::Table(entries) => CoreValue::Map(
            entries
                .iter()
                .map(|(inner, item)| Ok((inner.clone(), as_core(whose, inner, item)?)))
                .collect::<Result<_, String>>()?,
        ),
        // A date is a value this model has no room for, and turning one into text
        // would decide a format nobody asked for.
        other => {
            return Err(format!(
                "{whose}: {key:?} is {}, which a resource cannot carry",
                other.type_str()
            ));
        }
    })
}
