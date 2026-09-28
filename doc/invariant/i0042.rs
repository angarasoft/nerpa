use crate::invariant::Status;
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-PROVIDER-002"),
    status: Status::Claimed { bypass: NonEmptyStr::new("A provider does the work itself. Nothing in the language stops `std::process::Command` inside `prepare`, and a cloud provider with its own HTTP client is exactly the shape that will want to. Claimed rather than enforced for that reason: taking the connection off the trait removes the invitation, not the possibility.") },
    statement: NonEmptyStr::new("A provider reaches a machine only to read: the engine sends what a change is made of."),
    rationale: r"A provider that opens its own journey makes the batch unrepresentable. The engine cannot group, order or count what it never sees, and `INV-BATCH-001` exists to keep one journey per machine rather than one per resource. So `prepare` is handed no connection, `conclude` is handed answers rather than a machine, and the only `Reach` left on the trait is the one `read` needs. See RFC 0001 §5.2 and decision 0044.",
    enforced_by: &[crate::anchor::inv_provider_002_2, crate::anchor::provider_trait],
    tests: &[crate::anchor::the_engine_sends_what_the_provider_prepared, crate::anchor::a_replacement_is_forgotten_between_the_half_that_removes_and_the_half_that_makes],
);
