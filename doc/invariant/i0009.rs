use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0009",
    specification: crate::rfc::rfc_2026_004,
    status: InvariantStatus::Enforced(Enforced::Adversarial {
        bypass: NonEmptyStr::new("A handle handed into a function's positional parameter and compared with a constant inside the body, which Starlark answers silently. A named argument is still not followed."),
        tests: &[crate::anchor::a_handle_passed_into_a_function_still_refuses_to_be_compared],
    }),
    statement: NonEmptyStr::new("INV-PLAN-001: An unresolved value never determines which resources exist."),
    rationale: r"This is what makes a single planning pass possible at all. Held by three mechanisms rather than one, because the toolchain allows only some of them to be precise: ordering, indexing and iteration are refused by the value itself; equality and containment are refused by a pass over the syntax tree, because Starlark answers them without consulting the value; and truthiness is recorded and the whole run refused, because `to_bool` has no error channel. See 0018.

The syntax pass follows the taint rather than matching a shape. It used to match one — a bare attribute access beside `==` — and a review got past it in two lines by storing the handle in a dictionary first. Opacity now spreads through assignments, containers, indexing, slicing, iteration, comprehensions and function returns.",
    enforced_by: &[crate::anchor::config_comparisons, crate::anchor::inv_plan_001_1],
    tests: &[crate::anchor::comparing_a_value_that_does_not_exist_yet_is_refused_before_anything_runs, crate::anchor::branching_on_a_value_that_does_not_exist_yet_refuses_the_whole_run, crate::anchor::iterating_a_value_that_does_not_exist_yet_is_refused, crate::anchor::indexing_a_value_that_does_not_exist_yet_is_refused, crate::anchor::comparing_ordinary_values_is_still_allowed, crate::anchor::a_value_that_does_not_exist_yet_cannot_be_handed_out_of_a_function, crate::anchor::a_value_that_does_not_exist_yet_cannot_be_laundered_through_a_dictionary, crate::anchor::a_value_that_does_not_exist_yet_cannot_be_laundered_through_a_lambda, crate::anchor::a_value_that_does_not_exist_yet_cannot_be_laundered_through_a_list, crate::anchor::a_value_that_does_not_exist_yet_cannot_be_laundered_through_a_method_call, crate::anchor::a_value_that_does_not_exist_yet_cannot_be_tested_for_membership, crate::anchor::a_comprehension_over_a_deferred_list_is_refused],
);
