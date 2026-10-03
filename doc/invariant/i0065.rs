use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0065",
    specification: crate::rfc::rfc_2026_003,
    status: InvariantStatus::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("An apply over a plan nobody but its proposer signed — the proposer signing their own plan, a signature over another plan carried into this one, a signer the deployment never named, or no signature at all where one is asked for — each writable as a plain file anybody can make. What this entry does not cover: a signer who is a lie. The approvers file says which key answers for which principal, and whether that file tells the truth about people is the deployment's claim (0067), not one made here — nor is a caller driving `run::apply` at the engine's door, where the actor's check is INV-TRUST-001's."), tests: &[crate::anchor::an_unsigned_apply_where_approvals_are_named_is_refused, crate::anchor::the_proposer_cannot_sign_their_own_plan, crate::anchor::a_signature_over_another_plan_is_refused_naming_what_moved, crate::anchor::a_signer_the_deployment_never_named_is_refused] }),
    statement: NonEmptyStr::new("INV-TRUST-002: Where a fleet names who may sign, nothing is applied over a plan that a third party has not signed, and whoever proposed the plan cannot sign it."),
    rationale: r#"0009 separates who writes a change from who is allowed to make it: "where configured, apply requires a detached signature over the plan hash from somebody other than whoever proposed it". Where configured is the fleet's [approval] naming an approvers file, and the signature is over the plan's number — the number covers what the plan intends and whose plan it is (INV-PLAN-006) — made as 0067 decides.

The approval carries the plan it approves, which is what makes the refusal honest rather than merely negative: an apply whose plan moved since the signature is refused naming what moved — which resource, which attribute — instead of saying only that a number did not match. And the world moving is refused from two directions, because the number is only half of what 0009 called the hash: a change the plan covers moves the number and the approval refuses, and a change only the premise saw is refused where the work happens, naming what moved (INV-PREMISE-001).

What this entry does not claim: that the signer is who they say. The approvers file binds a principal to a key and nothing binds a principal to a person — that is the deployment's claim, the same one allowed_signers makes everywhere. Claiming more here would make this entry the one a reader trusts wrongly."#,
    enforced_by: &[crate::anchor::plan_approval],
    tests: &[crate::anchor::an_unsigned_apply_where_approvals_are_named_is_refused, crate::anchor::the_proposer_cannot_sign_their_own_plan, crate::anchor::a_signature_over_another_plan_is_refused_naming_what_moved, crate::anchor::a_signer_the_deployment_never_named_is_refused],
);
