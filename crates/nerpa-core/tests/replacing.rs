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

    /// A replacement is one batch with the forgetting in the middle: what is
    /// there is destroyed, and then the new one is made. Whether the second half
    /// works is the machine's answer, not this provider's decision.
    fn prepare(
        &self,
        change: &nerpa_core::provider::Prepared<'_>,
    ) -> Result<nerpa_core::provider::Planned, ProviderError> {
        let step = |what: &str| nerpa_proto::Operation::Process {
            program: what.to_owned(),
            arguments: Vec::new(),
            stdin: Vec::new(),
            private: false,
            expect: vec![0],
        };
        match change.kind {
            nerpa_core::provider::Sending::Delete => Ok(nerpa_core::provider::Planned {
                operations: vec![step("destroy")],
                forget_after: None,
                carries: BTreeMap::new(),
            }),
            nerpa_core::provider::Sending::Replace => Ok(nerpa_core::provider::Planned {
                operations: vec![step("destroy"), step("create")],
                forget_after: Some(1),
                carries: BTreeMap::new(),
            }),
            _ => Ok(nerpa_core::provider::Planned {
                operations: vec![step("create")],
                forget_after: None,
                carries: BTreeMap::new(),
            }),
        }
    }

    fn conclude(
        &self,
        change: &nerpa_core::provider::Concluding<'_>,
        _became: &[nerpa_core::provider::Answered],
    ) -> Result<nerpa_core::provider::Concluded, ProviderError> {
        if change.kind == nerpa_core::provider::Sending::Delete {
            return Ok(nerpa_core::provider::Concluded::Gone);
        }
        Ok(nerpa_core::provider::Concluded::Became(
            change.plain.clone(),
        ))
    }

    fn afterwards(
        &self,
        _address: &Address,
        _kind: nerpa_core::provider::Sending,
    ) -> nerpa_core::provider::Afterwards {
        nerpa_core::provider::Afterwards::Read
    }

    fn fake() -> Self {
        Self
    }
}

// ATTACKS the bypass INV-STATE-005 names: a change applied without the record
// being called after it, in a branch of the loop that did not have one.
#[test]
fn a_replacement_that_could_not_create_does_not_leave_state_saying_the_old_one_is_there() {
    // On a machine, because the engine sends a change to where the resource
    // lives: a resource in the `control` domain has no machine to send to, and
    // the engine says so rather than sending anywhere.
    let instance = address("yandex:instance.db@web-01");

    let mut graph = Graph::new();
    graph.preexisting(nerpa_core::Target::new("web-01").unwrap());
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
        &Machines::new().everywhere(Box::new(NoRoom::default())),
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
        vec![1, 0, 0],
        "three writes before the run stopped: the mark, while the old one was \
         still recorded; the deletion; and the mark turning into an entry, \
         because the half that makes the new one had already run when it failed"
    );
}

/// A machine with room for nothing more: it destroys what is there, and refuses
/// to make anything after that.
///
/// The ordinary shape of a replacement that goes wrong — no quota, no address,
/// no image — and the failure arrives where it really does, on the second half
/// of the batch, after the first half has already happened.
#[derive(Debug, Default)]
struct NoRoom {
    batches: std::sync::Mutex<usize>,
}

impl Reach for NoRoom {
    fn ask(&self, ask: &nerpa_proto::Ask) -> Result<nerpa_proto::Answer, ProviderError> {
        let nerpa_proto::Ask::Perform { operations, .. } = ask else {
            panic!("nothing is looked at in this test: {ask:?}");
        };
        let mut batches = self.batches.lock().unwrap();
        *batches += 1;
        if *batches == 1 {
            return Ok(nerpa_proto::Answer::Done {
                became: operations
                    .iter()
                    .map(|_| nerpa_proto::Seen::Nothing)
                    .collect(),
            });
        }
        Ok(nerpa_proto::Answer::Failed {
            at: nerpa_proto::At::Operation(0),
            problem: "there is no room for another one".to_owned(),
        })
    }
}
