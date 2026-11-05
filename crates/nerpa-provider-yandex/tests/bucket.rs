//! The bucket kind: a declared `yandex:bucket` converges against the fake
//! cloud, changes in place where the storage API allows it, refuses a shrink
//! of what may only grow, and goes when its declaration does. The wire is
//! pinned against exchanges recorded from a real folder: an ignored test
//! takes the recording, and the replay walks it without a live cloud.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use nerpa_core::address::address;
use nerpa_core::provider::{Machines, Provider, Providers};
use nerpa_core::run::{apply, observe};
use nerpa_core::schema::Schema;
use nerpa_core::secret::{Key, Remembered, Secrets};
use nerpa_core::state::State;
use nerpa_core::{ChangeKind, Graph, Resource, Value};
use nerpa_provider_yandex::{Api, ApiError, Auth, Http, Response, Yandex};
use serde::{Deserialize, Serialize};

fn plan(
    graph: &Graph,
    observed: &nerpa_core::Observed,
    state: &State,
    schema: &Schema,
    secrets: Secrets<'_>,
) -> Result<nerpa_core::Plan, nerpa_core::planner::PlanError> {
    nerpa_core::planner::plan(graph, observed, state, schema, secrets).map(|planned| {
        planned.attributed_to(
            nerpa_core::Actor::new("tester", nerpa_core::Grant::default(), None).expect("an actor"),
        )
    })
}

static DIGEST_KEY: Key = Key::from_bytes([5; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

fn secrets() -> Secrets<'static> {
    Secrets::new(&DIGEST_KEY, &NO_STORE)
}

/// The declaration the bucket scenario starts from: the bucket is made with
/// every managed attribute, and the run after it has nothing to do.
fn first_declaration(bucket: &nerpa_core::Address) -> Graph {
    let mut graph = Graph::new();
    graph
        .insert(
            Resource::new(bucket.clone())
                .with("default_storage_class", "STANDARD")
                .with("versioning", "enabled")
                .with("max_size", 1_073_741_824)
                .with(
                    "tags",
                    Value::Map(BTreeMap::from([(
                        "team".to_owned(),
                        Value::Text("nerpa".to_owned()),
                    )])),
                ),
        )
        .expect("a resource");
    graph
}

/// The declaration it becomes: every managed attribute changes, each in
/// place.
fn second_declaration(bucket: &nerpa_core::Address) -> Graph {
    let mut graph = Graph::new();
    graph
        .insert(
            Resource::new(bucket.clone())
                .with("default_storage_class", "COLD")
                .with("versioning", "suspended")
                .with("max_size", 2_147_483_648)
                .with(
                    "tags",
                    Value::Map(BTreeMap::from([
                        ("team".to_owned(), Value::Text("nerpa".to_owned())),
                        ("env".to_owned(), Value::Text("test".to_owned())),
                    ])),
                ),
        )
        .expect("a resource");
    graph
}

/// Drives the bucket scenario a recording and a replay both walk: the bucket
/// is made, settles, changes in place, settles, goes, and settles again. The
/// declarations are shared, so the requests a replay makes are the requests
/// the recording took. The credential differs between the two walks — the
/// live one for the recording, a token for the replay — but no credential
/// reaches the wire the recording holds.
fn the_scenario(api: Box<dyn Api>, auth: Auth, folder: &str, bucket: &str) {
    let address = address(&format!("yandex:bucket.{bucket}"));
    let providers = Providers::new().with(Box::new(Yandex::new(
        folder,
        "https://compute.api.cloud.yandex.net/compute/v1",
        "https://vpc.api.cloud.yandex.net/vpc/v1",
        "https://storage.api.cloud.yandex.net/storage/v1",
        auth,
        api,
    )));
    let machines = Machines::new();
    let mut state = State::new();
    for graph in [
        first_declaration(&address),
        first_declaration(&address),
        second_declaration(&address),
        second_declaration(&address),
        Graph::new(),
        Graph::new(),
    ] {
        let observed = observe(&graph, &providers, &machines, &state).expect("observed");
        let planned =
            plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
        apply(
            &planned,
            &providers,
            &machines,
            &observed,
            &mut state,
            secrets(),
            &mut |_| Ok(()),
        )
        .expect("applied");
    }
}

#[test]
fn a_bucket_converges_against_the_fake() {
    let bucket = address("yandex:bucket.backup");
    let graph = first_declaration(&bucket);

    let mut state = State::new();
    let providers = Providers::new().with(Box::new(Yandex::fake()));
    let machines = Machines::new();

    let observed = observe(&graph, &providers, &machines, &state).expect("observed");
    let first = plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        first.stages()[0].changes()[0].kind(),
        ChangeKind::Create,
        "a bucket not out there is created"
    );

    apply(
        &first,
        &providers,
        &machines,
        &observed,
        &mut state,
        secrets(),
        &mut |_| Ok(()),
    )
    .expect("applied");
    assert!(state.get(&bucket).is_some(), "the bucket was recorded");

    let observed = observe(&graph, &providers, &machines, &state).expect("observed");
    let second =
        plan(&graph, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        second.changes().count(),
        0,
        "a second run has nothing to do"
    );
}

#[test]
fn a_bucket_changes_in_place_where_the_api_allows() {
    let bucket = address("yandex:bucket.backup");

    let mut state = State::new();
    let providers = Providers::new().with(Box::new(Yandex::fake()));
    let machines = Machines::new();

    let first_graph = first_declaration(&bucket);
    let observed = observe(&first_graph, &providers, &machines, &state).expect("observed");
    let first = plan(
        &first_graph,
        &observed,
        &state,
        &providers.schema(),
        secrets(),
    )
    .expect("plannable");
    apply(
        &first,
        &providers,
        &machines,
        &observed,
        &mut state,
        secrets(),
        &mut |_| Ok(()),
    )
    .expect("applied");

    let changed = second_declaration(&bucket);
    let observed = observe(&changed, &providers, &machines, &state).expect("observed");
    let update =
        plan(&changed, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        update.stages()[0].changes()[0].kind(),
        ChangeKind::Update,
        "a change to what the storage API allows is made in place"
    );
    apply(
        &update,
        &providers,
        &machines,
        &observed,
        &mut state,
        secrets(),
        &mut |_| Ok(()),
    )
    .expect("applied");

    let observed = observe(&changed, &providers, &machines, &state).expect("observed");
    let settled =
        plan(&changed, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        settled.changes().count(),
        0,
        "the corrected bucket is what the declaration asks for"
    );
}

#[test]
fn a_shrink_of_max_size_is_refused_while_planning() {
    let bucket = address("yandex:bucket.backup");

    let mut state = State::new();
    let providers = Providers::new().with(Box::new(Yandex::fake()));
    let machines = Machines::new();

    let first_graph = first_declaration(&bucket);
    let observed = observe(&first_graph, &providers, &machines, &state).expect("observed");
    let first = plan(
        &first_graph,
        &observed,
        &state,
        &providers.schema(),
        secrets(),
    )
    .expect("plannable");
    apply(
        &first,
        &providers,
        &machines,
        &observed,
        &mut state,
        secrets(),
        &mut |_| Ok(()),
    )
    .expect("applied");

    let mut shrunk = Graph::new();
    shrunk
        .insert(
            Resource::new(bucket.clone())
                .with("default_storage_class", "STANDARD")
                .with("versioning", "enabled")
                .with("max_size", 536_870_912)
                .with(
                    "tags",
                    Value::Map(BTreeMap::from([(
                        "team".to_owned(),
                        Value::Text("nerpa".to_owned()),
                    )])),
                ),
        )
        .expect("a resource");

    let observed = observe(&shrunk, &providers, &machines, &state).expect("observed");
    let refused = plan(&shrunk, &observed, &state, &providers.schema(), secrets())
        .expect_err("a shrink is refused rather than planned");
    assert!(
        matches!(
            &refused,
            nerpa_core::planner::PlanError::ShrinkRefused { attribute, .. } if attribute == "max_size"
        ),
        "the refusal names what may only grow: {refused}"
    );
}

#[test]
fn a_bucket_goes_when_its_declaration_does() {
    let bucket = address("yandex:bucket.backup");

    let mut state = State::new();
    let providers = Providers::new().with(Box::new(Yandex::fake()));
    let machines = Machines::new();

    let first_graph = first_declaration(&bucket);
    let observed = observe(&first_graph, &providers, &machines, &state).expect("observed");
    let first = plan(
        &first_graph,
        &observed,
        &state,
        &providers.schema(),
        secrets(),
    )
    .expect("plannable");
    apply(
        &first,
        &providers,
        &machines,
        &observed,
        &mut state,
        secrets(),
        &mut |_| Ok(()),
    )
    .expect("applied");

    let gone = Graph::new();
    let observed = observe(&gone, &providers, &machines, &state).expect("observed");
    let removal =
        plan(&gone, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert!(
        removal
            .changes()
            .any(|change| change.kind() == ChangeKind::Delete),
        "a bucket no longer declared goes"
    );
    apply(
        &removal,
        &providers,
        &machines,
        &observed,
        &mut state,
        secrets(),
        &mut |_| Ok(()),
    )
    .expect("applied");

    let observed = observe(&gone, &providers, &machines, &state).expect("observed");
    let settled =
        plan(&gone, &observed, &state, &providers.schema(), secrets()).expect("plannable");
    assert_eq!(
        settled.changes().count(),
        0,
        "the removal leaves nothing to do"
    );
}

/// One exchange a run made, as the recording holds it.
#[derive(Debug, Serialize, Deserialize)]
struct RecordedExchange {
    method: String,
    url: String,
    body: Option<String>,
    status: u16,
    response: String,
}

/// The recording of the bucket scenario: the folder and bucket it walked, and
/// the storage exchanges in the order they happened. No header is held, so no
/// token is.
#[derive(Debug, Serialize, Deserialize)]
struct BucketRecording {
    folder: String,
    bucket: String,
    exchanges: Vec<RecordedExchange>,
}

/// An API that answers from the real cloud and remembers the storage
/// exchanges it made. The token exchange passes through unremembered: it is
/// not storage, and its body is not reproducible.
struct Recording {
    real: Http,
    storage: String,
    exchanges: Arc<Mutex<Vec<RecordedExchange>>>,
}

impl Recording {
    /// Remembers the exchange, where it is one this recording is about.
    fn remember(&self, method: &str, url: &str, body: Option<&str>, answer: &Response) {
        if url.starts_with(&self.storage) {
            self.exchanges
                .lock()
                .expect("the recording is not poisoned")
                .push(RecordedExchange {
                    method: method.to_owned(),
                    url: url.to_owned(),
                    body: body.map(str::to_owned),
                    status: answer.status,
                    response: answer.body.clone(),
                });
        }
    }
}

impl Api for Recording {
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, ApiError> {
        let answer = self.real.get(url, headers)?;
        self.remember("GET", url, None, &answer);
        Ok(answer)
    }

    fn post(&self, url: &str, headers: &[(&str, &str)], body: &str) -> Result<Response, ApiError> {
        let answer = self.real.post(url, headers, body)?;
        self.remember("POST", url, Some(body), &answer);
        Ok(answer)
    }

    fn patch(&self, url: &str, headers: &[(&str, &str)], body: &str) -> Result<Response, ApiError> {
        let answer = self.real.patch(url, headers, body)?;
        self.remember("PATCH", url, Some(body), &answer);
        Ok(answer)
    }

    fn delete(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, ApiError> {
        let answer = self.real.delete(url, headers)?;
        self.remember("DELETE", url, None, &answer);
        Ok(answer)
    }
}

#[test]
#[ignore = "needs a live cloud: set YC_FOLDER_ID and YC_KEY_FILE, then run with --ignored"]
fn take_a_recording_of_the_bucket_scenario() {
    let folder = std::env::var("YC_FOLDER_ID").expect("YC_FOLDER_ID");
    let key_file = std::env::var("YC_KEY_FILE").expect("YC_KEY_FILE");
    let auth = Auth::service_account_key_file(&key_file).expect("a usable key file");
    // A bucket's name is unique across the whole platform, so the fixture owns
    // one of its own. The scenario deletes it before the recording is written,
    // so the replay walks a world that no longer exists there.
    let bucket = "nerpa-fixture-bucket";
    let exchanges: Arc<Mutex<Vec<RecordedExchange>>> = Arc::default();
    let api = Recording {
        real: Http::new(std::time::Duration::from_secs(30)),
        storage: "https://storage.api.cloud.yandex.net/storage/v1".to_owned(),
        exchanges: Arc::clone(&exchanges),
    };
    the_scenario(Box::new(api), auth, &folder, bucket);
    let recording = BucketRecording {
        folder: folder.clone(),
        bucket: bucket.to_owned(),
        exchanges: Arc::try_unwrap(exchanges)
            .expect("one holder of the recording")
            .into_inner()
            .expect("the recording is not poisoned"),
    };
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/recordings/bucket.json");
    std::fs::create_dir_all(path.parent().expect("a directory to write into"))
        .expect("the recordings directory");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&recording).expect("a serialisable recording"),
    )
    .expect("the recording is written");
}
