//! The properties a digest has to have before it is worth keeping instead of a
//! value.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};

use nerpa_core::secret::{Digest, Key, Recorded, Remembered, SecretStore, Source};
use nerpa_core::state::Record;
use nerpa_core::value::Value;

fn key(byte: u8) -> Key {
    Key::from_bytes([byte; 32])
}

fn text(of: &str) -> Value {
    Value::Text(of.to_owned())
}

/// The point of keying: a digest lifted out of one state file says nothing about
/// the same secret in another, and a table of likely passwords built against one
/// is useless against the next.
#[test]
#[dacc_derive::doc_anchor(id = "the-same-value-under-two-keys-digests-differently")]
fn the_same_value_under_two_keys_digests_differently() {
    let value = text("hunter2");
    assert_ne!(
        Digest::of(&key(1), &value),
        Digest::of(&key(2), &value),
        "a digest must depend on the key, or keying it achieved nothing"
    );
    assert_eq!(
        Digest::of(&key(1), &value),
        Digest::of(&key(1), &value),
        "and it must be the same digest every time, or nothing ever converges"
    );
}

/// Change detection is only as good as the encoding underneath it. Two values
/// that encode to the same bytes would compare equal, and a rotation between
/// them would be reported as no change at all.
#[test]
#[dacc_derive::doc_anchor(id = "values-that-are-not-equal-do-not-digest-equal")]
fn values_that_are_not_equal_do_not_digest_equal() {
    let under = key(3);
    let distinct = [
        text("[1]"),
        Value::List(vec![Value::Integer(1)]),
        text("1"),
        Value::Integer(1),
        Value::Boolean(true),
        text("true"),
        Value::List(vec![text("ab"), text("c")]),
        Value::List(vec![text("a"), text("bc")]),
        Value::Map(BTreeMap::from([("a".to_owned(), text("b"))])),
        text("ab"),
        Value::List(vec![text("a"), text("b")]),
        // These two exist to hold the length prefix honest. Drop it and each
        // one encodes to exactly the bytes of the list above: the tag byte of
        // the second element reappears inside the first element's text, and
        // without a length nothing says where one stops. One is built from the
        // zero byte and one from the tag byte, because a prefix that is
        // constant rather than absent collides just as happily.
        Value::List(vec![Value::Text(
            ["a", "\u{1}", &"\0".repeat(8), "b"].concat(),
        )]),
        Value::List(vec![Value::Text(
            ["a", "\u{1}", &"\u{1}".repeat(8), "b"].concat(),
        )]),
    ];

    for (index, left) in distinct.iter().enumerate() {
        for right in distinct.iter().skip(index + 1) {
            assert_ne!(
                Digest::of(&under, left),
                Digest::of(&under, right),
                "{left:?} and {right:?} encode to the same bytes"
            );
        }
    }
}

#[test]
fn a_key_and_a_digest_survive_hexadecimal() {
    let original = key(9);
    let read_back = Key::from_hex(&original.to_hex()).expect("round trips");
    let value = text("hunter2");
    assert_eq!(
        Digest::of(&original, &value),
        Digest::of(&read_back, &value)
    );

    let digest = Digest::of(&original, &value);
    assert_eq!(Digest::from_hex(&digest.to_hex()), Some(digest));
}

#[test]
fn something_that_is_not_a_key_is_refused_rather_than_padded() {
    assert_eq!(Key::from_hex(""), None, "empty");
    assert_eq!(Key::from_hex(&"0".repeat(63)), None, "one short");
    assert_eq!(Key::from_hex(&"0".repeat(65)), None, "one long");
    assert_eq!(Key::from_hex(&"z".repeat(64)), None, "not hexadecimal");
}

/// A key that prints itself ends up in a log, and then the digests it protects
/// are worth what unkeyed ones were.
#[test]
fn a_key_does_not_print_itself() {
    let printed = format!("{:?}", key(0xab));
    assert!(
        !printed.contains("ab"),
        "the key is in its own Debug: {printed}"
    );
}

#[test]
#[dacc_derive::doc_anchor(id = "a-recorded-secret-shows-that-it-is-one-and-nothing-more")]
fn a_recorded_secret_shows_that_it_is_one_and_nothing_more() {
    let recorded = Recorded::Secret(Digest::of(&key(1), &text("hunter2")));
    assert_eq!(recorded.to_string(), "(secret)");
    assert!(
        recorded.plain().is_none(),
        "there is no way back to the value"
    );
    assert!(recorded.is_secret());

    let ordinary = Recorded::Plain(text("/etc/motd"));
    assert!(!ordinary.is_secret());
    assert_eq!(ordinary.plain(), Some(&text("/etc/motd")));
}

/// A provider reading a resource back from state — to remove it, or to find it
/// where it was left — is handed the locator, never a secret. State held a
/// digest, not a value, and `Record::locator` drops the digests.
#[test]
#[dacc_derive::doc_anchor(id = "a-record-handed-back-to-a-provider-holds-no-secret")]
fn a_record_handed_back_to_a_provider_holds_no_secret() {
    let mut attributes = BTreeMap::new();
    attributes.insert("path".to_owned(), Recorded::Plain(text("/etc/motd")));
    attributes.insert(
        "content".to_owned(),
        Recorded::Secret(Digest::of(&key(1), &text("hunter2"))),
    );
    let record = Record::new(attributes, BTreeSet::new());

    let locator = record.locator();

    assert_eq!(
        locator.get("path"),
        Some(&text("/etc/motd")),
        "the locator carries what finds the resource"
    );
    assert!(
        !locator.contains_key("content"),
        "the locator never carries a secret, because state never held one: {locator:?}"
    );
}

#[test]
fn a_source_reads_as_where_to_look() {
    let anywhere = Source::Stored {
        provider: "lockbox".to_owned(),
        path: "prod/db/password".to_owned(),
        version: None,
    };
    assert_eq!(anywhere.to_string(), "lockbox:prod/db/password");

    let pinned = Source::Stored {
        provider: "lockbox".to_owned(),
        path: "prod/db/password".to_owned(),
        version: Some("v3".to_owned()),
    };
    assert_eq!(pinned.to_string(), "lockbox:prod/db/password@v3");
}

#[test]
fn a_store_that_has_nothing_says_where_it_looked() {
    let store = Remembered::new().holding("db/password", "hunter2");
    let missing = Source::Stored {
        provider: "remembered".to_owned(),
        path: "db/absent".to_owned(),
        version: None,
    };

    assert_eq!(
        store
            .fetch(&Source::Stored {
                provider: "remembered".to_owned(),
                path: "db/password".to_owned(),
                version: None,
            })
            .expect("held"),
        text("hunter2")
    );

    let refusal = store.fetch(&missing).expect_err("nothing is there");
    assert!(
        refusal.to_string().contains("db/absent"),
        "the refusal should name what was looked for: {refusal}"
    );
}

/// A machine's seal is its own: another machine's differs, another state file's
/// differs, the same pair always gives the same one, and it prints as nothing.
#[test]
#[dacc_derive::doc_anchor(id = "a-seal-belongs-to-one-machine-under-one-state-file")]
fn a_seal_belongs_to_one_machine_under_one_state_file() {
    let key = Key::from_bytes([3; 32]);
    let other = Key::from_bytes([4; 32]);
    let web = nerpa_core::Target::new("web-01").expect("a machine");
    let db = nerpa_core::Target::new("db-01").expect("a machine");

    assert_eq!(key.seal_for(&web), key.seal_for(&web));
    assert_ne!(key.seal_for(&web), key.seal_for(&db));
    assert_ne!(key.seal_for(&web), other.seal_for(&web));
    assert!(
        !format!("{:?}", key.seal_for(&web)).contains(|c: char| c.is_ascii_digit()),
        "a seal prints as nothing"
    );
}
