//! A machine reached by running a command.
//!
//! The command here is the executor itself. `ssh web-01 nerpa-executor` differs
//! by the `ssh`, and `ssh web-01 sudo -n nerpa-executor` differs by that — which
//! is the whole of decision 0035: privilege is an argument, not a field on the
//! wire.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use nerpa_core::Target;
use nerpa_core::provider::{ProviderError, Reach};
use nerpa_proto::{Answer, Ask, At, Entry, Observation, Operation, Seen};
use nerpa_transport::Piped;

fn node(name: &str) -> Target {
    Target::new(name).expect("a node name")
}

/// The executor built beside this one, since a transport has to run something.
///
/// Found by path rather than by `CARGO_BIN_EXE_*`, which only knows about
/// binaries in the same package. This test binary lives in `target/<profile>/deps`
/// and the executor is its sibling one directory up.
fn executor() -> String {
    let mut path = std::env::current_exe().expect("this test knows where it is");
    path.pop();
    path.pop();
    path.push("nerpa-executor");
    assert!(
        path.exists(),
        "{} is not built; this test needs the whole workspace",
        path.display()
    );
    path.display().to_string()
}

#[test]
fn one_connection_carries_a_whole_run() {
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let file = scratch.path().join("motd");
    let reach = Piped::running(&node("localhost"), &executor(), &[]).expect("it starts");

    let looking = Ask::Look {
        observations: vec![Observation::Path {
            path: file.display().to_string(),
        }],
        seal: None,
    };

    assert_eq!(
        reach.ask(&looking).unwrap(),
        Answer::Found {
            seen: vec![Seen::Nothing]
        }
    );

    let done = reach
        .ask(&Ask::Perform {
            plan: Some([2; 32]),
            premise: Vec::new(),
            operations: vec![Operation::File {
                path: file.display().to_string(),
                content: b"hello\n".to_vec(),
                mode: 0o644,
                secret: false,
            }],
            seal: None,
        })
        .unwrap();
    assert!(matches!(done, Answer::Done { .. }), "{done:?}");

    // The same connection, a third time. One handshake for the whole run is the
    // point: Ansible's default is five or six per task.
    assert_eq!(
        reach.ask(&looking).unwrap(),
        Answer::Found {
            seen: vec![Seen::Entry(Entry::File {
                content: b"hello\n".to_vec(),
                mode: 0o644,
            })]
        }
    );
}

/// A machine that cannot be started is a machine out of reach, which is the one
/// failure a run survives — not a failure about whatever it was going to be
/// asked. See decision 0020.
#[test]
fn a_command_that_does_not_exist_is_a_machine_out_of_reach() {
    let refusal = Piped::running(&node("web-01"), "no-such-program-anywhere", &[])
        .expect_err("it cannot start");

    let ProviderError::Unreachable {
        node: named,
        problem,
    } = refusal
    else {
        panic!("a machine that cannot be started is unreachable, not broken")
    };
    assert_eq!(named.as_str(), "web-01");
    assert!(problem.contains("no-such-program-anywhere"), "{problem}");
}

/// The far end going away mid-run is the same kind of thing, reported the same
/// way, rather than as a mystery about a resource.
#[test]
fn a_far_end_that_dies_is_reported_as_the_machine_and_not_as_a_resource() {
    let reach = Piped::running(
        &node("web-01"),
        "sh",
        &["-c".to_owned(), "exit 0".to_owned()],
    )
    .expect("sh starts");

    let refusal = reach
        .ask(&Ask::Look {
            observations: Vec::new(),
            seal: None,
        })
        .expect_err("there is nobody there to answer");
    assert!(
        matches!(refusal, ProviderError::Unreachable { .. }),
        "{refusal:?}"
    );
}

/// The name of the executor these tests run, as the engine would compute it
/// over the file it carries.
fn carried() -> String {
    nerpa_transport::build_of(&std::fs::read(executor()).expect("the executor can be read"))
}

/// The executor this nerpa carries is recognised, and the connection that asked
/// is still the one the run goes on to use.
#[test]
#[dacc_derive::doc_anchor(id = "the-executor-this-nerpa-carries-is-recognised")]
fn the_executor_this_nerpa_carries_is_recognised() {
    let reach = Piped::running(&node("web-01"), &executor(), &[]).expect("it starts");

    nerpa_transport::recognised(&reach, &node("web-01"), &carried()).expect("the same build");

    let after = reach
        .ask(&Ask::Look {
            observations: Vec::new(),
            seal: None,
        })
        .unwrap();
    assert!(matches!(after, Answer::Found { .. }), "{after:?}");
}

/// An executor from another build answers everything the protocol has, so only
/// asking tells it apart. It is refused naming both builds, because the way out
/// is installing one of them.
#[test]
#[dacc_derive::doc_anchor(id = "an-executor-from-another-build-is-refused-naming-both")]
fn an_executor_from_another_build_is_refused_naming_both() {
    let reach = Piped::running(&node("web-01"), &executor(), &[]).expect("it starts");
    let expected = nerpa_proto::build_of(b"the executor after a fix");

    let refusal = nerpa_transport::recognised(&reach, &node("web-01"), &expected)
        .expect_err("it is another build");

    let ProviderError::Unreachable {
        node: named,
        problem,
    } = refusal
    else {
        panic!(
            "a machine without the executor this nerpa ships is left out, not an error about a resource"
        )
    };
    assert_eq!(named.as_str(), "web-01");
    assert!(
        problem.contains(&carried()[..16]) && problem.contains(&expected[..16]),
        "{problem}"
    );
}

/// A far end that cannot say which build it is is not assumed to be the right one.
#[test]
#[dacc_derive::doc_anchor(
    id = "a-far-end-that-cannot-say-which-build-it-is-is-not-taken-for-the-right-one"
)]
fn a_far_end_that_cannot_say_which_build_it_is_is_not_taken_for_the_right_one() {
    #[derive(Debug)]
    struct Older;
    impl Reach for Older {
        fn ask(&self, _: &Ask) -> Result<Answer, ProviderError> {
            Ok(Answer::Failed {
                at: At::Before,
                problem: "this executor does not understand that request".to_owned(),
            })
        }
    }

    let refusal =
        nerpa_transport::recognised(&Older, &node("web-01"), &nerpa_proto::build_of(b"anything"))
            .expect_err("it did not say");
    assert!(
        matches!(&refusal, ProviderError::Unreachable { problem, .. } if problem.contains("did not say")),
        "{refusal:?}"
    );
}

/// A conversation that broke part way is not asked again as though it were in
/// step. A far end that answered with something unreadable and is still there
/// would otherwise have the next ask wait for ever on an answer that will not
/// come. Nothing but `Drop` used to end a conversation; review found that.
#[test]
fn a_conversation_that_broke_is_not_asked_again_as_though_it_were_in_step() {
    use std::sync::{Arc, mpsc};
    use std::time::Duration;

    // One byte announced, one byte that is not CBOR, and then a far end that
    // listens and never answers.
    let reach = Arc::new(
        Piped::running(
            &node("web-01"),
            "sh",
            &[
                "-c".to_owned(),
                "printf '\\000\\000\\000\\001\\377'; cat > /dev/null".to_owned(),
            ],
        )
        .expect("sh starts"),
    );
    let looking = || Ask::Look {
        observations: Vec::new(),
        seal: None,
    };

    let first = reach.ask(&looking()).expect_err("an unreadable answer");
    assert!(
        matches!(first, ProviderError::Unreachable { .. }),
        "{first:?}"
    );

    let (tell, told) = mpsc::channel();
    let again = Arc::clone(&reach);
    std::thread::spawn(move || {
        let _ = tell.send(again.ask(&looking()));
    });
    let second = told
        .recv_timeout(Duration::from_secs(5))
        .expect("the second ask returns rather than waiting for an answer that will not come");
    let Err(ProviderError::Unreachable { problem, .. }) = second else {
        panic!("a broken conversation is out of reach, and was answered {second:?}")
    };
    assert!(problem.contains("gone away"), "{problem}");
}

/// A far end that says why it is going says it into the refusal.
///
/// An executor built for an older protocol refuses a frame it cannot read by
/// writing to its standard error and exiting without answering — which is how
/// this nerpa's first frame will look to every executor already installed
/// (0040, RFC 0001 §5.3). That standard error went to whatever terminal the run
/// had, so what reached the report was a pipe that broke and nothing about why.
#[test]
fn a_far_end_that_complains_before_it_goes_is_quoted_in_the_refusal() {
    // Said from a file rather than written into the command, because the
    // refusal names the command it ran: a phrase in the arguments would be
    // found there whether or not anybody listened to the far end. The first
    // version of this test passed for exactly that reason.
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let words = scratch.path().join("complaint");
    std::fs::write(&words, "this executor speaks protocol version 4, not 5\n").unwrap();

    let reach = Piped::running(
        &node("web-01"),
        "sh",
        &[
            "-c".to_owned(),
            format!("cat {} >&2; exit 0", words.display()),
        ],
    )
    .expect("sh starts");

    let refusal = reach
        .ask(&Ask::Look {
            observations: Vec::new(),
            seal: None,
        })
        .expect_err("there is nobody there to answer");

    let ProviderError::Unreachable { problem, .. } = &refusal else {
        panic!("a far end that went away is the machine being out of reach: {refusal:?}")
    };
    assert!(
        problem.contains("protocol version 4"),
        "the refusal should carry what the far end said: {problem}"
    );
}

/// A connection `ssh` itself could not make is tried again, handshake and all.
///
/// The same failure delivery rides out: a shared bastion drops connections at
/// random once ten are being made at once, and the handshake is the very next
/// connection after delivery's. Nothing but `Build` has been written by then and
/// no byte of an answer has been read, so starting over costs a connection and
/// risks nothing — which is exactly the condition RFC 0001 §5.3 puts on it.
#[test]
fn a_handshake_ssh_could_not_make_is_tried_again() {
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let counted = scratch.path().join("attempts");
    let far_end = scratch.path().join("far-end");
    std::fs::write(
        &far_end,
        format!(
            "#!/bin/sh\n\
             tries=$(cat {counted} 2>/dev/null || echo 0)\n\
             tries=$((tries + 1))\n\
             echo $tries > {counted}\n\
             if [ $tries -lt 3 ]; then exit 255; fi\n\
             exec {executor}\n",
            counted = counted.display(),
            executor = executor(),
        ),
    )
    .unwrap();
    std::fs::set_permissions(
        &far_end,
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();

    let reach = nerpa_transport::opened(
        &node("web-01"),
        &far_end.display().to_string(),
        &[],
        Some(&carried()),
    )
    .expect("the third connection gets through");

    let answer = reach
        .ask(&Ask::Look {
            observations: Vec::new(),
            seal: None,
        })
        .expect("and the executor behind it answers");
    assert_eq!(answer, Answer::Found { seen: Vec::new() });
    assert_eq!(
        std::fs::read_to_string(&counted).unwrap().trim(),
        "3",
        "it tried until it got through"
    );
}
