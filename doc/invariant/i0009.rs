use crate::invariant::Status;
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-PLAN-001"),
    status: Status::Claimed { bypass: NonEmptyStr::new("An opaque value reaching a comparison through a function *parameter*, including a function that hands it straight back. The analysis follows names, containers, indexing, iteration, method calls, the arguments handed to one, lambda bodies and returns — but not a parameter into the body that receives it, which needs the call sites of every function and a single walk of one module does not have them. Inside such a function the value still refuses when both sides are its own type, and Starlark still answers silently when one side is a constant. Twice this list has grown after somebody found a shape that was not on it.") },
    statement: NonEmptyStr::new("An unresolved value never determines which resources exist."),
    rationale: r"This is what makes a single planning pass possible at all. Held by three mechanisms rather than one, because the toolchain allows only some of them to be precise: ordering, indexing and iteration are refused by the value itself; equality and containment are refused by a pass over the syntax tree, because Starlark answers them without consulting the value; and truthiness is recorded and the whole run refused, because `to_bool` has no error channel. See 0018.

The syntax pass follows the taint rather than matching a shape. It used to match one — a bare attribute access beside `==` — and a review got past it in two lines by storing the handle in a dictionary first. Opacity now spreads through assignments, containers, indexing, slicing, iteration, comprehensions and function returns.",
    enforced_by: &[crate::anchor::config_comparisons, crate::anchor::inv_plan_001_1],
    tests: &[crate::anchor::comparing_a_value_that_does_not_exist_yet_is_refused_before_anything_runs, crate::anchor::branching_on_a_value_that_does_not_exist_yet_refuses_the_whole_run, crate::anchor::iterating_a_value_that_does_not_exist_yet_is_refused, crate::anchor::indexing_a_value_that_does_not_exist_yet_is_refused, crate::anchor::comparing_ordinary_values_is_still_allowed],
);
