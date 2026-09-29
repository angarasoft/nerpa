use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-VALUE-001"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A map whose only key is the reserved one, holding text that decodes as base64, written down and read back as bytes. Or a byte sequence that is valid UTF-8 built as `Value::Bytes` directly rather than through `Value::bytes`, which then never equals the text the provider reads back."), tests: &[crate::anchor::a_map_shaped_like_bytes_is_refused_rather_than_silently_misread, crate::anchor::a_map_that_only_resembles_the_reserved_shape_is_left_alone, crate::anchor::text_and_bytes_are_not_the_same_value, crate::anchor::a_value_either_survives_the_state_file_or_is_refused_by_name] }),
    statement: NonEmptyStr::new("A value written into state or a plan reads back as the value that was written."),
    rationale: r#"State and the plan are the two places a value survives the process that made it, and both are JSON. JSON has no way to say "these are bytes": an array of numbers is indistinguishable from a list of integers, and a string from text. So binary content is written as base64 under a reserved key — and the one map that would read back as bytes rather than as itself is refused at the point of writing, so no reader is ever left to guess. The same value goes onto the wire as a real CBOR byte string, which is the property CBOR was chosen for and had not been asked to do.

Bytes that are text are text. Without that a file holding `hello` could be either variant depending on which side of the model built it, the two would compare unequal, and no second run would ever be empty."#,
    enforced_by: &[crate::anchor::value_encode],
    tests: &[crate::anchor::every_other_shape_still_round_trips, crate::anchor::bytes_survive_a_state_file, crate::anchor::bytes_that_are_text_become_text, crate::anchor::a_value_survives_the_wire_as_the_kind_it_was, crate::anchor::a_value_read_back_from_the_state_file_keeps_its_number],
);
