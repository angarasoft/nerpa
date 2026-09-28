use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-STATE-006"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A resource that matches is planned as nothing although state does not record it — the planner's arm for a match taken before the one for adoption — or an apply loop that passes over a change it sends nothing for. Either plans and applies cleanly and leaves a file that removing the declaration does not remove."), tests: &[crate::anchor::a_file_that_was_adopted_is_removed_once_its_declaration_is] }),
    statement: NonEmptyStr::new("A declared resource that exists is recorded, whether or not anything was done to it."),
    rationale: r#"State is what removal reads, so a resource that is not in it can never be removed by taking its declaration away. One that already matched on the first run used to be planned as nothing and so never recorded, while the same file one byte different was an update and was. It is adopted instead: shown in the plan, sent nowhere, and recorded with what was read and every edge it comes after. A run that stopped between a write and its record leaves the same situation, and the next plan adopts what it left. Adoptions are written once for the run, since they made nothing and lose nothing if the run stops first. See decision 0042."#,
    enforced_by: &[crate::anchor::planner_change_for, crate::anchor::run_apply],
    tests: &[crate::anchor::a_resource_that_already_matches_and_is_not_recorded_is_adopted, crate::anchor::a_file_that_was_already_there_is_adopted_rather_than_remade, crate::anchor::what_is_adopted_is_written_down_once_rather_than_once_for_each],
);
