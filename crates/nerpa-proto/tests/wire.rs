//! What survives the wire, and what it refuses to carry.
//!
//! The refusals matter more than the round trips. Every one of them is a way a
//! peer at the other end of a pipe — confused, old, or hostile — could otherwise
//! make this end do something it should not.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::io::Cursor;

use nerpa_proto::{
    Answer, Ask, At, Entry, MAX_FRAME, Observation, Operation, PROTOCOL_VERSION, Premise, Request,
    Response, Seal, Seen, WireError,
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
        observations: vec![
            Observation::Path {
                path: "/etc/motd".to_owned(),
            },
            Observation::Output {
                program: "qlist".to_owned(),
                arguments: vec!["-I".to_owned()],
            },
        ],
        seal: None,
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
        plan: Some([7; 32]),
        premise: Vec::new(),
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
        seal: None,
    };

    let back = round_trip_request(asked.clone());
    assert_eq!(
        back.body(),
        &asked,
        "including the mode bits and a non-ASCII body"
    );
    let Ask::Perform { operations, .. } = back.body() else {
        panic!("shape changed")
    };
    assert_eq!(operations[1].path(), Some("/etc/app/conf"));
}

#[test]
fn every_answer_survives_the_trip() {
    for answer in [
        Answer::Found {
            seen: vec![
                Seen::Nothing,
                Seen::Entry(Entry::Symlink {
                    target: "/etc/hostname".to_owned(),
                }),
                Seen::Said {
                    status: 0,
                    out: b"nginx-1.27\n".to_vec(),
                    complaint: Vec::new(),
                },
            ],
        },
        Answer::Done {
            became: vec![Seen::Entry(Entry::File {
                content: b"hello\n".to_vec(),
                mode: 0o644,
            })],
        },
        Answer::Moved { at: 2 },
        Answer::Fingerprint {
            digest: "9f86d081".to_owned(),
        },
        Answer::Failed {
            at: At::Operation(2),
            problem: "permission denied".to_owned(),
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
        observations: vec![Observation::Path {
            path: "/etc/motd".to_owned(),
        }],
        seal: None,
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

/// A read interrupted by a signal inside a frame carries on, as one interrupted in
/// its prefix already did. The body turned every error into a lost connection, so
/// a signal handler installed without restarting reads would have lost a machine
/// in the middle of its answer. Found by the review of RFC 0001's fourth revision.
#[test]
fn a_read_interrupted_inside_a_frame_carries_on() {
    /// Gives the bytes up to `at`, is interrupted once there, then gives the rest.
    struct Interrupting {
        bytes: Cursor<Vec<u8>>,
        at: u64,
        interrupted: bool,
    }

    impl std::io::Read for Interrupting {
        fn read(&mut self, into: &mut [u8]) -> std::io::Result<usize> {
            let position = self.bytes.position();
            if position == self.at && !self.interrupted {
                self.interrupted = true;
                return Err(std::io::ErrorKind::Interrupted.into());
            }
            let room = if position < self.at {
                usize::try_from(self.at - position).unwrap()
            } else {
                into.len()
            };
            let take = into.len().min(room);
            std::io::Read::read(&mut self.bytes, &mut into[..take])
        }
    }

    let asked = Ask::Look {
        observations: vec![Observation::Path {
            path: "/etc/motd".to_owned(),
        }],
        seal: None,
    };
    let mut wire = Vec::new();
    nerpa_proto::write(&mut wire, &Request::new(asked.clone())).unwrap();
    let mut reader = Interrupting {
        bytes: Cursor::new(wire),
        at: 7,
        interrupted: false,
    };

    let back = nerpa_proto::read_request(&mut reader).expect("the whole frame");
    assert_eq!(back.body(), &asked);
    assert!(reader.interrupted, "and the read was interrupted inside it");
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
        observations: vec![Observation::Path {
            path: "/etc/motd".to_owned(),
        }],
        seal: None,
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
        plan: Some([7; 32]),
        premise: Vec::new(),
        operations: vec![Operation::File {
            path: "/var/big".to_owned(),
            content: content.clone(),
            mode: 0o644,
        }],
        seal: None,
    };

    let back = round_trip_request(asked);
    let Ask::Perform { operations, .. } = back.body() else {
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

/// The number a premise is checked by. Computed on both ends, so two answers
/// that differ must never hash alike — otherwise a machine that moved reads as
/// one that did not, which is the failure the premise exists to prevent.
#[test]
fn two_different_answers_never_hash_alike() {
    let answers = [
        Seen::Nothing,
        Seen::Entry(Entry::Directory { mode: 0o755 }),
        Seen::Entry(Entry::Directory { mode: 0o750 }),
        Seen::Entry(Entry::File {
            content: b"hello\n".to_vec(),
            mode: 0o644,
        }),
        Seen::Entry(Entry::File {
            content: b"hello\n".to_vec(),
            mode: 0o600,
        }),
        Seen::Entry(Entry::File {
            content: b"hello".to_vec(),
            mode: 0o644,
        }),
        Seen::Entry(Entry::Symlink {
            target: "/etc/hostname".to_owned(),
        }),
        Seen::Said {
            status: 0,
            out: b"ab".to_vec(),
            complaint: b"c".to_vec(),
        },
        // The same bytes, split differently. Without a length in front of each
        // part these two run together into one sequence and collide.
        Seen::Said {
            status: 0,
            out: b"a".to_vec(),
            complaint: b"bc".to_vec(),
        },
        Seen::Said {
            status: 1,
            out: b"ab".to_vec(),
            complaint: b"c".to_vec(),
        },
    ];

    for (index, left) in answers.iter().enumerate() {
        assert_eq!(
            nerpa_proto::digest(left),
            nerpa_proto::digest(left),
            "the same answer twice: {left:?}"
        );
        for right in answers.iter().skip(index + 1) {
            assert_ne!(
                nerpa_proto::digest(left),
                nerpa_proto::digest(right),
                "{left:?} and {right:?} hash alike"
            );
        }
    }
}

/// A batch says which plan it came from and what that plan was reasoning about,
/// and both survive the trip. Without them the far end is being asked to act on
/// a reading it cannot check and an intent it cannot name.
#[test]
fn a_batch_carries_the_plan_it_came_from_and_what_it_assumed() {
    let premise = Premise {
        observation: Observation::Path {
            path: "/etc/motd".to_owned(),
        },
        digest: nerpa_proto::digest(&Seen::Nothing),
    };
    let asked = Ask::Perform {
        plan: Some([9; 32]),
        premise: vec![premise.clone()],
        operations: vec![Operation::Process {
            program: "emerge".to_owned(),
            arguments: vec!["--quiet".to_owned(), "nginx".to_owned()],
            stdin: Vec::new(),
            private: false,
            expect: vec![0],
        }],
        seal: None,
    };

    let mut wire = Vec::new();
    nerpa_proto::write(&mut wire, &Request::new(asked.clone())).unwrap();
    let back = nerpa_proto::read_request(&mut Cursor::new(wire)).unwrap();
    assert_eq!(back.body(), &asked);

    let Ask::Perform {
        plan,
        premise: carried,
        operations,
        ..
    } = back.body()
    else {
        panic!("shape changed")
    };
    assert_eq!(*plan, Some([9; 32]));
    assert_eq!(carried, &vec![premise]);
    assert_eq!(
        operations[0].path(),
        None,
        "a process acts wherever it acts, and this wire cannot say where"
    );
}

/// Which build an executor is travels as a question with nothing in it and an
/// answer that is only a name.
#[test]
fn asking_which_build_survives_the_trip() {
    assert_eq!(round_trip_request(Ask::Build).body(), &Ask::Build);

    let answer = Response::new(Answer::Build {
        digest: nerpa_proto::build_of(b"an executor"),
    });
    let mut wire = Vec::new();
    nerpa_proto::write(&mut wire, &answer).unwrap();
    assert_eq!(
        nerpa_proto::read_response(&mut Cursor::new(wire))
            .unwrap()
            .body(),
        answer.body()
    );
}

/// Two builds a byte apart go by different names, and the same bytes always by
/// the same one. Comparing builds rests on nothing else.
#[test]
fn a_build_is_named_by_every_byte_of_it() {
    assert_eq!(
        nerpa_proto::build_of(b"executor"),
        nerpa_proto::build_of(b"executor")
    );
    assert_ne!(
        nerpa_proto::build_of(b"executor"),
        nerpa_proto::build_of(b"executoR")
    );
    assert_eq!(nerpa_proto::build_of(b"").len(), 64);
}

/// Something a later version asks for, which this one has never heard of.
#[derive(serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum NotYetInvented {
    Teleport { somewhere: String },
}

/// A frame from a later version usually carries something new, and the new thing
/// is not the reason to give. Measured: an executor of version 2, handed the
/// first frame of version 3, said `unknown variant build` — it decoded the body
/// before looking at the stamp. The stamp is read first, so the refusal names
/// the version, which is what somebody reading it can act on.
#[test]
fn a_frame_from_another_version_is_refused_as_a_version_even_when_what_it_asks_is_new() {
    #[derive(serde::Serialize)]
    struct FromTheFuture {
        version: u32,
        body: NotYetInvented,
    }

    let mut wire = Vec::new();
    nerpa_proto::write(
        &mut wire,
        &FromTheFuture {
            version: PROTOCOL_VERSION + 1,
            body: NotYetInvented::Teleport {
                somewhere: "elsewhere".to_owned(),
            },
        },
    )
    .unwrap();

    let refusal = nerpa_proto::read_request(&mut Cursor::new(wire.clone())).unwrap_err();
    assert!(
        matches!(refusal, WireError::Version { found } if found == PROTOCOL_VERSION + 1),
        "{refusal:?}"
    );
    let refusal = nerpa_proto::read_response(&mut Cursor::new(wire)).unwrap_err();
    assert!(
        matches!(refusal, WireError::Version { found } if found == PROTOCOL_VERSION + 1),
        "{refusal:?}"
    );
}

/// Reading the stamp first is not believing the frame first: one of this version
/// asking for something nobody defined is still refused, as unreadable.
#[test]
fn a_frame_of_this_version_asking_for_something_nobody_defined_is_still_refused() {
    #[derive(serde::Serialize)]
    struct Current {
        version: u32,
        body: NotYetInvented,
    }

    let mut wire = Vec::new();
    nerpa_proto::write(
        &mut wire,
        &Current {
            version: PROTOCOL_VERSION,
            body: NotYetInvented::Teleport {
                somewhere: "elsewhere".to_owned(),
            },
        },
    )
    .unwrap();

    let refusal = nerpa_proto::read_request(&mut Cursor::new(wire)).unwrap_err();
    assert!(matches!(refusal, WireError::Malformed(_)), "{refusal:?}");
}

/// A seal travels with the asks that need one, and prints as nothing: a seal in a
/// log is a key in a log.
#[test]
fn a_seal_travels_with_what_needs_one_and_prints_as_nothing() {
    let seal = Seal([9; 32]);
    let look = Ask::Look {
        observations: vec![Observation::Sealed {
            of: Box::new(Observation::Path {
                path: "/etc/shadow".to_owned(),
            }),
        }],
        seal: Some(seal),
    };
    assert_eq!(round_trip_request(look.clone()).body(), &look);

    let perform = Ask::Perform {
        plan: None,
        premise: Vec::new(),
        operations: Vec::new(),
        seal: Some(seal),
    };
    assert_eq!(round_trip_request(perform.clone()).body(), &perform);

    let printed = format!("{look:?} {perform:?}");
    assert!(!printed.contains("9, 9"), "{printed}");
}

/// A sealed digest changes with the seal and with what it seals, and with
/// nothing else.
#[test]
fn a_sealed_digest_changes_with_the_seal_and_with_what_it_seals() {
    let nothing = nerpa_proto::digest(&Seen::Nothing);
    let something = nerpa_proto::digest(&Seen::Entry(Entry::Directory { mode: 0o755 }));
    let one = Seal([1; 32]);
    let two = Seal([2; 32]);

    assert_eq!(
        nerpa_proto::sealed(&one, &nothing),
        nerpa_proto::sealed(&one, &nothing)
    );
    assert_ne!(
        nerpa_proto::sealed(&one, &nothing),
        nerpa_proto::sealed(&two, &nothing)
    );
    assert_ne!(
        nerpa_proto::sealed(&one, &nothing),
        nerpa_proto::sealed(&one, &something)
    );
    assert_ne!(
        nerpa_proto::sealed(&one, &nothing),
        nothing,
        "a sealed digest is not the digest it seals"
    );
}

/// What a sealed observation found travels as a seal, and digests apart from
/// every other answer.
#[test]
fn what_a_sealed_observation_found_survives_the_trip() {
    let found = Response::new(Answer::Found {
        seen: vec![Seen::Sealed { digest: [4; 32] }],
    });
    let mut wire = Vec::new();
    nerpa_proto::write(&mut wire, &found).unwrap();
    assert_eq!(
        nerpa_proto::read_response(&mut Cursor::new(wire))
            .unwrap()
            .body(),
        found.body()
    );
    assert_ne!(
        nerpa_proto::digest(&Seen::Sealed { digest: [0; 32] }),
        nerpa_proto::digest(&Seen::Nothing)
    );
}

/// The body of a frame, without the four bytes that say how long it is.
fn body_of(request: &Request) -> Vec<u8> {
    let mut wire = Vec::new();
    nerpa_proto::write(&mut wire, request).unwrap();
    wire.split_off(4)
}

/// One frame, announced as exactly this many bytes.
fn framed(body: &[u8]) -> Vec<u8> {
    let mut wire = u32::try_from(body.len()).unwrap().to_be_bytes().to_vec();
    wire.extend_from_slice(body);
    wire
}

/// A frame is read to its end, not until it has enough.
///
/// Trailing bytes inside a frame were ignored: the decoder stopped at the end of
/// the shape it wanted and never looked at the rest. What arrives after a
/// complete shape is not a shape this end agreed to read, and a peer that can put
/// bytes there unseen is a peer with somewhere to put them.
#[test]
fn a_frame_with_more_after_its_shape_is_refused() {
    let mut body = body_of(&Request::new(Ask::Fingerprint));
    body.push(0xf6);

    let refusal = nerpa_proto::read_request(&mut Cursor::new(framed(&body))).unwrap_err();
    assert!(matches!(refusal, WireError::Malformed(_)), "{refusal:?}");
}

/// A key this version never defined is refused rather than skipped.
///
/// Skipping one means two ends disagreeing about what they agreed, quietly: the
/// version stamp exists to make that disagreement loud, and a tolerated key walks
/// around it.
#[test]
fn a_frame_with_a_key_nobody_defined_is_refused() {
    #[derive(serde::Serialize)]
    struct WithSomethingExtra {
        version: u32,
        body: Ask,
        extra: u32,
    }

    let mut wire = Vec::new();
    nerpa_proto::write(
        &mut wire,
        &WithSomethingExtra {
            version: PROTOCOL_VERSION,
            body: Ask::Fingerprint,
            extra: 7,
        },
    )
    .unwrap();

    let refusal = nerpa_proto::read_request(&mut Cursor::new(wire)).unwrap_err();
    assert!(matches!(refusal, WireError::Malformed(_)), "{refusal:?}");
}

/// And inside the shapes a frame carries, which is where a later version would
/// put one.
#[test]
fn a_key_nobody_defined_inside_a_frame_is_refused() {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "snake_case")]
    enum LooseObservation {
        Path { path: String, extra: bool },
    }

    #[derive(serde::Serialize)]
    #[serde(rename_all = "snake_case")]
    enum LooseAsk {
        Look {
            observations: Vec<LooseObservation>,
            seal: Option<Seal>,
        },
    }

    #[derive(serde::Serialize)]
    struct Loose {
        version: u32,
        body: LooseAsk,
    }

    let mut wire = Vec::new();
    nerpa_proto::write(
        &mut wire,
        &Loose {
            version: PROTOCOL_VERSION,
            body: LooseAsk::Look {
                observations: vec![LooseObservation::Path {
                    path: "/etc/motd".to_owned(),
                    extra: true,
                }],
                seal: None,
            },
        },
    )
    .unwrap();

    let refusal = nerpa_proto::read_request(&mut Cursor::new(wire)).unwrap_err();
    assert!(matches!(refusal, WireError::Malformed(_)), "{refusal:?}");
}
