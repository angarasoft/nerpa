//! One number standing for everything a plan intends to do.
//!
//! What it is for: `0033` lets a batch of operations run anything, and what
//! limits that authority is the plan it came from; `0034` binds the premise to
//! it; `0009` has a third party sign it. Every one of those is worth exactly
//! what the number is worth, so what it covers and what it refuses to cover are
//! the whole of it.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use nerpa_core::address::address;
use nerpa_core::attribute::{Attribute, Reference};
use nerpa_core::plan::PlanDigest;
use nerpa_core::planner::plan_at;
use nerpa_core::provider::{Attributes, Provider, ProviderError, Providers, Reach, Read, Reading};
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
        _facts: nerpa_core::observed::Facts<'_>,
        _reach: &dyn Reach,
    ) -> Result<Read, ProviderError> {
        Ok(Read::new())
    }
    fn apply(
        &self,
        _address: &Address,
        _kind: nerpa_core::ChangeKind,
        _roused: bool,
        attributes: &Attributes,
        _premise: &[nerpa_proto::Premise],
        _facts: nerpa_core::observed::Facts<'_>,
        _reach: &dyn Reach,
    ) -> Result<Attributes, ProviderError> {
        Ok(attributes.clone())
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

/// Two references naming a different resource and a different attribute must
/// not share a number. They are encoded as one printed string, and printing
/// joins two pieces with a separator either piece may itself contain: a resource
/// name may hold a dot (`a_name_may_contain_dots_because_kind_is_split_off_first`),
/// so moving the dot one place moves the boundary and prints the same.
///
/// In the control domain, which is where cloud resources live and where an
/// address ends with the name rather than with `@machine`. Both plans hold the
/// same three resources, so the only thing that differs is which of them the
/// attribute reads from.
#[test]
fn two_references_that_differ_only_in_where_the_dot_falls_do_not_collide() {
    let plan = |source: &str, attribute: &str| {
        let mut graph = Graph::new();
        graph.preexisting(nerpa_core::Target::new("web-01").expect("a node"));
        for named in ["os:file.a", "os:file.a.b"] {
            graph
                .insert(Resource::new(address(named)))
                .expect("a resource to read from");
        }
        graph
            .insert(Resource::new(address("os:file.motd")).with(
                "path",
                Attribute::Unresolved(Reference::to(address(source), attribute)),
            ))
            .expect("the resource that reads");
        let providers = Providers::new().with(Box::new(Empty));
        plan_at(
            &graph,
            &Observed::new(),
            &State::new(),
            &providers.schema(),
            secrets(),
            Written::at(1_700_000_000),
        )
        .expect("plannable")
        .digest()
        .expect("hashable")
    };

    assert_ne!(
        plan("os:file.a.b", "c"),
        plan("os:file.a", "b.c"),
        "a different resource and a different attribute are a different plan"
    );
}

/// The same, for a secret. A store path may hold an `@`, which is what the
/// printed form uses to introduce a version — so two places in a store print
/// alike, and a plan says which secret it reads.
///
/// The two hold the same value, because the value's digest is in the number as
/// well: what is being attacked is whether the *source* is.
#[test]
fn two_secrets_that_differ_only_in_where_the_version_begins_do_not_collide() {
    let store = Remembered::new()
        .holding("a@b", Value::Text("same".to_owned()))
        .holding("a", Value::Text("same".to_owned()));
    let plan = |path: &str, version: Option<&str>| {
        let source = nerpa_core::secret::Source::Stored {
            provider: "vault".to_owned(),
            path: path.to_owned(),
            version: version.map(ToOwned::to_owned),
        };
        let mut graph = Graph::new();
        graph.preexisting(nerpa_core::Target::new("web-01").expect("a node"));
        graph
            .insert(
                Resource::new(address("os:file.motd@web-01"))
                    .with("path", Attribute::Secret(source)),
            )
            .expect("the resource");
        let providers = Providers::new().with(Box::new(Empty));
        plan_at(
            &graph,
            &Observed::new(),
            &State::new(),
            &providers.schema(),
            Secrets::new(&DIGEST_KEY, &store),
            Written::at(1_700_000_000),
        )
        .expect("plannable")
        .digest()
        .expect("hashable")
    };

    assert_ne!(
        plan("a@b", None),
        plan("a", Some("b")),
        "two places in a store are two secrets, whatever they happen to hold"
    );
}
