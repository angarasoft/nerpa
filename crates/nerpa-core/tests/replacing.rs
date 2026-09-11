//! A replacement destroys before it creates, and the destruction is a change.
//!
//! Everything else in the apply loop was rearranged so that what a run has
//! already done is written down before it touches the next thing. The delete
//! that starts a replacement was not: it removed the resource and left the
//! record saying where the old one was, so a run that then failed left state
//! describing something that no longer existed.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeMap;

use nerpa_core::address::address;
use nerpa_core::planner::plan;
use nerpa_core::provider::{
    Attributes, Machines, Provider, ProviderError, Providers, Reach, Read, Reading,
};
use nerpa_core::run::apply;
use nerpa_core::schema::Schema;
use nerpa_core::secret::{Key, Recorded, Remembered, Secrets};
use nerpa_core::state::{Record, State};
use nerpa_core::{Address, ChangeKind, Graph, Observed, Resource, Value};

static DIGEST_KEY: Key = Key::from_bytes([5; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

fn secrets() -> Secrets<'static> {
    Secrets::new(&DIGEST_KEY, &NO_STORE)
}

/// Deletes willingly and refuses to create, which is the ordinary shape of a
/// replacement that goes wrong: the old machine is gone and the new one cannot
/// be had — no quota, no address, no image.
#[derive(Debug)]
struct DeletesButCannotCreate;

impl Provider for DeletesButCannotCreate {
    fn name(&self) -> &nerpa_core::Provider {
        static NAME: std::sync::OnceLock<nerpa_core::Provider> = std::sync::OnceLock::new();
        NAME.get_or_init(|| nerpa_core::Provider::new("yandex").unwrap())
    }

    fn schema(&self) -> Schema {
        Schema::new().immutable(
            nerpa_core::Provider::new("yandex").unwrap(),
            nerpa_core::Kind::new("instance").unwrap(),
            "zone",
        )
    }

    fn read(
        &self,
        _batch: &[Reading],
        _facts: nerpa_core::observed::Facts<'_>,
        _reach: &dyn Reach,
    ) -> Result<Read, ProviderError> {
        Ok(Read::new())
    }

    fn apply(
        &self,
        address: &Address,
        _kind: ChangeKind,
        _roused: bool,
        _attributes: &Attributes,
        _premise: &[nerpa_proto::Premise],
        _facts: nerpa_core::observed::Facts<'_>,
        _reach: &dyn Reach,
    ) -> Result<Attributes, ProviderError> {
        Err(ProviderError::World {
            address: address.clone(),
            problem: "there is no room for another one".to_owned(),
        })
    }

    fn delete(
        &self,
        _address: &Address,
        _recorded: &Attributes,
        _premise: &[nerpa_proto::Premise],
        _facts: nerpa_core::observed::Facts<'_>,
        _reach: &dyn Reach,
    ) -> Result<(), ProviderError> {
        Ok(())
    }

    fn fake() -> Self {
        Self
    }
}

// ATTACKS the bypass INV-STATE-005 names: a change applied without the record
// being called after it, in a branch of the loop that did not have one.
#[test]
fn a_replacement_that_could_not_create_does_not_leave_state_saying_the_old_one_is_there() {
    let instance = address("yandex:instance.db");

    let mut graph = Graph::new();
    graph
        .insert(
            Resource::new(instance.clone())
                .with("zone", "ru-central1-b")
                .with("cores", 8),
        )
        .expect("a resource");

    // What is out there now, and what state says about it: a different zone,
    // which the schema calls immutable, so the change is a replacement.
    let mut was = Attributes::new();
    was.insert("zone".to_owned(), Value::Text("ru-central1-a".to_owned()));
    was.insert("cores".to_owned(), Value::Integer(8));
    let mut observed = Observed::new();
    observed.record(instance.clone(), was.clone());

    let mut state = State::new();
    let mut recorded = BTreeMap::new();
    for (name, value) in &was {
        recorded.insert(name.clone(), Recorded::Plain(value.clone()));
    }
    state.record(
        instance.clone(),
        Record::new(recorded, std::collections::BTreeSet::new()),
    );

    let providers = Providers::new().with(Box::new(DeletesButCannotCreate));
    let plan = plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        plan.stages()[0].changes()[0].kind(),
        ChangeKind::Replace,
        "the zone cannot change in place"
    );

    let mut written = Vec::new();
    let outcome = apply(
        &plan,
        &providers,
        &Machines::new().everywhere(Box::new(Nowhere)),
        &observed,
        &mut state,
        secrets(),
        &mut |state: &State| {
            written.push(state.len());
            Ok(())
        },
    );

    assert!(outcome.is_err(), "creating the replacement failed");
    assert!(
        state.get(&instance).is_none(),
        "the old one was deleted, so state must not still say where it is: \
         a plan computed from this would reason about a resource that is gone"
    );
    assert_eq!(
        written,
        vec![0],
        "and the deletion was written down before the run stopped"
    );
}

/// Reaches nothing, because this provider never asks.
#[derive(Debug)]
struct Nowhere;

impl Reach for Nowhere {
    fn ask(&self, _ask: &nerpa_proto::Ask) -> Result<nerpa_proto::Answer, ProviderError> {
        panic!("this provider does not speak to a machine")
    }
}
