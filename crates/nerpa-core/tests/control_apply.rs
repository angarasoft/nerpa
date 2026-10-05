//! A control-domain change applies through a provider's `act`, not through a
//! machine.
//!
//! A control resource has no machine to send operations to, so the apply loop
//! branches: the provider performs the change itself, and the mark still goes
//! down before it and the conclusion after. This fake provider makes the change
//! in memory, so a second run reads it back and has nothing to do.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeMap;

use nerpa_core::address::address;
use nerpa_core::provider::{
    Attributes, Concluded, Machines, Prepared, Provider, ProviderError, Providers, Read, Reading,
    Sending,
};
use nerpa_core::run::{apply, observe};
use nerpa_core::schema::Schema;
use nerpa_core::secret::{Key, Remembered, Secrets};
use nerpa_core::state::State;
use nerpa_core::{Address, ChangeKind, Graph, Resource};

/// The plan a test drives, attributed to a tester: `run::apply` proceeds only
/// with an actor identity (`INV-TRUST-001`), and a test is a run of a kind.
fn plan(
    graph: &Graph,
    observed: &nerpa_core::Observed,
    state: &State,
    schema: &Schema,
    secrets: Secrets<'_>,
) -> Result<nerpa_core::Plan, nerpa_core::planner::PlanError> {
    nerpa_core::planner::plan(graph, observed, state, schema, secrets).map(|planned| {
        planned.attributed_to(nerpa_core::Actor::new("tester", "run", None).expect("an actor"))
    })
}

static DIGEST_KEY: Key = Key::from_bytes([5; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

fn secrets() -> Secrets<'static> {
    Secrets::new(&DIGEST_KEY, &NO_STORE)
}

/// A control-domain provider that acts in memory: what it made is what it reads
/// back, so a second run has nothing to do.
#[derive(Debug, Default)]
struct Cloud {
    made: std::sync::Mutex<BTreeMap<Address, Attributes>>,
}

impl Provider for Cloud {
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
        batch: &[Reading],
        _facts: nerpa_core::observed::Facts<'_>,
        _reach: &dyn nerpa_core::provider::Reach,
    ) -> Result<Read, ProviderError> {
        let made = self.made.lock().unwrap();
        let mut found = Read::new();
        for reading in batch {
            if let Some(attributes) = made.get(reading.address()) {
                found = found.with(reading.address().clone(), attributes.clone(), Vec::new());
            }
        }
        Ok(found)
    }

    fn prepare(
        &self,
        _change: &Prepared<'_>,
    ) -> Result<nerpa_core::provider::Planned, ProviderError> {
        unreachable!("a control change is applied through act, never prepared")
    }

    fn act(&self, change: &Prepared<'_>) -> Result<Concluded, ProviderError> {
        if change.kind == Sending::Delete {
            self.made.lock().unwrap().remove(change.address);
            Ok(Concluded::Gone)
        } else {
            self.made
                .lock()
                .unwrap()
                .insert(change.address.clone(), change.desired.clone());
            Ok(Concluded::Became(change.desired.clone()))
        }
    }

    fn conclude(
        &self,
        _change: &nerpa_core::provider::Concluding<'_>,
        _became: &[nerpa_core::provider::Answered],
    ) -> Result<Concluded, ProviderError> {
        unreachable!("a control change is concluded by act, not by answers")
    }

    fn afterwards(&self, _address: &Address, _kind: Sending) -> nerpa_core::provider::Afterwards {
        nerpa_core::provider::Afterwards::Read
    }

    fn fake() -> Self {
        Self::default()
    }
}

/// A control resource is created through `act`, recorded, and a second run has
/// nothing to do — the control form of convergence, without a machine anywhere.
#[test]
fn a_control_resource_converges_through_act() {
    let instance = address("yandex:instance.db");
    let mut graph = Graph::new();
    graph
        .insert(
            Resource::new(instance.clone())
                .with("zone", "ru-central1-b")
                .with("cores", 8),
        )
        .expect("a resource");

    let mut state = State::new();
    let providers = Providers::new().with(Box::new(Cloud::default()));
    let machines = Machines::new();

    // First run: nothing out there, so the plan creates it.
    let observed = observe(&graph, &providers, &machines, &state).expect("observed");
    let first = plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        first.stages()[0].changes()[0].kind(),
        ChangeKind::Create,
        "a resource not out there is created"
    );

    let mut written = Vec::new();
    apply(
        &first,
        &providers,
        &machines,
        &observed,
        &mut state,
        secrets(),
        &mut |state: &State| {
            written.push(state.len());
            Ok(())
        },
    )
    .expect("applied");

    assert!(
        state.get(&instance).is_some(),
        "the resource was recorded after it was acted on"
    );
    assert!(!written.is_empty(), "state was written down");

    // Second run: the fake now has the resource, so nothing is left to do.
    let observed = observe(&graph, &providers, &machines, &state).expect("observed");
    let second =
        plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        second.changes().count(),
        0,
        "a second run has nothing to do"
    );
}
