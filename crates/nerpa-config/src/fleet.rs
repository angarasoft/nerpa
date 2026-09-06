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
    // ENFORCES: INV-FLEET-001
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
