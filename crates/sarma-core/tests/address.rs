//! The address grammar, exercised from outside the crate.
//!
//! These are the tests that must not be relaxed. Every case here is a property
//! the state format depends on, and state files outlive the code that wrote them.

// Tests are allowed to be blunt. An `unwrap` here is an expectation, indexing is
// a claim about shape, and a panic is how a failure gets reported. The rules that
// ban all three apply to code that runs against somebody's infrastructure — see
// docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
use sarma_core::address::{AddressError, address};
use sarma_core::{Address, ExecutionDomain, Kind, Name, Provider, Slot, Target};

#[test]
fn control_resources_have_no_node() {
    let vm = address("yandex:instance.db");

    assert_eq!(vm.provider().as_str(), "yandex");
    assert_eq!(vm.kind().as_str(), "instance");
    assert_eq!(vm.name().as_str(), "db");
    assert_eq!(vm.slot(), None);
    assert_eq!(vm.domain(), ExecutionDomain::Control);
}

#[test]
fn target_resources_name_the_node_they_live_on() {
    let package = address("os:package.nginx@web-01");

    assert_eq!(package.target().map(Target::as_str), Some("web-01"));
    assert!(!package.domain().is_control());
    assert_eq!(
        package.domain().target().map(Target::as_str),
        Some("web-01")
    );
}

#[test]
fn slots_let_two_versions_coexist() {
    let older = address("os:package.python:3.11@web-01");
    let newer = address("os:package.python:3.12@web-01");

    assert_ne!(older, newer, "two slots of one package are two resources");
    assert_eq!(older.name(), newer.name());
    assert_eq!(older.slot().map(Slot::as_str), Some("3.11"));
}

#[test]
fn a_name_may_contain_dots_because_kind_is_split_off_first() {
    let file = address("os:file.nginx.conf@web-01");

    assert_eq!(file.kind().as_str(), "file");
    assert_eq!(file.name().as_str(), "nginx.conf");
}

/// The plan hash is taken over canonical text. A value that printed differently
/// from the way it parsed would produce two hashes for one plan, and quietly
/// invalidate a signature that was given in good faith.
#[test]
fn every_shape_round_trips() {
    for text in [
        "yandex:instance.db",
        "yandex:subnet.app:ru-central1-a",
        "os:package.nginx@web-01",
        "os:package.python:3.11@web-01",
        "os:file.nginx.conf@web-01",
    ] {
        let parsed: Address = text.parse().expect("valid address");
        assert_eq!(parsed.to_string(), text);
    }
}

#[test]
fn nothing_can_be_mistaken_for_a_command_line_option() {
    for text in [
        "os:package.-rf@web-01",
        "os:package.nginx@-force",
        "os:package.x:-1@web-01",
    ] {
        assert!(
            matches!(
                text.parse::<Address>(),
                Err(AddressError::LeadingDash { .. })
            ),
            "{text} should have been rejected"
        );
    }
    assert!(matches!(
        Name::new("-rf"),
        Err(AddressError::LeadingDash { .. })
    ));
    assert!(matches!(
        Target::new("-force"),
        Err(AddressError::LeadingDash { .. })
    ));
}

#[test]
fn vocabulary_components_stay_lowercase() {
    assert!(Provider::new("yandex").is_ok());
    assert!(matches!(
        Provider::new("Yandex"),
        Err(AddressError::BadCharacter { .. })
    ));
    assert!(matches!(
        Kind::new("Instance"),
        Err(AddressError::BadCharacter { .. })
    ));
}

#[test]
fn separators_cannot_hide_inside_components() {
    assert!(matches!(
        Name::new("a@b"),
        Err(AddressError::BadCharacter { .. })
    ));
    assert!(matches!(
        Name::new("a:b"),
        Err(AddressError::BadCharacter { .. })
    ));
    assert!(matches!(
        Target::new("web@01"),
        Err(AddressError::BadCharacter { .. })
    ));
}

#[test]
fn malformed_text_is_reported_rather_than_guessed_at() {
    assert!(matches!(
        "instance.db".parse::<Address>(),
        Err(AddressError::NoProvider(_))
    ));
    assert!(matches!(
        "yandex:instance".parse::<Address>(),
        Err(AddressError::NoKind(_))
    ));
    assert!("yandex:.db".parse::<Address>().is_err());
    assert!(":instance.db".parse::<Address>().is_err());
}

#[test]
fn addresses_are_built_as_well_as_parsed() {
    let built = Address::control(
        Provider::new("os").unwrap(),
        Kind::new("package").unwrap(),
        Name::new("python").unwrap(),
    )
    .in_slot(Slot::new("3.11").unwrap())
    .on(Target::new("web-01").unwrap());

    assert_eq!(built, address("os:package.python:3.11@web-01"));
}
