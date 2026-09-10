//! What a resource was born with, and what it must remain.
//!
//! Decision 0021. The case that matters is an image reference being edited: in
//! every other tool that means "destroy the fleet", and here it means "the fleet
//! is older than the configuration, and you are being told so".

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};

use nerpa_core::address::address;
use nerpa_core::plan::{ChangeKind, Confidence};
use nerpa_core::planner::plan;
use nerpa_core::secret::{Key, Recorded, Remembered, Secrets};
use nerpa_core::state::{Record, State};

/// A fixed key and a store holding nothing.
///
/// No test in this file declares a secret; planning still has to be told where
/// one would be fetched from, and a fixed key keeps digests reproducible.
static DIGEST_KEY: Key = Key::from_bytes([7; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

fn secrets() -> Secrets<'static> {
    Secrets::new(&DIGEST_KEY, &NO_STORE)
}

use nerpa_core::{Graph, Kind, Observed, Provider, Resource, Schema, Value};

fn instances() -> Schema {
    Schema::new()
        .at_birth(
            Provider::new("yandex").unwrap(),
            Kind::new("instance").unwrap(),
            "image",
        )
        .immutable(
            Provider::new("yandex").unwrap(),
            Kind::new("instance").unwrap(),
            "zone",
        )
}

fn graph_of(resources: Vec<Resource>) -> Graph {
    let mut graph = Graph::new();
    for resource in resources {
        graph.insert(resource).unwrap();
    }
    graph
}

/// A state recording each of these, as the run that made them would have left it.
///
/// Without one, a machine that matches everything but its image is adopted, which
/// is work, and these tests are about what is proposed for one already managed.
fn recorded(addresses: &[&str]) -> State {
    let mut state = State::new();
    for address_text in addresses {
        state.record(
            address(address_text),
            Record::new(BTreeMap::new(), BTreeSet::new()),
        );
    }
    state
}

fn running(image: &str, cores: i64) -> Observed {
    let mut observed = Observed::new();
    let mut attributes = BTreeMap::new();
    attributes.insert("image".to_owned(), Value::Text(image.to_owned()));
    attributes.insert("cores".to_owned(), Value::Integer(cores));
    observed.record(address("yandex:instance.web"), attributes);
    observed
}

/// The whole point. In Terraform this is a plan to destroy and rebuild.
#[test]
fn editing_the_image_does_not_propose_to_destroy_anything() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.web"))
            .with("image", "gentoo-2026-09")
            .with("cores", 8),
    ]);

    let plan = plan(
        &graph,
        &running("gentoo-2026-01", 8),
        &recorded(&["yandex:instance.web"]),
        &instances(),
        secrets(),
    )
    .unwrap();

    assert!(
        plan.is_empty(),
        "nothing to do: the machine is running, just older"
    );
    assert_eq!(plan.blast_radius().replaces(), 0);
    assert_eq!(plan.blast_radius().total(), 0);
}

/// And it is not silence either, which is the difference from `ignore_changes`.
#[test]
fn but_it_is_reported_every_time() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.web")).with("image", "gentoo-2026-09"),
    ]);

    let plan = plan(
        &graph,
        &running("gentoo-2026-01", 8),
        &State::new(),
        &instances(),
        secrets(),
    )
    .unwrap();

    assert_eq!(plan.blast_radius().outdated(), 1);
    let told = &plan.outdated()[0];
    assert_eq!(told.address(), &address("yandex:instance.web"));
    assert_eq!(told.attribute(), "image");
    assert_eq!(
        told.born_with(),
        Some(&Recorded::Plain(Value::Text("gentoo-2026-01".to_owned())))
    );
    assert_eq!(
        told.declared(),
        &Confidence::Known(Value::Text("gentoo-2026-09".to_owned()))
    );
}

/// A machine that does not exist yet is made from what is declared now. There is
/// nothing old about it.
#[test]
fn a_new_machine_is_born_from_what_is_declared_today() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.web")).with("image", "gentoo-2026-09"),
    ]);

    let plan = plan(
        &graph,
        &Observed::new(),
        &State::new(),
        &instances(),
        secrets(),
    )
    .unwrap();

    assert_eq!(plan.blast_radius().creates(), 1);
    assert_eq!(
        plan.blast_radius().outdated(),
        0,
        "nothing was born with anything else"
    );
    let change = plan.changes().next().unwrap();
    assert!(
        change.desired().iter().any(|(name, _)| name == "image"),
        "and the image is carried, because that is what it will be made from"
    );
}

/// Everything else on the same resource is still converged. Being born with an
/// old image does not put a machine outside management.
#[test]
fn the_rest_of_the_resource_is_still_held_to_its_declaration() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.web"))
            .with("image", "gentoo-2026-09")
            .with("cores", 16),
    ]);

    let plan = plan(
        &graph,
        &running("gentoo-2026-01", 8),
        &State::new(),
        &instances(),
        secrets(),
    )
    .unwrap();

    assert_eq!(plan.blast_radius().updates(), 1);
    assert_eq!(plan.blast_radius().outdated(), 1, "and it still says so");
    let change = plan.changes().next().unwrap();
    assert_eq!(change.kind(), ChangeKind::Update);
    let changing: Vec<&str> = change
        .deltas()
        .iter()
        .map(nerpa_core::AttributeDelta::name)
        .collect();
    assert_eq!(changing, ["cores"], "the image is not among the work");
}

/// Converged is the default; silence has to be asked for.
#[test]
fn an_attribute_nobody_declared_as_a_birth_attribute_is_converged() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.web")).with("image", "gentoo-2026-09"),
    ]);

    let plan = plan(
        &graph,
        &running("gentoo-2026-01", 8),
        &State::new(),
        &Schema::new(),
        secrets(),
    )
    .unwrap();

    assert_eq!(plan.blast_radius().outdated(), 0);
    assert_eq!(
        plan.blast_radius().updates(),
        1,
        "without the schema saying so, it is drift"
    );
}

/// A birth attribute never forces a replacement, even if it is also marked
/// immutable — the more specific answer wins, and it is the one that does not
/// destroy a running machine.
#[test]
fn being_born_with_something_never_becomes_a_replacement() {
    let schema = Schema::new()
        .at_birth(
            Provider::new("yandex").unwrap(),
            Kind::new("instance").unwrap(),
            "image",
        )
        .immutable(
            Provider::new("yandex").unwrap(),
            Kind::new("instance").unwrap(),
            "image",
        );

    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.web")).with("image", "gentoo-2026-09"),
    ]);

    let plan = plan(
        &graph,
        &running("gentoo-2026-01", 8),
        &State::new(),
        &schema,
        secrets(),
    )
    .unwrap();

    assert_eq!(plan.blast_radius().replaces(), 0);
    assert!(!plan.blast_radius().has_irreversible());
    assert_eq!(plan.blast_radius().outdated(), 1);
}

/// The two slots are two machines, and that is how a fleet moves onto a new
/// image without anything being destroyed to do it.
#[test]
fn two_slots_run_side_by_side_on_different_images() {
    let graph = graph_of(vec![
        Resource::new(address("yandex:instance.web:v1")).with("image", "gentoo-2026-01"),
        Resource::new(address("yandex:instance.web:v2")).with("image", "gentoo-2026-09"),
    ]);

    let mut observed = Observed::new();
    let mut old = BTreeMap::new();
    old.insert("image".to_owned(), Value::Text("gentoo-2026-01".to_owned()));
    observed.record(address("yandex:instance.web:v1"), old);

    let plan = plan(
        &graph,
        &observed,
        &recorded(&["yandex:instance.web:v1"]),
        &instances(),
        secrets(),
    )
    .unwrap();

    assert_eq!(plan.blast_radius().creates(), 1, "the new slot is made");
    assert_eq!(
        plan.blast_radius().outdated(),
        0,
        "and neither is behind its own declaration"
    );
    assert_eq!(
        plan.changes().next().unwrap().address(),
        &address("yandex:instance.web:v2")
    );
}
