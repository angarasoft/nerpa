//! What a provider owes back when it is handed an attribute.
//!
//! Reporting an attribute is the provider's claim that it took it. One that goes
//! in and does not come out never settles: the next plan finds it missing from
//! the reading, proposes the same change, and every run after that does the same
//! for ever. The tool stops converging and says nothing about it.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeMap;

use nerpa_core::address::address;
use nerpa_core::plan::ChangeKind;
use nerpa_core::planner::plan;
use nerpa_core::provider::{
    Attributes, Machines, Provider, ProviderError, Providers, Reach, Reading,
};
use nerpa_core::run::{RunError, apply, observe};
use nerpa_core::schema::Schema;
use nerpa_core::secret::{Key, Remembered, Secrets};
use nerpa_core::state::State;
use nerpa_core::{Graph, Resource, Value};

static DIGEST_KEY: Key = Key::from_bytes([7; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

fn secrets() -> Secrets<'static> {
    Secrets::new(&DIGEST_KEY, &NO_STORE)
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "the shape belongs to `apply`, not to this function"
)]
fn kept(_: &State) -> Result<(), String> {
    Ok(())
}

/// Reports back only the attributes it recognises, the way a provider handed
/// something it does not own behaves without meaning to.
#[derive(Debug)]
struct Selective {
    owns: &'static str,
}

impl Provider for Selective {
    fn name(&self) -> &nerpa_core::Provider {
        static NAME: std::sync::OnceLock<nerpa_core::Provider> = std::sync::OnceLock::new();
        NAME.get_or_init(|| nerpa_core::Provider::new("os").unwrap())
    }

    fn schema(&self) -> Schema {
        Schema::new()
    }

    fn read(
        &self,
        _batch: &[Reading],
        _reach: &dyn Reach,
    ) -> Result<BTreeMap<Address, Attributes>, ProviderError> {
        Ok(BTreeMap::new())
    }

    fn apply(
        &self,
        _address: &Address,
        _kind: ChangeKind,
        attributes: &Attributes,
        _reach: &dyn Reach,
    ) -> Result<Attributes, ProviderError> {
        Ok(attributes
            .iter()
            .filter(|(name, _)| name.as_str() == self.owns)
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect())
    }

    fn delete(
        &self,
        _address: &Address,
        _recorded: &Attributes,
        _reach: &dyn Reach,
    ) -> Result<(), ProviderError> {
        Ok(())
    }

    fn fake() -> Self {
        Self { owns: "path" }
    }
}

use nerpa_core::Address;

/// A far end nobody asks. These providers decide their answers themselves; the
/// seam is there because the trait has one, not because this uses it.
#[derive(Debug)]
struct Deaf;

impl Reach for Deaf {
    fn ask(&self, _ask: &nerpa_proto::Ask) -> Result<nerpa_proto::Answer, ProviderError> {
        Err(ProviderError::NotOnAMachine)
    }
}

fn machines() -> Machines {
    Machines::new().everywhere(Box::new(Deaf))
}

fn graph_of(attributes: &[(&str, &str)]) -> Graph {
    let mut graph = Graph::new();
    let mut resource = Resource::new(address("os:file.motd@web-01"));
    for (name, value) in attributes {
        resource = resource.with(*name, Value::Text((*value).to_owned()));
    }
    graph.preexisting(address("os:file.motd@web-01").target().unwrap().clone());
    graph.insert(resource).unwrap();
    graph
}

fn run(graph: &Graph, provider: Selective) -> (Result<(), RunError>, State) {
    let providers = Providers::new().with(Box::new(provider));
    let mut state = State::new();
    let observed = observe(graph, &providers, &machines(), &state).expect("readable");
    let intended =
        plan(graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    let outcome = apply(
        &intended,
        &providers,
        &machines(),
        &observed,
        &mut state,
        secrets(),
        &mut kept,
    );
    (outcome.map(|_| ()), state)
}

/// The measured bug: an attribute the provider does not own applied cleanly and
/// re-planned for ever.
#[test]
fn an_attribute_that_is_not_reported_back_stops_the_run_rather_than_never_settling() {
    let graph = graph_of(&[("path", "/etc/motd"), ("owner", "root")]);
    let (outcome, _) = run(&graph, Selective { owns: "path" });

    let Err(RunError::Unreported { attribute, .. }) = outcome else {
        panic!("an attribute nothing reports back is not a converging configuration");
    };
    assert_eq!(attribute, "owner");
}

/// It is refused *after* the record is written. The resource has been made by
/// then, and refusing without writing that down would leave it owned by nobody.
#[test]
fn what_was_made_before_the_refusal_is_still_recorded() {
    let graph = graph_of(&[("path", "/etc/motd"), ("owner", "root")]);
    let (outcome, state) = run(&graph, Selective { owns: "path" });

    assert!(outcome.is_err());
    assert!(
        state.get(&address("os:file.motd@web-01")).is_some(),
        "the resource exists now, whatever else went wrong"
    );
}

/// A provider that cannot read a value back — a credential the cloud will not
/// return — echoes it, and the echo is the claim. What it may not do is say
/// nothing.
#[test]
fn a_provider_that_echoes_what_it_cannot_read_back_is_accepted() {
    let graph = graph_of(&[("path", "/etc/motd")]);
    let (outcome, state) = run(&graph, Selective { owns: "path" });

    assert!(outcome.is_ok(), "{outcome:?}");
    assert!(state.get(&address("os:file.motd@web-01")).is_some());
}
