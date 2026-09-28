//! The address grammar, exercised from outside the crate.
//!
//! These are the tests that must not be relaxed. Every case here is a property
//! the state format depends on, and state files outlive the code that wrote them.

// Tests are allowed to be blunt. An `unwrap` here is an expectation, indexing is
// a claim about shape, and a panic is how a failure gets reported. The rules that
// ban all three apply to code that runs against somebody's infrastructure — see
// docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
use nerpa_core::address::{AddressError, address};
use nerpa_core::{Address, ExecutionDomain, Kind, Name, Provider, Slot, Target};

#[test]
#[dacc_derive::doc_anchor(id = "control-resources-have-no-node")]
fn control_resources_have_no_node() {
    let vm = address("yandex:instance.db");

    assert_eq!(vm.provider().as_str(), "yandex");
    assert_eq!(vm.kind().as_str(), "instance");
    assert_eq!(vm.name().as_str(), "db");
    assert_eq!(vm.slot(), None);
    assert_eq!(vm.domain(), ExecutionDomain::Control);
}

#[test]
#[dacc_derive::doc_anchor(id = "target-resources-name-the-node-they-live-on")]
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
#[dacc_derive::doc_anchor(id = "slots-let-two-versions-coexist")]
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
#[dacc_derive::doc_anchor(id = "every-shape-round-trips")]
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
#[dacc_derive::doc_anchor(id = "nothing-can-be-mistaken-for-a-command-line-option")]
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
#[dacc_derive::doc_anchor(id = "addresses-are-built-as-well-as-parsed")]
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

/// Each alternative in the charsets is its own case. Mutation testing found this
/// gap: with only "lowercase passes, uppercase fails" written down, narrowing the
/// rule to reject digits or underscores broke nothing.
#[test]
fn vocabulary_components_accept_digits_and_underscores() {
    for text in ["os", "yandex", "s3", "my_provider", "vpc2"] {
        assert!(
            Provider::new(text).is_ok(),
            "{text} should be a valid provider"
        );
    }
    for text in ["package", "compute_instance", "subnet6"] {
        assert!(Kind::new(text).is_ok(), "{text} should be a valid kind");
    }
}

#[test]
fn chosen_components_accept_every_character_the_grammar_allows() {
    for text in ["nginx", "nginx.conf", "web-01", "web_01", "db2", "a.b-c_d9"] {
        assert!(Name::new(text).is_ok(), "{text} should be a valid name");
        assert!(Target::new(text).is_ok(), "{text} should be a valid target");
    }

    // A name and a slot reach into somebody else's namespace and a machine does
    // not, so the two sets are not the same one. See decision 0038.
    for text in ["www-servers/nginx", "dev-libs/openssl"] {
        assert!(Name::new(text).is_ok(), "{text} should be a valid name");
        assert!(Slot::new(text).is_ok(), "{text} should be a valid slot");
        assert!(
            Target::new(text).is_err(),
            "{text} should not be a valid machine"
        );
    }
}

#[test]
fn a_control_resource_says_so_when_asked() {
    assert!(address("yandex:instance.db").domain().is_control());
    assert!(!address("os:package.nginx@web-01").domain().is_control());
}

/// In portage the identity of a package is `category/name`, and three different
/// packages are called `nginx` — `acct-group/`, `acct-user/` and `www-servers/`
/// on a stock tree. An address that could not tell them apart would let two
/// declarations collide in the graph over a name neither of them owns. So a name
/// may hold a slash, and so may a slot, where `dev-libs/openssl:0/3` is a
/// subslot. See decision 0038.
#[test]
#[dacc_derive::doc_anchor(
    id = "a-name-and-a-slot-may-hold-a-slash-because-somebody-elses-namespace-does"
)]
fn a_name_and_a_slot_may_hold_a_slash_because_somebody_elses_namespace_does() {
    let package = address("os:package.www-servers/nginx@web-01");
    assert_eq!(package.kind().as_str(), "package");
    assert_eq!(package.name().as_str(), "www-servers/nginx");
    assert_eq!(package.target().map(Target::as_str), Some("web-01"));

    let subslot = address("os:package.dev-libs/openssl:0/3@web-01");
    assert_eq!(subslot.name().as_str(), "dev-libs/openssl");
    assert_eq!(subslot.slot().map(Slot::as_str), Some("0/3"));

    for text in [
        "os:package.www-servers/nginx@web-01",
        "os:package.dev-libs/openssl:0/3@web-01",
    ] {
        let parsed: Address = text.parse().expect("valid address");
        assert_eq!(parsed.to_string(), text, "and it prints as it parsed");
    }
}

/// A machine is not in somebody else's namespace. A slash there would be a
/// character the thing being named cannot contain, and letting it through would
/// be the type saying something untrue.
#[test]
fn a_machine_may_not_hold_a_slash() {
    let refused: Result<Address, _> = "os:file.motd@web/01".parse();
    assert!(
        matches!(refused, Err(AddressError::BadCharacter { .. })),
        "{refused:?}"
    );
}
