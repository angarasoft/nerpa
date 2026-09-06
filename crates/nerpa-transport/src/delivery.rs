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
    let said = run(node, host, &format!("test -x {path}"), &[])?;
    if said.answered {
        return Ok(path);
    }

    // Asked over ssh rather than through the wire, because the wire needs the
    // thing that is not there yet. This is the chicken-and-egg the architecture
    // note mentions, and asking directly is the honest way out of it.
    let said = run(node, host, "uname -m", &[])?;
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

    // Written beside and moved into place. A half-copied file that is executable
    // is worse than no file at all, and an interrupted copy under the final name
    // would pass the test above for ever.
    let partial = format!("{path}.part");
    let script = format!(
        "mkdir -p .cache/nerpa && cat > {partial} && chmod +x {partial} && mv {partial} {path}"
    );
    let said = run(node, host, &script, &bytes)?;
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
fn run(node: &Target, host: &str, command: &str, stdin: &[u8]) -> Result<Said, ProviderError> {
    let out_of_reach = |problem: String| ProviderError::Unreachable {
        node: node.clone(),
        problem: format!("{host}: {problem}"),
    };

    let mut child = Command::new("ssh")
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg(format!("ConnectTimeout={}", crate::ANSWERS_WITHIN))
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
