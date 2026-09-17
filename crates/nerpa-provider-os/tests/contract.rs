//! What a provider is told to build, and what it makes of the answers.
//!
//! The contract RFC 0001 §5.2 settles: a provider says which operations make a
//! change and what their answers mean, and never reaches a machine to do it.
//! These tests call `prepare` and `conclude` with no [`Reach`] anywhere — the
//! machine is absent by construction, which is the whole of the change.
//!
//! [`Reach`]: nerpa_core::provider::Reach

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeSet;

use nerpa_core::address::address;
use nerpa_core::plan::ChangeKind;
use nerpa_core::provider::{
    Afterwards, Answered, Attributes, Concluded, Concluding, Prepared, Provider, Sending,
};
use nerpa_core::value::Value;
use nerpa_core::{Observed, Target};
use nerpa_proto::{Entry, Operation, Seen};
use nerpa_provider_os::Os;

/// A file's attributes, as a declaration settles them.
fn file_of(path: &str, content: &str, mode: Option<&str>) -> Attributes {
    let mut attributes = Attributes::new();
    attributes.insert("path".to_owned(), Value::Text(path.to_owned()));
    attributes.insert("content".to_owned(), Value::Text(content.to_owned()));
    if let Some(mode) = mode {
        attributes.insert("mode".to_owned(), Value::Text(mode.to_owned()));
    }
    attributes
}

/// A machine that said these things about itself.
fn machine(said: &[(&str, &str)]) -> (Observed, Target) {
    let node = Target::new("web-01").unwrap();
    let mut observed = Observed::new();
    for (name, value) in said {
        observed.learn(node.clone(), *name, *value);
    }
    (observed, node)
}

/// The programs a batch runs, in order.
fn programs(operations: &[Operation]) -> Vec<Vec<String>> {
    operations
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

#[test]
fn a_file_is_one_write_that_carries_what_it_was_given() {
    let observed = Observed::new();
    let desired = file_of("/etc/motd", "hello\n", Some("0644"));

    let planned = Os::new()
        .prepare(&Prepared {
            address: &address("os:file.motd@web-01"),
            kind: Sending::Create,
            roused: false,
            resent: false,
            desired: &desired,
            recorded: None,
            facts: observed.facts_of(None),
        })
        .unwrap();

    assert_eq!(
        planned.operations,
        vec![Operation::File {
            path: "/etc/motd".to_owned(),
            content: b"hello\n".to_vec(),
            mode: 0o644,
        }]
    );
    assert_eq!(
        planned.forget_after, None,
        "nothing is forgotten mid-change"
    );
    let carries: BTreeSet<String> = ["content", "mode", "path"]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect();
    assert_eq!(
        planned.carries.get(&0),
        Some(&carries),
        "a write confirms what it wrote: {:?}",
        planned.carries
    );
}

#[test]
fn what_a_file_became_is_read_from_the_answer_and_not_from_what_was_sent() {
    let observed = Observed::new();
    let plain = file_of("/etc/motd", "hello\n", Some("0644"));
    let secret = BTreeSet::new();

    let concluded = Os::new()
        .conclude(
            &Concluding {
                address: &address("os:file.motd@web-01"),
                kind: Sending::Create,
                roused: false,
                facts: observed.facts_of(None),
                plain: &plain,
                secret: &secret,
            },
            &[Answered::Saw(Seen::Entry(Entry::File {
                content: b"what is really there\n".to_vec(),
                mode: 0o600,
            }))],
        )
        .unwrap();

    let Concluded::Became(became) = concluded else {
        panic!("a file that was written is not gone: {concluded:?}");
    };
    assert_eq!(
        became.get("content"),
        Some(&Value::bytes(b"what is really there\n".to_vec())),
        "what the machine read back, not what was sent"
    );
    assert_eq!(became.get("mode"), Some(&Value::Text("0600".to_owned())));
    assert_eq!(
        became.get("path"),
        Some(&Value::Text("/etc/motd".to_owned()))
    );
}

#[test]
fn a_replacement_is_one_batch_with_the_forgetting_between_its_halves() {
    let observed = Observed::new();
    let desired = file_of("/etc/motd.new", "new\n", None);
    let recorded = file_of("/etc/motd", "old\n", Some("0644"));

    let planned = Os::new()
        .prepare(&Prepared {
            address: &address("os:file.motd@web-01"),
            kind: Sending::Replace,
            roused: false,
            resent: false,
            desired: &desired,
            recorded: Some(&recorded),
            facts: observed.facts_of(None),
        })
        .unwrap();

    assert_eq!(
        planned.operations[0],
        Operation::Remove {
            path: "/etc/motd".to_owned()
        },
        "what state remembers goes first: {:?}",
        planned.operations
    );
    assert!(
        matches!(&planned.operations[1], Operation::File { path, .. } if path == "/etc/motd.new"),
        "and what is declared is made after it: {:?}",
        planned.operations
    );
    assert_eq!(planned.operations.len(), 2);
    assert_eq!(
        planned.forget_after,
        Some(1),
        "the record is dropped between the two halves"
    );
    assert!(
        planned.carries.contains_key(&1),
        "the write is the second operation, and it is the one that carries: {:?}",
        planned.carries
    );
}

#[test]
fn a_removal_is_built_from_what_was_recorded_and_concludes_gone() {
    let observed = Observed::new();
    let recorded = file_of("/etc/motd", "old\n", Some("0644"));
    let nothing = Attributes::new();
    let os = Os::new();

    let planned = os
        .prepare(&Prepared {
            address: &address("os:file.motd@web-01"),
            kind: Sending::Delete,
            roused: false,
            resent: false,
            desired: &nothing,
            recorded: Some(&recorded),
            facts: observed.facts_of(None),
        })
        .unwrap();
    assert_eq!(
        planned.operations,
        vec![Operation::Remove {
            path: "/etc/motd".to_owned()
        }]
    );

    let secret = BTreeSet::new();
    let concluded = os
        .conclude(
            &Concluding {
                address: &address("os:file.motd@web-01"),
                kind: Sending::Delete,
                roused: false,
                facts: observed.facts_of(None),
                plain: &recorded,
                secret: &secret,
            },
            &[Answered::Saw(Seen::Nothing)],
        )
        .unwrap();
    assert_eq!(concluded, Concluded::Gone);
}

#[test]
fn a_package_is_installed_and_then_looked_at_again() {
    let (observed, node) = machine(&[("os_family", "rhel")]);
    let mut desired = Attributes::new();
    desired.insert("version".to_owned(), Value::Text("3.5a".to_owned()));

    let planned = Os::new()
        .prepare(&Prepared {
            address: &address("os:package.tmux@web-01"),
            kind: Sending::Create,
            roused: false,
            resent: false,
            desired: &desired,
            recorded: None,
            facts: observed.facts_of(Some(&node)),
        })
        .unwrap();

    assert_eq!(
        planned.operations.len(),
        2,
        "installing and reading back are one batch: {:?}",
        programs(&planned.operations)
    );
    assert!(
        planned.carries.is_empty(),
        "nothing a package sends writes a file"
    );
}

#[test]
fn a_service_that_is_roused_settles_before_it_is_restarted_and_is_watched_after() {
    let (observed, node) = machine(&[("init", "systemd")]);
    let mut desired = Attributes::new();
    desired.insert("running".to_owned(), Value::Boolean(true));
    desired.insert("enabled".to_owned(), Value::Boolean(true));

    let prepare = |roused| {
        Os::new()
            .prepare(&Prepared {
                address: &address("os:service.nginx@web-01"),
                kind: Sending::Update,
                roused,
                resent: false,
                desired: &desired,
                recorded: None,
                facts: observed.facts_of(Some(&node)),
            })
            .unwrap()
            .operations
    };

    let roused = prepare(true);
    let quiet = prepare(false);
    assert_eq!(
        roused.len(),
        quiet.len() + 1,
        "being roused adds the restart and nothing else: {:?} against {:?}",
        programs(&roused),
        programs(&quiet)
    );
    assert_eq!(
        roused.last(),
        quiet.last(),
        "both end by reading back what the service became"
    );
}

#[test]
fn a_package_left_unrecorded_is_sent_again_and_a_file_is_read() {
    let os = Os::new();
    assert_eq!(
        os.afterwards(&address("os:package.tmux@web-01"), Sending::Create),
        Afterwards::SendAgain,
        "a package is registered before its post-install runs"
    );
    assert_eq!(
        os.afterwards(&address("os:file.motd@web-01"), Sending::Create),
        Afterwards::Read,
        "reading a file sees every attribute a write sets"
    );
}

#[test]
fn only_a_change_that_sends_something_reaches_a_provider() {
    assert_eq!(Sending::of(ChangeKind::Adopt), None);
    assert_eq!(Sending::of(ChangeKind::Reorder), None);
    assert_eq!(Sending::of(ChangeKind::Replace), Some(Sending::Replace));
    assert_eq!(Sending::of(ChangeKind::React), Some(Sending::React));
}
