//! What the executor does when it is asked, and what it refuses to do.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use nerpa_executor::answer;
use nerpa_proto::{Answer, Ask, Entry, Observation, Operation, Premise, Seen};

fn scratch() -> tempfile::TempDir {
    tempfile::tempdir().expect("a scratch directory")
}

fn at(path: &Path) -> Observation {
    Observation::Path {
        path: path.display().to_string(),
    }
}

fn look(observations: Vec<Observation>) -> Answer {
    answer(&Ask::Look { observations })
}

fn perform(premise: Vec<Premise>, operations: Vec<Operation>) -> Answer {
    answer(&Ask::Perform {
        plan: Some([1; 32]),
        premise,
        operations,
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
        digest: nerpa_proto::digest(&Seen::Entry(Entry::File {
            content: b"as planned\n".to_vec(),
            mode: fs::metadata(&watched).unwrap().permissions().mode() & 0o7777,
        })),
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
