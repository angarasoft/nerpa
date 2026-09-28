use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-FLEET-001"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A fleet that is not settled before evaluation — a dynamic inventory read at run time and used directly rather than materialised. Then \"data by the time the program sees it\" stops being true, and a graph built from it is a graph nobody can reproduce or hash. Nothing does this yet, which is why the dynamic half is unbuilt rather than half-built."), tests: &[crate::anchor::two_groups_giving_one_node_the_same_key_is_an_error_naming_both, crate::anchor::a_value_the_fleet_does_not_give_is_a_refusal_rather_than_a_default, crate::anchor::what_the_fleet_says_may_decide_which_resources_exist] }),
    statement: NonEmptyStr::new("What the fleet says may decide which resources exist; what a machine says may not."),
    rationale: r#"The pair to `INV-FACT-001`, and the line decision 0025 draws. A fleet document is settled before the program is evaluated: which machines exist is decided by a person or by the cloud stage, and it is data by the time anything reads it. So it may be iterated, compared and branched on, and `for host in group("web")` builds a graph for a hundred machines without breaking the rule that composition is known before anything runs.

A fact is the other thing entirely — what a machine turned out to be, discovered by looking — and letting one decide which resources exist would take the property a single planning pass rests on.

Two groups giving one node the same key is an error naming both. Ansible answers "where did this value come from" with twenty-two precedence levels; here the question has one answer per value, and a collision is something somebody decides rather than something a rule decides for them. A node overriding its groups is allowed, because that is a statement rather than a rank."#,
    enforced_by: &[crate::anchor::inv_fleet_001],
    tests: &[crate::anchor::a_group_is_iterated_and_a_hundred_machines_are_not_a_hundred_declarations, crate::anchor::a_node_may_say_what_its_groups_do_not, crate::anchor::only_the_machines_a_run_reaches_for_join_its_inventory],
);
