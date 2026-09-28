use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-REACT-003"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A restart that starts what is stopped: `systemctl restart`, or `rc-service` without `--ifstarted`. It plans and applies cleanly, and a service meant to stay down is up afterwards."), tests: &[crate::anchor::being_roused_restarts_a_service_only_if_it_is_running] }),
    statement: NonEmptyStr::new("Being roused restarts a service only if it is running."),
    rationale: r"A reaction exists so that a running process takes a changed file. A service that was stopped before the file changed was not running on the old one either, and starting it would be a change nobody declared. The command sent was a plain restart, which starts a stopped service, while the comments beside it and the architecture said the opposite; only the running case had been shown on a machine. See decision 0026.",
    enforced_by: &[crate::anchor::inv_react_003],
    tests: &[crate::anchor::being_roused_restarts_a_service_only_if_it_is_running],
);
