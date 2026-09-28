use crate::invariant::Status;
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-SECRET-004"),
    status: Status::Claimed { bypass: NonEmptyStr::new("A secret reaching a comparison through a function *parameter*, including a function that hands it straight back, which the analysis does not follow: doing so needs the call sites of every function. Inside such a function the value refuses when both sides are secrets, and Starlark answers silently when one side is a constant. Reading it back out of a container by method rather than by index was such a shape until a review found it.") },
    statement: NonEmptyStr::new("A secret can be passed to a resource and nothing else: not compared, indexed, iterated or used as a condition."),
    rationale: r#"A configuration that can ask questions about a credential is a configuration that can answer them, and the answers travel in whatever the graph turns out to contain. Ordering, indexing and iteration are refused by the value itself, with a line number. Equality and containment cannot be — Starlark decides a comparison across types as `False` without consulting either value — so the syntax pass refuses them before evaluation, for a name bound to a secret as well as for a direct call. Truthiness has no error channel, so it is recorded and the whole evaluation is refused afterwards. Same three mechanisms as decision 0018, same reasons."#,
    enforced_by: &[crate::anchor::config_comparisons, crate::anchor::inv_secret_004_1],
    tests: &[crate::anchor::a_secret_refuses_to_be_compared, crate::anchor::a_secret_held_in_a_variable_refuses_to_be_compared_too, crate::anchor::a_secret_refuses_to_be_a_condition],
);
