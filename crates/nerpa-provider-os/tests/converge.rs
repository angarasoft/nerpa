//! The whole loop, on a real filesystem: read, plan, apply, plan again.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::fs;
use std::path::PathBuf;

use nerpa_core::Observed;
use nerpa_core::address::address;
use nerpa_core::attribute::{Choice, Fact};
use nerpa_core::planner::plan;
use nerpa_core::provider::Machines;
use nerpa_core::provider::{Attributes, Provider, Providers};
use nerpa_core::run::{apply, observe};
use nerpa_core::secret::{Key, Remembered, Secrets};
use nerpa_core::state::State;
use nerpa_core::{ChangeKind, Graph, Reference, Resource, Target, Value};
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

/// A fixed key and a store holding nothing.
///
/// No test in this file declares a secret; planning still has to be told where
/// one would be fetched from, and a fixed key keeps digests reproducible.
static DIGEST_KEY: Key = Key::from_bytes([7; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

fn secrets() -> Secrets<'static> {
    Secrets::new(&DIGEST_KEY, &NO_STORE)
}

/// A directory that removes itself, so a failing test cannot leave litter behind.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("nerpa-{name}-{}", std::process::id()));
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

/// A node by name, for a choice that has to say whose fact it reads.
fn node(name: &str) -> Target {
    Target::new(name).expect("a node name")
}

fn providers() -> Providers {
    Providers::new().with(Box::new(Os::new()))
}

fn graph_of(resources: Vec<Resource>) -> Graph {
    let mut graph = Graph::new();
    for resource in resources {
        if let Some(target) = resource.address().target() {
            graph.preexisting(target.clone());
        }
        graph.insert(resource).expect("distinct addresses");
    }
    graph
}

/// Reads, plans and applies once, and says what failed if anything did.
fn run_once(graph: &Graph, state: &mut State) -> Result<(), String> {
    let providers = providers();
    let observed =
        observe(graph, &providers, &machines(), state).map_err(|error| error.to_string())?;
    let planned = plan(graph, &observed, state, &providers.schema(), secrets())
        .map_err(|error| error.to_string())?;
    apply(
        &planned,
        &providers,
        &machines(),
        &observed,
        state,
        secrets(),
        &mut kept,
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

/// What the next run would do.
fn next_plan(graph: &Graph, state: &State) -> nerpa_core::Plan {
    let providers = providers();
    let observed = observe(graph, &providers, &machines(), state).expect("readable");
    plan(graph, &observed, state, &providers.schema(), secrets()).expect("plannable")
}

#[test]
fn a_file_is_written_and_the_second_run_has_nothing_to_do() {
    let scratch = Scratch::new("write");
    let path = scratch.join("motd");
    let providers = providers();
    let mut state = State::new();

    let graph = graph_of(vec![
        Resource::new(address("os:file.motd@localhost"))
            .with("path", path.clone())
            .with("content", "hello\n")
            .with("mode", "0640"),
    ]);

    let before = observe(&graph, &providers, &machines(), &state).expect("readable");
    assert!(before.is_empty(), "nothing exists yet");

    let first = plan(&graph, &before, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(first.blast_radius().creates(), 1);

    let applied = apply(
        &first,
        &providers,
        &machines(),
        &before,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect("applies");
    assert_eq!(applied.changed(), 1);
    assert_eq!(fs::read_to_string(&path).expect("written"), "hello\n");

    // The property the whole tool exists to have.
    let after = observe(&graph, &providers, &machines(), &state).expect("readable");
    let second = plan(&graph, &after, &state, &providers.schema(), secrets()).expect("plannable");
    assert!(
        second.is_empty(),
        "the second run must change nothing: {second:?}"
    );
}

#[test]
fn a_file_edited_by_hand_is_brought_back() {
    let scratch = Scratch::new("drift");
    let path = scratch.join("nginx.conf");
    let providers = providers();
    let mut state = State::new();
    let graph = graph_of(vec![
        Resource::new(address("os:file.nginx.conf@localhost"))
            .with("path", path.clone())
            .with("content", "listen 80;\n"),
    ]);

    let observed = observe(&graph, &providers, &machines(), &state).expect("readable");
    apply(
        &plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable"),
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect("applies");

    fs::write(&path, "listen 8080;\n").expect("somebody edits it during an incident");

    let drifted = observe(&graph, &providers, &machines(), &state).expect("readable");
    let repair = plan(&graph, &drifted, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(repair.blast_radius().updates(), 1);
    assert_eq!(
        repair.changes().next().expect("one change").kind(),
        ChangeKind::Update
    );

    apply(
        &repair,
        &providers,
        &machines(),
        &drifted,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect("applies");
    assert_eq!(fs::read_to_string(&path).expect("written"), "listen 80;\n");
}

#[test]
fn a_directory_comes_before_the_file_in_it() {
    let scratch = Scratch::new("order");
    let directory = scratch.join("etc");
    let providers = providers();
    let mut state = State::new();

    let graph = graph_of(vec![
        Resource::new(address("os:directory.etc@localhost")).with("path", directory.clone()),
        Resource::new(address("os:file.conf@localhost"))
            .with("path", format!("{directory}/app.conf"))
            .with("content", "on\n")
            .after(address("os:directory.etc@localhost")),
    ]);

    let observed = observe(&graph, &providers, &machines(), &state).expect("readable");
    let plan = plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(plan.stages().len(), 2, "the file waits for the directory");

    let applied = apply(
        &plan,
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect("applies");
    assert_eq!(applied.changed(), 2, "the directory and the file in it");
    assert!(fs::metadata(&directory).expect("made").is_dir());
    assert_eq!(
        fs::read_to_string(format!("{directory}/app.conf")).expect("written"),
        "on\n"
    );
}

/// A path is where the resource is. Changing it is a replacement, and only
/// state can see that: the new declaration points somewhere empty, so without a
/// memory of where the file was, this reads as making a second one and
/// abandoning the first. It used to.
#[test]
fn moving_a_file_replaces_it_and_takes_the_old_one_with_it() {
    let scratch = Scratch::new("move");
    let providers = providers();
    let mut state = State::new();
    let old = scratch.join("motd");
    let new = scratch.join("motd.new");

    let before = graph_of(vec![
        Resource::new(address("os:file.motd@localhost"))
            .with("path", old.clone())
            .with("content", "hello\n"),
    ]);
    let observed = observe(&before, &providers, &machines(), &state).expect("readable");
    apply(
        &plan(&before, &observed, &state, &providers.schema(), secrets()).expect("plannable"),
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect("applies");
    assert!(fs::metadata(&old).is_ok(), "the first file was made");

    let moved = graph_of(vec![
        Resource::new(address("os:file.motd@localhost"))
            .with("path", new.clone())
            .with("content", "hello\n"),
    ]);
    let observed = observe(&moved, &providers, &machines(), &state).expect("readable");
    let plan = plan(&moved, &observed, &state, &providers.schema(), secrets()).expect("plannable");

    let change = plan.changes().next().expect("one change");
    assert_eq!(change.kind(), ChangeKind::Replace);
    assert_eq!(change.cause().expect("a cause").name(), "path");
    assert!(plan.blast_radius().has_irreversible());

    apply(
        &plan,
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect("applies");
    assert!(fs::metadata(&new).is_ok(), "the new file is there");
    assert!(fs::metadata(&old).is_err(), "and the old one went with it");
}

/// A resource that leaves the configuration leaves the world. Nothing in the
/// world says so — only state remembers that Nerpa made it.
#[test]
fn a_resource_removed_from_the_configuration_is_removed_from_the_machine() {
    let scratch = Scratch::new("delete");
    let providers = providers();
    let mut state = State::new();
    let path = scratch.join("temporary");

    let declared = graph_of(vec![
        Resource::new(address("os:file.temporary@localhost"))
            .with("path", path.clone())
            .with("content", "for now\n"),
    ]);
    let observed = observe(&declared, &providers, &machines(), &state).expect("readable");
    apply(
        &plan(&declared, &observed, &state, &providers.schema(), secrets()).expect("plannable"),
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect("applies");
    assert!(fs::metadata(&path).is_ok());
    assert_eq!(state.len(), 1);

    // The declaration is gone.
    let empty = Graph::new();
    let observed = observe(&empty, &providers, &machines(), &state).expect("readable");
    let plan = plan(&empty, &observed, &state, &providers.schema(), secrets()).expect("plannable");

    assert_eq!(plan.blast_radius().deletes(), 1);
    assert!(
        plan.blast_radius().has_irreversible(),
        "removal cannot be undone"
    );
    assert_eq!(
        plan.changes().next().expect("one change").kind(),
        ChangeKind::Delete
    );

    apply(
        &plan,
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect("applies");
    assert!(fs::metadata(&path).is_err(), "the file is gone");
    assert!(state.is_empty(), "and so is the memory of it");
}

/// Removal order comes from what was recorded, because the configuration that
/// described the dependency is exactly what was deleted.
#[test]
fn what_depended_on_a_thing_is_removed_before_the_thing() {
    let scratch = Scratch::new("order-delete");
    let providers = providers();
    let mut state = State::new();
    let directory = scratch.join("etc");

    let declared = graph_of(vec![
        Resource::new(address("os:directory.etc@localhost")).with("path", directory.clone()),
        Resource::new(address("os:file.conf@localhost"))
            .with("path", format!("{directory}/app.conf"))
            .with(
                "content",
                Reference::to(address("os:directory.etc@localhost"), "path"),
            ),
    ]);
    let observed = observe(&declared, &providers, &machines(), &state).expect("readable");
    apply(
        &plan(&declared, &observed, &state, &providers.schema(), secrets()).expect("plannable"),
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect("applies");

    let empty = Graph::new();
    let observed = observe(&empty, &providers, &machines(), &state).expect("readable");
    let plan = plan(&empty, &observed, &state, &providers.schema(), secrets()).expect("plannable");

    assert_eq!(plan.blast_radius().deletes(), 2);
    assert_eq!(
        plan.stages().len(),
        2,
        "the file cannot go with the directory"
    );
    assert_eq!(
        plan.stages()[0].changes()[0].address(),
        &address("os:file.conf@localhost"),
        "whatever read from the directory goes first"
    );

    apply(
        &plan,
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect("applies");
    assert!(fs::metadata(&directory).is_err());
    assert!(state.is_empty());
}

/// Removal order has to hold for every edge, not only a reference that was still
/// unknown when planning. State kept only those, so an ordering written as
/// `after` was forgotten, and the file and its directory were removed in one
/// stage in any order. Review found it.
#[test]
fn what_came_after_a_thing_is_removed_before_the_thing() {
    let scratch = Scratch::new("order-after");
    let providers = providers();
    let mut state = State::new();
    let directory = scratch.join("etc");

    let declared = graph_of(vec![
        Resource::new(address("os:directory.etc@localhost")).with("path", directory.clone()),
        Resource::new(address("os:file.conf@localhost"))
            .with("path", format!("{directory}/app.conf"))
            .with("content", "listen 80\n")
            .after(address("os:directory.etc@localhost")),
    ]);
    let observed = observe(&declared, &providers, &machines(), &state).expect("readable");
    apply(
        &plan(&declared, &observed, &state, &providers.schema(), secrets()).expect("plannable"),
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect("applies");

    let empty = Graph::new();
    let observed = observe(&empty, &providers, &machines(), &state).expect("readable");
    let plan = plan(&empty, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        plan.stages().len(),
        2,
        "what came after the directory cannot go with it"
    );
    assert_eq!(
        plan.stages()[0].changes()[0].address(),
        &address("os:file.conf@localhost")
    );
}

/// An edge added to a resource that already matches is remembered too. State took
/// its edges from what was applied, and a file that matched was not applied, so an
/// `after` written in later was never recorded, and the file and its directory
/// were removed together in any order.
#[test]
fn an_edge_added_to_a_resource_that_already_matches_is_remembered() {
    let scratch = Scratch::new("order-added");
    let providers = providers();
    let mut state = State::new();
    let directory = scratch.join("etc");
    let file = format!("{directory}/app.conf");

    let converge = |graph: &Graph, state: &mut State| {
        let observed = observe(graph, &providers, &machines(), state).expect("readable");
        apply(
            &plan(graph, &observed, state, &providers.schema(), secrets()).expect("plannable"),
            &providers,
            &machines(),
            &observed,
            state,
            secrets(),
            &mut kept,
        )
        .expect("applies");
    };
    let directory_alone =
        || Resource::new(address("os:directory.etc@localhost")).with("path", directory.clone());
    let conf = || {
        Resource::new(address("os:file.conf@localhost"))
            .with("path", file.clone())
            .with("content", "listen 80\n")
    };

    // The directory, then the file in it with nothing said about order.
    converge(&graph_of(vec![directory_alone()]), &mut state);
    converge(&graph_of(vec![directory_alone(), conf()]), &mut state);
    // Then the order is written in, and nothing on the machine differs.
    let long_ago = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
    fs::File::options()
        .write(true)
        .open(&file)
        .expect("writable")
        .set_modified(long_ago)
        .expect("a time can be set");
    converge(
        &graph_of(vec![
            directory_alone(),
            conf().after(address("os:directory.etc@localhost")),
        ]),
        &mut state,
    );
    assert_eq!(
        fs::metadata(&file)
            .expect("still there")
            .modified()
            .expect("a time"),
        long_ago,
        "and nothing was written to it"
    );

    let empty = Graph::new();
    let observed = observe(&empty, &providers, &machines(), &state).expect("readable");
    let plan = plan(&empty, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        plan.stages().len(),
        2,
        "what came after the directory cannot go with it: {plan:?}"
    );
    assert_eq!(
        plan.stages()[0].changes()[0].address(),
        &address("os:file.conf@localhost")
    );
}

/// The same for a reference that was already resolved when planning, because the
/// thing it points at existed. State kept no edge for it, since only a value
/// still unknown was taken to mean a dependency. Review found it.
#[test]
fn what_read_from_a_thing_that_already_existed_is_removed_before_the_thing() {
    let scratch = Scratch::new("order-resolved");
    let providers = providers();
    let mut state = State::new();
    let directory = scratch.join("etc");

    let first = graph_of(vec![
        Resource::new(address("os:directory.etc@localhost")).with("path", directory.clone()),
    ]);
    let observed = observe(&first, &providers, &machines(), &state).expect("readable");
    apply(
        &plan(&first, &observed, &state, &providers.schema(), secrets()).expect("plannable"),
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect("applies");

    let second = graph_of(vec![
        Resource::new(address("os:directory.etc@localhost")).with("path", directory.clone()),
        Resource::new(address("os:file.conf@localhost"))
            .with("path", format!("{directory}/app.conf"))
            .with(
                "content",
                Reference::to(address("os:directory.etc@localhost"), "path"),
            ),
    ]);
    let observed = observe(&second, &providers, &machines(), &state).expect("readable");
    let planned =
        plan(&second, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        planned.blast_radius().unresolved(),
        0,
        "the directory exists, so the reference is resolved while planning"
    );
    apply(
        &planned,
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect("applies");

    let empty = Graph::new();
    let observed = observe(&empty, &providers, &machines(), &state).expect("readable");
    let plan = plan(&empty, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        plan.stages().len(),
        2,
        "the file cannot go with the directory"
    );
    assert_eq!(
        plan.stages()[0].changes()[0].address(),
        &address("os:file.conf@localhost")
    );
}

/// A value that does not exist at plan time is filled in from what the earlier
/// stage produced, and only then written.
#[test]
fn an_unresolved_value_is_settled_from_the_stage_before_it() {
    let scratch = Scratch::new("resolve");
    let providers = providers();
    let mut state = State::new();
    let directory = scratch.join("data");

    let graph = graph_of(vec![
        Resource::new(address("os:directory.data@localhost")).with("path", directory.clone()),
        Resource::new(address("os:file.marker@localhost"))
            .with("path", format!("{directory}/where"))
            .with(
                "content",
                Reference::to(address("os:directory.data@localhost"), "path"),
            ),
    ]);

    let observed = observe(&graph, &providers, &machines(), &state).expect("readable");
    let plan = plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        plan.blast_radius().unresolved(),
        1,
        "the content is not known yet"
    );

    apply(
        &plan,
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect("applies");
    assert_eq!(
        fs::read_to_string(format!("{directory}/where")).expect("written"),
        directory,
        "the directory's own path was written into the file"
    );
}

/// The fake moved. A provider that only translates has no world in it to stand
/// in for, so what a test wanting no disk touched actually wants is a far end
/// that has no disk. That is `Held`, and this is the property it carries.
#[test]
fn a_machine_in_memory_touches_nothing() {
    let providers = Providers::new().with(Box::new(Os::fake()));
    let mut state = State::new();
    let path = "/etc/definitely-not-writing-here";
    let graph = graph_of(vec![
        Resource::new(address("os:file.dangerous@localhost"))
            .with("path", path)
            .with("content", "no\n"),
    ]);

    let nowhere = Machines::new().everywhere(Box::new(nerpa_transport::Held::new()));
    let observed = observe(&graph, &providers, &nowhere, &state).expect("readable");
    let plan = plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    let applied = apply(
        &plan,
        &providers,
        &nowhere,
        &observed,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect("applies");

    assert_eq!(applied.changed(), 1);
    assert!(
        !std::path::Path::new(path).exists(),
        "it wrote to memory, not to /etc"
    );
    assert_eq!(
        applied
            .observed()
            .attribute(&address("os:file.dangerous@localhost"), "content"),
        Some(&Value::Text("no\n".to_owned()))
    );
}

#[test]
fn a_declaration_without_a_path_is_refused_rather_than_guessed_at() {
    let providers = providers();
    let mut state = State::new();
    let graph = graph_of(vec![Resource::new(address("os:file.nowhere@localhost"))]);

    let observed = observe(&graph, &providers, &machines(), &state).expect("readable");
    let plan = plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");

    let refusal = apply(
        &plan,
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect_err("a file needs a path");
    assert!(refusal.to_string().contains("needs a `path`"), "{refusal}");
}

#[test]
fn a_kind_this_provider_does_not_own_is_refused() {
    let providers = providers();
    let state = State::new();
    let graph = graph_of(vec![Resource::new(address("os:user.deploy@localhost"))]);

    let refusal = observe(&graph, &providers, &machines(), &state).expect_err("users arrive later");
    assert!(
        refusal.to_string().contains("does not know how to handle"),
        "{refusal}"
    );
    let _ = Target::new("localhost").expect("a valid target");
}

/// Read against whatever this actually runs on. Only reading: nothing here
/// installs or removes anything, and the machine a test runs on is not a machine
/// anybody agreed to have changed.
#[test]
fn a_package_that_is_installed_is_read_back_with_its_version() {
    let providers = providers();
    let state = State::new();
    // `bash` under two names, because the two families spell it differently and
    // only one of them will be the family of the machine this runs on.
    let atom = if std::path::Path::new("/usr/bin/portageq").exists() {
        "app-shells/bash"
    } else if std::path::Path::new("/usr/bin/rpm").exists() {
        "bash"
    } else {
        return;
    };
    let graph = graph_of(vec![
        Resource::new(address(&format!("os:package.{atom}@localhost"))).with("version", "0"),
    ]);

    let observed = observe(&graph, &providers, &machines(), &state).expect("readable");
    let found = observed
        .get(&address(&format!("os:package.{atom}@localhost")))
        .expect("bash is installed on anything that can run this test");
    let version = found.get("version").expect("and it has a version");
    assert!(
        matches!(version, Value::Text(text) if !text.is_empty()),
        "the version comes off the machine rather than out of the declaration: {version:?}"
    );
}

#[test]
fn the_registry_merges_what_each_provider_refuses_to_change() {
    let providers = providers();
    let schema = providers.schema();

    assert!(schema.forces_replacement(&address("os:file.motd@localhost"), "path"));
    assert!(schema.forces_replacement(&address("os:directory.etc@localhost"), "path"));
    assert!(!schema.forces_replacement(&address("os:file.motd@localhost"), "content"));
    assert_eq!(providers.len(), 1);
    assert!(!providers.is_empty());
    assert!(Providers::new().is_empty());
    assert_eq!(
        Providers::new().len(),
        0,
        "a registry that knows nobody knows nobody"
    );
}

/// A file that is already on disk and that state has never heard of. Nerpa has
/// to find it from the declaration alone, or it would propose making one that is
/// already there and overwrite whatever is in it.
///
/// Found, it has to be written down as well. It used to plan nothing, so nothing
/// recorded it, and a file Nerpa had been told about could never be removed by
/// taking its declaration away. Found by the review of RFC 0001.
#[test]
fn a_file_that_was_already_there_is_adopted_rather_than_remade() {
    let scratch = Scratch::new("adopt");
    let providers = providers();
    let mut state = State::new();
    let path = scratch.join("existing.conf");
    fs::write(&path, "already here\n").expect("somebody put it there first");
    // Long ago, so that any write at all would leave a later time behind.
    let long_ago = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
    fs::File::options()
        .write(true)
        .open(&path)
        .expect("writable")
        .set_modified(long_ago)
        .expect("a time can be set");

    let graph = graph_of(vec![
        Resource::new(address("os:file.existing@localhost"))
            .with("path", path.clone())
            .with("content", "already here\n"),
    ]);

    let observed = observe(&graph, &providers, &machines(), &state).expect("readable");
    assert!(
        observed.contains(&address("os:file.existing@localhost")),
        "it was found"
    );

    let adopting =
        plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(adopting.blast_radius().adopts(), 1, "{adopting:?}");
    assert_eq!(
        adopting.changes().next().expect("one change").kind(),
        ChangeKind::Adopt
    );
    apply(
        &adopting,
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect("applies");

    assert!(
        state.get(&address("os:file.existing@localhost")).is_some(),
        "it is recorded, although nothing was done to it: {adopting:?}"
    );
    assert_eq!(
        fs::metadata(&path)
            .expect("still there")
            .modified()
            .expect("a time"),
        long_ago,
        "and nothing was written to it"
    );

    let observed = observe(&graph, &providers, &machines(), &state).expect("readable");
    let again = plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert!(
        again.is_empty(),
        "once recorded, there is nothing to do: {again:?}"
    );
}

/// Adopted is Nerpa's, the same as written by it: taking the declaration away
/// removes the file. Whether a file happened to match on the first run used to
/// decide whether it could ever be removed at all.
#[test]
fn a_file_that_was_adopted_is_removed_once_its_declaration_is() {
    let scratch = Scratch::new("adopt-remove");
    let providers = providers();
    let mut state = State::new();
    let path = scratch.join("existing.conf");
    fs::write(&path, "already here\n").expect("somebody put it there first");

    let declared = graph_of(vec![
        Resource::new(address("os:file.existing@localhost"))
            .with("path", path.clone())
            .with("content", "already here\n"),
    ]);
    let observed = observe(&declared, &providers, &machines(), &state).expect("readable");
    apply(
        &plan(&declared, &observed, &state, &providers.schema(), secrets()).expect("plannable"),
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect("applies");

    // The declaration is gone.
    let empty = Graph::new();
    let observed = observe(&empty, &providers, &machines(), &state).expect("readable");
    let plan = plan(&empty, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        plan.blast_radius().deletes(),
        1,
        "the file goes with its declaration: {plan:?}"
    );

    apply(
        &plan,
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect("applies");
    assert!(fs::metadata(&path).is_err(), "the file is gone");
    assert!(state.is_empty(), "and so is the memory of it");
}

/// Adopting sends nothing, so a run that stops before writing an adoption down
/// loses nothing: the next plan finds the same file as declared and adopts it
/// again. It is written down once for the run, then, rather than once for each.
/// A first run over machines that are already configured adopts everything it
/// declares, and a whole state file written for each of those is thousands of
/// writes saying that nothing happened.
#[test]
fn what_is_adopted_is_written_down_once_rather_than_once_for_each() {
    let scratch = Scratch::new("adopt-once");
    let providers = providers();
    let mut state = State::new();
    let mut resources = Vec::new();
    for name in ["one", "two", "three"] {
        let path = scratch.join(name);
        fs::write(&path, "already here\n").expect("somebody put it there first");
        resources.push(
            Resource::new(address(&format!("os:file.{name}@localhost")))
                .with("path", path)
                .with("content", "already here\n"),
        );
    }
    let graph = graph_of(resources);

    let observed = observe(&graph, &providers, &machines(), &state).expect("readable");
    let plan = plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    let mut seen = Vec::new();
    apply(
        &plan,
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut |state: &State| {
            seen.push(state.len());
            Ok(())
        },
    )
    .expect("applies");

    assert_eq!(seen, vec![3], "all three, in one write");
}

/// Renaming a resource that keeps its path, with no `moved`, converges in one run.
///
/// The old address is removed and the new one declared at the same path. The
/// removal deleted the file, and the new resource was adopted from the reading
/// taken before the delete, so the second run proposed making it again. Found by
/// the second review of RFC 0001.
#[test]
fn a_resource_renamed_at_the_same_path_converges_in_one_run() {
    let scratch = Scratch::new("rename-same-path");
    let providers = providers();
    let mut state = State::new();
    let path = scratch.join("motd");

    let converge = |graph: &Graph, state: &mut State| {
        let observed = observe(graph, &providers, &machines(), state).expect("readable");
        apply(
            &plan(graph, &observed, state, &providers.schema(), secrets()).expect("plannable"),
            &providers,
            &machines(),
            &observed,
            state,
            secrets(),
            &mut kept,
        )
        .expect("applies");
    };
    let named = |name: &str| {
        graph_of(vec![
            Resource::new(address(&format!("os:file.{name}@localhost")))
                .with("path", path.clone())
                .with("content", "hello\n"),
        ])
    };

    converge(&named("old"), &mut state);
    converge(&named("new"), &mut state);

    assert_eq!(
        fs::read_to_string(&path).ok().as_deref(),
        Some("hello\n"),
        "the file is there after the rename"
    );
    let renamed = named("new");
    let observed = observe(&renamed, &providers, &machines(), &state).expect("readable");
    let again =
        plan(&renamed, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert!(
        again.is_empty(),
        "the second run has nothing to do: {again:?}"
    );
}

/// A directory whose declaration goes, holding a file that moves out of it, is
/// removed after the file has moved. Removals came first, so the directory was
/// removed while the file was still in it, the removal refused, and every later run
/// did the same. Found by the narrow review of RFC 0001.
#[test]
fn a_directory_is_removed_after_the_file_moved_out_of_it() {
    let scratch = Scratch::new("move-out");
    let providers = providers();
    let mut state = State::new();
    let directory = scratch.join("old");
    let inside = format!("{directory}/app.conf");
    let outside = scratch.join("app.conf");

    let converge = |graph: &Graph, state: &mut State| {
        let observed = observe(graph, &providers, &machines(), state).expect("readable");
        apply(
            &plan(graph, &observed, state, &providers.schema(), secrets()).expect("plannable"),
            &providers,
            &machines(),
            &observed,
            state,
            secrets(),
            &mut kept,
        )
        .expect("applies");
    };

    converge(
        &graph_of(vec![
            Resource::new(address("os:directory.old@localhost")).with("path", directory.clone()),
            Resource::new(address("os:file.conf@localhost"))
                .with("path", inside.clone())
                .with("content", "listen 80\n")
                .after(address("os:directory.old@localhost")),
        ]),
        &mut state,
    );

    // The directory's declaration goes, and the file moves out of it.
    let moved = graph_of(vec![
        Resource::new(address("os:file.conf@localhost"))
            .with("path", outside.clone())
            .with("content", "listen 80\n"),
    ]);
    converge(&moved, &mut state);

    assert!(fs::metadata(&directory).is_err(), "the directory is gone");
    assert_eq!(
        fs::read_to_string(&outside).ok().as_deref(),
        Some("listen 80\n"),
        "and the file is where it moved to"
    );
    let observed = observe(&moved, &providers, &machines(), &state).expect("readable");
    let again = plan(&moved, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert!(
        again.is_empty(),
        "the next run has nothing to do: {again:?}"
    );
}

/// A directory whose path changes is replaced after the removal of a file state
/// recorded inside it, whatever else that removal waits for. Removals first had
/// kept that order without the planner saying so; once a removal could wait for
/// the change of what was recorded after it, it moved past the replacement, the
/// old directory was deleted with the file still inside, and every run refused.
/// Found by the review of RFC 0001's fourth revision.
#[test]
fn a_replaced_directory_waits_for_the_removal_of_what_was_inside_it() {
    let scratch = Scratch::new("replaced-after-removal");
    let (old, new) = (scratch.join("d"), scratch.join("e"));
    let later = scratch.join("z");
    let mut state = State::new();
    run_once(
        &graph_of(vec![
            Resource::new(address("os:directory.d@localhost")).with("path", old.clone()),
            Resource::new(address("os:file.inner@localhost"))
                .with("path", format!("{old}/inner"))
                .with("content", "x\n")
                .after(address("os:directory.d@localhost")),
            Resource::new(address("os:file.z@localhost"))
                .with("path", later.clone())
                .with("content", "one\n")
                .after(address("os:file.inner@localhost")),
        ]),
        &mut state,
    )
    .expect("the first run applies");

    let second = graph_of(vec![
        Resource::new(address("os:directory.d@localhost")).with("path", new.clone()),
        Resource::new(address("os:file.z@localhost"))
            .with("path", later)
            .with("content", "two\n"),
    ]);
    run_once(&second, &mut state).expect("the second run applies");

    assert!(
        fs::metadata(&old).is_err() && fs::metadata(&new).is_ok(),
        "the directory moved"
    );
    let again = next_plan(&second, &state);
    assert!(
        again.is_empty(),
        "and the next run has nothing to do: {again:?}"
    );
}

/// And a directory whose path changes is replaced after a declared file recorded
/// inside it moves out, rather than beside it in one stage, where the directory
/// came first and every run refused. Found by the review of RFC 0001's fourth
/// revision.
#[test]
fn a_replaced_directory_waits_for_the_file_moving_out_of_it() {
    let scratch = Scratch::new("replaced-after-change");
    let (old, new) = (scratch.join("d"), scratch.join("e"));
    let outside = scratch.join("z");
    let mut state = State::new();
    run_once(
        &graph_of(vec![
            Resource::new(address("os:directory.d@localhost")).with("path", old.clone()),
            Resource::new(address("os:file.z@localhost"))
                .with("path", format!("{old}/z"))
                .with("content", "z\n")
                .after(address("os:directory.d@localhost")),
        ]),
        &mut state,
    )
    .expect("the first run applies");

    let second = graph_of(vec![
        Resource::new(address("os:directory.d@localhost")).with("path", new.clone()),
        Resource::new(address("os:file.z@localhost"))
            .with("path", outside.clone())
            .with("content", "z\n"),
    ]);
    run_once(&second, &mut state).expect("the second run applies");

    assert!(
        fs::metadata(&old).is_err() && fs::metadata(&new).is_ok(),
        "the directory moved"
    );
    assert_eq!(
        fs::read_to_string(&outside).ok().as_deref(),
        Some("z\n"),
        "and the file moved out of it"
    );
    let again = next_plan(&second, &state);
    assert!(
        again.is_empty(),
        "and the next run has nothing to do: {again:?}"
    );
}

/// A recorded file moved onto the path a removal frees is replaced, not made again
/// beside the old one. Naming what a removal takes away dropped its reading, which
/// had been taken where state left it, so it was planned as a creation: the old
/// path stayed on the machine with nothing recording it, and the next run had
/// nothing to do. Found by the review of RFC 0001's fourth revision.
#[test]
fn a_file_moved_onto_a_path_a_removal_frees_leaves_nothing_at_the_old_path() {
    let scratch = Scratch::new("moved-onto-freed");
    let (old, freed) = (scratch.join("x"), scratch.join("y"));
    let mut state = State::new();
    run_once(
        &graph_of(vec![
            Resource::new(address("os:directory.y@localhost")).with("path", freed.clone()),
            Resource::new(address("os:file.a@localhost"))
                .with("path", old.clone())
                .with("content", "a\n"),
        ]),
        &mut state,
    )
    .expect("the first run applies");

    let moved = graph_of(vec![
        Resource::new(address("os:file.a@localhost"))
            .with("path", freed.clone())
            .with("content", "a\n"),
    ]);
    run_once(&moved, &mut state).expect("the second run applies");

    assert!(
        fs::metadata(&old).is_err(),
        "nothing is left at the old path"
    );
    assert_eq!(
        fs::read_to_string(&freed).ok().as_deref(),
        Some("a\n"),
        "and the file is where it moved to"
    );
    let again = next_plan(&moved, &state);
    assert!(
        again.is_empty(),
        "and the next run has nothing to do: {again:?}"
    );
}

/// A mode is compared as the number it spells. Written "644" or "0o644", it never
/// matched the "0644" reading reports, and every run changed it again. Found by the
/// review of RFC 0001's fourth revision.
#[test]
fn a_mode_is_compared_as_the_number_it_spells() {
    for (name, written) in [("short", "644"), ("prefixed", "0o644")] {
        let scratch = Scratch::new(&format!("mode-{name}"));
        let graph = graph_of(vec![
            Resource::new(address(&format!("os:file.{name}@localhost")))
                .with("path", scratch.join("f"))
                .with("content", "m\n")
                .with("mode", written),
        ]);
        let mut state = State::new();
        run_once(&graph, &mut state).expect("the first run applies");
        let again = next_plan(&graph, &state);
        assert!(
            again.is_empty(),
            "{written}: the next run has nothing to do: {again:?}"
        );
    }
}

/// And a mode beyond the permission bits is refused while planning: the machine
/// keeps only those, so it never matched either. Found by the review of RFC 0001's
/// fourth revision.
#[test]
fn a_mode_beyond_the_permission_bits_is_refused_while_planning() {
    let scratch = Scratch::new("mode-beyond");
    let graph = graph_of(vec![
        Resource::new(address("os:file.beyond@localhost"))
            .with("path", scratch.join("f"))
            .with("content", "m\n")
            .with("mode", "100644"),
    ]);
    let providers = providers();
    let state = State::new();
    let observed = observe(&graph, &providers, &machines(), &state).expect("readable");
    let refused = plan(&graph, &observed, &state, &providers.schema(), secrets()).unwrap_err();
    assert!(
        matches!(
            &refused,
            nerpa_core::planner::PlanError::NotAMode { attribute, value, .. }
                if attribute == "mode" && value == "100644"
        ),
        "{refused}"
    );
}

/// A file declared where another moves away from is made after the move, whether
/// its content is the same or not. It was read there: adopted from the file about
/// to be deleted, which the move then deleted, or updated so that the move's own
/// premise moved; either took more runs to come back. Found by the review of RFC
/// 0001's fifth revision.
#[test]
fn a_file_declared_where_a_replacement_moves_one_away_is_made_after_it() {
    for (name, content) in [("same", "x\n"), ("other", "t\n")] {
        let scratch = Scratch::new(&format!("freed-by-move-{name}"));
        let (old, new) = (scratch.join("a"), scratch.join("b"));
        let mut state = State::new();
        run_once(
            &graph_of(vec![
                Resource::new(address("os:file.x@localhost"))
                    .with("path", old.clone())
                    .with("content", "x\n"),
            ]),
            &mut state,
        )
        .expect("the first run applies");

        let second = graph_of(vec![
            Resource::new(address("os:file.x@localhost"))
                .with("path", new.clone())
                .with("content", "x\n"),
            Resource::new(address("os:file.t@localhost"))
                .with("path", old.clone())
                .with("content", content),
        ]);
        run_once(&second, &mut state).expect("the second run applies");

        assert_eq!(
            fs::read_to_string(&old).ok().as_deref(),
            Some(content),
            "{name}: the file declared where x was is there"
        );
        assert_eq!(
            fs::read_to_string(&new).ok().as_deref(),
            Some("x\n"),
            "{name}: and x is where it moved"
        );
        let again = next_plan(&second, &state);
        assert!(
            again.is_empty(),
            "{name}: the next run has nothing to do: {again:?}"
        );
    }
}

/// A mode written as a number, as Starlark's `0o600` evaluates to, is the mode the
/// file gets, and converges. It was neither refused nor applied: the provider fell
/// back to its default, and every run changed it again. Found by the review of RFC
/// 0001's fifth revision.
#[test]
fn a_mode_written_as_a_number_is_the_mode_the_file_gets() {
    let scratch = Scratch::new("mode-number");
    let path = scratch.join("f");
    let graph = graph_of(vec![
        Resource::new(address("os:file.number@localhost"))
            .with("path", path.clone())
            .with("content", "m\n")
            .with("mode", 0o600_i64),
    ]);
    let mut state = State::new();
    run_once(&graph, &mut state).expect("the first run applies");

    let on_disk = std::os::unix::fs::PermissionsExt::mode(
        &fs::metadata(&path).expect("written").permissions(),
    );
    assert_eq!(
        on_disk & 0o7777,
        0o600,
        "the declared mode is the one on disk"
    );
    let again = next_plan(&graph, &state);
    assert!(
        again.is_empty(),
        "and the next run has nothing to do: {again:?}"
    );
}

/// State is written as the run goes, not once when it finishes.
///
/// A run that stops half way has made real changes. Recording them only at the
/// end means a machine that goes away, a contract violation panicking by design,
/// or somebody's Ctrl-C leaves every resource already made with nothing saying
/// it exists — not rolled back, forgotten.
#[test]
fn what_was_already_made_is_recorded_before_the_next_thing_is_touched() {
    let scratch = Scratch::new("checkpoint");
    let first = scratch.join("first");

    let graph = graph_of(vec![
        Resource::new(address("os:file.first@localhost"))
            .with("path", first.clone())
            .with("content", "one\n"),
        // The second one cannot be made: its parent is the first, which is a
        // file, so the directory cannot be created under it.
        Resource::new(address("os:file.second@localhost"))
            .with("path", format!("{first}/under-a-file"))
            .with("content", "two\n")
            .after(address("os:file.first@localhost")),
    ]);

    let providers = providers();
    let mut state = State::new();
    let observed = observe(&graph, &providers, &machines(), &state).expect("readable");
    let plan = plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");

    let mut seen = Vec::new();
    let outcome = apply(
        &plan,
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut |state: &State| {
            seen.push(state.len());
            Ok(())
        },
    );

    assert!(outcome.is_err(), "the second file cannot be written");
    assert_eq!(
        seen,
        vec![1],
        "the first resource was recorded before the second was attempted"
    );
    assert!(
        state.get(&address("os:file.first@localhost")).is_some(),
        "and it is still in state after the failure"
    );
}

/// A provider is shared across threads, because machines are visited in
/// parallel and one registry serves all of them. Decision 0031.
///
/// A compile-time bound would say the type is `Sync`; this says the code behind
/// it survives being used that way, which is the part a bound cannot promise.
#[test]
fn one_provider_serves_two_threads_at_once() {
    fn shareable<T: Send + Sync>(_: &T) {}

    let scratch = Scratch::new("threads");
    let providers = providers();
    shareable(&providers);

    let paths: Vec<String> = (0..8).map(|n| scratch.join(&format!("file-{n}"))).collect();

    std::thread::scope(|threads| {
        for (index, path) in paths.iter().enumerate() {
            let providers = &providers;
            threads.spawn(move || {
                let mut wanted = Attributes::new();
                wanted.insert("path".to_owned(), Value::Text(path.clone()));
                wanted.insert("content".to_owned(), Value::Text(format!("{index}\n")));
                providers
                    .for_address(&address("os:file.subject@localhost"))
                    .expect("registered")
                    .apply(
                        &address("os:file.subject@localhost"),
                        ChangeKind::Create,
                        false,
                        &wanted,
                        &[],
                        Observed::new().facts_of(None),
                        &Here,
                    )
                    .expect("it writes");
            });
        }
    });

    for (index, path) in paths.iter().enumerate() {
        assert_eq!(
            fs::read_to_string(path).expect("written"),
            format!("{index}\n"),
            "every thread's file survived the others"
        );
    }
}

/// The whole loop with a value that depends on what the machine turned out to
/// be. Facts are gathered in the same run, from the same executor, and the
/// choice is settled before anything is written.
#[test]
fn a_value_chosen_by_a_fact_settles_from_the_machine() {
    let scratch = Scratch::new("facts");
    let wide = scratch.join("wide.conf");
    let arm = scratch.join("arm.conf");

    let mut chosen = std::collections::BTreeMap::new();
    chosen.insert("x86_64".to_owned(), Value::Text(wide.clone()));
    chosen.insert("aarch64".to_owned(), Value::Text(arm.clone()));

    let graph = graph_of(vec![
        Resource::new(address("os:file.conf@localhost"))
            .with(
                "path",
                Choice::among(Fact::of(node("localhost"), "arch"), chosen),
            )
            .with("content", "managed\n"),
    ]);

    let providers = providers();
    let mut state = State::new();
    let observed = observe(&graph, &providers, &machines(), &state).expect("readable");

    // The machine answered, so the choice is settled and the plan writes one
    // path rather than offering two.
    let intended =
        plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    apply(
        &intended,
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect("applies");

    let here = std::env::consts::ARCH;
    let expected = if here == "aarch64" { &arm } else { &wide };
    assert_eq!(
        fs::read_to_string(expected).expect("written"),
        "managed\n",
        "the branch this machine's architecture picks"
    );

    let again = observe(&graph, &providers, &machines(), &state).expect("readable");
    assert!(
        plan(&graph, &again, &state, &providers.schema(), secrets())
            .expect("plannable")
            .is_empty(),
        "and the second run has nothing to do"
    );
}

/// A machine that says something no branch accounts for is refused, not
/// defaulted. A silent default is a machine configured as though it were a
/// different one, which is worse than one that is not configured.
#[test]
fn a_machine_that_says_something_unaccounted_for_is_refused() {
    let scratch = Scratch::new("unaccounted");
    let mut only_arm = std::collections::BTreeMap::new();
    only_arm.insert(
        "definitely-not-this-machine".to_owned(),
        Value::Text(scratch.join("conf")),
    );

    let graph = graph_of(vec![Resource::new(address("os:file.conf@localhost")).with(
        "path",
        Choice::among(Fact::of(node("localhost"), "arch"), only_arm),
    )]);

    let providers = providers();
    let state = State::new();
    let observed = observe(&graph, &providers, &machines(), &state).expect("readable");

    let refusal = plan(&graph, &observed, &state, &providers.schema(), secrets())
        .expect_err("this machine is not in the set");
    assert!(
        refusal.to_string().contains("arch"),
        "the refusal should name the fact and what the machine said: {refusal}"
    );
}

/// Between the looking and the doing, the machine can change.
///
/// Reading everything once and applying once makes that window the whole run,
/// where Ansible's is one task — so the batch carries what the plan was computed
/// from and the far end looks again before it does anything. Here somebody edits
/// the file after the plan is made and before it is applied: the change must not
/// happen, and what they wrote must still be there. See decision 0034.
#[test]
fn a_file_that_moved_after_the_plan_was_made_is_not_written_over() {
    let scratch = Scratch::new("premise");
    let path = scratch.join("motd");
    fs::write(&path, "as it was\n").expect("a file to plan against");

    let graph = graph_of(vec![
        Resource::new(address("os:file.motd@localhost"))
            .with("path", path.clone())
            .with("content", "as declared\n"),
    ]);

    let providers = providers();
    let mut state = State::new();
    let observed = observe(&graph, &providers, &machines(), &state).expect("readable");
    let plan = plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        plan.blast_radius().updates(),
        1,
        "there is a change to make"
    );

    // Somebody else, between the two.
    fs::write(&path, "somebody else was here\n").expect("their edit");

    let refusal = apply(
        &plan,
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut kept,
    )
    .expect_err("the world is not what the plan was computed from");

    assert!(
        refusal
            .to_string()
            .contains("not what the plan was computed from"),
        "the refusal says the plan is stale rather than that something broke: {refusal}"
    );
    assert_eq!(
        fs::read_to_string(&path).expect("still there"),
        "somebody else was here\n",
        "and their edit is untouched: a batch acting on half a stale reading is \
         worse than one that did not act"
    );
}
