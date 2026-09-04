//! The whole loop, on a real filesystem: read, plan, apply, plan again.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::fs;
use std::path::PathBuf;

use sarma_core::address::address;
use sarma_core::planner::plan;
use sarma_core::provider::{Provider, Providers};
use sarma_core::run::{apply, observe};
use sarma_core::{ChangeKind, Graph, Reference, Resource, Target, Value};
use sarma_provider_os::Os;

/// A directory that removes itself, so a failing test cannot leave litter behind.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("sarma-{name}-{}", std::process::id()));
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

#[test]
fn a_file_is_written_and_the_second_run_has_nothing_to_do() {
    let scratch = Scratch::new("write");
    let path = scratch.join("motd");
    let providers = providers();

    let graph = graph_of(vec![
        Resource::new(address("os:file.motd@localhost"))
            .with("path", path.clone())
            .with("content", "hello\n")
            .with("mode", "0640"),
    ]);

    let before = observe(&graph, &providers).expect("readable");
    assert!(before.is_empty(), "nothing exists yet");

    let first = plan(&graph, &before, &providers.schema()).expect("plannable");
    assert_eq!(first.blast_radius().creates(), 1);

    let applied = apply(&first, &providers, &before).expect("applies");
    assert_eq!(applied.changed(), 1);
    assert_eq!(fs::read_to_string(&path).expect("written"), "hello\n");

    // The property the whole tool exists to have.
    let after = observe(&graph, &providers).expect("readable");
    let second = plan(&graph, &after, &providers.schema()).expect("plannable");
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
    let graph = graph_of(vec![
        Resource::new(address("os:file.nginx.conf@localhost"))
            .with("path", path.clone())
            .with("content", "listen 80;\n"),
    ]);

    let observed = observe(&graph, &providers).expect("readable");
    apply(
        &plan(&graph, &observed, &providers.schema()).expect("plannable"),
        &providers,
        &observed,
    )
    .expect("applies");

    fs::write(&path, "listen 8080;\n").expect("somebody edits it during an incident");

    let drifted = observe(&graph, &providers).expect("readable");
    let repair = plan(&graph, &drifted, &providers.schema()).expect("plannable");
    assert_eq!(repair.blast_radius().updates(), 1);
    assert_eq!(
        repair.changes().next().expect("one change").kind(),
        ChangeKind::Update
    );

    apply(&repair, &providers, &drifted).expect("applies");
    assert_eq!(fs::read_to_string(&path).expect("written"), "listen 80;\n");
}

#[test]
fn a_directory_comes_before_the_file_in_it() {
    let scratch = Scratch::new("order");
    let directory = scratch.join("etc");
    let providers = providers();

    let graph = graph_of(vec![
        Resource::new(address("os:directory.etc@localhost")).with("path", directory.clone()),
        Resource::new(address("os:file.conf@localhost"))
            .with("path", format!("{directory}/app.conf"))
            .with("content", "on\n")
            .after(address("os:directory.etc@localhost")),
    ]);

    let observed = observe(&graph, &providers).expect("readable");
    let plan = plan(&graph, &observed, &providers.schema()).expect("plannable");
    assert_eq!(plan.stages().len(), 2, "the file waits for the directory");

    let applied = apply(&plan, &providers, &observed).expect("applies");
    assert_eq!(applied.changed(), 2, "the directory and the file in it");
    assert!(fs::metadata(&directory).expect("made").is_dir());
    assert_eq!(
        fs::read_to_string(format!("{directory}/app.conf")).expect("written"),
        "on\n"
    );
}

/// A path is where the resource is, so changing it should be a replacement —
/// and it is, at the planner level. It is not one here, and the reason is worth
/// pinning: without state there is nothing that remembers where the file used to
/// be, so Sarma looks at the new path, finds nothing, and makes a new file. The
/// old one stays, and nothing plans to remove it.
///
/// The same gap as deletions, from the same cause. This test will start failing
/// when state lands, which is the point of it.
#[test]
fn moving_a_file_leaves_the_old_one_behind_until_state_exists() {
    let scratch = Scratch::new("move");
    let providers = providers();
    let old = scratch.join("motd");
    let new = scratch.join("motd.new");

    let before = graph_of(vec![
        Resource::new(address("os:file.motd@localhost"))
            .with("path", old.clone())
            .with("content", "hello\n"),
    ]);
    let observed = observe(&before, &providers).expect("readable");
    apply(
        &plan(&before, &observed, &providers.schema()).expect("plannable"),
        &providers,
        &observed,
    )
    .expect("applies");

    let moved = graph_of(vec![
        Resource::new(address("os:file.motd@localhost"))
            .with("path", new.clone())
            .with("content", "hello\n"),
    ]);
    let observed = observe(&moved, &providers).expect("readable");
    let plan = plan(&moved, &observed, &providers.schema()).expect("plannable");

    let change = plan.changes().next().expect("one change");
    assert_eq!(
        change.kind(),
        ChangeKind::Create,
        "nothing remembers the old path"
    );
    assert!(!plan.blast_radius().has_irreversible());

    apply(&plan, &providers, &observed).expect("applies");
    assert!(fs::metadata(&new).is_ok(), "the new file was made");
    assert!(
        fs::metadata(&old).is_ok(),
        "and the old one is still there, unclaimed"
    );
}

/// A value that does not exist at plan time is filled in from what the earlier
/// stage produced, and only then written.
#[test]
fn an_unresolved_value_is_settled_from_the_stage_before_it() {
    let scratch = Scratch::new("resolve");
    let providers = providers();
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

    let observed = observe(&graph, &providers).expect("readable");
    let plan = plan(&graph, &observed, &providers.schema()).expect("plannable");
    assert_eq!(
        plan.blast_radius().unresolved(),
        1,
        "the content is not known yet"
    );

    apply(&plan, &providers, &observed).expect("applies");
    assert_eq!(
        fs::read_to_string(format!("{directory}/where")).expect("written"),
        directory,
        "the directory's own path was written into the file"
    );
}

#[test]
fn the_fake_touches_nothing() {
    let providers = Providers::new().with(Box::new(Os::fake()));
    let path = "/etc/definitely-not-writing-here";
    let graph = graph_of(vec![
        Resource::new(address("os:file.dangerous@localhost"))
            .with("path", path)
            .with("content", "no\n"),
    ]);

    let observed = observe(&graph, &providers).expect("readable");
    let plan = plan(&graph, &observed, &providers.schema()).expect("plannable");
    let applied = apply(&plan, &providers, &observed).expect("applies");

    assert_eq!(applied.changed(), 1);
    assert!(
        !std::path::Path::new(path).exists(),
        "the fake wrote to memory, not to /etc"
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
    let graph = graph_of(vec![Resource::new(address("os:file.nowhere@localhost"))]);

    let observed = observe(&graph, &providers).expect("readable");
    let plan = plan(&graph, &observed, &providers.schema()).expect("plannable");

    let refusal = apply(&plan, &providers, &observed).expect_err("a file needs a path");
    assert!(refusal.to_string().contains("needs a `path`"), "{refusal}");
}

#[test]
fn a_kind_this_provider_does_not_own_is_refused() {
    let providers = providers();
    let graph = graph_of(vec![
        Resource::new(address("os:package.nginx@localhost")).with("state", "present"),
    ]);

    let refusal = observe(&graph, &providers).expect_err("packages arrive with the executor");
    assert!(
        refusal.to_string().contains("does not know how to handle"),
        "{refusal}"
    );
    let _ = Target::new("localhost").expect("a valid target");
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
