use crate::invariant::Status;
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-DRIFT-001"),
    status: Status::Planned,
    statement: NonEmptyStr::new("A resource marked for merging is never overwritten without a person."),
    rationale: r#"Portage's CONFIG_PROTECT, and for the same reason: people edit machines during incidents, and the edit is often the only record of what the incident was."#,
    enforced_by: &[crate::anchor::inv_drift_001],
    tests: &[crate::anchor::drift_is_corrected_unless_the_resource_asks_to_be_merged],
);
