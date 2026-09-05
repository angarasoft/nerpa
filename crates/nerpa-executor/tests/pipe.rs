//! The executor as it will actually be used: a program on the other end of a
//! pipe, spoken to in frames.
//!
//! `ssh host nerpa-executor` differs from this by the `ssh`. That is the whole
//! point of decision 0032 — it owns no socket, so the transport underneath is
//! somebody else's problem and this is the same program either way.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::io::Write as _;
use std::process::{Command, Stdio};

use nerpa_proto::{Answer, Ask, Entry, Observation, Operation, Request, Seen};

/// Sends these asks down a pipe to the executor and collects what comes back.
fn through_a_pipe(asks: &[Ask]) -> Vec<Answer> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_nerpa-executor"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the executor should start");

    {
        let mut sending = child.stdin.take().expect("its input");
        for ask in asks {
            nerpa_proto::write(&mut sending, &Request::new(ask.clone())).expect("written");
        }
        sending.flush().expect("flushed");
        // Dropped on purpose: the engine hanging up is how a run ends, and the
        // executor has to treat that as an ending rather than as a failure.
    }

    let mut reading = child.stdout.take().expect("its output");
    let answers: Vec<Answer> = asks
        .iter()
        .map(|_| {
            nerpa_proto::read_response(&mut reading)
                .expect("an answer for every ask")
                .body()
                .clone()
        })
        .collect();

    let done = child.wait_with_output().expect("it should finish");
    assert!(
        done.status.success(),
        "the executor should end cleanly when the engine hangs up: {}",
        String::from_utf8_lossy(&done.stderr)
    );
    answers
}

#[test]
fn a_whole_conversation_survives_the_pipe() {
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let file = scratch.path().join("motd");

    let answers = through_a_pipe(&[
        Ask::Look {
            observations: vec![Observation::Path {
                path: file.display().to_string(),
            }],
        },
        Ask::Perform {
            plan: Some([4; 32]),
            premise: Vec::new(),
            operations: vec![Operation::File {
                path: file.display().to_string(),
                content: b"hello\n".to_vec(),
                mode: 0o644,
            }],
        },
        Ask::Look {
            observations: vec![Observation::Path {
                path: file.display().to_string(),
            }],
        },
    ]);

    assert_eq!(
        answers[0],
        Answer::Found {
            seen: vec![Seen::Nothing]
        },
        "there was nothing there to begin with"
    );
    assert!(
        matches!(&answers[1], Answer::Done { became } if matches!(&became[0], Seen::Entry(Entry::File { content, .. }) if content == b"hello\n")),
        "{:?}",
        answers[1]
    );
    assert_eq!(
        answers[2],
        Answer::Found {
            seen: vec![Seen::Entry(Entry::File {
                content: b"hello\n".to_vec(),
                mode: 0o644,
            })]
        },
        "and looking again finds what was written"
    );
}

/// One journey, everything on the machine. The reason this wire exists at all.
#[test]
fn one_frame_carries_a_whole_machine_worth_of_looking() {
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let observations: Vec<Observation> = (0..64)
        .map(|n| Observation::Path {
            path: scratch
                .path()
                .join(format!("file-{n}"))
                .display()
                .to_string(),
        })
        .collect();

    let answers = through_a_pipe(&[Ask::Look {
        observations: observations.clone(),
    }]);
    let Answer::Found { seen } = &answers[0] else {
        panic!("shape changed")
    };
    assert_eq!(
        seen.len(),
        observations.len(),
        "one answer for each, in order"
    );
}
