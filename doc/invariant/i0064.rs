use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-LOG-001"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("An entry rewritten in place, or taken out of the middle, while the rest of the log is left exactly as it was — one line and no tooling. What this entry does not cover: the tail truncated, and the whole log rewritten with recomputed hashes. Both are named in the rationale rather than claimed away, and the second is what authorship rests with the detached signature of 0009 for."), tests: &[crate::anchor::a_rewritten_entry_is_found_and_named, crate::anchor::a_removed_entry_is_found] }),
    statement: NonEmptyStr::new("The run log is an append-only hash chain: an entry rewritten, reordered or removed breaks it and is named, from the first entry on."),
    rationale: r#"0009 makes the log an append-only hash chain because the journal is where what has to be believed rather than merely read moves: who acted, and what the run was. Every entry's hash is taken over its own line and the hash of the entry before it, so an edit to a standing record cannot hide inside an otherwise intact log — the chain breaks at the entry that moved, `nerpa journal verify` names it, and it says how much of the record stands rather than answering only "broken".

What a chain over itself cannot hold is said here rather than left implied. Truncating the tail leaves a chain that verifies, which is why the count is part of the answer and the log is not proof of its own completeness. Rewriting the whole log with recomputed hashes verifies too: the chain makes an edit to a standing record visible and says nothing about who wrote any of it — that is what the detached signature of 0009 is for, and claiming otherwise here would make this entry the one a reader trusts wrongly."#,
    enforced_by: &[crate::anchor::inv_log_001],
    tests: &[crate::anchor::a_rewritten_entry_is_found_and_named, crate::anchor::a_removed_entry_is_found],
);
