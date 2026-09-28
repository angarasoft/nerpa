use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-WIRE-003"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("An executor installed before a fix, speaking the current protocol and answering every question correctly, driven as though it were the build with the fix. Or a far end that refuses the question, taken for a yes. Not covered, on purpose: the local route, whose executor is the file beside this program. Covered by a real machine rather than a test: that the CLI's opening calls the check at all, because the route there runs `ssh` by name and a test would have to fake the one thing it is about."), tests: &[crate::anchor::an_executor_from_another_build_is_refused_naming_both, crate::anchor::a_far_end_that_cannot_say_which_build_it_is_is_not_taken_for_the_right_one] }),
    statement: NonEmptyStr::new("A machine reached over ssh is worked on only by the executor build this nerpa carries."),
    rationale: r"The executor is what acts on a machine, often as root, and a fix inside it changes nothing on the wire — so the protocol version cannot tell an executor with the fix from one without. Measured: three machines ran a build from before three fixes through every stage-1 run, and nothing said so. Each connection asks the far end which build it is before anything else, by a digest of its own binary, and a machine that names another build or cannot say is left out and reported with both names. See 0040.",
    enforced_by: &[crate::anchor::inv_wire_003_1, crate::anchor::inv_wire_003_2],
    tests: &[crate::anchor::the_executor_this_nerpa_carries_is_recognised, crate::anchor::an_executor_from_another_build_is_refused_naming_both, crate::anchor::a_far_end_that_cannot_say_which_build_it_is_is_not_taken_for_the_right_one, crate::anchor::which_build_is_read_off_the_binary_that_is_running, crate::anchor::a_build_is_named_by_every_byte_of_it],
);
