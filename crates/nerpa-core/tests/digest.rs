//! One number standing for everything a plan intends to do.
//!
//! What it is for: `0033` lets a batch of operations run anything, and what
//! limits that authority is the plan it came from; `0034` binds the premise to
//! it; `0009` has a third party sign it. Every one of those is worth exactly
//! what the number is worth, so what it covers and what it refuses to cover are
//! the whole of it.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeMap;

use nerpa_core::address::address;
use nerpa_core::plan::PlanDigest;
use nerpa_core::planner::plan_at;
use nerpa_core::provider::{Attributes, Provider, ProviderError, Providers, Reach, Reading};
use nerpa_core::schema::Schema;
use nerpa_core::secret::{Key, Remembered, Secrets};
use nerpa_core::state::{State, Written};
use nerpa_core::{Address, Graph, Observed, Plan, Resource, Value};

static DIGEST_KEY: Key = Key::from_bytes([7; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

fn secrets() -> Secrets<'static> {
    Secrets::new(&DIGEST_KEY, &NO_STORE)
}

/// Answers about nothing, so every declared resource has to be created.
#[derive(Debug)]
struct Empty;

impl Provider for Empty {
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
        _kind: nerpa_core::ChangeKind,
        attributes: &Attributes,
        _reach: &dyn Reach,
    ) -> Result<Attributes, ProviderError> {
        Ok(attributes.clone())
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
        Self
    }
}

fn planned(resources: Vec<Resource>, observed: &Observed) -> Plan {
    let mut graph = Graph::new();
    for resource in resources {
        if let Some(target) = resource.address().target() {
            graph.preexisting(target.clone());
        }
        graph.insert(resource).unwrap();
    }
    let providers = Providers::new().with(Box::new(Empty));
    plan_at(
        &graph,
        observed,
        &State::new(),
        &providers.schema(),
        secrets(),
        Written::at(1_700_000_000),
    )
    .expect("plannable")
}

fn one(attributes: &[(&str, &str)]) -> Plan {
    let mut resource = Resource::new(address("os:file.motd@web-01"));
    for (name, value) in attributes {
        resource = resource.with(*name, Value::Text((*value).to_owned()));
    }
    planned(vec![resource], &Observed::new())
}

#[test]
fn the_same_plan_is_the_same_number_every_time() {
    let first = one(&[("path", "/etc/motd")]).digest().expect("hashable");
    let second = one(&[("path", "/etc/motd")]).digest().expect("hashable");
    assert_eq!(first, second);
    assert_eq!(
        PlanDigest::from_hex(&first.to_hex()),
        Some(first),
        "and it survives being written down, which is how it will travel"
    );
}

/// Anything a person is approving has to be inside the number, or their
/// signature covers a plan that is not the one that runs.
#[test]
fn every_part_of_a_change_moves_the_number() {
    let base = one(&[("path", "/etc/motd")]).digest().expect("hashable");

    let other_value = one(&[("path", "/etc/issue")]).digest().expect("hashable");
    assert_ne!(base, other_value, "the value it will write");

    let other_name = one(&[("target", "/etc/motd")]).digest().expect("hashable");
    assert_ne!(base, other_name, "the attribute's name");

    let more = one(&[("path", "/etc/motd"), ("mode", "0644")])
        .digest()
        .expect("hashable");
    assert_ne!(base, more, "an attribute nobody asked about before");

    let elsewhere = planned(
        vec![Resource::new(address("os:file.issue@web-01")).with("path", "/etc/motd")],
        &Observed::new(),
    )
    .digest()
    .expect("hashable");
    assert_ne!(base, elsewhere, "which resource it is about");
}

/// Two attributes whose names and values run together the same way must not
/// hash the same. This is what the length in front of every piece of text is
/// for, and nothing else would notice if it went.
#[test]
fn text_that_runs_together_does_not_collide() {
    let split = one(&[("ab", "cd")]).digest().expect("hashable");
    let joined = one(&[("abc", "d")]).digest().expect("hashable");
    assert_ne!(split, joined);
}

/// A plan that could not look at everything carries how old that knowledge is,
/// an age moves every second, and a number over it would never settle. So it
/// has none, and says why.
#[test]
fn a_plan_that_could_not_look_at_everything_has_no_number() {
    let mut observed = Observed::new();
    observed.unreachable(address("os:file.motd@web-01").target().unwrap().clone());

    let plan = planned(
        vec![Resource::new(address("os:file.motd@web-01")).with("path", "/etc/motd")],
        &observed,
    );

    let refusal = plan
        .digest()
        .expect_err("an incomplete plan cannot be signed");
    assert_eq!(refusal.machines, 1);
    assert!(refusal.to_string().contains("never settle"), "{refusal}");
}

/// An empty plan still has a number: "there is nothing to do" is a statement
/// somebody may want to have signed.
#[test]
fn a_plan_with_nothing_in_it_still_has_one() {
    let nothing = planned(Vec::new(), &Observed::new());
    assert!(nothing.is_empty());
    assert!(nothing.digest().is_ok());
}
