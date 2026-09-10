//! The program that acts on one machine, and the only thing that does.
//!
//! Everything the engine decided has already been decided by the time it gets
//! here. Provider logic lives on the control machine
//! ([0032](../../../docs/decisions)); this reads what it is told to read, does
//! what it is told to do, and reports back. It holds no opinion about packages,
//! services or files — only about paths and programs.
//!
//! # How the pieces fit
//!
//! ```text
//!   stdin ── Request ── answer() ─┬─ Look    ── observe each, in order
//!                                 ├─ Perform ── check the premise, then act
//!                                 └─ Fingerprint
//!   stdout ─ Response ────────────┘
//! ```
//!
//! # It owns no socket
//!
//! A filter: bytes in, bytes out, and no idea how it was reached. That is what
//! leaves `ssh`, a package on the machine, or something that listens as delivery
//! choices rather than as different programs — and it is why privilege is the
//! transport's business (`0035`) rather than a field on the wire.
//!
//! # The premise is checked before anything is done
//!
//! Between the engine looking and the engine acting, the machine can change. The
//! batch carries what the plan was computed from; this looks again, compares,
//! and refuses the whole batch if any of it moved. Refusing is not failing —
//! [`Answer::Moved`] says the plan is stale, which is different advice.

use std::fs;
use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::Path;
use std::process::{Command, Stdio};

use nerpa_proto::{Answer, Ask, Entry, Observation, Operation, Premise, Seal, Seen};

/// What this executor says to one request.
#[must_use]
pub fn answer(ask: &Ask) -> Answer {
    match ask {
        Ask::Look { observations, seal } => look(observations, seal.as_ref()),
        Ask::Perform {
            premise,
            operations,
            seal,
            ..
        } => perform(premise, operations, seal.as_ref()),
        Ask::Build => build(),
        // Deliberately not answered yet: a fingerprint is a claim about what
        // this executor last applied, and it has nowhere to remember that. An
        // invented number would be worse than none, because the engine would
        // skip a machine on the strength of it.
        Ask::Fingerprint => Answer::Failed {
            at: 0,
            problem: "this executor does not keep a fingerprint yet".to_owned(),
        },
        _ => Answer::Failed {
            at: 0,
            problem: "this executor does not understand that request".to_owned(),
        },
    }
}

/// Which build this is, read off the binary that is running rather than written
/// into it.
///
/// A number compiled in is a claim somebody has to remember to change, and not
/// changing one is how an executor without three fixes answered a whole stage of
/// runs as though it had them. `/proc/self/exe` is the file this process was
/// started from, still, if it has since been replaced on disk. Where there is no
/// `/proc` this says so rather than naming something.
fn build() -> Answer {
    match fs::read("/proc/self/exe") {
        Ok(binary) => Answer::Build {
            digest: nerpa_proto::build_of(&binary),
        },
        Err(error) => Answer::Failed {
            at: 0,
            problem: format!("this executor cannot read itself to say which build it is: {error}"),
        },
    }
}

fn look(observations: &[Observation], seal: Option<&Seal>) -> Answer {
    let mut seen = Vec::with_capacity(observations.len());
    for (at, observation) in observations.iter().enumerate() {
        match observe(observation, seal) {
            Ok(found) => seen.push(found),
            Err(problem) => return Answer::Failed { at, problem },
        }
    }
    Answer::Found { seen }
}

fn perform(premise: &[Premise], operations: &[Operation], seal: Option<&Seal>) -> Answer {
    // ENFORCES: INV-PREMISE-002
    // Compared sealed or not at all. A premise that arrived bare would be
    // compared against an unkeyed digest, which is the digest a plan's JSON must
    // not carry, so it is refused before anything is looked at. Decision 0041.
    let seal = match (seal, premise.is_empty()) {
        (Some(seal), _) => Some(seal),
        (None, true) => None,
        (None, false) => {
            return Answer::Failed {
                at: 0,
                problem: "a premise arrived without its seal, and one is not compared bare"
                    .to_owned(),
            };
        }
    };

    // ENFORCES: INV-PREMISE-001
    // Before anything, and all of it. A batch acting on half a stale reading is
    // worse than one that did not act at all.
    for (at, held) in premise.iter().enumerate() {
        let Some(seal) = seal else { break };
        match observe(&held.observation, Some(seal)) {
            Ok(now) if nerpa_proto::sealed(seal, &nerpa_proto::digest(&now)) == held.digest => {}
            Ok(_) => return Answer::Moved { at },
            Err(problem) => return Answer::Failed { at, problem },
        }
    }

    let mut became = Vec::with_capacity(operations.len());
    for (at, operation) in operations.iter().enumerate() {
        match act(operation) {
            Ok(result) => became.push(result),
            Err(problem) => return Answer::Failed { at, problem },
        }
    }
    Answer::Done { became }
}

// ENFORCES: INV-READ-001
/// What is at a path, or what a program says.
///
/// Three answers about a path and never two: what is there, that nothing is
/// there, or that it could not tell. A file nobody may open is not a file that
/// is not there, and reporting it as absent is what makes an engine propose
/// creating it and then overwrite it.
fn observe(observation: &Observation, seal: Option<&Seal>) -> Result<Seen, String> {
    match observation {
        Observation::Path { path } => at(Path::new(path)),
        // Followed on purpose. The caller asked what the path holds, not what
        // object is at it, and a link is how a file is usually there.
        Observation::Contents { path } => match fs::read(path) {
            Ok(content) => Ok(Seen::Entry(Entry::File { content, mode: 0 })),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Seen::Nothing),
            Err(error) => Err(format!("{path}: {error}")),
        },
        Observation::Output { program, arguments } => {
            let (status, out, complaint) = run(program, arguments, &[])?;
            Ok(Seen::Said {
                status,
                out,
                complaint,
            })
        }
        // Only what it hashed to leaves, sealed for this machine; the thing itself
        // stays here. For what the engine has to compare and must not hold — a
        // password hash, a secret file. Decision 0041.
        Observation::Sealed { of } => {
            let Some(seal) = seal else {
                return Err("a sealed observation arrived without a seal".to_owned());
            };
            if matches!(of.as_ref(), Observation::Sealed { .. }) {
                return Err("a sealed observation inside another is refused".to_owned());
            }
            let found = observe(of, None)?;
            Ok(Seen::Sealed {
                digest: nerpa_proto::sealed(seal, &nerpa_proto::digest(&found)),
            })
        }
        _ => Err("this executor does not understand that observation".to_owned()),
    }
}

fn at(path: &Path) -> Result<Seen, String> {
    let shown = path.display();
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Seen::Nothing),
        Err(error) => return Err(format!("{shown}: {error}")),
    };

    let kind = metadata.file_type();
    // Asked first, because `symlink_metadata` describes the link and reading the
    // path describes its target. One of each would be a record of an object that
    // does not exist.
    if kind.is_symlink() {
        let target = fs::read_link(path).map_err(|error| format!("{shown}: {error}"))?;
        return Ok(Seen::Entry(Entry::Symlink {
            target: target.display().to_string(),
        }));
    }

    let mode = metadata.permissions().mode() & 0o7777;
    if kind.is_dir() {
        return Ok(Seen::Entry(Entry::Directory { mode }));
    }
    if !kind.is_file() {
        return Err(format!(
            "{shown} is neither a file nor a directory, and this has no way to describe what it is"
        ));
    }

    let content = fs::read(path).map_err(|error| format!("{shown}: {error}"))?;
    Ok(Seen::Entry(Entry::File { content, mode }))
}

fn act(operation: &Operation) -> Result<Seen, String> {
    match operation {
        Operation::Directory { path, mode } => {
            let path = Path::new(path);
            // A symlink here points at a directory the plan does not name.
            // `create_dir_all` sees a link to a directory and calls the work
            // done, and the mode below is `chmod`, which follows the link — so
            // a declared mode would land on whatever somebody else pointed it
            // at, and everything here is privileged. Removing a pointer
            // destroys nothing. The same reasoning as the file below, which had
            // it and this did not.
            if fs::symlink_metadata(path).is_ok_and(|found| found.file_type().is_symlink()) {
                fs::remove_file(path).map_err(|error| format!("{}: {error}", path.display()))?;
            }
            fs::create_dir_all(path).map_err(|error| format!("{}: {error}", path.display()))?;
            fs::set_permissions(path, fs::Permissions::from_mode(*mode))
                .map_err(|error| format!("{}: {error}", path.display()))?;
            at(path)
        }
        Operation::File {
            path,
            content,
            mode,
        } => {
            let path = Path::new(path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("{}: {error}", parent.display()))?;
            }
            // A symlink here points at a file the plan does not name, and writing
            // through it would change that one instead. Removing a pointer
            // destroys nothing.
            if fs::symlink_metadata(path).is_ok_and(|found| found.file_type().is_symlink()) {
                fs::remove_file(path).map_err(|error| format!("{}: {error}", path.display()))?;
            }
            // Opened with the mode it was declared rather than written and then
            // narrowed. `fs::write` opens at 0o666 less whatever umask this
            // process inherited, so a file declared 0600 spent a moment
            // world-readable — and a file's content is one of the two places
            // the protocol says a secret goes. `mode` is set on the open, and
            // set again after, because the open only applies it to a file that
            // did not already exist.
            let mut opened = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(*mode)
                .open(path)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            opened
                .write_all(content)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            drop(opened);
            fs::set_permissions(path, fs::Permissions::from_mode(*mode))
                .map_err(|error| format!("{}: {error}", path.display()))?;
            at(path)
        }
        Operation::Remove { path } => {
            let path = Path::new(path);
            let gone = match fs::symlink_metadata(path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(Seen::Nothing);
                }
                Err(error) => return Err(format!("{}: {error}", path.display())),
                // ENFORCES: INV-DELETE-001
                // Never recursive. A directory this made may since have been
                // filled by somebody else, and removing what nothing recorded is
                // a second, unannounced change.
                Ok(found) if found.is_dir() => fs::remove_dir(path),
                Ok(_) => fs::remove_file(path),
            };
            gone.map_err(|error| format!("{}: {error}", path.display()))?;
            Ok(Seen::Nothing)
        }
        Operation::Process {
            program,
            arguments,
            stdin,
            private,
            expect,
        } => {
            let (status, out, complaint) = run(program, arguments, stdin)?;
            let wanted = if expect.is_empty() { &[0][..] } else { expect };
            if wanted.contains(&status) {
                Ok(Seen::Said {
                    status,
                    out,
                    complaint,
                })
            } else {
                // What went in is not repeated back when it was private, even
                // into an error. An error message is the most reliably logged
                // thing in any system.
                let said = String::from_utf8_lossy(&complaint);
                let quiet = if *private { " (input withheld)" } else { "" };
                Err(format!(
                    "{program} exited {status}, expected one of {wanted:?}{quiet}: {said}"
                ))
            }
        }
        _ => Err("this executor does not understand that operation".to_owned()),
    }
}

/// Runs a program and collects what it did.
///
/// Arguments are passed one at a time and never through a shell, so nobody
/// else's quoting can become somebody else's command.
fn run(
    program: &str,
    arguments: &[String],
    stdin: &[u8],
) -> Result<(i32, Vec<u8>, Vec<u8>), String> {
    let mut child = Command::new(program)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("{program}: {error}"))?;

    // Fed from a thread while this one drains the child's output. Writing it
    // all first deadlocks the moment the input outgrows a pipe buffer and the
    // program says anything before it has finished reading: it blocks on a full
    // stdout nobody is emptying, and this blocks on a stdin it is not reading.
    // A few megabytes of certificate is enough, and `stdin` is where a secret
    // goes.
    let mut fed = Ok(());
    let done = std::thread::scope(|scope| {
        let feeding = child
            .stdin
            .take()
            .map(|mut pipe| scope.spawn(move || pipe.write_all(stdin)));
        let done = child.wait_with_output();
        if let Some(handle) = feeding {
            fed = handle.join().unwrap_or(Ok(()));
        }
        done
    })
    .map_err(|error| format!("{program}: {error}"))?;

    // A program that exits without reading its input closes the pipe, and that
    // is its business rather than a failure — the same thing a shell pipeline
    // does. Anything else that went wrong writing is not.
    if let Err(error) = fed
        && error.kind() != std::io::ErrorKind::BrokenPipe
    {
        return Err(format!("{program}: writing its input: {error}"));
    }

    // A program killed by a signal has no exit code, and calling that zero — or
    // anything else — would be inventing an answer.
    let status = done.status.code().ok_or_else(|| {
        use std::os::unix::process::ExitStatusExt as _;
        match done.status.signal() {
            Some(signal) => format!("{program} was killed by signal {signal}"),
            None => format!("{program} ended without an exit code"),
        }
    })?;

    Ok((status, done.stdout, done.stderr))
}
