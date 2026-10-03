use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0057",
    specification: crate::rfc::rfc_2026_002,
    status: InvariantStatus::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("Writing the debt when the reacting resource's own change runs, or only for a reacting resource state already records. Either plans and applies cleanly until a run stops between the two stages."), tests: &[crate::anchor::a_run_that_stops_between_a_cause_and_its_reaction_leaves_the_reaction_owed, crate::anchor::a_reactor_nothing_recorded_yet_is_owed_its_reaction_all_the_same] }),
    statement: NonEmptyStr::new("INV-REACT-002: A reaction is owed from before its cause is sent."),
    rationale: r"A run that stops between a cause and its reaction — a machine gone, a Ctrl-C, a failure in a stage between them — has written the new file and not restarted what reads it. The debt used to be written when the reaction's own stage came, which is exactly what that run never reached, and the next plan found the file matching, nothing changing and nothing owed. It is written before the cause is sent, and a reacting resource that state does not record yet is recorded from what was read so that it can carry one. The window resolves toward a debt that was in fact paid: one redundant restart. See decision 0026.",
    enforced_by: &[crate::anchor::run_owe],
    tests: &[crate::anchor::a_reaction_that_did_not_finish_is_owed_and_the_next_plan_says_so, crate::anchor::a_debt_survives_the_state_file, crate::anchor::a_run_that_stops_between_a_cause_and_its_reaction_leaves_the_reaction_owed, crate::anchor::a_reactor_nothing_recorded_yet_is_owed_its_reaction_all_the_same],
);
