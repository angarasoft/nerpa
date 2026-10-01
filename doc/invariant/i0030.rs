use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-TRUST-001"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A run that starts with nobody attached — the command line letting one through without `--as`, or a caller driving `run::apply` at the engine's door — or a static key reaching the transport through the fleet document. What this entry does not cover: an actor that is a lie. The engine cannot tell a real OIDC subject from a string, and that a name is who it says it is is the grant's claim (0009), not one made here."), tests: &[crate::anchor::a_run_without_an_actor_is_refused_naming_the_flag, crate::anchor::a_run_proceeds_only_with_an_actor_identity, crate::anchor::a_fleet_that_names_a_key_is_refused_naming_it, crate::anchor::no_identity_reaches_the_ssh_command] }),
    statement: NonEmptyStr::new("No run proceeds without an actor identity; static keys are not a mode."),
    rationale: r"Both halves hold. A run carries who acts — a person over OIDC or Teleport, CI over workload identity, an agent over a grant (0009) — as `--as`, with the scope and expiry a grant will one day check; the actor goes into the plan and its number, so a signature over the number is a signature over somebody's plan, and into a journal beside state for a person reading afterwards. The static-key half is an absence that is now pinned from three sides: the transport's `ssh` invocation is one shape a test reads word by word, the fleet document refuses a `key` field by name because the port, the jump host and the key belong to `ssh`, and no code path reads a long-lived key — so no deadline can produce one.",
    enforced_by: &[crate::anchor::inv_trust_001],
    tests: &[crate::anchor::a_run_without_an_actor_is_refused_naming_the_flag, crate::anchor::a_run_proceeds_only_with_an_actor_identity, crate::anchor::a_fleet_that_names_a_key_is_refused_naming_it, crate::anchor::no_identity_reaches_the_ssh_command],
);
