//! The two claims a plan number rests on, attacked with generated values.
//!
//! `value::encode` says of itself that "no two different values can encode to
//! the same bytes", and `INV-VALUE-001` registers that. It is an assertion about
//! every value there is, and a list of chosen examples cannot attack it — the
//! interesting inputs are the ones nobody thought to write down.
//!
//! Two properties, and both are about the same failure. If two different worlds
//! encode alike, they get one number and a signature means nothing. If a value
//! changes shape on its way through the state file, a converged world gets a
//! different number on the next run and nothing ever settles.
//!
//! # The alphabet is the test
//!
//! Injectivity is broken by a **minimal pair** — two values differing in one
//! place that encode alike — and two independently generated values almost never
//! form one. They differ somewhere else as well, the encodings differ there, and
//! the property passes while the encoder is broken. Measured, not supposed: with
//! arbitrary strings and `i64`s these tests stayed green against an encoder that
//! wrote a boolean's tag and dropped its value.
//!
//! So the alphabet is tiny on purpose. Four strings, four integers, four byte
//! strings, four keys — a universe small enough that two draws land on the same
//! shape constantly and differ in exactly one leaf. `"a"`, `"b"` and `"ab"` are
//! there so that pieces which could run together do; `__bytes__` is there
//! because random strings never produce it and it is the one key the encoding
//! reserves, which makes it the one place a hand-written escape hatch breaks.
//!
//! Bytes are built through `Value::bytes` rather than the variant, because that
//! constructor is what makes a value canonical: octets that are text become
//! `Text`, and a raw `Value::Bytes` holding text is a shape the model says
//! should not exist.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeMap;

use nerpa_core::secret::{Digest, Key};
use nerpa_core::value::Value;
use proptest::prelude::*;
use proptest::test_runner::TestRunner;

/// Keys: the reserved one, one that resembles it, and two ordinary ones that can
/// run together.
fn key() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("__bytes__".to_owned()),
        Just("bytes".to_owned()),
        Just("a".to_owned()),
        Just("ab".to_owned()),
    ]
}

/// Any value the model can carry, out of an alphabet chosen to collide.
fn value() -> impl Strategy<Value = Value> {
    let text = prop_oneof![
        Just(String::new()),
        Just("a".to_owned()),
        Just("b".to_owned()),
        Just("ab".to_owned()),
    ];
    // Zero and one so that a boolean's byte and an integer's first byte look
    // alike; 256 because a length that is written in too few bytes loses it.
    let number = prop_oneof![Just(0_i64), Just(1), Just(-1), Just(256)];
    // Not text, so that these stay `Bytes` rather than being canonicalised, and
    // one that is, so that the canonicalisation itself is exercised.
    let octets = prop_oneof![
        Just(Vec::new()),
        Just(vec![0xff_u8]),
        Just(vec![0xff_u8, 0xfe]),
        Just(b"a".to_vec()),
    ];
    let leaf = prop_oneof![
        text.prop_map(Value::Text),
        number.prop_map(Value::Integer),
        any::<bool>().prop_map(Value::Boolean),
        octets.prop_map(Value::bytes),
    ];
    leaf.prop_recursive(3, 12, 3, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..3).prop_map(Value::List),
            prop::collection::btree_map(key(), inner, 0..3).prop_map(Value::Map),
        ]
    })
}

fn key_for_digests() -> Key {
    Key::from_bytes([7; 32])
}

/// Written down and read back is the same value, or it was refused — never
/// something else. The refusal has exactly one cause and the message says so,
/// which is what lets a reader decide without guessing.
#[test]
#[dacc_derive::doc_anchor(id = "a-value-either-survives-the-state-file-or-is-refused-by-name")]
fn a_value_either_survives_the_state_file_or_is_refused_by_name() {
    let mut runner = TestRunner::new(ProptestConfig::with_cases(2048));
    runner
        .run(&value(), |value| {
            match serde_json::to_string(&value) {
                Ok(written) => {
                    let read: Value = serde_json::from_str(&written).unwrap();
                    prop_assert_eq!(read, value);
                }
                Err(refusal) => prop_assert!(
                    refusal.to_string().contains("__bytes__"),
                    "the only shape that may be refused is the reserved one: {refusal}"
                ),
            }
            Ok(())
        })
        .unwrap();
}

/// The wire keeps the kinds apart. CBOR was chosen over JSON precisely
/// because it can say "these are bytes", so bytes that came back as a list of
/// integers would be the choice not paying for itself.
#[test]
#[dacc_derive::doc_anchor(id = "a-value-survives-the-wire-as-the-kind-it-was")]
fn a_value_survives_the_wire_as_the_kind_it_was() {
    let mut runner = TestRunner::new(ProptestConfig::with_cases(2048));
    runner
        .run(&value(), |value| {
            let mut written = Vec::new();
            ciborium::into_writer(&value, &mut written).unwrap();
            let read: Value = ciborium::from_reader(written.as_slice()).unwrap();
            prop_assert_eq!(read, value);
            Ok(())
        })
        .unwrap();
}

/// The claim in `value.rs`, stated as a test. Two values that differ in any
/// way at all get different numbers; if they did not, two different worlds
/// would carry one signature.
#[test]
#[dacc_derive::doc_anchor(id = "two-values-that-differ-never-get-the-same-number")]
fn two_values_that_differ_never_get_the_same_number() {
    let mut runner = TestRunner::new(ProptestConfig::with_cases(2048));
    runner
        .run(&(value(), value()), |(left, right)| {
            prop_assume!(left != right);
            prop_assert_ne!(
                Digest::of(&key_for_digests(), &left),
                Digest::of(&key_for_digests(), &right)
            );
            Ok(())
        })
        .unwrap();
}

/// A value that has been through the state file has the number it had. This
/// is the two properties above tied together, and it is the one that decides
/// whether a converged world stays converged: a value that changes shape on
/// the way to disk re-plans for ever, and nothing says why.
#[test]
#[dacc_derive::doc_anchor(id = "a-value-read-back-from-the-state-file-keeps-its-number")]
fn a_value_read_back_from_the_state_file_keeps_its_number() {
    let mut runner = TestRunner::new(ProptestConfig::with_cases(2048));
    runner
        .run(&value(), |value| {
            let Ok(written) = serde_json::to_string(&value) else {
                return Ok(());
            };
            let read: Value = serde_json::from_str(&written).unwrap();
            prop_assert_eq!(
                Digest::of(&key_for_digests(), &read),
                Digest::of(&key_for_digests(), &value)
            );
            Ok(())
        })
        .unwrap();
}

/// A container holding nothing is still a value, and the two empty ones are not
/// each other. Generated tests reach this rarely; it costs one line to be sure.
#[test]
#[dacc_derive::doc_anchor(id = "an-empty-list-and-an-empty-map-are-different-numbers")]
fn an_empty_list_and_an_empty_map_are_different_numbers() {
    let key = key_for_digests();
    assert_ne!(
        Digest::of(&key, &Value::List(Vec::new())),
        Digest::of(&key, &Value::Map(BTreeMap::new()))
    );
}

/// A value arriving from outside is bounded, or every traversal below it is a
/// stack overflow waiting for a big enough input — and an overflow is a fault
/// rather than an unwind, so the state lock's destructor never runs.
///
/// Both directions of both formats, because a state file and the wire are the
/// two places a value is built from something nerpa did not write.
#[test]
fn a_value_nested_past_all_reason_is_refused_rather_than_walked() {
    let deep = |levels: usize| {
        let mut text = String::new();
        for _ in 0..levels {
            text.push('[');
        }
        for _ in 0..levels {
            text.push(']');
        }
        text
    };

    let refused = serde_json::from_str::<Value>(&deep(10_000));
    assert!(
        refused.is_err(),
        "ten thousand levels of list is not a value anybody wrote"
    );

    let shallow = serde_json::from_str::<Value>(&deep(8)).expect("eight is ordinary");
    assert!(matches!(shallow, Value::List(_)));

    // The wire matters more than the file: a frame arrives from a machine, and
    // `MAX_FRAME` admits sixty-four megabytes — room for millions of levels.
    // 0x81 is a CBOR array of one; 0x80 is an empty one.
    let mut wire = vec![0x81_u8; 10_000];
    wire.push(0x80);
    let refused = ciborium::from_reader::<Value, _>(wire.as_slice());
    assert!(
        refused.is_err(),
        "a frame nested past all reason is not a value to walk"
    );
}
