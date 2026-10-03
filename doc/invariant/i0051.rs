use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0051",
    specification: crate::rfc::rfc_2026_004,
    status: InvariantStatus::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A filter, test or global added to the list that reaches the world — `now`, `env`, anything that opens a file. The list is the boundary, so widening it is the way through it, and it is one line of a diff. Below that, a template source that resolves a name outside the configuration's directory: shape is checked before the disk is touched and the resolved path afterwards, but a source written elsewhere answers however it likes."), tests: &[crate::anchor::a_value_that_was_not_handed_over_is_a_refusal_rather_than_a_blank, crate::anchor::nothing_is_in_scope_that_was_not_handed_over, crate::anchor::a_filter_that_is_not_on_the_list_is_refused_by_name, crate::anchor::a_template_that_would_not_stop_is_stopped, crate::anchor::a_template_cannot_be_named_outside_the_configuration] }),
    statement: NonEmptyStr::new("INV-TEMPLATE-001: A template sees the values it was handed and nothing else, and cannot reach the world."),
    rationale: r"A template that reads the world renders differently every time, and a plan computed from it could not be compared, hashed or signed — the same reasoning that chose Starlark over a scripting language in `0005`. So: no clock, no environment, no process, no network, and no filesystem beyond the directory the configuration sits in. There is no `lookup` and nothing like one.

There is no ambient scope either. Ansible's twenty-two precedence levels are refused here as everywhere else: what a template can see is what the call handed it, and a value that was not handed over is a refusal rather than an empty string — for a configuration file the difference is a directive that silently disappears.

The filters are a list this project keeps rather than whatever `minijinja` ships. Every one of them is part of what a configuration may rely on (`0022`), so an upgrade must not widen the set without somebody deciding to. Execution is bounded by fuel and a recursion limit.",
    enforced_by: &[crate::anchor::inv_template_001],
    tests: &[crate::anchor::templates_written_for_ansible_render_here],
);
