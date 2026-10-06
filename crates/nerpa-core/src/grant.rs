//! A grant limits a run to named kinds, targets and operations (adr-2026-009).

use std::collections::BTreeSet;

use crate::address::{Address, Kind, Target};
use crate::plan::{Change, ChangeKind, Plan};

/// What a run may touch, as a set of kinds, targets and operations.
///
/// An empty set is no restriction on that axis, so a grant that names nothing
/// allows everything — the default, before grants are written down.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
