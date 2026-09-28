use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-REACT-006"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("Owing reactions only through the reacting resources a plan carries. The cause is sent while its reactor is out of reach, and the reaction is lost with nothing recording it."), tests: &[crate::anchor::a_reaction_on_a_machine_that_did_not_answer_is_owed_on_its_record] }),
    statement: NonEmptyStr::new("A change rousing a resource that cannot be planned leaves the reaction owed on its record, or waits while it has none."),
    rationale: r"Every reaction test ran on one machine. A file on a machine that answered was sent while the service reacting to it was on one that did not, or held behind one, and no debt was written: when that machine came back the file matched, nothing was changing and nothing was owed, which is the state 0026 exists to make unreachable. The reaction is owed on the reactor's record before the cause is sent, as INV-REACT-002 does for a reactor in the plan, and the plan carries it on the cause. A reactor with no record has nowhere to carry a debt, so the cause waits and the plan says so. The rule was decided by the owner after the narrow review of RFC 0001.",
    enforced_by: &[crate::anchor::inv_react_006_1, crate::anchor::inv_react_006_2, crate::anchor::run_owe],
    tests: &[crate::anchor::a_reaction_on_a_machine_that_did_not_answer_is_owed_on_its_record, crate::anchor::a_cause_waits_while_what_it_rouses_cannot_be_planned_and_has_no_record, crate::anchor::a_replaced_cause_owes_its_reaction_on_a_machine_that_did_not_answer],
);
