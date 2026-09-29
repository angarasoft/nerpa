use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-COMPAT-001"),
    status: Status::Enforced(Enforced::Adversarial {
        bypass: NonEmptyStr::new("a build whose version differs from what the configuration declared running anyway and behaving differently"),
        tests: &[crate::anchor::a_configuration_that_asks_for_another_build_is_refused],
    }),
    statement: NonEmptyStr::new("A configuration declares the versions it needs, and a build that does not satisfy it refuses."),
    rationale: r"Ansible's playbooks broke every release because a playbook declares nothing: it calls a module by name and hopes, and finds out at runtime on a machine. A stated requirement turns a silent change of behaviour into a refusal before anything is touched. See 0022.",
    enforced_by: &[crate::anchor::config_requires_version],
    tests: &[crate::anchor::a_configuration_that_asks_for_this_build_runs, crate::anchor::a_configuration_that_asks_for_another_build_is_refused],
);
