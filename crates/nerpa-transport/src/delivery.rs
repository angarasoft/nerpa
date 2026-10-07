//! Getting the executor onto a machine.
//!
//! Delivery comes **before** the protocol: until the binary is there, there is
//! nothing to speak to. So it is not on the wire and cannot be — it uses the
//! transport's own means, which for `ssh` is `ssh`. That is also why the
//! architecture calls the executor a filter that owns no socket: something else
//! has to put it there, and that something is per-transport.
//!
//! # Named by what it is, not by what it claims to be
//!
//! The remote path carries a digest of the binary, so a machine holding a
//! different build holds it under a different name and the two do not fight. "Is
//! the right executor there" becomes a file test rather than a version
//! negotiation, and a copy that was interrupted cannot be mistaken for a
//! complete one — it is written beside and moved into place, because a
//! half-copied file that is executable is worse than no file.
//!
//! # What this does not solve
//!
//! One architecture. The binary here is for one target, and a machine that is
//! not that target is told so by name rather than handed something it cannot
//! run. Building for several and choosing between them is `W-008` and is not
//! done.

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use nerpa_core::Target;
use nerpa_core::provider::ProviderError;

/// Where the executor lives on a machine, under a name that says which one.
fn named(digest: &str) -> String {
    format!(".cache/nerpa/nerpa-executor-{digest}")
}

#[dacc_derive::doc_anchor(id = "inv-delivery-001-1")]
/// The command that says whether the whole executor is already at `path`.
///
/// Executable is not enough. A copy cut short under this name, which the copy
/// below used to leave when its input ended early, passed a file test for ever:
/// delivery was skipped on every run, and the short binary died without a word.
fn present(path: &str, length: usize) -> String {
    format!("test -x {path} && test \"$(($(wc -c < {path})))\" -eq {length}")
}

#[dacc_derive::doc_anchor(id = "inv-delivery-001-2")]
/// The command that puts the executor at `path` from its standard input, whole or
/// not at all, and sweeps every executor this machine no longer needs.
///
/// Written beside, under a name of its own for each attempt, counted, and only
/// then moved into place. `cat` takes an input that ends early for the end of the
/// file, so an `ssh` killed part way left a short copy, and it was moved under the
/// final name, where the test above took it for the executor. A copy an earlier
/// session is still writing never lands on this one's name.
///
/// What earlier attempts left is swept first. A far side that is killed never
/// reaches the removal at the end, and §5.3 of RFC 0001 retries delivery, so
/// without this each attempt left another part of a binary behind for good. A copy
/// another session is writing at that moment goes on writing to a file with no
/// name, and its own move finds nothing, which it reports.
///
/// The sweep at the end is the same idea for whole executors: once the new one
/// is under its name, every other `nerpa-executor-*` is removed, so a long-lived
/// machine holds one executor rather than one per build it was ever converged by
/// (W-008). It cannot fail the copy — `rm -f` succeeds whether the file is there
/// or not — so a machine that holds only the new executor and one that held none
/// both count as swept.
fn copy(path: &str, length: usize) -> String {
    format!(
        "mkdir -p .cache/nerpa && rm -f {path}.part.* && partial={path}.part.$$ && cat > \"$partial\" \
         && test \"$(($(wc -c < \"$partial\")))\" -eq {length} \
         && chmod +x \"$partial\" && mv \"$partial\" {path} \
         && for old in .cache/nerpa/nerpa-executor-*; do [ \"$old\" = \"{path}\" ] || rm -f \"$old\"; done \
         || {{ rm -f \"$partial\"; exit 1; }}"
    )
}

/// Puts the executor on a machine unless the right one is already there.
///
/// Returns the path it can be run by. Cheap when nothing has changed: one `ssh`
/// that tests for a file.
///
/// # Errors
///
/// Returns [`ProviderError::Unreachable`] if the machine cannot be reached, if
/// it is of an architecture this build is not for, or if the copy failed.
///
/// # Panics
///
/// Never in fact: the one `expect` names a property of the digest type —
/// sixty-four hexadecimal characters always carry sixteen — and it is written
/// as a loud stop rather than a silent short name because rule 7 says so.
pub fn deliver(
    node: &Target,
    host: &str,
    config: Option<&str>,
    binary: &Path,
    architecture: &str,
) -> Result<String, ProviderError> {
    let out_of_reach = |problem: String| ProviderError::Unreachable {
        node: node.clone(),
        problem,
    };

    let bytes = std::fs::read(binary)
        .map_err(|error| out_of_reach(format!("{}: {error}", binary.display())))?;
    let digest = blake3::hash(&bytes).to_hex();
    let short = digest.get(..16).expect(
        "a blake3 digest is sixty-four hexadecimal characters, so sixteen are always there",
    );
    let path = named(short);

    // Already there is the common case, and it costs one round trip rather than
    // eight hundred kilobytes.
    let said = asked(node, host, config, &present(&path, bytes.len()), &[])?;
    if said.answered {
        return Ok(path);
    }

    // Asked over ssh rather than through the wire, because the wire needs the
    // thing that is not there yet. This is the chicken-and-egg the architecture
    // note mentions, and asking directly is the honest way out of it.
    let said = asked(node, host, config, "uname -m", &[])?;
    let found = said.text.trim();
    if !said.answered {
        return Err(out_of_reach(format!(
            "it said nothing about itself: {found}"
        )));
    }
    if found != architecture {
        return Err(out_of_reach(format!(
            "it is {found:?} and this nerpa carries an executor for {architecture:?} only"
        )));
    }

    // Written beside, counted, and moved into place. A half-copied file that is
    // executable is worse than no file at all, and an interrupted copy under the
    // final name would pass the test above for ever.
    let said = asked(node, host, config, &copy(&path, bytes.len()), &bytes)?;
    if !said.answered {
        return Err(out_of_reach(format!(
            "the executor could not be put there: {}",
            said.text
        )));
    }
    Ok(path)
}

/// What one `ssh` came back with.
struct Said {
    /// Whether the command on the far side succeeded. False also when nothing
    /// ran because the machine could not be reached — [`Said::text`] is then
    /// `ssh`'s own account, and it is quoted rather than interpreted.
    answered: bool,
    /// What was said, on either stream.
    text: String,
}

/// How many times a connection `ssh` itself could not make is tried.
///
/// Three. What this rides out is a shared bastion dropping connections at random
/// once ten are being made at once, which `MaxStartups` does by design (RFC 0001
/// §5.10): a second or third attempt arrives in a different moment and gets
/// through. More than three is holding a worker of the pool against a machine
/// that is genuinely not there, which the run would rather hear about.
pub(crate) const ATTEMPTS: usize = 3;

/// How long the first wait between attempts is. Each one after it doubles.
pub(crate) const WAITS: std::time::Duration = std::time::Duration::from_millis(200);

/// Why one attempt came back with nothing useful.
///
/// The difference decides whether asking again is worth anything: `ssh` failing
/// to connect is a moment that may pass, and the far side answering is an answer
/// that will not change.
#[derive(Debug)]
pub(crate) enum Refused {
    /// `ssh` never reached the far side, and this is what it said about that.
    Connecting(String),
    /// The far side answered, and this is what it said.
    Answered(String),
}

/// Runs one of the delivery commands, asking again when `ssh` could not connect.
///
/// The three commands are idempotent — a file test, `uname -m`, and a copy
/// written beside and moved into place — so a second attempt costs a round trip
/// and risks nothing. See RFC 0001 §5.3.
///
/// # Errors
///
/// Returns [`ProviderError::Unreachable`], naming the machine and what `ssh` or
/// the far side said.
fn asked(
    node: &Target,
    host: &str,
    config: Option<&str>,
    command: &str,
    stdin: &[u8],
) -> Result<Said, ProviderError> {
    attempted(WAITS, || run(host, config, command, stdin)).map_err(|refused| {
        let problem = match refused {
            Refused::Connecting(problem) | Refused::Answered(problem) => problem,
        };
        ProviderError::Unreachable {
            node: node.clone(),
            problem: format!("{host}: {problem}"),
        }
    })
}

/// The asking again itself, with the command it asks handed in.
///
/// Separate from what it runs so that the rule can be shown rather than argued
/// about: a test drives it with an attempt that fails as many times as it likes,
/// and nothing here needs an `ssh` or a machine.
///
/// The wait doubles and carries jitter, because every machine of a fleet that
/// was dropped by one bastion would otherwise come back at the same instant and
/// be dropped together again. The jitter comes from the clock rather than from a
/// dependency: spreading a handful of retries does not need randomness anybody
/// could bet on.
pub(crate) fn attempted<T>(
    waits: std::time::Duration,
    mut attempt: impl FnMut() -> Result<T, Refused>,
) -> Result<T, Refused> {
    let mut waiting = waits;
    for left in (0..ATTEMPTS).rev() {
        match attempt() {
            Ok(said) => return Ok(said),
            Err(Refused::Answered(problem)) => return Err(Refused::Answered(problem)),
            Err(refused) if left == 0 => return Err(refused),
            Err(_) => {}
        }
        std::thread::sleep(waiting.saturating_add(jitter(waiting)));
        waiting = waiting.saturating_mul(2);
    }
    // Unreachable: the last attempt returns above, whichever way it went.
    Err(Refused::Connecting(
        "it was never asked, which cannot happen".to_owned(),
    ))
}

/// A little less than one wait, from the clock.
fn jitter(waits: std::time::Duration) -> std::time::Duration {
    let spread = waits.as_millis();
    if spread == 0 {
        return std::time::Duration::ZERO;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        // A clock before the epoch has no nanoseconds to spread.
        .map_or(0, |since| u128::from(since.subsec_nanos()));
    let millis = u64::try_from(now % spread)
        .expect("the nanoseconds of a second fit a u64, and so does their remainder");
    std::time::Duration::from_millis(millis)
}

/// One `ssh`, and what it said.
///
/// The command is one string rather than separate words, because that is what
/// `ssh` actually offers: it joins whatever it is given and hands the result to
/// a shell on the far side, so passing words only creates the illusion of
/// quoting. Everything in these commands is written here — a fixed shape and a
/// digest — and nothing in them comes from a configuration.
/// What one `ssh` is asked with, and everything it is asked with.
///
/// The identity is `ssh`'s own — its agent and the configuration this points
/// at. Nothing here names a key, which is `INV-TRUST-001`'s half about static
/// keys, and a test reads every word of this.
fn invocation(host: &str, config: Option<&str>, command: &str) -> Command {
    let mut asking = Command::new("ssh");
    asking
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg(format!("ConnectTimeout={}", crate::ANSWERS_WITHIN));
    if let Some(named) = config {
        asking.arg("-F").arg(named);
    }
    asking.arg(host).arg(command);
    asking
}

fn run(host: &str, config: Option<&str>, command: &str, stdin: &[u8]) -> Result<Said, Refused> {
    let asking = invocation(host, config, command);

    // `ssh` not starting at all is this machine missing it, which asking again
    // will not fix.
    let (status, said, complained) =
        spoken(asking, stdin).map_err(|error| Refused::Answered(error.to_string()))?;

    // 255 is `ssh` failing rather than the far side answering: it could not
    // connect, could not authenticate, was refused. The three commands here
    // return 0 or 1 and never 255, so the code says which of the two happened —
    // and which of them is worth asking again.
    if status.code() == Some(255) {
        return Err(Refused::Connecting(
            String::from_utf8_lossy(&complained).trim().to_owned(),
        ));
    }

    let text = if said.is_empty() {
        String::from_utf8_lossy(&complained).into_owned()
    } else {
        String::from_utf8_lossy(&said).into_owned()
    };
    Ok(Said {
        answered: status.success(),
        text,
    })
}

/// How much of each stream is kept, as a conversation keeps its standard error.
pub(crate) const KEPT: usize = 64 * 1024;

/// Runs a command, writing `stdin` to it while both its streams are read.
///
/// Both at once, on threads of their own. Writing eight hundred kilobytes and only
/// then reading hangs for ever against a far side that fills a pipe with what it
/// says and stops reading — a verbose route, a shell complaining about every line —
/// and delivery is the one place that pushes a whole binary. There is no deadline
/// on it and no way to kill it: it is not a transport, and it holds a worker of the
/// pool. Only the last [`KEPT`] bytes of each stream are kept.
fn spoken(
    mut asking: Command,
    stdin: &[u8],
) -> std::io::Result<(std::process::ExitStatus, Vec<u8>, Vec<u8>)> {
    let mut child = asking
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut input = child.stdin.take();
    let out = child.stdout.take();
    let err = child.stderr.take();

    let (said, complained) = std::thread::scope(|scope| {
        // No pipe is no output, and no bytes is the answer.
        let reading_out = scope.spawn(move || out.map_or_else(Vec::new, tail));
        let reading_err = scope.spawn(move || err.map_or_else(Vec::new, tail));
        if let Some(mut pipe) = input.take() {
            // A far side that has gone, or that refuses the rest, says so by its
            // status; what it wrote is read either way.
            let _ = pipe.write_all(stdin);
        }
        (
            reading_out
                .join()
                .expect("the threads reading the pipes do not panic"),
            reading_err
                .join()
                .expect("the threads reading the pipes do not panic"),
        )
    });

    Ok((child.wait()?, said, complained))
}

/// The last [`KEPT`] bytes of a stream, read to its end so that nothing on the far
/// side is ever left blocked writing.
pub(crate) fn tail(mut stream: impl std::io::Read) -> Vec<u8> {
    let mut kept: Vec<u8> = Vec::new();
    let mut chunk = [0_u8; 8192];
    while let Ok(read) = stream.read(&mut chunk) {
        if read == 0 {
            break;
        }
        kept.extend_from_slice(
            chunk
                .get(..read)
                .expect("a read never returns more than the buffer it read into"),
        );
        if kept.len() > KEPT {
            kept.drain(..kept.len() - KEPT);
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;
    use std::process::{Command, Stdio};

    use super::{copy, named, present};

    /// Runs a delivery command as the far side would, in `dir`, given `input`.
    fn far_side(dir: &std::path::Path, command: &str, input: &[u8]) -> bool {
        let mut shell = Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("a shell");
        if let Some(mut pipe) = shell.stdin.take() {
            pipe.write_all(input).expect("the input is taken");
        }
        shell.wait().expect("the shell ends").success()
    }

    /// A connection `ssh` itself could not make is tried again.
    ///
    /// A shared bastion drops unauthenticated connections at random once ten are
    /// in flight (RFC 0001 §5.10), and the three commands delivery sends are
    /// idempotent: a file test, `uname -m`, and a copy that writes beside and
    /// moves into place. So asking again costs a round trip and risks nothing.
    /// `INV-TRUST-001` names its bypass: a static key reaching the transport.
    /// The invocation is built in one place, and this looks at every word of
    /// it: the identity is `ssh`'s own, and nothing here names a key.
    #[test]
    #[dacc_derive::doc_anchor(id = "no-identity-reaches-the-ssh-command")]
    fn no_identity_reaches_the_ssh_command() {
        let asked = super::invocation("web-01", Some("/tmp/ssh_config"), "nerpa-executor");
        let words: Vec<String> = asked
            .get_args()
            .map(|word| word.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            words,
            [
                "-o".to_owned(),
                "BatchMode=yes".to_owned(),
                "-o".to_owned(),
                format!("ConnectTimeout={}", crate::ANSWERS_WITHIN),
                "-F".to_owned(),
                "/tmp/ssh_config".to_owned(),
                "web-01".to_owned(),
                "nerpa-executor".to_owned(),
            ]
        );
    }

    #[test]
    fn a_connection_ssh_could_not_make_is_tried_again() {
        let tries = std::cell::Cell::new(0);
        let outcome = super::attempted(std::time::Duration::ZERO, || {
            tries.set(tries.get() + 1);
            if tries.get() < 3 {
                Err(super::Refused::Connecting(
                    "kex_exchange_identification".to_owned(),
                ))
            } else {
                Ok("delivered")
            }
        });

        assert_eq!(outcome.ok(), Some("delivered"));
        assert_eq!(tries.get(), 3, "it asked again until it got through");
    }

    /// What the far side answered is not asked again. It would say the same
    /// thing, and a machine that is out of room or of the wrong architecture is
    /// not a machine that was unlucky.
    #[test]
    fn a_far_side_that_answered_is_not_asked_twice() {
        let tries = std::cell::Cell::new(0);
        let outcome = super::attempted(std::time::Duration::ZERO, || {
            tries.set(tries.get() + 1);
            Err::<&str, _>(super::Refused::Answered("no such file".to_owned()))
        });

        assert!(outcome.is_err());
        assert_eq!(tries.get(), 1, "an answer is an answer");
    }

    /// And a connection that never comes gives up rather than holding a worker
    /// of the pool for ever.
    #[test]
    fn a_connection_that_never_comes_gives_up() {
        let tries = std::cell::Cell::new(0);
        let outcome = super::attempted(std::time::Duration::ZERO, || {
            tries.set(tries.get() + 1);
            Err::<&str, _>(super::Refused::Connecting("timed out".to_owned()))
        });

        assert!(outcome.is_err());
        assert_eq!(tries.get(), super::ATTEMPTS);
    }

    /// A far side that fills its standard error while the copy is being written to
    /// it is read as it goes. Writing the whole executor first and reading
    /// afterwards hung for ever, holding a worker of the pool with no deadline and
    /// nothing to kill. Found by the narrow review of RFC 0001's sixth revision.
    #[test]
    fn a_far_side_that_talks_while_it_is_written_to_does_not_hang() {
        let (sending, sent) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut asking = Command::new("sh");
            asking
                .arg("-c")
                .arg("yes error | head -c 400000 >&2; cat > /dev/null");
            let outcome = super::spoken(asking, &vec![7_u8; 800_000]);
            let _ = sending
                .send(outcome.map(|(status, _, complained)| (status.success(), complained.len())));
        });

        let (ended, complained) = sent
            .recv_timeout(std::time::Duration::from_secs(30))
            .expect("it ends rather than hanging")
            .expect("the shell runs");
        assert!(ended, "the far side ended well");
        assert_eq!(
            complained,
            64 * 1024,
            "and the last of what it said was kept"
        );
    }

    /// A copy whose input ends early is never put under the executor's name. It
    /// was: `cat` took the end of a cut-off input for the end of the file, and the
    /// short copy was moved into place, where a file test took it for the executor
    /// on every later run. Found by the review of RFC 0001's fifth revision.
    #[test]
    #[dacc_derive::doc_anchor(id = "an-interrupted-copy-is-never-put-under-the-executors-name")]
    fn an_interrupted_copy_is_never_put_under_the_executors_name() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let path = named("0123456789abcdef");
        let whole = vec![7_u8; 8192];
        let cut = whole.get(..3000).expect("a part of it");

        assert!(
            !far_side(dir.path(), &copy(&path, whole.len()), cut),
            "a short copy is refused"
        );
        assert!(
            !dir.path().join(&path).exists(),
            "and nothing is under the name"
        );
        let beside = std::fs::read_dir(dir.path().join(".cache/nerpa"))
            .expect("the directory")
            .count();
        assert_eq!(beside, 0, "nor beside it");

        assert!(
            far_side(dir.path(), &copy(&path, whole.len()), &whole),
            "a whole copy is put in place"
        );
        assert_eq!(
            std::fs::read(dir.path().join(&path)).expect("there").len(),
            whole.len()
        );
    }

    /// And what an attempt that was killed left behind is swept by the next one. A
    /// far side that is killed never reaches the removal at the end, and delivery is
    /// retried, so each attempt used to leave another part of a binary behind. Found
    /// by the narrow review of RFC 0001's sixth revision.
    #[test]
    fn what_an_earlier_attempt_left_behind_is_swept() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let path = named("0123456789abcdef");
        let beside = dir.path().join(".cache/nerpa");
        std::fs::create_dir_all(&beside).expect("made");
        std::fs::write(
            beside.join("nerpa-executor-0123456789abcdef.part.99"),
            [7_u8; 3000],
        )
        .expect("a part of an earlier attempt");

        let whole = vec![7_u8; 8192];
        assert!(
            far_side(dir.path(), &copy(&path, whole.len()), &whole),
            "the copy is put in place"
        );
        let left: Vec<String> = std::fs::read_dir(&beside)
            .expect("the directory")
            .filter_map(|entry| Some(entry.ok()?.file_name().to_string_lossy().into_owned()))
            .filter(|name| name.contains(".part."))
            .collect();
        assert!(
            left.is_empty(),
            "and nothing of the earlier attempt is left: {left:?}"
        );
    }

    /// A machine converged by several builds holds only the latest: the copy
    /// sweeps the executors it replaces. W-008 watched nothing delete them, so a
    /// long-lived machine accumulated one per build it was ever converged by.
    #[test]
    fn a_copy_sweeps_the_executors_it_replaces() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let path = named("0123456789abcdef");
        let cache = dir.path().join(".cache/nerpa");
        std::fs::create_dir_all(&cache).expect("made");
        std::fs::write(cache.join("nerpa-executor-aaaaaaaaaaaaaaaa"), [1_u8; 64])
            .expect("an old executor");
        std::fs::write(
            cache.join("nerpa-executor-bbbbbbbbbbbbbbbb.part.7"),
            [2_u8; 64],
        )
        .expect("an old partial");

        let whole = vec![7_u8; 8192];
        assert!(
            far_side(dir.path(), &copy(&path, whole.len()), &whole),
            "the copy is put in place"
        );

        let left: Vec<String> = std::fs::read_dir(&cache)
            .expect("the directory")
            .filter_map(|entry| Some(entry.ok()?.file_name().to_string_lossy().into_owned()))
            .collect();
        assert_eq!(left, vec!["nerpa-executor-0123456789abcdef".to_owned()]);
        assert!(dir.path().join(&path).exists(), "the new one stays");
    }

    /// And a short copy already under the name, left by a delivery before this,
    /// is not taken for the executor, so it is delivered again.
    #[test]
    #[dacc_derive::doc_anchor(id = "a-short-copy-already-there-is-not-taken-for-the-executor")]
    fn a_short_copy_already_there_is_not_taken_for_the_executor() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let path = named("0123456789abcdef");
        let at = dir.path().join(&path);
        std::fs::create_dir_all(at.parent().expect("a parent")).expect("made");
        std::fs::write(&at, [7_u8; 3000]).expect("written");
        let mut permissions = std::fs::metadata(&at).expect("there").permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        std::fs::set_permissions(&at, permissions).expect("executable");

        assert!(
            !far_side(dir.path(), &present(&path, 8192), b""),
            "a short executable is not the executor"
        );
        assert!(
            far_side(dir.path(), &present(&path, 3000), b""),
            "one of the right length is"
        );
    }
}
