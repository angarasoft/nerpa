use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-FACT-001"),
    status: Status::Enforced(Enforced::Adversarial {
        bypass: NonEmptyStr::new("A fact handed into a function's positional parameter and compared with a constant inside the body, which Starlark answers silently. A named argument is still not followed."),
        tests: &[crate::anchor::a_fact_passed_into_a_function_still_cannot_decide_resources],
    }),
    statement: NonEmptyStr::new("Something read from a machine may decide what an attribute says and never whether a resource exists."),
    rationale: r#"A single planning pass is possible because the set of resources is known before anything runs. A fact is not known then — it is what the machine turned out to be — so letting one decide which resources exist would take the property the whole design rests on.

But a path that differs by distribution is a *value*, and refusing that outright would make the tool unusable on a mixed fleet. So `select(fact, {closed set})` is the mechanism and the only one: every branch is written down, the graph is the same whichever way it goes, and what is open is the value — within a set the plan prints, which is more useful than "unknown after apply" because a person can approve a set.

A machine that says something no branch accounts for is refused, not defaulted. A silent default is a machine configured as though it were a different one, which is worse than a machine that is not configured."#,
    enforced_by: &[crate::anchor::inv_fact_001_1, crate::anchor::inv_fact_001_2, crate::anchor::planner_resolve],
    tests: &[crate::anchor::a_value_may_be_chosen_by_what_a_machine_turns_out_to_be, crate::anchor::a_value_chosen_by_a_fact_settles_from_the_machine],
);
