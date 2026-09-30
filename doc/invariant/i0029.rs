use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-DRIFT-001"),
    status: Status::Enforced(Enforced::Adversarial {
        bypass: NonEmptyStr::new("Drift on a resource marked for merging being corrected in place — the declaration silently overwriting a hand-edited file, which is the only record of what an incident was."),
        tests: &[crate::anchor::a_drifting_resource_marked_for_merging_is_reported_never_corrected, crate::anchor::drift_is_corrected_unless_the_resource_asks_to_be_merged],
    }),
    statement: NonEmptyStr::new("A resource marked for merging is never overwritten without a person."),
    rationale: r"Portage's CONFIG_PROTECT, and for the same reason: people edit machines during incidents, and the edit is often the only record of what the incident was. Drift on a resource marked for merging is reported on every plan and handed to a person as a three-way merge, rather than corrected in place.",
    enforced_by: &[crate::anchor::inv_drift_001],
    tests: &[crate::anchor::drift_is_corrected_unless_the_resource_asks_to_be_merged, crate::anchor::a_drifting_resource_marked_for_merging_is_reported_never_corrected],
);
