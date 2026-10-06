//! A grant limits a run to named kinds, targets and operations (adr-2026-009).

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use crate::address::{Address, Kind, Target};
use crate::plan::{Change, ChangeKind, Plan};
use crate::value;

/// What a run may touch, as a set of kinds, targets and operations.
///
/// An empty set is no restriction on that axis, so a grant that names nothing
/// allows everything — the default, before grants are written down.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Grant {
    /// The resource kinds the run may change.
    pub kinds: BTreeSet<Kind>,
    /// The targets — nodes — the run may change resources on.
    pub targets: BTreeSet<Target>,
    /// The operations the run may perform.
    pub operations: BTreeSet<ChangeKind>,
}

impl Grant {
    /// Whether a change at `address` of `kind` is within the grant.
    #[must_use]
    pub fn allows(&self, address: &Address, kind: ChangeKind) -> bool {
        if !self.kinds.is_empty() && !self.kinds.contains(address.kind()) {
            return false;
        }
        if !self.operations.is_empty() && !self.operations.contains(&kind) {
            return false;
        }
        if !self.targets.is_empty()
            && let Some(target) = address.target()
            && !self.targets.contains(target)
        {
            return false;
        }
        true
    }

    /// The first change in `plan` that reaches outside the grant, if any.
    #[must_use]
    pub fn refuses<'p>(&self, plan: &'p Plan) -> Option<&'p Change> {
        plan.changes()
            .find(|change| !self.allows(change.address(), change.kind()))
    }

    /// Encodes the grant for the plan number, so two grants cannot collide.
    pub(crate) fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&value::length(self.kinds.len()));
        for kind in &self.kinds {
            value::text(kind.as_str(), out);
        }
        out.extend_from_slice(&value::length(self.targets.len()));
        for target in &self.targets {
            value::text(target.as_str(), out);
        }
        out.extend_from_slice(&value::length(self.operations.len()));
        for operation in &self.operations {
            out.push(operation_tag(*operation));
        }
    }
}

impl fmt::Display for Grant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.kinds.is_empty() && self.targets.is_empty() && self.operations.is_empty() {
            return f.write_str("all");
        }
        let mut parts = Vec::new();
        if !self.kinds.is_empty() {
            parts.push(format!(
                "kind={}",
                self.kinds
                    .iter()
                    .map(Kind::as_str)
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        if !self.targets.is_empty() {
            parts.push(format!(
                "target={}",
                self.targets
                    .iter()
                    .map(Target::as_str)
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        if !self.operations.is_empty() {
            parts.push(format!(
                "op={}",
                self.operations
                    .iter()
                    .map(|operation| operation_name(*operation))
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        f.write_str(&parts.join(";"))
    }
}

impl FromStr for Grant {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut grant = Grant::default();
        for axis in text.split(';') {
            let axis = axis.trim();
            if axis.is_empty() || axis == "all" {
                continue;
            }
            let Some((key, values)) = axis.split_once('=') else {
                return Err(format!("grant axis {axis:?} is not key=value"));
            };
            match key.trim() {
                "kind" => {
                    for value in values.split(',') {
                        let value = value.trim();
                        if value.is_empty() {
                            continue;
                        }
                        grant
                            .kinds
                            .insert(Kind::new(value).map_err(|error| error.to_string())?);
                    }
                }
                "target" => {
                    for value in values.split(',') {
                        let value = value.trim();
                        if value.is_empty() {
                            continue;
                        }
                        grant
                            .targets
                            .insert(Target::new(value).map_err(|error| error.to_string())?);
                    }
                }
                "op" => {
                    for value in values.split(',') {
                        let value = value.trim();
                        if value.is_empty() {
                            continue;
                        }
                        let operation = parse_operation(value).ok_or_else(|| {
                            format!(
                                "grant operation {value:?} is not one of create, update, replace, delete, react, adopt, reorder"
                            )
                        })?;
                        grant.operations.insert(operation);
                    }
                }
                other => return Err(format!("grant axis {other:?} is not kind, target or op")),
            }
        }
        Ok(grant)
    }
}

fn parse_operation(text: &str) -> Option<ChangeKind> {
    match text {
        "create" => Some(ChangeKind::Create),
        "update" => Some(ChangeKind::Update),
        "replace" => Some(ChangeKind::Replace),
        "delete" => Some(ChangeKind::Delete),
        "react" => Some(ChangeKind::React),
        "adopt" => Some(ChangeKind::Adopt),
        "reorder" => Some(ChangeKind::Reorder),
        _ => None,
    }
}

fn operation_name(operation: ChangeKind) -> &'static str {
    match operation {
        ChangeKind::Create => "create",
        ChangeKind::Update => "update",
        ChangeKind::Replace => "replace",
        ChangeKind::Delete => "delete",
        ChangeKind::React => "react",
        ChangeKind::Adopt => "adopt",
        ChangeKind::Reorder => "reorder",
    }
}

fn operation_tag(operation: ChangeKind) -> u8 {
    match operation {
        ChangeKind::Create => 1,
        ChangeKind::Update => 2,
        ChangeKind::Replace => 3,
        ChangeKind::Delete => 4,
        ChangeKind::React => 5,
        ChangeKind::Adopt => 6,
        ChangeKind::Reorder => 7,
    }
}

impl serde::Serialize for Grant {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for Grant {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn kind(name: &str) -> Kind {
        Kind::new(name).unwrap()
    }

    fn target(name: &str) -> Target {
        Target::new(name).unwrap()
    }

    #[test]
    fn a_grant_that_names_nothing_allows_everything() {
        let grant = Grant::default();
        assert!(grant.allows(&"yandex:instance.db".parse().unwrap(), ChangeKind::Create));
        assert!(grant.allows(&"os:file.a@web-01".parse().unwrap(), ChangeKind::Update));
    }

    #[test]
    fn a_grant_refuses_a_kind_it_does_not_name() {
        let grant = Grant {
            kinds: [kind("file")].into_iter().collect(),
            ..Grant::default()
        };
        assert!(grant.allows(&"os:file.a@web-01".parse().unwrap(), ChangeKind::Create));
        assert!(!grant.allows(&"yandex:instance.db".parse().unwrap(), ChangeKind::Create));
    }

    #[test]
    fn a_grant_refuses_an_operation_it_does_not_name() {
        let grant = Grant {
            operations: [ChangeKind::Update].into_iter().collect(),
            ..Grant::default()
        };
        assert!(grant.allows(&"os:file.a@web-01".parse().unwrap(), ChangeKind::Update));
        assert!(!grant.allows(&"os:file.a@web-01".parse().unwrap(), ChangeKind::Delete));
    }

    #[test]
    fn a_grant_refuses_a_target_it_does_not_name() {
        let grant = Grant {
            targets: [target("web-01")].into_iter().collect(),
            ..Grant::default()
        };
        assert!(grant.allows(&"os:file.a@web-01".parse().unwrap(), ChangeKind::Create));
        assert!(!grant.allows(&"os:file.a@web-02".parse().unwrap(), ChangeKind::Create));
        // A control resource has no target, so targets do not restrict it.
        assert!(grant.allows(&"yandex:instance.db".parse().unwrap(), ChangeKind::Create));
    }
}
