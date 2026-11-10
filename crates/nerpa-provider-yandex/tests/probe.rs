//! A temporary live probe for w-yc-real-create: a network and a subnet are
//! made when the cloud's quota allows them and reused from the folder when
//! it does not, an instance is created with a boot disk and a NAT interface,
//! every operation — create, stop, start, delete — is polled until the cloud
//! says it is done, the stopped instance is patched (the mask question asked
//! both ways) and its boot disk grown, the instance is read back after each
//! transition, and what the probe made is deleted at the end. Every step
//! lands in /tmp/probe-rc.log immediately. Delete once the wire is known.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::too_many_lines,
    clippy::map_unwrap_or,
    clippy::manual_assert,
    clippy::print_stdout,
    clippy::assigning_clones,
    reason = "a temporary live probe: blunt by design, deleted once the wire is known"
)]

use std::collections::BTreeMap;
use std::io::Write as _;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nerpa_core::address::address;
use nerpa_core::observed::Observed;
use nerpa_core::provider::{Machines, Provider, Reading};
use nerpa_provider_yandex::{Api, ApiError, Auth, Http, Response, Yandex};

struct Capturing {
    real: Http,
    authorization: Arc<Mutex<String>>,
}

impl Api for Capturing {
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, ApiError> {
        self.remember(headers);
        self.real.get(url, headers)
    }

    fn post(&self, url: &str, headers: &[(&str, &str)], body: &str) -> Result<Response, ApiError> {
        self.remember(headers);
        self.real.post(url, headers, body)
    }

    fn patch(&self, url: &str, headers: &[(&str, &str)], body: &str) -> Result<Response, ApiError> {
        self.remember(headers);
        self.real.patch(url, headers, body)
    }

    fn put(&self, url: &str, headers: &[(&str, &str)], body: &str) -> Result<Response, ApiError> {
        self.remember(headers);
        self.real.put(url, headers, body)
    }

    fn delete(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, ApiError> {
        self.remember(headers);
        self.real.delete(url, headers)
    }
}

impl Capturing {
    fn remember(&self, headers: &[(&str, &str)]) {
        for (name, value) in headers {
            if *name == "Authorization" {
                *self.authorization.lock().unwrap() = (*value).to_owned();
            }
        }
    }
}

// The operation is polled until the cloud says it is done; its final shape —
// the response, the metadata, the error — is what the caller needs.
fn poll_until_done(
    wire: &Http,
    operations: &str,
    headers: &[(&str, &str)],
    operation_id: &str,
    note: &dyn Fn(String),
) -> serde_json::Value {
    for _ in 0..60 {
        let answer = wire
            .get(&format!("{operations}/{operation_id}"), headers)
            .map(|answer| answer.body)
            .unwrap_or_else(|error| format!("ERR {error}"));
        let operation: serde_json::Value = serde_json::from_str(&answer).unwrap_or_default();
        if operation["done"].as_bool().unwrap_or(false) {
            return operation;
        }
        note(format!("PROBE poll {operation_id} not done"));
        std::thread::sleep(Duration::from_secs(5));
    }
    panic!("the operation {operation_id} never finished");
}

fn id_of(operation: &serde_json::Value, metadata_field: &str) -> String {
    operation["response"]["id"]
        .as_str()
        .or(operation["metadata"][metadata_field].as_str())
        .unwrap_or_default()
        .to_owned()
}

fn find_by_name(
    wire: &Http,
    headers: &[(&str, &str)],
    folder: &str,
    base: &str,
    kind: &str,
    name: &str,
) -> String {
    let list = wire
        .get(&format!("{base}/{kind}?folderId={folder}"), headers)
        .map(|answer| answer.body)
        .unwrap_or_default();
    serde_json::from_str::<serde_json::Value>(&list)
        .ok()
        .and_then(|list| {
            list[kind].as_array().and_then(|entries| {
                entries
                    .iter()
                    .find(|entry| entry["name"].as_str() == Some(name))
                    .cloned()
            })
        })
        .and_then(|entry| entry["id"].as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn list_kind(
    wire: &Http,
    headers: &[(&str, &str)],
    folder: &str,
    base: &str,
    kind: &str,
) -> serde_json::Value {
    wire.get(&format!("{base}/{kind}?folderId={folder}"), headers)
        .map(|answer| answer.body)
        .ok()
        .and_then(|body| serde_json::from_str(&body).ok())
        .unwrap_or_default()
}

fn first_id_of(list: &serde_json::Value, kind: &str) -> String {
    list[kind]
        .as_array()
        .and_then(|entries| entries.first())
        .and_then(|entry| entry["id"].as_str())
        .unwrap_or_default()
        .to_owned()
}

fn status_of(
    wire: &Http,
    headers: &[(&str, &str)],
    compute: &str,
    instance_id: &str,
) -> Option<String> {
    wire.get(&format!("{compute}/instances/{instance_id}"), headers)
        .ok()
        .and_then(|answer| serde_json::from_str::<serde_json::Value>(&answer.body).ok())
        .and_then(|machine| machine["status"].as_str().map(str::to_owned))
}

#[test]
#[ignore = "a live probe: set YC_FOLDER_ID and YC_KEY_FILE, then run with --ignored --nocapture"]
fn probe_the_real_create() {
    let folder = std::env::var("YC_FOLDER_ID").expect("YC_FOLDER_ID");
    let key_file = std::env::var("YC_KEY_FILE").expect("YC_KEY_FILE");
    let auth = Auth::service_account_key_file(&key_file).expect("a usable key file");
    let compute = "https://compute.api.cloud.yandex.net/compute/v1";
    let vpc = "https://vpc.api.cloud.yandex.net/vpc/v1";
    let operations = "https://operation.api.cloud.yandex.net/operations";
    let authorization = Arc::new(Mutex::new(String::new()));
    let provider = Yandex::new(
        &folder,
        compute,
        vpc,
        operations,
        "https://storage.api.cloud.yandex.net/storage/v1",
        "https://storage.yandexcloud.net",
        auth,
        Box::new(Capturing {
            real: Http::new(Duration::from_secs(60)),
            authorization: Arc::clone(&authorization),
        }),
    );
    let probe = address("yandex:bucket.anything");
    let observed = Observed::new();
    let facts = observed.facts_of(None);
    let machines = Machines::new();
    let reach = machines
        .reach(None)
        .expect("a control resource has nowhere");
    let _ = provider.read(&[Reading::new(probe, BTreeMap::new())], facts, reach);
    let token = authorization.lock().unwrap().clone();
    assert!(!token.is_empty(), "the token was not resolved");
    let headers = [("Authorization", token.as_str())];
    let wire = Http::new(Duration::from_secs(60));
    let log = Arc::new(Mutex::new(
        std::fs::File::create("/tmp/probe-rc.log").expect("a log file"),
    ));
    let note = |line: String| {
        println!("{line}");
        writeln!(log.lock().expect("the log"), "{line}").expect("the log takes it");
    };
    let clip = |text: &str, at: usize| -> String { text.chars().take(at).collect() };
    let said = |answer: &Result<Response, ApiError>| {
        answer
            .as_ref()
            .map(|answer| {
                let body: String = answer.body.chars().take(500).collect();
                format!("{} {}", answer.status, body)
            })
            .unwrap_or_else(|error| format!("{error}"))
    };
    let operation_of = |answer: Result<Response, ApiError>| -> String {
        let body = answer
            .map(|answer| answer.body)
            .unwrap_or_else(|error| format!("ERR {error}"));
        serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|operation| operation["id"].as_str().map(str::to_owned))
            .unwrap_or_default()
    };
    // The public Ubuntu image the develop line's real scenarios used, and the
    // zone they declared.
    let image = "fd8em71f8oo1hdpali1k";
    let zone = "ru-central1-d";
    let network = "nerpa-rc-net";
    let subnet = "nerpa-rc-sub";
    let instance = "nerpa-rc-instance";
    note("PROBE begins".to_owned());

    // What is already there is logged first, for the cloud's network quota is
    // small: a refusal falls back to reusing what the folder has.
    let networks_here = list_kind(&wire, &headers, &folder, vpc, "networks");
    note(format!(
        "PROBE networks here => {}",
        clip(&networks_here.to_string(), 500)
    ));
    let body = serde_json::json!({"folderId": folder, "name": network}).to_string();
    let answer = wire.post(&format!("{vpc}/networks"), &headers, &body);
    note(format!("PROBE network create => {}", said(&answer)));
    let operation_id = operation_of(answer);
    let network_created = !operation_id.is_empty();
    let mut network_id = if network_created {
        let done = poll_until_done(&wire, operations, &headers, &operation_id, &note);
        note(format!(
            "PROBE network operation => {}",
            clip(&done.to_string(), 400)
        ));
        id_of(&done, "networkId")
    } else {
        first_id_of(&networks_here, "networks")
    };
    if network_id.is_empty() && network_created {
        network_id = find_by_name(&wire, &headers, &folder, vpc, "networks", network);
    }
    note(format!(
        "PROBE network id => {network_id} (created {network_created})"
    ));
    if network_id.is_empty() {
        panic!("no network: the answers above say what the cloud said and what is there");
    }

    // The subnet the interface's address lives in: the same fallback, and
    // the reuse must be in the chosen network and the declared zone.
    let subnets_here = list_kind(&wire, &headers, &folder, vpc, "subnets");
    note(format!(
        "PROBE subnets here => {}",
        clip(&subnets_here.to_string(), 600)
    ));
    let body = serde_json::json!({
        "folderId": folder,
        "name": subnet,
        "networkId": network_id,
        "zoneId": zone,
        "v4CidrBlocks": ["10.190.0.0/24"],
    })
    .to_string();
    let answer = wire.post(&format!("{vpc}/subnets"), &headers, &body);
    note(format!("PROBE subnet create => {}", said(&answer)));
    let operation_id = operation_of(answer);
    let subnet_created = !operation_id.is_empty();
    let mut subnet_id = if subnet_created {
        let done = poll_until_done(&wire, operations, &headers, &operation_id, &note);
        note(format!(
            "PROBE subnet operation => {}",
            clip(&done.to_string(), 400)
        ));
        id_of(&done, "subnetId")
    } else {
        subnets_here["subnets"]
            .as_array()
            .and_then(|entries| {
                entries.iter().find(|entry| {
                    entry["networkId"].as_str() == Some(network_id.as_str())
                        && entry["zoneId"].as_str() == Some(zone)
                })
            })
            .and_then(|entry| entry["id"].as_str())
            .unwrap_or_default()
            .to_owned()
    };
    if subnet_id.is_empty() && subnet_created {
        subnet_id = find_by_name(&wire, &headers, &folder, vpc, "subnets", subnet);
    }
    note(format!(
        "PROBE subnet id => {subnet_id} (created {subnet_created})"
    ));
    if subnet_id.is_empty() {
        panic!("no subnet: the answers above say what the cloud said and what is there");
    }

    // The instance: a boot disk from the image, an interface with a public
    // address in the subnet.
    let body = serde_json::json!({
        "folderId": folder,
        "name": instance,
        "zoneId": zone,
        "platformId": "standard-v3",
        "resourcesSpec": {"memory": "2147483648", "cores": "2", "coreFraction": "100"},
        "bootDiskSpec": {
            "mode": "READ_WRITE",
            "autoDelete": true,
            "disk": {"size": "10737418240", "imageId": image}
        },
        "networkInterfaceSpecs": [{
            "subnetId": subnet_id,
            "primaryV4AddressSpec": {"oneToOneNatSpec": {"ipVersion": "IPV4"}}
        }],
    })
    .to_string();
    let answer = wire.post(&format!("{compute}/instances"), &headers, &body);
    note(format!("PROBE instance create => {}", said(&answer)));
    let operation_id = operation_of(answer);
    let instance_id = if operation_id.is_empty() {
        find_by_name(&wire, &headers, &folder, compute, "instances", instance)
    } else {
        let done = poll_until_done(&wire, operations, &headers, &operation_id, &note);
        note(format!(
            "PROBE instance operation => {}",
            clip(&done.to_string(), 400)
        ));
        let id = id_of(&done, "instanceId");
        if id.is_empty() {
            find_by_name(&wire, &headers, &folder, compute, "instances", instance)
        } else {
            id
        }
    };
    note(format!("PROBE instance id => {instance_id}"));
    if instance_id.is_empty() {
        panic!(
            "no instance: the answers above say whether the folder refused or the name is taken"
        );
    }

    // The read-back: the shape the provider's read has to fold into facts.
    let answer = wire
        .get(&format!("{compute}/instances/{instance_id}"), &headers)
        .map(|answer| answer.body)
        .unwrap_or_else(|error| format!("ERR {error}"));
    let machine: serde_json::Value = serde_json::from_str(&answer).unwrap_or_default();
    note(format!(
        "PROBE instance read => status {:?} fqdn {:?} resources {} bootDisk {} interfaces {}",
        machine["status"].as_str(),
        machine["fqdn"].as_str(),
        machine["resources"],
        machine["bootDisk"],
        machine["networkInterfaces"]
    ));
    let disk_id = machine["bootDisk"]["diskId"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    note(format!("PROBE disk id => {disk_id}"));

    // Stop the running instance, await the stop, then start and await: the
    // update flow the provider will drive.
    let answer = wire.post(
        &format!("{compute}/instances/{instance_id}:stop"),
        &headers,
        "{}",
    );
    note(format!("PROBE stop => {}", said(&answer)));
    let operation_id = operation_of(answer);
    if !operation_id.is_empty() {
        let done = poll_until_done(&wire, operations, &headers, &operation_id, &note);
        note(format!(
            "PROBE stop operation => {}",
            clip(&done.to_string(), 300)
        ));
    }
    note(format!(
        "PROBE instance after stop => status {:?}",
        status_of(&wire, &headers, compute, &instance_id)
    ));

    // The changes a stopped instance takes: the resources, and the boot disk
    // grown a gibibyte. The mask question is asked both ways, for the bucket
    // API answered it one way and the compute API may answer it another.
    let body =
        serde_json::json!({"resourcesSpec": {"memory": "2147483648", "cores": "2"}}).to_string();
    let answer = wire.patch(
        &format!("{compute}/instances/{instance_id}"),
        &headers,
        &body,
    );
    note(format!(
        "PROBE patch instance, no mask => {}",
        said(&answer)
    ));
    let answer = wire.patch(
        &format!("{compute}/instances/{instance_id}?updateMask=resourcesSpec"),
        &headers,
        &body,
    );
    note(format!("PROBE patch instance, mask => {}", said(&answer)));
    let answer = wire
        .get(&format!("{compute}/instances/{instance_id}"), &headers)
        .map(|answer| answer.body)
        .unwrap_or_default();
    let machine: serde_json::Value = serde_json::from_str(&answer).unwrap_or_default();
    note(format!(
        "PROBE instance after patches => name {:?} status {:?} resources {}",
        machine["name"].as_str(),
        machine["status"].as_str(),
        machine["resources"]
    ));
    let body = serde_json::json!({"size": "11811160064"}).to_string();
    let answer = wire.patch(&format!("{compute}/disks/{disk_id}"), &headers, &body);
    note(format!("PROBE patch disk, no mask => {}", said(&answer)));
    let answer = wire.patch(
        &format!("{compute}/disks/{disk_id}?updateMask=size"),
        &headers,
        &body,
    );
    note(format!("PROBE patch disk, mask => {}", said(&answer)));
    let answer = wire
        .get(&format!("{compute}/disks/{disk_id}"), &headers)
        .map(|answer| answer.body)
        .unwrap_or_default();
    note(format!(
        "PROBE disk after patches => {}",
        clip(&answer, 300)
    ));

    let answer = wire.post(
        &format!("{compute}/instances/{instance_id}:start"),
        &headers,
        "{}",
    );
    note(format!("PROBE start => {}", said(&answer)));
    let operation_id = operation_of(answer);
    if !operation_id.is_empty() {
        let done = poll_until_done(&wire, operations, &headers, &operation_id, &note);
        note(format!(
            "PROBE start operation => {}",
            clip(&done.to_string(), 300)
        ));
    }
    note(format!(
        "PROBE instance after start => status {:?}",
        status_of(&wire, &headers, compute, &instance_id)
    ));

    // The cleanup: the instance, then what the probe made — what the folder
    // already had stays.
    let answer = wire.delete(&format!("{compute}/instances/{instance_id}"), &headers);
    note(format!("PROBE delete instance => {}", said(&answer)));
    let operation_id = operation_of(answer);
    if !operation_id.is_empty() {
        let done = poll_until_done(&wire, operations, &headers, &operation_id, &note);
        note(format!(
            "PROBE delete instance operation => {}",
            clip(&done.to_string(), 300)
        ));
    }
    if subnet_created {
        let answer = wire.delete(&format!("{vpc}/subnets/{subnet_id}"), &headers);
        note(format!("PROBE delete subnet => {}", said(&answer)));
        let operation_id = operation_of(answer);
        if !operation_id.is_empty() {
            let done = poll_until_done(&wire, operations, &headers, &operation_id, &note);
            note(format!(
                "PROBE delete subnet operation => {}",
                clip(&done.to_string(), 300)
            ));
        }
    }
    if network_created {
        let answer = wire.delete(&format!("{vpc}/networks/{network_id}"), &headers);
        note(format!("PROBE delete network => {}", said(&answer)));
        let operation_id = operation_of(answer);
        if !operation_id.is_empty() {
            let done = poll_until_done(&wire, operations, &headers, &operation_id, &note);
            note(format!(
                "PROBE delete network operation => {}",
                clip(&done.to_string(), 300)
            ));
        }
    }
    note("PROBE ends".to_owned());
}
