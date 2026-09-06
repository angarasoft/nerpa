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
use nerpa_proto::{Answer, Ask, Entry, Observation, Operation, Seen};
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
            }],
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
        })
        .expect_err("there is nobody there to answer");
    assert!(
        matches!(refusal, ProviderError::Unreachable { .. }),
        "{refusal:?}"
    );
}
