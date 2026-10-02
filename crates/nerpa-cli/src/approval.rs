//! An approval: somebody else's detached signature over a plan's number.
//!
//! `0009` separates who writes a change from who is allowed to make it: where
//! a deployment says who may sign, `apply` proceeds only over a plan a third
//! party has signed, and whoever proposed the plan cannot sign it. The
//! signature is over the plan's number — the number covers everything the plan
//! intends and whose plan it is (`INV-PLAN-006`) — and the approval carries
//! the plan it approves, so a refusal can name what moved between the two
//! (`INV-TRUST-002`).
//!
//! The mechanism is `0067`: an OpenSSH signature, made with `ssh-keygen -Y
//! sign` and verified with `ssh-keygen -Y verify` against an `allowed_signers`
//! file the fleet document names. nerpa signs nothing itself and reads no
//! private key — `ssh-keygen` reads the key, and the key may have only a
//! public half on disk with the other half in `ssh-agent`, exactly as the
//! transport hands hosts to `ssh`.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use nerpa_core::Actor;
use nerpa_core::address::Address;
use nerpa_core::plan::{AttributeDelta, Change, ChangeKind, Plan};

/// What an approval says it is. A document of another shape is refused by
/// name rather than read as this one.
const FORMAT: &str = "nerpa-plan-approval-1";

/// The namespace the signature is made in, so a signature over a plan cannot
/// be moved onto another kind of document. `ssh-keygen`'s own advice for a
/// private use is a name of this shape (`0067`).
const NAMESPACE: &str = "plan@nerpa.run";

/// An approval of one plan by one signer.
///
/// The plan travels inside it on purpose: a plan is what gets signed and what
/// gets handed to somebody who never saw the configuration, and carrying it is
/// what lets a refusal say what moved rather than only that something did.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Approval {
    /// The shape of this document, [`FORMAT`] or nothing.
    format: String,
    /// Who signed: a principal in the deployment's approvers.
    signer: String,
    /// The plan that was approved.
    plan: Plan,
    /// The detached signature over the plan's number, as `ssh-keygen` writes
    /// one: a block of its own, travelling beside what it signs.
    signature: String,
}

/// Makes an approval: a detached signature over the plan's number, carrying
/// the plan it approves.
///
/// # Errors
///
/// Returns the problem if the plan has no number to sign — one that could not
/// look at everything has none (`INV-REACH-003`) — or if `ssh-keygen` could
/// not be run or would not sign.
pub(crate) fn approved(signer: &str, with_key: &Path, plan: &Plan) -> Result<Approval, String> {
    // Refused here too, so the mistake is named at the act rather than only at
    // the door it would be refused at (`INV-TRUST-002`).
    if plan.actor().map(Actor::who) == Some(signer) {
        return Err(format!(
            "whoever proposed a plan cannot sign it: {signer} proposed this one (0009)"
        ));
    }
    let number = number_of(plan)?;
    Ok(Approval {
        format: FORMAT.to_owned(),
        signer: signer.to_owned(),
        plan: plan.clone(),
        signature: signed(with_key, &number)?,
    })
}

/// Reads an approval back from where it was written.
///
/// # Errors
///
/// Returns the problem if the file is not readable or not an approval this
/// build understands.
pub(crate) fn read(path: &Path) -> Result<Approval, String> {
    let text =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_str(&text).map_err(|error| {
        format!(
            "{}: this is not an approval this build can read: {error}",
            path.display()
        )
    })
}

/// Writes an approval out.
///
/// # Errors
///
/// Returns the problem if it could not be written: an approval that silently
/// did not arrive would leave somebody believing a plan is signed.
pub(crate) fn write(path: &Path, approval: &Approval) -> Result<(), String> {
    let text = serde_json::to_string_pretty(approval)
        .map_err(|error| format!("the approval could not be written as JSON: {error}"))?;
    std::fs::write(path, format!("{text}\n"))
        .map_err(|error| format!("{}: {error}", path.display()))
}

/// Checks an approval against a deployment's approvers and against the plan
/// this run computed, and answers who signed.
///
/// # Errors
///
/// Refuses, by name, when the document is not an approval this build knows,
/// when whoever proposed the plan is the signer, when the signature does not
/// verify under a key the deployment gave that principal, or when this run's
/// plan is not the one that was signed — naming what moved between them.
#[dacc_derive::doc_anchor(id = "plan-approval")]
pub(crate) fn checked(
    approvers: &Path,
    approval: &Approval,
    fresh: &Plan,
) -> Result<String, String> {
    if approval.format != FORMAT {
        return Err(format!(
            "the approval is of a shape this build does not know: it says {:?}, \
             and this build makes and reads {FORMAT}",
            approval.format
        ));
    }

    let signed_plan = &approval.plan;
    // A plan nobody has taken up is nobody's to sign, and a signature over it
    // would be a signature over a number rather than over somebody's plan.
    let Some(proposer) = signed_plan.actor() else {
        return Err(
            "a plan nobody has taken up cannot be approved: there is nobody whose \
             plan it is (INV-TRUST-001)"
                .to_owned(),
        );
    };
    // Separation of duties, checked rather than assumed: whoever writes the
    // change cannot approve it alone (`0009`).
    if proposer.who() == approval.signer {
        return Err(format!(
            "whoever proposed a plan cannot sign it: {} proposed this one and \
             signed it (0009)",
            approval.signer
        ));
    }

    let number = number_of(signed_plan)?;
    verified(approvers, &approval.signer, &approval.signature, &number)?;

    // The signature covers the plan that was carried here, and this run has
    // just computed its own. They are the same plan or the world moved.
    let now = number_of(fresh)?;
    if now != number {
        let mut refused = format!(
            "this plan is not the one {} signed: the world moved between the \
             signature and this run — ",
            approval.signer
        );
        let moved = what_moved(signed_plan, fresh);
        refused.push_str(&moved.join("; "));
        return Err(refused);
    }
    Ok(approval.signer.clone())
}

/// The plan's number, in the form a signature is made over.
fn number_of(plan: &Plan) -> Result<String, String> {
    plan.digest()
        .map(|digest| digest.to_hex())
        .map_err(|problem| {
            format!("a plan that could not look at everything has no number to sign: {problem}")
        })
}

/// What moved between the plan that was signed and the one this run computed,
/// named rather than summarised.
fn what_moved(signed: &Plan, fresh: &Plan) -> Vec<String> {
    let mut moved = Vec::new();
    let was_who = signed.actor().map(Actor::who);
    let is_who = fresh.actor().map(Actor::who);
    if was_who != is_who {
        moved.push(format!(
            "who acts moved: the signed plan is {}'s and this run is {}'s",
            was_who.unwrap_or("nobody"),
            is_who.unwrap_or("nobody")
        ));
    }

    let was: BTreeMap<&Address, &Change> = signed
        .changes()
        .map(|change| (change.address(), change))
        .collect();
    let is: BTreeMap<&Address, &Change> = fresh
        .changes()
        .map(|change| (change.address(), change))
        .collect();
    for (address, change) in &was {
        match is.get(address) {
            None => moved.push(format!(
                "{address} moved: the signed plan had it and this one has dropped it"
            )),
            Some(now) if *now != *change => moved_at(address, change, now, &mut moved),
            Some(_) => {}
        }
    }
    for address in is.keys() {
        if !was.contains_key(address) {
            moved.push(format!(
                "{address} moved: this plan has it and the signed one never did"
            ));
        }
    }
    moved
}

/// One address both plans carry, with a change that is not the same one: the
/// attributes that moved, or the change itself when it is not one attribute.
fn moved_at(address: &Address, was: &Change, is: &Change, moved: &mut Vec<String>) {
    if was.kind() != is.kind() {
        moved.push(format!(
            "{address} moved: {} becomes {}",
            said_kind(was.kind()),
            said_kind(is.kind())
        ));
        return;
    }
    let before: BTreeMap<&str, &AttributeDelta> =
        was.deltas().iter().map(|d| (d.name(), d)).collect();
    let after: BTreeMap<&str, &AttributeDelta> =
        is.deltas().iter().map(|d| (d.name(), d)).collect();
    let mut named = 0;
    for (name, delta) in &before {
        if after.get(name).is_none_or(|now| *now != *delta) {
            moved.push(format!("{address}.{name} moved since the signature"));
            named += 1;
        }
    }
    for name in after.keys() {
        if !before.contains_key(name) {
            moved.push(format!("{address}.{name} moved since the signature"));
            named += 1;
        }
    }
    if named == 0 {
        moved.push(format!("{address} moved since the signature"));
    }
}

/// What a change of this kind is called, in a sentence.
fn said_kind(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Create => "a creation",
        ChangeKind::Update => "an update",
        ChangeKind::Replace => "a replacement",
        ChangeKind::Delete => "a removal",
        ChangeKind::React => "a restart",
        ChangeKind::Adopt => "an adoption",
        ChangeKind::Reorder => "a reorder",
        // A kind this build does not know is named no better than by what it is.
        _ => "a change",
    }
}

/// `ssh-keygen -Y sign`: the detached signature over one message. The message
/// goes in on standard input and the signature comes out on standard output,
/// so nothing is written down beside the key (`0067`).
fn signed(with_key: &Path, number: &str) -> Result<String, String> {
    let key = with_key.display().to_string();
    asked(
        &["-Y", "sign", "-f", &key, "-n", NAMESPACE],
        number,
        &format!("the plan's number could not be signed with {key}"),
    )
}

/// `ssh-keygen -Y verify`: the signature holds for this principal and no other.
/// The message goes in on standard input; the signature is a file because
/// `ssh-keygen` wants one, and it exists only for as long as the question.
fn verified(approvers: &Path, signer: &str, signature: &str, number: &str) -> Result<(), String> {
    let mut held = tempfile::NamedTempFile::new().map_err(|error| {
        format!("there is nowhere to put the signature while it is checked: {error}")
    })?;
    held.write_all(signature.as_bytes()).map_err(|error| {
        format!("the signature could not be written down to be checked: {error}")
    })?;
    let held = held.path().display().to_string();
    let named = approvers.display().to_string();
    asked(
        &[
            "-Y", "verify", "-f", &named, "-I", signer, "-n", NAMESPACE, "-s", &held,
        ],
        number,
        &format!(
            "the signature is not one {signer} gave over this plan, or {signer} \
             is not among those who may sign"
        ),
    )
    .map(|_| ())
}

/// One question to `ssh-keygen`, answered with what it printed or refused
/// with its own reason: a refusal at three in the morning is worth more with
/// the tool's words in it than with a paraphrase.
fn asked(arguments: &[&str], message: &str, refused: &str) -> Result<String, String> {
    let mut child = Command::new("ssh-keygen")
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("ssh-keygen could not be asked: {error} (0067)"))?;
    // A number is sixty-four characters: this cannot block or fail midway.
    let written = child
        .stdin
        .take()
        .map(|mut stdin| stdin.write_all(message.as_bytes()));
    if let Some(Err(error)) = written {
        return Err(format!(
            "ssh-keygen did not take the plan's number: {error}"
        ));
    }
    let answer = child
        .wait_with_output()
        .map_err(|error| format!("ssh-keygen did not answer: {error}"))?;
    if !answer.status.success() {
        let said = String::from_utf8_lossy(&answer.stderr);
        return Err(format!("{refused}: {}", said.trim()));
    }
    Ok(String::from_utf8_lossy(&answer.stdout).into_owned())
}
