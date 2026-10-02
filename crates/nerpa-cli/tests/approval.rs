//! One party applies and another signs. `INV-TRUST-002`, decisions 0009 and
//! 0067.
//!
//! Run as a person runs it — the built binary — because what this is about is
//! an apply that must not happen without somebody else's signature, and a
//! library cannot show that. The signatures are made by `ssh-keygen` itself:
//! the tests exercise the real mechanism, keys in a scratch directory that
//! removes itself.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Output, Stdio};

/// The namespace the signature is made in, as `0067` names it.
const NAMESPACE: &str = "plan@nerpa.run";

/// Runs nerpa in one scratch directory.
fn nerpa(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nerpa"))
        .args(arguments)
        .arg("--state")
        .arg(root.join("state.json"))
        .arg("-f")
        .arg(root.join("main.star"))
        .arg("--fleet")
        .arg(root.join("fleet.toml"))
        .output()
        .unwrap()
}

/// A configuration with one file, on this machine, and a fleet that says who
/// may sign.
fn world(root: &Path) {
    std::fs::write(
        root.join("main.star"),
        format!(
            "node(\"localhost\")\nresource(\"os:file.note@localhost\", {{\n    \"path\": {:?},\n    \"content\": \"one\\n\",\n}})\n",
            root.join("note").display().to_string()
        ),
    )
    .unwrap();
    std::fs::write(
        root.join("fleet.toml"),
        format!(
            "[approval]\napprovers = {:?}\n",
            root.join("approvers").display().to_string()
        ),
    )
    .unwrap();
}

/// An ed25519 key in the scratch directory, and its public half as a line of
/// an `allowed_signers` file would hold it.
fn keygen(root: &Path, name: &str) -> String {
    let made = Command::new("ssh-keygen")
        .args([
            "-q",
            "-t",
            "ed25519",
            "-N",
            "",
            "-f",
            &root.join(name).display().to_string(),
        ])
        .status()
        .unwrap();
    assert!(made.success(), "ssh-keygen makes keys");
    let said = std::fs::read_to_string(root.join(format!("{name}.pub"))).unwrap();
    let mut parts = said.split_whitespace();
    format!("{} {}", parts.next().unwrap(), parts.next().unwrap())
}

/// The deployment's approvers: a principal and the public half that answers
/// for it.
fn approvers(root: &Path, listed: &[(&str, &str)]) {
    let mut text = String::new();
    for (who, key) in listed {
        text.push_str(who);
        text.push(' ');
        text.push_str(key);
        text.push('\n');
    }
    std::fs::write(root.join("approvers"), text).unwrap();
}

/// The plan as `plan --json` writes it, proposed by alice.
fn planned(root: &Path) -> String {
    let said = nerpa(root, &["plan", "--json", "--as", "alice@example.com"]);
    assert!(
        said.status.success(),
        "{}",
        String::from_utf8_lossy(&said.stderr)
    );
    String::from_utf8_lossy(&said.stdout).into_owned()
}

/// The signature `ssh-keygen` gives over a number, with a named key.
fn signature_of(root: &Path, key: &str, over: &str) -> String {
    let mut child = Command::new("ssh-keygen")
        .args([
            "-Y",
            "sign",
            "-f",
            &root.join(key).display().to_string(),
            "-n",
            NAMESPACE,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(over.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// An approval written by hand: the document `nerpa sign` makes, with any
/// signer at all. For the bypasses that a person with a text editor has.
fn crafted(root: &Path, signer: &str, key: &str) {
    let plan_json = planned(root);
    let over = serde_json::from_str::<serde_json::Value>(&plan_json)
        .unwrap()
        .get("digest")
        .and_then(serde_json::Value::as_str)
        .unwrap()
        .to_owned();
    let hold = serde_json::json!({
        "format": "nerpa-plan-approval-1",
        "signer": signer,
        "plan": serde_json::from_str::<serde_json::Value>(&plan_json).unwrap(),
        "signature": signature_of(root, key, &over),
    });
    std::fs::write(
        root.join("crafted.approval"),
        serde_json::to_string_pretty(&hold).unwrap(),
    )
    .unwrap();
}

/// The whole story: alice proposes, carol signs, alice applies.
#[test]
fn one_party_applies_and_another_signs() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path();
    world(root);
    let carol = keygen(root, "carol");
    approvers(root, &[("carol@example.com", &carol)]);

    let plan_json = planned(root);
    std::fs::write(root.join("plan.json"), &plan_json).unwrap();
    let said = nerpa(
        root,
        &[
            "sign",
            root.join("plan.json").to_str().unwrap(),
            "--as",
            "carol@example.com",
            "--with-key",
            root.join("carol").to_str().unwrap(),
        ],
    );
    assert!(
        said.status.success(),
        "{}",
        String::from_utf8_lossy(&said.stderr)
    );

    let said = nerpa(
        root,
        &[
            "apply",
            "--as",
            "alice@example.com",
            "--approval",
            root.join("plan.json.approval").to_str().unwrap(),
        ],
    );
    assert!(
        said.status.success(),
        "{}",
        String::from_utf8_lossy(&said.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(root.join("note")).unwrap(),
        "one\n",
        "the signed plan is the one that ran"
    );

    // And the log says who signed: the durable half of the two duties.
    let journal = std::fs::read_to_string(root.join("state.json.journal")).unwrap();
    assert!(journal.contains("signed by carol@example.com"), "{journal}");
}

/// `INV-TRUST-002` names its bypass: an apply with nobody's signature on the
/// plan. Where the deployment names who may sign, there is no way round it.
#[test]
#[dacc_derive::doc_anchor(id = "an-unsigned-apply-where-approvals-are-named-is-refused")]
fn an_unsigned_apply_where_approvals_are_named_is_refused() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path();
    world(root);
    let carol = keygen(root, "carol");
    approvers(root, &[("carol@example.com", &carol)]);

    planned(root);
    let said = nerpa(root, &["apply", "--as", "alice@example.com"]);
    assert!(!said.status.success(), "an unsigned apply must not happen");
    let problem = String::from_utf8_lossy(&said.stderr);
    assert!(problem.contains("--approval"), "{problem}");
    assert!(
        !root.join("note").exists(),
        "and nothing at all was written"
    );
}

/// The separation of duties, attacked both ways: `nerpa sign` refuses the
/// proposer, and a self-signed approval written by hand is refused at apply.
#[test]
#[dacc_derive::doc_anchor(id = "the-proposer-cannot-sign-their-own-plan")]
fn the_proposer_cannot_sign_their_own_plan() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path();
    world(root);
    let alice = keygen(root, "alice");
    approvers(root, &[("alice@example.com", &alice)]);

    let plan_json = planned(root);
    std::fs::write(root.join("plan.json"), &plan_json).unwrap();
    let said = nerpa(
        root,
        &[
            "sign",
            root.join("plan.json").to_str().unwrap(),
            "--as",
            "alice@example.com",
            "--with-key",
            root.join("alice").to_str().unwrap(),
        ],
    );
    assert!(!said.status.success(), "a plan is not its own approval");
    let problem = String::from_utf8_lossy(&said.stderr);
    assert!(problem.contains("cannot sign"), "{problem}");

    // The bypass the entry names: a person with a text editor and a key they
    // hold already. The engine's door refuses it, not the signing step.
    crafted(root, "alice@example.com", "alice");
    let said = nerpa(
        root,
        &[
            "apply",
            "--as",
            "alice@example.com",
            "--approval",
            root.join("crafted.approval").to_str().unwrap(),
        ],
    );
    assert!(!said.status.success(), "the engine refuses it too");
    let problem = String::from_utf8_lossy(&said.stderr);
    assert!(
        problem.contains("whoever proposed a plan cannot sign it"),
        "{problem}"
    );
}

/// A signature taken before an out-of-band change is refused, naming what
/// moved — the world changed underneath the approval and the plan this run
/// computed is not the one carol signed.
#[test]
#[dacc_derive::doc_anchor(id = "a-signature-over-another-plan-is-refused-naming-what-moved")]
fn a_signature_over_another_plan_is_refused_naming_what_moved() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path();
    world(root);
    let carol = keygen(root, "carol");
    approvers(root, &[("carol@example.com", &carol)]);

    let plan_json = planned(root);
    std::fs::write(root.join("plan.json"), &plan_json).unwrap();
    let said = nerpa(
        root,
        &[
            "sign",
            root.join("plan.json").to_str().unwrap(),
            "--as",
            "carol@example.com",
            "--with-key",
            root.join("carol").to_str().unwrap(),
        ],
    );
    assert!(said.status.success());

    // Out of band: somebody wrote the file between the signature and the run.
    std::fs::write(root.join("note"), "moved by hand\n").unwrap();

    let said = nerpa(
        root,
        &[
            "apply",
            "--as",
            "alice@example.com",
            "--approval",
            root.join("plan.json.approval").to_str().unwrap(),
        ],
    );
    assert!(!said.status.success(), "the world moved: the apply stops");
    let problem = String::from_utf8_lossy(&said.stderr);
    assert!(
        problem.contains("os:file.note@localhost") && problem.contains("moved"),
        "the refusal names what moved: {problem}"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("note")).unwrap(),
        "moved by hand\n",
        "and nothing was done to it"
    );
}

/// A signer the deployment never named has no key under their name, and a
/// signature from one is refused whatever it says.
#[test]
#[dacc_derive::doc_anchor(id = "a-signer-the-deployment-never-named-is-refused")]
fn a_signer_the_deployment_never_named_is_refused() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path();
    world(root);
    // Carol holds a key; the deployment simply never named her.
    keygen(root, "carol");
    let dave = keygen(root, "dave");
    approvers(root, &[("dave@example.com", &dave)]);

    let plan_json = planned(root);
    std::fs::write(root.join("plan.json"), &plan_json).unwrap();
    let said = nerpa(
        root,
        &[
            "sign",
            root.join("plan.json").to_str().unwrap(),
            "--as",
            "carol@example.com",
            "--with-key",
            root.join("carol").to_str().unwrap(),
        ],
    );
    assert!(
        said.status.success(),
        "signing needs no permission; applying does"
    );

    let said = nerpa(
        root,
        &[
            "apply",
            "--as",
            "alice@example.com",
            "--approval",
            root.join("plan.json.approval").to_str().unwrap(),
        ],
    );
    assert!(!said.status.success(), "carol is not among the approvers");
    let problem = String::from_utf8_lossy(&said.stderr);
    assert!(
        problem.contains("carol@example.com"),
        "the refusal names the signer: {problem}"
    );
}

/// An approval over the plan's number is a signature over somebody's plan and
/// nothing else: a document that is not an approval is refused by name.
#[test]
fn an_approval_of_another_shape_is_refused_by_name() {
    let scratch = tempfile::tempdir().unwrap();
    let root = scratch.path();
    world(root);
    let carol = keygen(root, "carol");
    approvers(root, &[("carol@example.com", &carol)]);
    planned(root);

    std::fs::write(
        root.join("plan.json.approval"),
        "{\"format\":\"something-else\"}\n",
    )
    .unwrap();
    let said = nerpa(
        root,
        &[
            "apply",
            "--as",
            "alice@example.com",
            "--approval",
            root.join("plan.json.approval").to_str().unwrap(),
        ],
    );
    assert!(!said.status.success());
    let problem = String::from_utf8_lossy(&said.stderr);
    assert!(
        problem.contains("not an approval this build can read")
            || problem.contains("shape this build does not know"),
        "{problem}"
    );
}
