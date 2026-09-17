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

// ENFORCES: INV-DELIVERY-001
/// The command that says whether the whole executor is already at `path`.
///
/// Executable is not enough. A copy cut short under this name, which the copy
/// below used to leave when its input ended early, passed a file test for ever:
/// delivery was skipped on every run, and the short binary died without a word.
fn present(path: &str, length: usize) -> String {
    format!("test -x {path} && test \"$(($(wc -c < {path})))\" -eq {length}")
}

// ENFORCES: INV-DELIVERY-001
/// The command that puts the executor at `path` from its standard input, whole or
/// not at all.
///
/// Written beside, under a name of its own for each attempt, counted, and only
/// then moved into place. `cat` takes an input that ends early for the end of the
/// file, so an `ssh` killed part way left a short copy, and it was moved under the
/// final name, where the test above took it for the executor. A copy an earlier
/// session is still writing never lands on this one's name.
fn copy(path: &str, length: usize) -> String {
    format!(
        "mkdir -p .cache/nerpa && partial={path}.part.$$ && cat > \"$partial\" \
         && test \"$(($(wc -c < \"$partial\")))\" -eq {length} \
         && chmod +x \"$partial\" && mv \"$partial\" {path} \
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
    let short = digest.get(..16).unwrap_or_default();
    let path = named(short);

    // Already there is the common case, and it costs one round trip rather than
    // eight hundred kilobytes.
    let said = run(node, host, config, &present(&path, bytes.len()), &[])?;
    if said.answered {
        return Ok(path);
    }

    // Asked over ssh rather than through the wire, because the wire needs the
    // thing that is not there yet. This is the chicken-and-egg the architecture
    // note mentions, and asking directly is the honest way out of it.
    let said = run(node, host, config, "uname -m", &[])?;
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
    let said = run(node, host, config, &copy(&path, bytes.len()), &bytes)?;
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

/// One `ssh`, and what it said.
///
/// The command is one string rather than separate words, because that is what
/// `ssh` actually offers: it joins whatever it is given and hands the result to
/// a shell on the far side, so passing words only creates the illusion of
/// quoting. Everything in these commands is written here — a fixed shape and a
/// digest — and nothing in them comes from a configuration.
fn run(
    node: &Target,
    host: &str,
    config: Option<&str>,
    command: &str,
    stdin: &[u8],
) -> Result<Said, ProviderError> {
    let out_of_reach = |problem: String| ProviderError::Unreachable {
        node: node.clone(),
        problem: format!("{host}: {problem}"),
    };

    let mut spoken = Command::new("ssh");
    spoken
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg(format!("ConnectTimeout={}", crate::ANSWERS_WITHIN));
    if let Some(named) = config {
        spoken.arg("-F").arg(named);
    }
    let mut child = spoken
        .arg(host)
        .arg(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| out_of_reach(error.to_string()))?;

    if let Some(mut pipe) = child.stdin.take() {
        pipe.write_all(stdin)
            .map_err(|error| out_of_reach(error.to_string()))?;
    }
    let done = child
        .wait_with_output()
        .map_err(|error| out_of_reach(error.to_string()))?;

    // 255 is `ssh` failing rather than the far side answering: it could not
    // connect, could not authenticate, was refused. The three commands here
    // return 0 or 1 and never 255, so the code says which of the two happened,
    // and a machine that was never reached is not asked again.
    if done.status.code() == Some(255) {
        return Err(out_of_reach(
            String::from_utf8_lossy(&done.stderr).trim().to_owned(),
        ));
    }

    let text = if done.stdout.is_empty() {
        String::from_utf8_lossy(&done.stderr).into_owned()
    } else {
        String::from_utf8_lossy(&done.stdout).into_owned()
    };
    Ok(Said {
        answered: done.status.success(),
        text,
    })
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

    /// A copy whose input ends early is never put under the executor's name. It
    /// was: `cat` took the end of a cut-off input for the end of the file, and the
    /// short copy was moved into place, where a file test took it for the executor
    /// on every later run. Found by the review of RFC 0001's fifth revision.
    #[test]
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

    /// And a short copy already under the name, left by a delivery before this,
    /// is not taken for the executor, so it is delivered again.
    #[test]
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
