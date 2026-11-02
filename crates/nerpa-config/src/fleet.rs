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
//!
//! # Who may sign a plan
//!
//! Where a deployment separates who writes a change from who may make it, the
//! fleet names the approvers: an OpenSSH `allowed_signers` file, the one
//! "where configured" in `0009` means. A fleet that names none asks for no
//! signature, and one that names one refuses every apply without one.
//!
//! ```toml
//! [approval]
//! approvers = "/etc/nerpa/approvers"
//! ```
//!
//! # Who may touch what
//!
//! The grants the fleet issued: an identity to what it may touch. A run whose
//! `--as` names an identity here carries this grant, and the engine refuses a
//! change that reaches outside it (`0009`, `0072`). A fleet that names none
//! issued no grant, and a machine it granted no grant refuses to act.
//!
//! ```toml
//! [grants]
//! web-ops = "kind=instance;target=web-01,web-02;op=create,update,delete"
//! ```

use std::collections::{BTreeMap, BTreeSet};

use nerpa_core::{Grant, Target, Value as CoreValue};

/// What is written down about the machines.
#[derive(Debug, Default, Clone)]
pub struct Fleet {
    nodes: BTreeMap<Target, Machine>,
    groups: BTreeMap<String, BTreeSet<Target>>,
    /// Where who may sign a plan is written down, for a deployment that keeps
    /// the two duties apart (`0009`, `0067`).
    approvers: Option<String>,
    /// The grants this fleet issued: an identity to what it may touch
    /// (`0009`, `0072`).
    grants: BTreeMap<String, Grant>,
    /// The cloud a run touches, if the document names one.
    cloud: Option<Cloud>,
}

/// What the control domain is written about: the cloud folder a run touches,
/// and how it proves who it is.
///
/// The `Debug` output says whether a token is there and prints none of it
/// (adr-2026-079).
#[derive(Clone)]
pub struct Cloud {
    /// The Yandex Cloud folder.
    pub folder_id: String,
    /// An IAM token, when the run hands one over. Absent, the credential is the
    /// yc profile (adr-2026-071). A plain string for now; it becomes a secret
    /// reference when the secret store of `0027` arrives.
    pub iam_token: Option<String>,
    /// The path of a service-account key, when the run proves who it is with one
    /// (adr-2026-079). The key is in the file, never in the fleet document. A
    /// run names this or an `iam_token`, not both.
    pub key_file: Option<String>,
}

impl std::fmt::Debug for Cloud {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cloud")
            .field("folder_id", &self.folder_id)
            .field("iam_token", &self.iam_token.as_ref().map(|_| "<redacted>"))
            .field("key_file", &self.key_file)
            .finish()
    }
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
        refuse_unread_tables(&document)?;

        let mut groups: BTreeMap<String, BTreeMap<String, CoreValue>> = BTreeMap::new();
        if let Some(table) = table(&document, "groups")? {
            for (name, values) in table {
                groups.insert(name.clone(), read_values(name, values)?);
            }
        }

        let mut fleet = Self {
            approvers: read_approval(&document)?,
            grants: read_grants(&document)?,
            cloud: read_cloud(&document)?,
            ..Self::default()
        };
        let Some(nodes) = table(&document, "nodes")? else {
            return Ok(fleet);
        };

        for (name, described) in nodes {
            let node = Target::new(name).map_err(|error| format!("{name:?}: {error}"))?;
            // The fields of a node are a closed set. A `key` written here would
            // be a static key in by the back door — the one thing `INV-TRUST-001`
            // says does not exist — and silently ignoring it would leave a
            // document that says something no run does.
            for field in described
                .as_table()
                .into_iter()
                .flatten()
                .map(|(key, _)| key)
            {
                if !matches!(
                    field.as_str(),
                    "groups" | "values" | "ssh" | "ssh_config" | "executor" | "become"
                ) {
                    return Err(format!(
                        "{name}: {field:?} is not a field of a node — the port, the jump \
                         host and the key belong to `ssh` and are named in `ssh_config`"
                    ));
                }
            }
            let belongs: Vec<String> = described
                .get("groups")
                .and_then(toml::Value::as_array)
                .map_or_else(
                    // No groups field is a node in no groups.
                    Vec::new,
                    |listed| {
                        listed
                            .iter()
                            .filter_map(|entry| entry.as_str().map(ToOwned::to_owned))
                            .collect()
                    },
                );

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
        Some(self.groups.get(name).map_or_else(
            // The name may be held by nodes alone, in which case the group
            // itself lists nobody.
            Vec::new,
            |members| members.iter().cloned().collect(),
        ))
    }

    /// How to reach a machine.
    ///
    /// A machine the document does not describe is this one, which is what a
    /// configuration naming its own nodes means by `node("localhost")`.
    #[must_use]
    pub fn route(&self, node: &Target) -> Route {
        self.nodes.get(node).map_or_else(
            // A node the fleet does not describe is reached here, which is
            // where a configuration naming its own machines expects it.
            Route::default,
            |machine| machine.route.clone(),
        )
    }

    /// Where the deployment says who may sign a plan, if it says so.
    ///
    /// An `allowed_signers` file: the "where configured" of `0009`. Absent is
    /// a deployment with no second party, which asks for no signature.
    #[must_use]
    pub fn approvers(&self) -> Option<&str> {
        self.approvers.as_deref()
    }

    /// The cloud a run touches, if the document names one.
    #[must_use]
    pub fn cloud(&self) -> Option<&Cloud> {
        self.cloud.as_ref()
    }

    /// The grant this fleet issued to an identity, if it issued one (`0072`).
    #[must_use]
    pub fn grant(&self, who: &str) -> Option<&Grant> {
        self.grants.get(who)
    }

    /// Everything written down about a machine.
    #[must_use]
    pub(crate) fn values_of(&self, node: &Target) -> BTreeMap<String, CoreValue> {
        self.nodes.get(node).map_or_else(
            // A node the fleet does not describe carries no values.
            BTreeMap::new,
            |machine| machine.values.clone(),
        )
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

/// The tables the fleet document is read from. The document is a closed set, as
/// a node's fields are: a table outside this list is refused and not skipped,
/// because a fleet that names no approvers asks for no signature, and a
/// misspelt `[approval]` would turn the requirement off and say nothing.
const TABLES: &[&str] = &["nodes", "groups", "approval", "grants", "cloud"];

/// The clouds this build has a provider for. A cloud outside this list is named
/// in the refusal rather than skipped.
const CLOUDS: &[&str] = &["yandex"];

/// Refuses a top-level table the fleet does not read.
fn refuse_unread_tables(document: &toml::Value) -> Result<(), String> {
    let Some(tables) = document.as_table() else {
        return Ok(());
    };
    for name in tables.keys() {
        if !TABLES.contains(&name.as_str()) {
            return Err(format!(
                "{name:?} is not a table of the fleet — it reads {}, and a table it does not \
                 read is refused, because a misspelt `approval` would ask for no signature",
                list(TABLES)
            ));
        }
    }
    Ok(())
}

/// The names as a list a person can read: `a`, `b` and `c`.
fn list(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [only] => format!("`{only}`"),
        [init @ .., last] => {
            let init: Vec<String> = init.iter().map(|name| format!("`{name}`")).collect();
            format!("{} and `{last}`", init.join(", "))
        }
    }
}

/// The table `name` of the document, if it has one. A part that is there and is
/// not a table is refused: skipping it would leave a document that says
/// something no run does.
fn table<'a>(document: &'a toml::Value, name: &str) -> Result<Option<&'a toml::Table>, String> {
    document
        .get(name)
        .map(|value| {
            value
                .as_table()
                .ok_or_else(|| format!("{name}: has to be a table, written as `[{name}]`"))
        })
        .transpose()
}

/// The cloud a run touches, as `[cloud.yandex]`. Its fields are a closed set,
/// like a node's: a field silently ignored would leave a document that says
/// something no run does. A cloud the build has no provider for is named in the
/// refusal.
fn read_cloud(document: &toml::Value) -> Result<Option<Cloud>, String> {
    let Some(cloud) = table(document, "cloud")? else {
        return Ok(None);
    };
    if let Some(name) = cloud.keys().find(|name| !CLOUDS.contains(&name.as_str())) {
        return Err(format!(
            "cloud.{name}: this build has no provider for a cloud by that name — the clouds \
             it has are {}",
            list(CLOUDS)
        ));
    }
    let Some(yandex) = cloud.get("yandex") else {
        return Ok(None);
    };
    let yandex = yandex
        .as_table()
        .ok_or_else(|| "cloud.yandex: has to be a table, written as `[cloud.yandex]`".to_owned())?;
    for (field, _) in yandex {
        if !matches!(field.as_str(), "folder_id" | "iam_token" | "key_file") {
            return Err(refused_cloud_field(field));
        }
    }
    let folder_id = yandex
        .get("folder_id")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| "cloud.yandex: `folder_id` is required".to_owned())?;
    let iam_token = yandex
        .get("iam_token")
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "cloud.yandex: `iam_token` must be text".to_owned())
        })
        .transpose()?;
    let key_file = yandex
        .get("key_file")
        .map(|value| {
            value
                .as_str()
                .filter(|path| !path.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| {
                    "cloud.yandex: `key_file` must be the path of a service-account key".to_owned()
                })
        })
        .transpose()?;
    if iam_token.is_some() && key_file.is_some() {
        return Err(
            "cloud.yandex: name one credential — an `iam_token` or a `key_file`, \
                    not both"
                .to_owned(),
        );
    }
    Ok(Some(Cloud {
        folder_id: folder_id.to_owned(),
        iam_token,
        key_file,
    }))
}

/// What the fleet says to a field of the cloud it does not take.
///
/// A long-lived credential is refused by name, in any spelling of the name, and
/// says what to write instead: the fleet document is read by people and checked
/// into repositories, and a credential that does not expire has no business in
/// one (adr-2026-079). The value is never repeated.
fn refused_cloud_field(field: &str) -> String {
    let name: String = field
        .chars()
        .filter(|&c| c != '_' && c != '-')
        .flat_map(char::to_lowercase)
        .collect();
    match name.as_str() {
        "apikey" | "secret" | "secretkey" | "accesskey" | "statictoken" | "oauthtoken"
        | "token" => format!(
            "cloud.yandex: {field:?} is a long-lived credential and is refused; name a \
             service-account key with `key_file`, which is exchanged for a short-lived \
             token, or hand over a short-lived `iam_token` (adr-2026-079)"
        ),
        "privatekey" | "serviceaccountkey" | "key" => format!(
            "cloud.yandex: {field:?} would put a key into the fleet document, and a key \
             does not belong in one; name the file it is in with `key_file` (adr-2026-079)"
        ),
        _ => format!(
            "cloud.yandex: {field:?} is not a field of the cloud — a run needs its \
             `folder_id` and one credential, an `iam_token` or a `key_file`, and that is \
             the whole of it"
        ),
    }
}

/// Where who may sign a plan is written down.
///
/// The table's fields are a closed set, like a node's: a field silently
/// ignored would leave a document that says something no run does.
fn read_approval(document: &toml::Value) -> Result<Option<String>, String> {
    let Some(table) = table(document, "approval")? else {
        return Ok(None);
    };
    for (field, _) in table {
        if field != "approvers" {
            return Err(format!(
                "approval: {field:?} is not a field of approval — who may sign is \
                 an `allowed_signers` file, and that is the whole of it"
            ));
        }
    }
    match table.get("approvers") {
        Some(named) => named.as_str().map(str::to_owned).map(Some).ok_or_else(|| {
            "approval: `approvers` has to be a path to an `allowed_signers` file".to_owned()
        }),
        None => {
            Err("approval: say who may sign — `approvers = \"/etc/nerpa/approvers\"`".to_owned())
        }
    }
}

/// The grants this fleet issued: an identity to what it may touch (`0072`).
fn read_grants(document: &toml::Value) -> Result<BTreeMap<String, Grant>, String> {
    let Some(table) = table(document, "grants")? else {
        return Ok(BTreeMap::new());
    };
    table
        .iter()
        .map(|(who, granted)| {
            let granted = granted
                .as_str()
                .ok_or_else(|| format!("grants: `{who}` has to be a grant, not a table"))?;
            let grant = granted
                .parse::<Grant>()
                .map_err(|problem| format!("grants: `{who}`: {problem}"))?;
            Ok((who.clone(), grant))
        })
        .collect()
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
