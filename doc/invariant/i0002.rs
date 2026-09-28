use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-ADDR-002"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("An address read from text whose component begins with `-`, then interpolated into a command line."), tests: &[crate::anchor::nothing_can_be_mistaken_for_a_command_line_option] }),
    statement: NonEmptyStr::new("No component of an address begins with `-`."),
    rationale: r#"Addresses are interpolated into SSH command lines and provider invocations. A name that reads as an option is a remote code execution waiting for a careless call site, so the type refuses to hold one."#,
    enforced_by: &[crate::anchor::inv_addr_002],
    tests: &[crate::anchor::nothing_can_be_mistaken_for_a_command_line_option],
);
