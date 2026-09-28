//! A password, all the way through a run, and everywhere it must not appear.
//!
//! The claim decision 0027 makes is negative — that a value is *not* somewhere —
//! and a negative claim is only worth as much as the search behind it. So these
//! tests take a real credential through a real filesystem and then go looking
//! for it in the state file, in the plan, and in what the plan is rendered from.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::fs;
use std::path::PathBuf;

use nerpa_core::address::address;
use nerpa_core::planner::plan;
use nerpa_core::provider::Machines;
use nerpa_core::provider::Providers;
use nerpa_core::run::{apply, observe};
use nerpa_core::secret::{Key, Recorded, Remembered, Secrets, Source};
use nerpa_core::state::State;
use nerpa_core::{Graph, Resource};
use nerpa_provider_os::Os;
use nerpa_transport::Here;

/// Every machine is this one, reached by the executor in this process — the same
/// code a far end runs. Where the pipe itself matters it is tested where the
/// pipe is the subject.
fn machines() -> Machines {
    Machines::new().everywhere(Box::new(Here))
}

/// A run that does not care where state goes, only what ends up in it.
#[allow(
    clippy::unnecessary_wraps,
    reason = "the shape belongs to `apply`, not to this function"
)]
fn kept(_: &State) -> Result<(), String> {
    Ok(())
}

/// The credential this whole file is about. Written once so that every search
/// below is looking for the same bytes.
const PASSWORD: &str = "correct-horse-battery-staple";

static DIGEST_KEY: Key = Key::from_bytes([7; 32]);

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("nerpa-secret-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("a scratch directory");
        Self(path)
    }

    fn join(&self, name: &str) -> String {
        self.0.join(name).display().to_string()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn at(path: &str) -> Source {
    Source::Stored {
        provider: "remembered".to_owned(),
        path: path.to_owned(),
        version: None,
    }
}

/// One file whose contents are a secret held in the store.
fn graph_of(path: &str) -> Graph {
    let mut graph = Graph::new();
    let resource = Resource::new(address("os:file.credentials@localhost"))
        .with("path", path)
        .with("content", at("db/password"));
    if let Some(target) = resource.address().target() {
        graph.preexisting(target.clone());
    }
    graph.insert(resource).expect("a graph of one file");
    graph
}

/// The whole loop, and then a search for the password in everything the run
/// wrote down.
#[test]
#[dacc_derive::doc_anchor(id = "a-password-reaches-the-file-and-nothing-else")]
fn a_password_reaches_the_file_and_nothing_else() {
    let scratch = Scratch::new("through");
    let path = scratch.join("credentials");
    let graph = graph_of(&path);

    let store = Remembered::new().holding("db/password", PASSWORD);
    let secrets = Secrets::new(&DIGEST_KEY, &store);
    let providers = Providers::new().with(Box::new(Os::new()));
    let mut state = State::new();

    let observed = observe(&graph, &providers, &machines(), &state).expect("readable");
    let intended =
        plan(&graph, &observed, &state, &providers.schema(), secrets).expect("plannable");

    // The plan is what a person reads and what a machine is asked to approve.
    let rendered = format!("{intended:?}");
    assert!(
        !rendered.contains(PASSWORD),
        "the plan carries the password: {rendered}"
    );
    let as_json = serde_json::to_string(&intended).expect("a plan serialises");
    assert!(
        !as_json.contains(PASSWORD),
        "the machine-facing plan carries the password: {as_json}"
    );

    apply(
        &intended,
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets,
        &mut kept,
    )
    .expect("applies");

    // The provider did get the real value: this is a credentials file, and a
    // masked one would be useless.
    assert_eq!(fs::read_to_string(&path).expect("written"), PASSWORD);

    // And state has the digest instead.
    let record = state
        .get(&address("os:file.credentials@localhost"))
        .expect("recorded");
    assert!(
        matches!(
            record.attributes().get("content"),
            Some(Recorded::Secret(_))
        ),
        "content should have been reduced to a digest: {:?}",
        record.attributes()
    );
    assert!(
        !record.locator().contains_key("content"),
        "a secret is never handed back to a provider from state"
    );
    let written = format!("{:?}", record.attributes());
    assert!(
        !written.contains(PASSWORD),
        "state holds the password: {written}"
    );
}

/// The second run is empty, which is the property that makes a tool convergent —
/// and it has to survive an attribute whose value the plan never saw.
#[test]
fn a_secret_that_did_not_change_converges() {
    let scratch = Scratch::new("converge");
    let path = scratch.join("credentials");
    let graph = graph_of(&path);

    let store = Remembered::new().holding("db/password", PASSWORD);
    let secrets = Secrets::new(&DIGEST_KEY, &store);
    let providers = Providers::new().with(Box::new(Os::new()));
    let mut state = State::new();

    let observed = observe(&graph, &providers, &machines(), &state).expect("readable");
    let first = plan(&graph, &observed, &state, &providers.schema(), secrets).expect("plannable");
    apply(
        &first,
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets,
        &mut kept,
    )
    .expect("applies");

    let again = observe(&graph, &providers, &machines(), &state).expect("readable");
    let second = plan(&graph, &again, &state, &providers.schema(), secrets).expect("plannable");
    assert!(
        second.is_empty(),
        "the second run should have nothing to do: {second:?}"
    );
}

/// Rotating the credential in the store is a change, and the plan says so
/// without saying what to.
#[test]
#[dacc_derive::doc_anchor(id = "a-rotated-secret-is-reported-as-changed")]
fn a_rotated_secret_is_reported_as_changed() {
    let scratch = Scratch::new("rotated");
    let path = scratch.join("credentials");
    let graph = graph_of(&path);

    let providers = Providers::new().with(Box::new(Os::new()));
    let mut state = State::new();

    let before = Remembered::new().holding("db/password", PASSWORD);
    let secrets = Secrets::new(&DIGEST_KEY, &before);
    let observed = observe(&graph, &providers, &machines(), &state).expect("readable");
    let first = plan(&graph, &observed, &state, &providers.schema(), secrets).expect("plannable");
    apply(
        &first,
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets,
        &mut kept,
    )
    .expect("applies");

    let after = Remembered::new().holding("db/password", "something-else-entirely");
    let rotated = Secrets::new(&DIGEST_KEY, &after);
    let again = observe(&graph, &providers, &machines(), &state).expect("readable");
    let second = plan(&graph, &again, &state, &providers.schema(), rotated).expect("plannable");

    assert_eq!(second.changes().count(), 1, "the rotation is one change");
    let rendered = format!("{second:?}");
    assert!(
        !rendered.contains("something-else-entirely") && !rendered.contains(PASSWORD),
        "neither credential belongs in the plan: {rendered}"
    );
}

/// A store that cannot answer stops the plan. Reporting "unchanged" would be the
/// answer somebody believes, and the one that could be wrong.
#[test]
fn a_secret_that_cannot_be_fetched_stops_the_plan() {
    let scratch = Scratch::new("missing");
    let path = scratch.join("credentials");
    let graph = graph_of(&path);

    let store = Remembered::new();
    let secrets = Secrets::new(&DIGEST_KEY, &store);
    let providers = Providers::new().with(Box::new(Os::new()));
    let state = State::new();

    let observed = observe(&graph, &providers, &machines(), &state).expect("readable");
    let refusal = plan(&graph, &observed, &state, &providers.schema(), secrets)
        .expect_err("a secret that is not there is not a plan");
    assert!(
        refusal.to_string().contains("db/password"),
        "the refusal should name what was looked for: {refusal}"
    );
}

/// A secret already on the machine, rotated in the store. The plan carries what it
/// read so the executor can look again before writing, and what it read is the old
/// credential: unsealed, that premise put an unkeyed digest of it into the plan's
/// JSON, where a dictionary defeats it. Watched failing before the seal existed,
/// and the rotation still goes through the sealed premise it carries now. See
/// decision 0041.
#[test]
#[dacc_derive::doc_anchor(id = "a-rotated-secret-leaves-no-unkeyed-digest-of-the-old-one-in-the-plan")]
fn a_rotated_secret_leaves_no_unkeyed_digest_of_the_old_one_in_the_plan() {
    use std::os::unix::fs::PermissionsExt as _;

    let scratch = Scratch::new("premise");
    let path = scratch.join("credentials");
    fs::write(&path, PASSWORD).expect("the old secret is on disk");
    let mode = fs::metadata(&path).expect("there").permissions().mode() & 0o7777;
    let graph = graph_of(&path);

    let store = Remembered::new().holding("db/password", "an-entirely-new-password");
    let secrets = Secrets::new(&DIGEST_KEY, &store);
    let providers = Providers::new().with(Box::new(Os::new()));
    let mut state = State::new();

    let observed = observe(&graph, &providers, &machines(), &state).expect("readable");
    let intended =
        plan(&graph, &observed, &state, &providers.schema(), secrets).expect("plannable");
    let carried: usize = intended
        .changes()
        .map(|change| change.premise().len())
        .sum();
    assert!(
        carried > 0,
        "the rotation carries a premise, or there is nothing to test"
    );

    let unkeyed = nerpa_proto::digest(&nerpa_proto::Seen::Entry(nerpa_proto::Entry::File {
        content: PASSWORD.as_bytes().to_vec(),
        mode,
    }));
    let needle = serde_json::to_string(&unkeyed).expect("a digest serialises");
    let as_json = serde_json::to_string(&intended).expect("a plan serialises");
    assert!(
        !as_json.contains(needle.trim_matches(['[', ']'])),
        "the plan's JSON carries an unkeyed digest of the secret on disk"
    );

    apply(
        &intended,
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets,
        &mut kept,
    )
    .expect("applies through the sealed premise");
    assert_eq!(
        fs::read_to_string(&path).expect("written"),
        "an-entirely-new-password"
    );
}
