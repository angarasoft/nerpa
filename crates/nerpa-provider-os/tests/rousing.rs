//! Being roused restarts a running service, and nothing else.
//!
//! What is sent is asserted rather than run: no service on the machine these
//! tests run on is theirs to restart. The commands were measured where noted.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::sync::Mutex;

use nerpa_core::address::address;
use nerpa_core::plan::ChangeKind;
use nerpa_core::provider::{Attributes, Provider, ProviderError, Reach};
use nerpa_core::value::Value;
use nerpa_core::{Observed, Target};
use nerpa_proto::{Answer, Ask, Operation, Seen};
use nerpa_provider_os::Os;

/// Answers every batch as though each operation succeeded, and keeps what it
/// was asked to do.
#[derive(Debug, Default)]
struct Obliging {
    asked: Mutex<Vec<Vec<Operation>>>,
}

impl Obliging {
    /// Each program of the one batch sent, with its arguments.
    fn sent(&self) -> Vec<Vec<String>> {
        let asked = self.asked.lock().unwrap();
        assert_eq!(asked.len(), 1, "one batch for one change: {asked:?}");
        asked[0]
            .iter()
            .map(|operation| match operation {
                Operation::Process {
                    program, arguments, ..
                } => std::iter::once(program.clone())
                    .chain(arguments.iter().cloned())
                    .collect(),
                other => panic!("a service is settled by running programs, not {other:?}"),
            })
            .collect()
    }
}

impl Reach for Obliging {
    fn ask(&self, ask: &Ask) -> Result<Answer, ProviderError> {
        let Ask::Perform { operations, .. } = ask else {
            panic!("only a batch is expected here: {ask:?}");
        };
        self.asked.lock().unwrap().push(operations.clone());
        let said = Seen::Said {
            status: 0,
            out: b"LoadState=loaded\nActiveState=active\nUnitFileState=enabled\n".to_vec(),
            complaint: Vec::new(),
        };
        Ok(Answer::Done {
            became: vec![said; operations.len()],
        })
    }
}

/// What a service change on a machine running `init` sends.
fn sent_for(init: &str, kind: ChangeKind) -> Vec<Vec<String>> {
    let node = Target::new("web-01").unwrap();
    let mut observed = Observed::new();
    observed.learn(node.clone(), "init", init);
    let mut attributes = Attributes::new();
    attributes.insert("running".to_owned(), Value::Boolean(true));
    attributes.insert("enabled".to_owned(), Value::Boolean(true));

    let machine = Obliging::default();
    let _ = Os::new().apply(
        &address("os:service.nginx@web-01"),
        kind,
        &attributes,
        &[],
        observed.facts_of(Some(&node)),
        &machine,
    );
    machine.sent()
}

/// A roused service is restarted only if it is running. A plain restart starts a
/// stopped one, which is a change nobody declared: it was not running on the old
/// file either. The comments said so and the command did the opposite; only the
/// running case had been shown on a machine. `rc-service --ifstarted` was
/// measured to exit 0 and do nothing for a stopped service. Found by the second
/// review of RFC 0001.
#[test]
fn being_roused_restarts_a_service_only_if_it_is_running() {
    assert_eq!(
        sent_for("systemd", ChangeKind::React)[0],
        ["systemctl", "try-restart", "nginx"]
    );
    assert_eq!(
        sent_for("openrc", ChangeKind::React)[0],
        ["rc-service", "--ifstarted", "nginx", "restart"]
    );
}
