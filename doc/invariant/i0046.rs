use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0046",
    specification: crate::rfc::rfc_2026_002,
    status: InvariantStatus::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A name known only later: a path that comes through a reference is not a declared value when planning, so two such resources are not caught. And a name that differs in spelling but not in effect beyond what is normalised: `..`, and two paths that reach one file through a symbolic link."), tests: &[crate::anchor::two_resources_naming_one_thing_on_a_machine_are_refused] }),
    statement: NonEmptyStr::new("INV-PLAN-008: One thing on a machine is declared by one resource."),
    rationale: r"Addresses are unique by construction, and what they declare was not. Two files at one path were both created, the last write won, and every later plan proposed the other again, so the second run was never empty. A provider says which attribute names the thing on the machine and in which namespace — a file and a directory share `path` — and the planner refuses a second resource naming the same thing on the same machine, however the path is spelled.",
    enforced_by: &[crate::anchor::inv_plan_008],
    tests: &[crate::anchor::two_resources_naming_one_thing_on_a_machine_are_refused, crate::anchor::the_os_provider_names_files_and_directories_by_path_in_one_namespace],
);
