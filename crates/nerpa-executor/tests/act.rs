//! What the executor does when it is asked, and what it refuses to do.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use nerpa_executor::answer;
use nerpa_proto::{Answer, Ask, Entry, Observation, Operation, Premise, Seal, Seen};

/// What every premise in these tests is sealed with, standing for one machine's.
const SEAL: Seal = Seal([5; 32]);

fn scratch() -> tempfile::TempDir {
    tempfile::tempdir().expect("a scratch directory")
}

fn at(path: &Path) -> Observation {
    Observation::Path {
        path: path.display().to_string(),
    }
}

fn look(observations: Vec<Observation>) -> Answer {
    answer(&Ask::Look {
        observations,
        seal: None,
    })
}

fn perform(premise: Vec<Premise>, operations: Vec<Operation>) -> Answer {
    answer(&Ask::Perform {
        plan: Some([1; 32]),
        premise,
        operations,
        seal: Some(SEAL),
    })
}

/// Three answers about a path, never two.
#[test]
fn looking_at_a_path_says_what_is_there_or_that_nothing_is() {
    let scratch = scratch();
    let file = scratch.path().join("motd");
    let link = scratch.path().join("link");
    fs::write(&file, "hello\n").unwrap();
    std::os::unix::fs::symlink(&file, &link).unwrap();

    let Answer::Found { seen } = look(vec![
        at(&file),
        at(&scratch.path().join("absent")),
        at(&link),
        at(scratch.path()),
    ]) else {
        panic!("looking should have found things")
    };

    assert!(matches!(&seen[0], Seen::Entry(Entry::File { content, .. }) if content == b"hello\n"));
    assert_eq!(seen[1], Seen::Nothing);
    assert!(
        matches!(&seen[2], Seen::Entry(Entry::Symlink { .. })),
        "a link is reported, not followed"
    );
    assert!(matches!(seen[3], Seen::Entry(Entry::Directory { .. })));
}

#[test]
fn a_path_it_may_not_read_is_a_failure_rather_than_nothing() {
    let scratch = scratch();
    let file = scratch.path().join("secret");
    fs::write(&file, "hello\n").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).unwrap();
    assert!(
        fs::read(&file).is_err(),
        "this test needs a file it cannot read; running as root?"
    );

    let Answer::Failed { at: index, problem } = look(vec![at(&file), at(&file)]) else {
        panic!("an unreadable file is not an absent one")
    };
    assert_eq!(index, 0, "and it names which one stopped it");
    assert!(problem.contains("Permission denied"), "{problem}");
}

#[test]
fn looking_at_a_program_reports_what_it_said() {
    let Answer::Found { seen } = look(vec![Observation::Output {
        program: "echo".to_owned(),
        arguments: vec!["nginx-1.27".to_owned()],
    }]) else {
        panic!("a program can be looked at")
    };
    let Seen::Said { status, out, .. } = &seen[0] else {
        panic!("shape changed")
    };
    assert_eq!(*status, 0);
    assert_eq!(out, b"nginx-1.27\n");
}

#[test]
fn what_it_did_is_read_back_rather_than_echoed() {
    let scratch = scratch();
    let file = scratch.path().join("nested").join("motd");

    let Answer::Done { became } = perform(
        Vec::new(),
        vec![Operation::File {
            path: file.display().to_string(),
            content: b"hello\n".to_vec(),
            mode: 0o600,
        }],
    ) else {
        panic!("it should have written the file")
    };
    assert!(
        matches!(&became[0], Seen::Entry(Entry::File { content, mode }) if content == b"hello\n" && *mode == 0o600)
    );
    assert_eq!(fs::read(&file).unwrap(), b"hello\n");
}

/// Between the looking and the doing, the machine can change. Our window is the
/// whole run where Ansible's is one task, so the batch carries what it assumed.
#[test]
fn a_batch_whose_premise_moved_does_nothing_and_says_which() {
    let scratch = scratch();
    let watched = scratch.path().join("conf");
    let written = scratch.path().join("motd");
    fs::write(&watched, "as planned\n").unwrap();

    let held = Premise {
        observation: at(&watched),
        digest: nerpa_proto::sealed(
            &SEAL,
            &nerpa_proto::digest(&Seen::Entry(Entry::File {
                content: b"as planned\n".to_vec(),
                mode: fs::metadata(&watched).unwrap().permissions().mode() & 0o7777,
            })),
        ),
    };
    let write = Operation::File {
        path: written.display().to_string(),
        content: b"hello\n".to_vec(),
        mode: 0o644,
    };

    assert!(
        matches!(
            perform(vec![held.clone()], vec![write.clone()]),
            Answer::Done { .. }
        ),
        "the premise holds, so the work happens"
    );

    fs::remove_file(&written).unwrap();
    fs::write(&watched, "somebody else was here\n").unwrap();

    let moved = perform(vec![held], vec![write]);
    assert_eq!(
        moved,
        Answer::Moved { at: 0 },
        "it should refuse and say which"
    );
    assert!(
        !written.exists(),
        "and nothing at all should have been done"
    );
}

/// A process that ends in a way nobody asked for is a failure, and what went
/// into it is not repeated back when it was private — an error message is the
/// most reliably logged thing in any system.
#[test]
fn a_private_input_is_not_repeated_into_the_failure() {
    let Answer::Failed { problem, .. } = perform(
        Vec::new(),
        vec![Operation::Process {
            program: "sh".to_owned(),
            arguments: vec!["-c".to_owned(), "cat >/dev/null; exit 3".to_owned()],
            stdin: b"hunter2".to_vec(),
            private: true,
            expect: vec![0],
        }],
    ) else {
        panic!("exit 3 was not asked for")
    };
    assert!(!problem.contains("hunter2"), "{problem}");
    assert!(problem.contains("input withheld"), "{problem}");
}

#[test]
fn an_exit_code_that_was_asked_for_is_not_a_failure() {
    let Answer::Done { became } = perform(
        Vec::new(),
        vec![Operation::Process {
            program: "sh".to_owned(),
            arguments: vec!["-c".to_owned(), "exit 1".to_owned()],
            stdin: Vec::new(),
            private: false,
            expect: vec![0, 1],
        }],
    ) else {
        panic!("1 was asked for")
    };
    assert!(matches!(became[0], Seen::Said { status: 1, .. }));
}

/// A directory this made may since have been filled by somebody else.
#[test]
fn removing_a_directory_does_not_take_what_is_in_it() {
    let scratch = scratch();
    let directory = scratch.path().join("conf.d");
    fs::create_dir(&directory).unwrap();
    let stranger = directory.join("theirs.conf");
    fs::write(&stranger, "not ours").unwrap();

    let Answer::Failed { problem, .. } = perform(
        Vec::new(),
        vec![Operation::Remove {
            path: directory.display().to_string(),
        }],
    ) else {
        panic!("a directory with something in it is not empty")
    };
    assert!(problem.contains("conf.d"), "{problem}");
    assert_eq!(fs::read_to_string(&stranger).unwrap(), "not ours");
}

/// A fingerprint is a claim about what this last applied, and it has nowhere to
/// remember that. An invented number would be worse than none: the engine would
/// skip a machine on the strength of it.
#[test]
fn a_fingerprint_is_refused_rather_than_invented() {
    assert!(matches!(answer(&Ask::Fingerprint), Answer::Failed { .. }));
}

/// Two questions about one path, and the answers differ on purpose.
///
/// "What is at this path" reports a link, because managing a path means knowing
/// what object is there — a record of one object's mode with another's contents
/// describes nothing that exists. "What does this path hold" follows it, because
/// `/etc/os-release` is a link on most machines and what the asker wants is what
/// it says. Found by running it on a real one.
#[test]
fn asking_what_a_path_holds_follows_a_link_and_asking_what_is_there_does_not() {
    let scratch = scratch();
    let real = scratch.path().join("os-release");
    let link = scratch.path().join("link");
    fs::write(&real, "ID=gentoo\n").unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();

    let Answer::Found { seen } = look(vec![
        at(&link),
        Observation::Contents {
            path: link.display().to_string(),
        },
    ]) else {
        panic!("both are answerable")
    };

    assert!(
        matches!(&seen[0], Seen::Entry(Entry::Symlink { .. })),
        "what is at the path is a link"
    );
    assert!(
        matches!(&seen[1], Seen::Entry(Entry::File { content, .. }) if content == b"ID=gentoo\n"),
        "what the path holds is what it points at"
    );
}

#[test]
fn a_path_that_holds_nothing_because_it_is_not_there_says_so() {
    let scratch = scratch();
    let Answer::Found { seen } = look(vec![Observation::Contents {
        path: scratch.path().join("absent").display().to_string(),
    }]) else {
        panic!("answerable")
    };
    assert_eq!(seen[0], Seen::Nothing);
}

/// Input larger than a pipe buffer, to a program that says something before it
/// has finished reading. Writing it all first and only then draining the child
/// deadlocked: it blocked on a full stdout nobody was emptying, and this blocked
/// on a stdin it was not reading. A few megabytes of certificate is enough, and
/// `stdin` is where a secret goes.
#[test]
fn input_larger_than_a_pipe_buffer_does_not_hold_both_ends() {
    let bytes = vec![b'x'; 4 * 1024 * 1024];
    let Answer::Done { became } = perform(
        Vec::new(),
        vec![Operation::Process {
            program: "cat".to_owned(),
            arguments: Vec::new(),
            stdin: bytes.clone(),
            private: false,
            expect: vec![0],
        }],
    ) else {
        panic!("cat reads its input and writes it back")
    };
    let Seen::Said { status, out, .. } = &became[0] else {
        panic!("a process says what it said")
    };
    assert_eq!(*status, 0);
    assert_eq!(out.len(), bytes.len(), "all of it came back");
}

/// A program that exits without reading is not a failure — that is what a shell
/// pipeline does too, and the closed pipe is its choice rather than a fault.
#[test]
fn a_program_that_does_not_read_its_input_is_not_a_failure() {
    let Answer::Done { became } = perform(
        Vec::new(),
        vec![Operation::Process {
            program: "true".to_owned(),
            arguments: Vec::new(),
            stdin: vec![b'y'; 1024 * 1024],
            private: false,
            expect: vec![0],
        }],
    ) else {
        panic!("true succeeds whatever it is handed")
    };
    assert!(matches!(became[0], Seen::Said { status: 0, .. }));
}

/// Which build this is comes off the binary that is running, so it is the name
/// the engine computes over the file it carries — here, the test binary this
/// executor is linked into.
#[test]
fn which_build_is_read_off_the_binary_that_is_running() {
    let running = fs::read(std::env::current_exe().unwrap()).unwrap();
    assert_eq!(
        answer(&Ask::Build),
        Answer::Build {
            digest: nerpa_proto::build_of(&running)
        }
    );
}

/// What `conf` holds in the premise tests below, as a premise digest before it is
/// sealed.
fn as_planned(watched: &Path) -> [u8; 32] {
    nerpa_proto::digest(&Seen::Entry(Entry::File {
        content: b"as planned\n".to_vec(),
        mode: fs::metadata(watched).unwrap().permissions().mode() & 0o7777,
    }))
}

/// Compared sealed or not at all. A premise that arrives bare would be compared
/// against an unkeyed digest, the digest a plan's JSON must not carry.
#[test]
fn a_premise_that_arrives_without_its_seal_is_refused_before_anything_is_done() {
    let scratch = scratch();
    let watched = scratch.path().join("conf");
    let written = scratch.path().join("motd");
    fs::write(&watched, "as planned\n").unwrap();

    let refused = answer(&Ask::Perform {
        plan: Some([1; 32]),
        premise: vec![Premise {
            observation: at(&watched),
            digest: as_planned(&watched),
        }],
        operations: vec![Operation::File {
            path: written.display().to_string(),
            content: b"hello\n".to_vec(),
            mode: 0o644,
        }],
        seal: None,
    });

    let Answer::Failed { problem, .. } = refused else {
        panic!("a premise without its seal must be refused, and was answered {refused:?}")
    };
    assert!(problem.contains("seal"), "{problem}");
    assert!(!written.exists(), "and nothing at all was done");
}

/// A premise sealed for one machine does not hold on another, over the very same
/// bytes: a seal is the machine's own.
#[test]
fn a_premise_sealed_for_another_machine_does_not_hold() {
    let scratch = scratch();
    let watched = scratch.path().join("conf");
    let written = scratch.path().join("motd");
    fs::write(&watched, "as planned\n").unwrap();

    let elsewhere = Premise {
        observation: at(&watched),
        digest: nerpa_proto::sealed(&Seal([6; 32]), &as_planned(&watched)),
    };
    let write = Operation::File {
        path: written.display().to_string(),
        content: b"hello\n".to_vec(),
        mode: 0o644,
    };

    assert_eq!(
        perform(vec![elsewhere], vec![write]),
        Answer::Moved { at: 0 }
    );
    assert!(!written.exists(), "and nothing at all was done");
}

/// A sealed observation answers with what the thing hashed to, sealed, and never
/// with the thing.
#[test]
fn a_sealed_observation_answers_with_a_seal_and_never_the_contents() {
    let scratch = scratch();
    let secret = scratch.path().join("shadow");
    fs::write(&secret, "correct-horse-battery-staple\n").unwrap();
    let mode = fs::metadata(&secret).unwrap().permissions().mode() & 0o7777;
    let sealed_look = |seal: Option<Seal>| {
        answer(&Ask::Look {
            observations: vec![Observation::Sealed {
                of: Box::new(at(&secret)),
            }],
            seal,
        })
    };

    let Answer::Found { seen } = sealed_look(Some(SEAL)) else {
        panic!("a sealed observation with its seal is answered")
    };
    let expected = nerpa_proto::sealed(
        &SEAL,
        &nerpa_proto::digest(&Seen::Entry(Entry::File {
            content: b"correct-horse-battery-staple\n".to_vec(),
            mode,
        })),
    );
    assert_eq!(seen, vec![Seen::Sealed { digest: expected }]);
    assert!(
        !format!("{seen:?}").contains("correct-horse"),
        "the contents never leave"
    );

    assert!(
        matches!(sealed_look(None), Answer::Failed { .. }),
        "without a seal there is nothing to answer with"
    );
    let nested = answer(&Ask::Look {
        observations: vec![Observation::Sealed {
            of: Box::new(Observation::Sealed {
                of: Box::new(at(&secret)),
            }),
        }],
        seal: Some(SEAL),
    });
    assert!(matches!(nested, Answer::Failed { .. }), "{nested:?}");
}
