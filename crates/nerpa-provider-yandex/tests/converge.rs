//! The provider against a fake cloud: a declared set converges, an immutable
//! change replaces, a birth change is reported without acting, a stopped
//! attribute change stops and starts, a disk grows in place, and a stopped
//! instance changes where it stands.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

mod common;

use nerpa_core::provider::Machines;
use nerpa_core::state::State;

use common::{Declared, FakeYandex, converge, declared, providers};

/// The folder everything in these tests lives in.
const FOLDER: &str = "b1g-folder";

#[test]
fn a_declared_set_converges() {
    let (fake, address) = FakeYandex::start();
    let providers = providers(&address);
    let machines = Machines::new();
    let mut state = State::new();
    let graph = declared(FOLDER, Declared::default());

    let first = converge(&graph, &providers, &machines, &mut state);
    assert_eq!(
        first.blast_radius().creates(),
        5,
        "one create per declared resource"
    );
    let ledger = fake.ledger();
    assert!(ledger.contains(&"create-network main".to_owned()));
    assert!(ledger.contains(&"create-subnet mgmt".to_owned()));
    assert!(ledger.contains(&"create-security-group web".to_owned()));
    assert!(ledger.contains(&"create-disk data".to_owned()));
    assert!(ledger.contains(&"create-instance db".to_owned()));

    let second = converge(&graph, &providers, &machines, &mut state);
    assert!(second.is_empty(), "the world matches the declaration now");
}

#[test]
fn an_immutable_change_replaces() {
    let (fake, address) = FakeYandex::start();
    let providers = providers(&address);
    let machines = Machines::new();
    let mut state = State::new();
    let graph = declared(FOLDER, Declared::default());
    converge(&graph, &providers, &machines, &mut state);

    // An instance's zone cannot move: the same declaration in another zone
    // replaces the instance, and the replacement is a removal then a make.
    let moved = declared(
        FOLDER,
        Declared {
            zone: "ru-central1-b",
            ..Declared::default()
        },
    );
    let planned = converge(&moved, &providers, &machines, &mut state);
    assert_eq!(
        planned.blast_radius().replaces(),
        1,
        "an immutable change replaces"
    );
    let ledger = fake.ledger();
    assert!(ledger.contains(&"delete-instance inst-1".to_owned()));
    assert!(
        ledger.iter().any(|note| note == "create-instance db"),
        "the replacement made a new instance: {ledger:?}"
    );

    let settled = converge(&moved, &providers, &machines, &mut state);
    assert!(settled.is_empty(), "the replacement converges");
}

#[test]
fn a_birth_change_is_reported_without_acting() {
    let (fake, address) = FakeYandex::start();
    let providers = providers(&address);
    let machines = Machines::new();
    let mut state = State::new();
    let graph = declared(FOLDER, Declared::default());
    converge(&graph, &providers, &machines, &mut state);

    // An instance's boot disk was born: another image means another instance,
    // which nothing can make while the old one holds the name. The plan
    // reports the change as outdated and acts on nothing.
    let reborn = declared(
        FOLDER,
        Declared {
            boot_disk: "fd8other",
            ..Declared::default()
        },
    );
    let before = fake.ledger();
    let planned = converge(&reborn, &providers, &machines, &mut state);
    assert!(
        planned
            .outdated()
            .iter()
            .any(|outdated| outdated.attribute() == "boot_disk"),
        "the birth change is reported: {:?}",
        planned.outdated()
    );
    assert_eq!(planned.blast_radius().creates(), 0);
    assert_eq!(planned.blast_radius().updates(), 0);
    assert_eq!(planned.blast_radius().replaces(), 0);
    assert_eq!(planned.blast_radius().deletes(), 0);
    assert_eq!(fake.ledger(), before, "nothing was sent");
}

#[test]
fn a_stopped_attribute_change_stops_updates_and_starts() {
    let (fake, address) = FakeYandex::start();
    let providers = providers(&address);
    let machines = Machines::new();
    let mut state = State::new();
    let graph = declared(FOLDER, Declared::default());
    converge(&graph, &providers, &machines, &mut state);

    // Cores change only while the instance is stopped: the provider stops,
    // updates and starts it.
    let resized = declared(
        FOLDER,
        Declared {
            cores: 4,
            ..Declared::default()
        },
    );
    let planned = converge(&resized, &providers, &machines, &mut state);
    assert_eq!(
        planned.blast_radius().updates(),
        1,
        "an in-place change updates"
    );
    let ledger = fake.ledger();
    let stop = ledger
        .iter()
        .position(|note| note == "stop inst-1")
        .expect("a stop");
    let update = ledger
        .iter()
        .position(|note| note == "update-instance inst-1")
        .expect("an update");
    let start = ledger
        .iter()
        .position(|note| note == "start inst-1")
        .expect("a start");
    assert!(
        stop < update && update < start,
        "stop, then update, then start"
    );

    let settled = converge(&resized, &providers, &machines, &mut state);
    assert!(settled.is_empty(), "the resized instance converges");
}

#[test]
fn a_disk_size_grows_in_place() {
    let (fake, address) = FakeYandex::start();
    let providers = providers(&address);
    let machines = Machines::new();
    let mut state = State::new();
    let graph = declared(FOLDER, Declared::default());
    converge(&graph, &providers, &machines, &mut state);

    // A disk's size only grows, and the growth changes the disk in place:
    // one call, and the disk is the same disk.
    let grown = declared(
        FOLDER,
        Declared {
            size: 20 * common::GIB,
            ..Declared::default()
        },
    );
    let before = fake.ledger();
    let planned = converge(&grown, &providers, &machines, &mut state);
    assert_eq!(
        planned.blast_radius().updates(),
        1,
        "a growth updates the disk in place"
    );
    let gained = fake.ledger().len() - before.len();
    assert_eq!(gained, 1, "one call, nothing else");
    assert!(
        fake.ledger()
            .iter()
            .any(|note| note == "update-disk disk-1"),
        "the disk grew in place: {:?}",
        fake.ledger()
    );

    let settled = converge(&grown, &providers, &machines, &mut state);
    assert!(settled.is_empty(), "the grown disk converges");
}

#[test]
fn a_stopped_instance_changes_where_it_stands() {
    let (fake, address) = FakeYandex::start();
    let providers = providers(&address);
    let machines = Machines::new();
    let mut state = State::new();
    let graph = declared(FOLDER, Declared::default());
    converge(&graph, &providers, &machines, &mut state);

    // A preemptible stop leaves the instance stopped behind the API's back:
    // the provider changes its resources where it stands and never starts it
    // — a start under preemption would lose the instance.
    fake.set_stopped("inst-1");
    let resized = declared(
        FOLDER,
        Declared {
            memory: 6 * common::GIB,
            ..Declared::default()
        },
    );
    let before = fake.ledger();
    let planned = converge(&resized, &providers, &machines, &mut state);
    assert_eq!(
        planned.blast_radius().updates(),
        1,
        "an in-place change updates"
    );
    let ledger = fake.ledger();
    assert!(
        ledger.iter().any(|note| note == "update-instance inst-1"),
        "the stopped instance was updated: {ledger:?}"
    );
    assert!(
        !ledger
            .iter()
            .skip(before.len())
            .any(|note| note == "stop inst-1" || note == "start inst-1"),
        "no stop or start around the update: {ledger:?}"
    );
    assert!(fake.is_stopped("inst-1"), "the instance stays stopped");

    let settled = converge(&resized, &providers, &machines, &mut state);
    assert!(settled.is_empty(), "the stopped instance converges");
}
