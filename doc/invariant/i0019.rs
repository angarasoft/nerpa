use crate::invariant::Status;
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-COMPAT-001"),
    status: Status::Planned,
    statement: NonEmptyStr::new("A configuration declares the versions it needs, and a build that does not satisfy it refuses."),
    rationale: r#"Ansible's playbooks broke every release because a playbook declares nothing: it calls a module by name and hopes, and finds out at runtime on a machine. A stated requirement turns a silent change of behaviour into a refusal before anything is touched. See 0022."#,
    enforced_by: &[],
    tests: &[],
);
