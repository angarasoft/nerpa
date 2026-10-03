use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0066",
    specification: crate::rfc::rfc_2026_003,
    status: InvariantStatus::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A far end that answers a write with a digest the write would not have left behind — a truncated write read back as whole, an executor answering about a path something else wrote, a machine lying about what it did. What this entry does not cover: a far end that answers with the digest of the right bytes having written the wrong ones. The check is over what came back, not over what was meant; nothing on the wire can make a liar tell the truth."), tests: &[crate::anchor::a_write_answered_with_the_wrong_digest_is_not_recorded_as_written] }),
    statement: NonEmptyStr::new("INV-WIRE-004: A write is answered with a digest read back, and the engine checks it."),
    rationale: r"A write is the one operation whose outcome cannot be read back afterwards without disturbing it — a secret file looked at is a secret file exposed. So the far end reads the path after writing and answers with what it hashed to, sealed for the machine when what was written is a secret (0041), and the engine compares that against what it knows the path must hash to because it holds what it sent.

An answer that is anything else means the path holds something other than what was written there. Recording it as written would be a record of something nobody made — and the record would look exactly like the truth, so no later reading could correct it. The failure is after the operations ran, which is stated rather than hidden: the write happened, and what it left is not what was sent.",
    enforced_by: &[crate::anchor::inv_wire_004],
    tests: &[crate::anchor::a_write_answered_with_the_wrong_digest_is_not_recorded_as_written],
);
