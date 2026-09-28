//! What a resource carries, and what it refuses to carry.

// Tests are allowed to be blunt. An `unwrap` here is an expectation, indexing is
// a claim about shape, and a panic is how a failure gets reported. The rules that
// ban all three apply to code that runs against somebody's infrastructure — see
// docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
use nerpa_core::address::address;
use nerpa_core::{Attribute, Protection, Reference, Resource, Value};

#[test]
fn attributes_and_ordering_are_recorded_as_declared() {
    let vm = Resource::new(address("yandex:instance.db"))
        .with("cores", 8)
        .with("zone", "ru-central1-e")
        .after(address("yandex:subnet.app"));

    assert_eq!(
        vm.attribute("cores").and_then(Attribute::known),
        Some(&Value::Integer(8))
    );
    assert_eq!(
        vm.attribute("zone").and_then(Attribute::known),
        Some(&Value::Text("ru-central1-e".to_owned()))
    );
    assert!(vm.depends_on().contains(&address("yandex:subnet.app")));
}

#[test]
fn the_last_value_written_is_the_one_that_stands() {
    let vm = Resource::new(address("yandex:instance.db"))
        .with("cores", 4)
        .with("cores", 8);

    assert_eq!(
        vm.attribute("cores").and_then(Attribute::known),
        Some(&Value::Integer(8))
    );
    assert_eq!(vm.attributes().len(), 1);
}

#[test]
#[dacc_derive::doc_anchor(id = "drift-is-corrected-unless-the-resource-asks-to-be-merged")]
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

#[test]
fn an_attribute_may_be_waiting_on_another_resource() {
    let conf = Resource::new(address("os:file.nginx.conf@web-01")).with(
        "upstream",
        Reference::to(address("yandex:instance.db"), "ip"),
    );

    let waiting = conf.attribute("upstream").expect("declared");
    assert_eq!(waiting.known(), None, "an unresolved value is not a value");

    let reference = waiting.reference().expect("it is waiting on something");
    assert_eq!(reference.source(), &address("yandex:instance.db"));
    assert_eq!(reference.attribute(), "ip");
    assert_eq!(reference.to_string(), "yandex:instance.db.ip");
}

#[test]
fn a_settled_attribute_is_waiting_on_nothing() {
    let vm = Resource::new(address("yandex:instance.db")).with("cores", 8);

    assert_eq!(vm.attribute("cores").and_then(Attribute::reference), None);
}

#[cfg(feature = "strict-contracts")]
#[test]
#[should_panic(expected = "reads from itself")]
fn an_attribute_cannot_read_from_its_own_resource() {
    let _ = Resource::new(address("yandex:instance.db"))
        .with("ip", Reference::to(address("yandex:instance.db"), "ip"));
}
