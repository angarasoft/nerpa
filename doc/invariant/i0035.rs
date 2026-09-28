use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-READ-001"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("Any filesystem call whose error is discarded — `.ok()?` on a read, a metadata call, a `read_link` — turns a failure back into absence, and nothing in the type of `read` prevents it. A provider tested only against its own fake would never notice: an in-memory backend has no permissions to deny and no bytes that are not text."), tests: &[crate::anchor::a_file_that_cannot_be_read_is_refused_rather_than_called_absent, crate::anchor::a_file_that_is_not_text_is_read_as_the_bytes_it_holds, crate::anchor::a_symlink_is_reported_as_a_symlink_and_not_followed, crate::anchor::a_broken_symlink_is_there_rather_than_absent] }),
    statement: NonEmptyStr::new("A read reports that nothing is there only when the world says nothing is there, never because looking failed."),
    rationale: r#"A provider answers one of three things — what is there, that nothing is there, or that it could not tell — and an `Option` can only carry two. Collapsing the third into absence is the worst available answer: the plan proposes creating what already exists, and the apply overwrites it, losing its contents and resetting its mode. A file that may not be opened, one whose bytes are not text, a symlink, a socket: each is something, and none of them is nothing. `ProviderError::Unreachable` already says this about machines, and says why.

Bytes that are not text were briefly refused here and are now simply read, because `Value::Bytes` can hold them. The refusal was the right answer while the model could not describe the file; reading it is the right answer now."#,
    enforced_by: &[crate::anchor::inv_read_001],
    tests: &[crate::anchor::a_file_is_written_and_the_second_run_has_nothing_to_do, crate::anchor::a_file_edited_by_hand_is_brought_back],
);
