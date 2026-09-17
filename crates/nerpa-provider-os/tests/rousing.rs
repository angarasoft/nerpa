//! Being roused restarts a running service, and nothing else.
//!
//! What is sent is asserted rather than run: no service on the machine these
//! tests run on is theirs to restart. The commands were measured where noted.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use nerpa_core::address::address;
use nerpa_core::observed::Facts;
use nerpa_core::plan::ChangeKind;
use nerpa_core::planner::plan;
use nerpa_core::provider::{
    Answered, Attributes, Concluded, Concluding, Machines, Planned, Prepared, Provider,
    ProviderError, Providers, Reach, Sending,
};
use nerpa_core::run::apply;
use nerpa_core::secret::{Key, Remembered, Secrets};
use nerpa_core::state::{Record, State};
use nerpa_core::value::Value;
use nerpa_core::{Address, Graph, Observed, Resource, Target};
use nerpa_proto::{Answer, Ask, Operation, Seen};
use nerpa_provider_os::Os;

/// Answers every batch as though each operation succeeded, and keeps what it
/// was asked to do.
#[derive(Debug, Default, Clone)]
struct Obliging {
    asked: Arc<Mutex<Vec<Vec<Operation>>>>,
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
fn sent_for(init: &str, kind: ChangeKind, roused: bool) -> Vec<Vec<String>> {
    let node = Target::new("web-01").unwrap();
    let mut observed = Observed::new();
    observed.learn(node.clone(), "init", init);
    let mut attributes = Attributes::new();
    attributes.insert("running".to_owned(), Value::Boolean(true));
    attributes.insert("enabled".to_owned(), Value::Boolean(true));

    let machine = Obliging::default();
    let _ = applied(
        &address("os:service.nginx@web-01"),
        Sending::of(kind).expect("a service change sends something"),
        roused,
        &attributes,
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
        sent_for("systemd", ChangeKind::React, true)[0],
        ["systemctl", "try-restart", "nginx"]
    );
    assert_eq!(
        sent_for("openrc", ChangeKind::React, true)[0],
        ["rc-service", "--ifstarted", "nginx", "restart"]
    );
}

/// Roused as well as changed: settled, then restarted if it is running. Not
/// roused: settled and nothing more, because a restart nobody asked for is still
/// a restart.
#[test]
fn a_service_changing_and_roused_is_settled_then_restarted() {
    let lines = |sent: Vec<Vec<String>>| -> Vec<String> {
        sent.into_iter().map(|program| program.join(" ")).collect()
    };
    let show = "systemctl show nginx -p LoadState -p ActiveState -p UnitFileState";
    assert_eq!(
        lines(sent_for("systemd", ChangeKind::Update, true)),
        [
            "systemctl enable nginx",
            "systemctl start nginx",
            "systemctl try-restart nginx",
            show,
        ]
    );
    assert_eq!(
        lines(sent_for("systemd", ChangeKind::Update, false)),
        ["systemctl enable nginx", "systemctl start nginx", show]
    );
}

/// The os provider says a package cannot react, and a service can.
#[test]
fn the_os_provider_says_a_package_cannot_react() {
    let schema = Os::new().schema();
    assert!(!schema.reacts(&address("os:package.app-misc/jq@web-01")));
    assert!(schema.reacts(&address("os:service.nginx@web-01")));
}

static DIGEST_KEY: Key = Key::from_bytes([3; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

/// A service owed a restart since an earlier run, and changing an attribute of
/// its own in this one, is restarted as well as settled.
///
/// It is one `Update`, not an update and a restart. The provider was never told
/// it was roused, restarted only a reaction on its own, and settled `enable` and
/// `start` instead; `start` does nothing to a running unit, and the fresh record
/// cleared the debt. Found by the second review of RFC 0001.
#[test]
fn a_service_owed_a_restart_is_restarted_when_it_changes_as_well() {
    const CONF: &str = "os:file.nginx-conf@web-01";
    const SERVICE: &str = "os:service.nginx@web-01";
    let node = Target::new("web-01").unwrap();

    let mut graph = Graph::new();
    graph.preexisting(node.clone());
    graph
        .insert(Resource::new(address(CONF)).with("content", "as it is\n"))
        .unwrap();
    graph
        .insert(
            Resource::new(address(SERVICE))
                .with("running", Value::Boolean(true))
                .with("enabled", Value::Boolean(true))
                .reacting_to(address(CONF)),
        )
        .unwrap();

    let mut observed = Observed::new();
    observed.learn(node.clone(), "init", "systemd");
    observed.record(
        address(CONF),
        [("content".to_owned(), Value::Text("as it is\n".to_owned()))]
            .into_iter()
            .collect(),
    );
    observed.record(
        address(SERVICE),
        [
            ("running".to_owned(), Value::Boolean(true)),
            ("enabled".to_owned(), Value::Boolean(false)),
        ]
        .into_iter()
        .collect(),
    );

    let mut state = State::new();
    state.record(address(CONF), Record::new(BTreeMap::new(), BTreeSet::new()));
    state.record(
        address(SERVICE),
        Record::new(BTreeMap::new(), [address(CONF)].into_iter().collect())
            .owing([address(CONF)].into_iter().collect()),
    );

    let providers = Providers::new().with(Box::new(Os::new()));
    let secrets = Secrets::new(&DIGEST_KEY, &NO_STORE);
    let planned = plan(&graph, &observed, &state, &providers.schema(), secrets).expect("plannable");
    let changes: Vec<_> = planned.changes().collect();
    assert_eq!(changes.len(), 1, "only the service: {planned:?}");
    assert_eq!(changes[0].kind(), ChangeKind::Update);
    assert_eq!(changes[0].owed(), [address(CONF)]);

    let machine = Obliging::default();
    let machines = Machines::new().with(node, Box::new(machine.clone()));
    let _ = apply(
        &planned,
        &providers,
        &machines,
        &observed,
        &mut state,
        secrets,
        &mut |_: &State| Ok(()),
    );

    let sent = machine.sent();
    assert!(
        sent.iter()
            .any(|program| program == &["systemctl", "try-restart", "nginx"]),
        "owed a restart, and restarted: {sent:?}"
    );
}

/// One change, the way the engine does it: prepared, sent as one batch, and
/// concluded from the answers. A provider does not send anything itself.
fn applied(
    address: &Address,
    kind: Sending,
    roused: bool,
    attributes: &Attributes,
    facts: Facts<'_>,
    reach: &dyn Reach,
) -> Result<Attributes, ProviderError> {
    let os = Os::new();
    let planned = os.prepare(&Prepared {
        address,
        kind,
        roused,
        resent: false,
        desired: attributes,
        recorded: None,
        facts,
    })?;
    let became = answered(planned, address, reach)?;
    let secret = BTreeSet::new();
    match os.conclude(
        &Concluding {
            address,
            kind,
            roused,
            facts,
            plain: attributes,
            secret: &secret,
        },
        &became,
    )? {
        Concluded::Became(attributes) => Ok(attributes),
        Concluded::Gone => Ok(Attributes::new()),
    }
}

/// What the far end said about each operation, counted as the engine counts it.
fn answered(
    planned: Planned,
    address: &Address,
    reach: &dyn Reach,
) -> Result<Vec<Answered>, ProviderError> {
    let asked = planned.operations.len();
    let answer = reach.ask(&Ask::Perform {
        plan: None,
        premise: Vec::new(),
        operations: planned.operations,
        seal: None,
    })?;
    let seen = match answer {
        Answer::Done { became } | Answer::Found { seen: became } if became.len() == asked => became,
        Answer::Failed { at, problem } => {
            return Err(ProviderError::World {
                address: address.clone(),
                problem: format!("at {at}: {problem}"),
            });
        }
        other => {
            return Err(ProviderError::World {
                address: address.clone(),
                problem: format!("the far end said something unexpected: {other:?}"),
            });
        }
    };
    Ok(seen.into_iter().map(Answered::Saw).collect())
}
