//! What a resource carries, and what it refuses to carry.

// Tests are allowed to be blunt. An `unwrap` here is an expectation, indexing is
// a claim about shape, and a panic is how a failure gets reported. The rules that
// ban all three apply to code that runs against somebody's infrastructure — see
// docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
use sarma_core::address::address;
use sarma_core::{Protection, Resource, Value};

#[test]
fn attributes_and_ordering_are_recorded_as_declared() {
    let vm = Resource::new(address("yandex:instance.db"))
        .with("cores", 8)
        .with("zone", "ru-central1-e")
        .after(address("yandex:subnet.app"));

    assert_eq!(vm.attribute("cores"), Some(&Value::Integer(8)));
    assert_eq!(
        vm.attribute("zone"),
        Some(&Value::Text("ru-central1-e".to_owned()))
    );
    assert!(vm.depends_on().contains(&address("yandex:subnet.app")));
}

#[test]
fn the_last_value_written_is_the_one_that_stands() {
    let vm = Resource::new(address("yandex:instance.db"))
        .with("cores", 4)
        .with("cores", 8);

    assert_eq!(vm.attribute("cores"), Some(&Value::Integer(8)));
    assert_eq!(vm.attributes().len(), 1);
}

#[test]
fn drift_is_corrected_unless_the_resource_asks_to_be_merged() {
    let plain = Resource::new(address("os:file.motd@web-01"));
    assert_eq!(plain.protection(), Protection::Converge);

    let edited_by_hand = Resource::new(address("os:file.sshd_config@web-01")).merge_on_drift();
    assert_eq!(edited_by_hand.protection(), Protection::Merge);
}

/// Asserts a contract, so it only applies to builds that compile contracts in.
#[cfg(feature = "strict-contracts")]
#[test]
#[should_panic(expected = "declared to depend on itself")]
fn a_resource_cannot_depend_on_itself() {
    let _ = Resource::new(address("yandex:instance.db")).after(address("yandex:instance.db"));
}
