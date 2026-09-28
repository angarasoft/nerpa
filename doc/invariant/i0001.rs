use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-ADDR-001"),
    status: Status::Enforced(Enforced::Unrepresentable),
    statement: NonEmptyStr::new("A resource address contains no index."),
    rationale: r"An ordinal position in a list is not an identity. Removing an entry from the middle renumbers everything after it, and a planner that believes those numbers proposes destroying live resources that merely moved. There is no field and no constructor that could hold one.",
    enforced_by: &[crate::anchor::address],
    tests: &[crate::anchor::slots_let_two_versions_coexist, crate::anchor::addresses_are_built_as_well_as_parsed],
);
