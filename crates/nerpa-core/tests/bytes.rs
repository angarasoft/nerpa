//! Bytes that are not text, and the one shape they must not be confused with.
//!
//! JSON has no way to say "these are bytes". An array of numbers cannot be told
//! apart from a list of integers, so a binary value is written as base64 under a
//! reserved key — and the map that would collide with it is refused at the point
//! of writing, so no reader ever has to guess.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeMap;

use nerpa_core::value::Value;

fn round_trip(value: &Value) -> Value {
    let written = serde_json::to_string(value).expect("a value can be written");
    serde_json::from_str(&written).expect("and read back")
}

/// The canonical form. Without it a file holding `hello` could be either
/// variant depending on which side of the model built it, the two would compare
/// unequal, and a run would never converge.
#[test]
#[dacc_derive::doc_anchor(id = "bytes-that-are-text-become-text")]
fn bytes_that_are_text_become_text() {
    assert_eq!(
        Value::bytes(b"listen 80;\n".to_vec()),
        Value::Text("listen 80;\n".to_owned())
    );
    assert_eq!(
        Value::bytes(vec![0xff, 0xfe, 0x00]),
        Value::Bytes(vec![0xff, 0xfe, 0x00])
    );
    assert_eq!(Value::bytes(Vec::new()), Value::Text(String::new()));
}

#[test]
#[dacc_derive::doc_anchor(id = "bytes-survive-a-state-file")]
fn bytes_survive_a_state_file() {
    let der = Value::Bytes(vec![0x30, 0x82, 0x01, 0x0a, 0xff, 0xfe]);
    assert_eq!(round_trip(&der), der);

    let written = serde_json::to_string(&der).expect("written");
    assert!(
        written.contains("__bytes__") && written.contains("MIIBCv/+"),
        "bytes are base64 under a reserved key: {written}"
    );
}

#[test]
#[dacc_derive::doc_anchor(id = "every-other-shape-still-round-trips")]
fn every_other_shape_still_round_trips() {
    for value in [
        Value::Text("hello".to_owned()),
        Value::Integer(-7),
        Value::Boolean(true),
        Value::List(vec![Value::Integer(1), Value::Text("two".to_owned())]),
        Value::Map(BTreeMap::from([
            ("path".to_owned(), Value::Text("/etc/motd".to_owned())),
            ("mode".to_owned(), Value::Integer(420)),
        ])),
    ] {
        assert_eq!(round_trip(&value), value, "{value:?}");
    }
}

/// A map that would read back as bytes is refused rather than written, which is
/// what lets the reader decide without guessing.
#[test]
#[dacc_derive::doc_anchor(id = "a-map-shaped-like-bytes-is-refused-rather-than-silently-misread")]
fn a_map_shaped_like_bytes_is_refused_rather_than_silently_misread() {
    let collides = Value::Map(BTreeMap::from([(
        "__bytes__".to_owned(),
        Value::Text("aGk=".to_owned()),
    )]));

    let refusal = serde_json::to_string(&collides).expect_err("this shape is reserved");
    assert!(
        refusal.to_string().contains("__bytes__"),
        "the refusal should name the key: {refusal}"
    );
}

/// Only that exact shape. A map that merely mentions the key, or has anything
/// else beside it, is an ordinary map and must stay one.
#[test]
#[dacc_derive::doc_anchor(id = "a-map-that-only-resembles-the-reserved-shape-is-left-alone")]
fn a_map_that_only_resembles_the_reserved_shape_is_left_alone() {
    for entries in [
        BTreeMap::from([
            ("__bytes__".to_owned(), Value::Text("aGk=".to_owned())),
            ("mode".to_owned(), Value::Text("0644".to_owned())),
        ]),
        BTreeMap::from([("__bytes__".to_owned(), Value::Integer(4))]),
        BTreeMap::from([("bytes".to_owned(), Value::Text("aGk=".to_owned()))]),
    ] {
        let value = Value::Map(entries);
        assert_eq!(round_trip(&value), value, "{value:?}");
    }
}

/// Text and bytes holding the same octets are different values, and a digest
/// has to say so or a rotation between them would read as no change.
#[test]
#[dacc_derive::doc_anchor(id = "text-and-bytes-are-not-the-same-value")]
fn text_and_bytes_are_not_the_same_value() {
    use nerpa_core::secret::{Digest, Key};

    let key = Key::from_bytes([3; 32]);
    let as_text = Value::Text("hi".to_owned());
    let as_bytes = Value::Bytes(b"hi".to_vec());

    assert_ne!(as_text, as_bytes);
    assert_ne!(Digest::of(&key, &as_text), Digest::of(&key, &as_bytes));
}

#[test]
fn bytes_read_as_something_a_person_can_look_at() {
    let short = Value::Bytes(vec![0xff, 0xfe, 0x00]);
    assert_eq!(short.to_string(), "<3 bytes: fffe00>");

    let long = Value::Bytes(vec![0xab; 20]);
    assert!(
        long.to_string().starts_with("<20 bytes: abababababababab"),
        "{long}"
    );

    // Two different files of the same size must not read alike, or a plan would
    // show a change as though it were none.
    let other = Value::Bytes(vec![0x01, 0x02, 0x03]);
    assert_ne!(short.to_string(), other.to_string());
}
