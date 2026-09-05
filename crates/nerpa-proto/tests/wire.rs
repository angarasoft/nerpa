//! What survives the wire, and what it refuses to carry.
//!
//! The refusals matter more than the round trips. Every one of them is a way a
//! peer at the other end of a pipe — confused, old, or hostile — could otherwise
//! make this end do something it should not.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeMap;
use std::io::Cursor;

use nerpa_proto::{
    Answer, Ask, Entry, MAX_FRAME, Operation, PROTOCOL_VERSION, Request, Response, WireError,
};

fn round_trip_request(body: Ask) -> Request {
    let request = Request::new(body);
    let mut wire = Vec::new();
    nerpa_proto::write(&mut wire, &request).unwrap();
    nerpa_proto::read_request(&mut Cursor::new(wire)).unwrap()
}

#[test]
fn a_batch_of_paths_survives_the_trip() {
    let asked = Ask::Look {
        paths: vec!["/etc/motd".to_owned(), "/etc/hosts".to_owned()],
    };
    let back = round_trip_request(asked.clone());
    assert_eq!(back.body(), &asked);
    assert_eq!(
        back.version(),
        PROTOCOL_VERSION,
        "and it is stamped on the way out"
    );
}

#[test]
fn every_operation_survives_the_trip() {
    let asked = Ask::Perform {
        operations: vec![
            Operation::Directory {
                path: "/etc/app".to_owned(),
                mode: 0o755,
            },
            Operation::File {
                path: "/etc/app/conf".to_owned(),
                content: "listen 80;\nserver_name пример;\n".into(),
                mode: 0o640,
            },
            Operation::Remove {
                path: "/etc/old".to_owned(),
            },
        ],
    };

    let back = round_trip_request(asked.clone());
    assert_eq!(
        back.body(),
        &asked,
        "including the mode bits and a non-ASCII body"
    );
    let Ask::Perform { operations } = back.body() else {
        panic!("shape changed")
    };
    assert_eq!(operations[1].path(), "/etc/app/conf");
}

#[test]
fn every_answer_survives_the_trip() {
    let mut entries = BTreeMap::new();
    entries.insert(
        "/etc/motd".to_owned(),
        Entry::File {
            content: "hello\n".into(),
            mode: 0o644,
        },
    );
    entries.insert("/etc".to_owned(), Entry::Directory { mode: 0o755 });

    for answer in [
        Answer::Found {
            entries: entries.clone(),
        },
        Answer::Done { entries },
        Answer::Fingerprint {
            digest: "9f86d081".to_owned(),
        },
        Answer::Failed {
            path: "/etc/motd".to_owned(),
            problem: "permission denied".to_owned(),
            completed: 2,
        },
    ] {
        let response = Response::new(answer.clone());
        let mut wire = Vec::new();
        nerpa_proto::write(&mut wire, &response).unwrap();
        let back = nerpa_proto::read_response(&mut Cursor::new(wire)).unwrap();
        assert_eq!(back.body(), &answer);
        assert_eq!(back.version(), PROTOCOL_VERSION);
    }
}

/// An executor built from one version outlives the engine that installed it, so
/// a version nobody understands is refused rather than partly believed.
#[test]
fn a_frame_from_another_version_is_refused_rather_than_guessed_at() {
    // Written by hand, because the type will not build a version it does not
    // have — which is the point: only the far end can produce this.
    #[derive(serde::Serialize)]
    struct FromTheFuture {
        version: u32,
        body: Ask,
    }

    let mut wire = Vec::new();
    nerpa_proto::write(
        &mut wire,
        &FromTheFuture {
            version: PROTOCOL_VERSION + 7,
            body: Ask::Fingerprint,
        },
    )
    .unwrap();

    let refusal = nerpa_proto::read_request(&mut Cursor::new(wire.clone())).unwrap_err();
    assert!(matches!(refusal, WireError::Version { found } if found == PROTOCOL_VERSION + 7));

    // Carried through rather than replaced on the way in, so a refusal can say
    // which version it actually was.
    let raw: Request = nerpa_proto::read(&mut Cursor::new(wire)).unwrap();
    assert_eq!(raw.version(), PROTOCOL_VERSION + 7);
}

/// The same for an answer.
#[test]
fn a_response_carries_the_version_it_was_written_by() {
    #[derive(serde::Serialize)]
    struct FromTheFuture {
        version: u32,
        body: Answer,
    }

    let mut wire = Vec::new();
    nerpa_proto::write(
        &mut wire,
        &FromTheFuture {
            version: PROTOCOL_VERSION + 3,
            body: Answer::Fingerprint {
                digest: "00".to_owned(),
            },
        },
    )
    .unwrap();

    let raw: Response = nerpa_proto::read(&mut Cursor::new(wire.clone())).unwrap();
    assert_eq!(raw.version(), PROTOCOL_VERSION + 3);

    let refusal = nerpa_proto::read_response(&mut Cursor::new(wire)).unwrap_err();
    assert!(matches!(refusal, WireError::Version { found } if found == PROTOCOL_VERSION + 3));
}

/// The length prefix is an instruction to allocate, and it arrives from
/// somewhere else. Refused before a byte is reserved.
#[test]
fn an_enormous_announced_frame_is_refused_before_anything_is_allocated() {
    let mut wire = u32::MAX.to_be_bytes().to_vec();
    wire.extend_from_slice(b"and then nothing like that much");

    let refusal: WireError = nerpa_proto::read::<_, Request>(&mut Cursor::new(wire)).unwrap_err();
    assert!(
        matches!(refusal, WireError::TooLarge { size } if size > MAX_FRAME),
        "{refusal:?}"
    );
}

#[test]
fn a_pipe_that_ends_mid_frame_says_so() {
    let request = Request::new(Ask::Look {
        paths: vec!["/etc/motd".to_owned()],
    });
    let mut wire = Vec::new();
    nerpa_proto::write(&mut wire, &request).unwrap();
    wire.truncate(wire.len() - 3);

    let refusal: WireError = nerpa_proto::read::<_, Request>(&mut Cursor::new(wire)).unwrap_err();
    let WireError::Truncated { read, expected } = refusal else {
        panic!("expected a truncated frame, got {refusal:?}");
    };
    assert_eq!(expected - read, 3, "and says exactly how much was missing");
}

#[test]
fn a_pipe_that_ends_before_the_prefix_is_a_closed_connection() {
    let refusal: WireError =
        nerpa_proto::read::<_, Request>(&mut Cursor::new(vec![1, 2])).unwrap_err();
    assert!(matches!(refusal, WireError::Connection(_)), "{refusal:?}");
}

#[test]
fn bytes_that_are_not_the_expected_shape_are_refused() {
    let mut wire = 4_u32.to_be_bytes().to_vec();
    wire.extend_from_slice(&[0xff, 0xff, 0xff, 0xff]);

    let refusal: WireError = nerpa_proto::read::<_, Request>(&mut Cursor::new(wire)).unwrap_err();
    assert!(matches!(refusal, WireError::Malformed(_)), "{refusal:?}");
}

/// Frames follow one another on the same pipe, and reading one must leave the
/// next where it is.
#[test]
fn frames_follow_one_another_on_one_pipe() {
    let first = Request::new(Ask::Fingerprint);
    let second = Request::new(Ask::Look {
        paths: vec!["/etc/motd".to_owned()],
    });

    let mut wire = Vec::new();
    nerpa_proto::write(&mut wire, &first).unwrap();
    nerpa_proto::write(&mut wire, &second).unwrap();

    let mut pipe = Cursor::new(wire);
    assert_eq!(
        nerpa_proto::read_request(&mut pipe).unwrap().body(),
        first.body()
    );
    assert_eq!(
        nerpa_proto::read_request(&mut pipe).unwrap().body(),
        second.body()
    );
}

/// A file's contents travel inside a frame, so the bound has to be generous
/// enough for real ones.
#[test]
fn a_large_but_reasonable_file_still_fits() {
    let content = vec![b'x'; 4 * 1024 * 1024];
    let asked = Ask::Perform {
        operations: vec![Operation::File {
            path: "/var/big".to_owned(),
            content: content.clone(),
            mode: 0o644,
        }],
    };

    let back = round_trip_request(asked);
    let Ask::Perform { operations } = back.body() else {
        panic!("shape changed")
    };
    let Operation::File {
        content: carried, ..
    } = &operations[0]
    else {
        panic!("shape changed")
    };
    assert_eq!(carried.len(), content.len());
}
