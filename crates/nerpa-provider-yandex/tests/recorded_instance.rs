//! The instance wire is pinned against recorded real-cloud exchanges: a
//! create and a stop-update-start-and-disk-grow are replayed exactly, so a
//! change to the request shape breaks the test rather than reaching a real
//! cloud.
//!
//! All exchanges are from the probe captured on 2026-11-10; the IAM-token
//! and bucket-probe exchanges (the first two in the log) are skipped because
//! the recorded provider uses `Auth::IamToken` directly.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeSet;
use std::time::Duration;

use nerpa_core::address::address;
use nerpa_core::observed::Observed;
use nerpa_core::provider::{Prepared, Provider, Sending};
use nerpa_provider_yandex::{Auth, Exchange, Recorded, Response, Yandex};

// ---------------------------------------------------------------------------
// Real IDs from the probe capture (2026-11-10).
// ---------------------------------------------------------------------------
const FOLDER: &str = "b1g5kreli0ft87heedo0";
const INSTANCE_ID: &str = "bg047br9nt0m5jk6vp14";
const DISK_ID: &str = "bg0i7ughb6sdu6ss49a7";
const SUBNET_ID: &str = "ajcrs2v15h80pegkhele";
const IMAGE_ID: &str = "fd8em71f8oo1hdpali1k";
const OP_CREATE: &str = "bg0pb7mr1o6784pshe6o";
const OP_STOP: &str = "bg0schlfrdikou09j3p9";
const OP_PATCH_INSTANCE: &str = "bg0uokldgihv903nbeqa";
const OP_PATCH_DISK: &str = "bg0msmlt45brh7oe08ec";
const OP_START: &str = "bg0i7nsqklhac5kiea2a";

const COMPUTE: &str = "https://compute.api.cloud.yandex.net/compute/v1";
const OPERATIONS: &str = "https://operation.api.cloud.yandex.net/operations";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A provider that replays the given exchanges with no poll delay.
fn provider_over(exchanges: Vec<Exchange>) -> Yandex {
    Yandex::new(
        FOLDER,
        COMPUTE,
        "https://vpc.api.cloud.yandex.net/vpc/v1",
        OPERATIONS,
        "https://storage.api.cloud.yandex.net/storage/v1",
        "https://storage.yandexcloud.net",
        Auth::IamToken("token".to_owned()),
        Box::new(Recorded::new(exchanges)),
    )
    .with_poll(Duration::ZERO)
}

/// An attributes map built from key-value pairs.
fn attrs(pairs: &[(&str, nerpa_core::Value)]) -> nerpa_core::provider::Attributes {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect()
}

/// The facts object used for all act calls (a control provider ignores it).
fn facts() -> nerpa_core::observed::Facts<'static> {
    static OBSERVED: std::sync::OnceLock<Observed> = std::sync::OnceLock::new();
    OBSERVED.get_or_init(Observed::new).facts_of(None)
}

/// An operation that has not finished yet.
fn op_pending(id: &str) -> Response {
    Response {
        status: 200,
        body: format!(r#"{{"id":"{id}","done":false}}"#),
    }
}

/// An operation that has finished without error.
fn op_done(id: &str) -> Response {
    Response {
        status: 200,
        body: format!(r#"{{"id":"{id}","done":true}}"#),
    }
}

/// A list response with an empty folder.
fn list_empty() -> Response {
    Response {
        status: 200,
        body: "{}".to_owned(),
    }
}

/// A list response with the probe instance present (RUNNING, 2 GB memory, 2 cores).
fn list_running_2gb() -> Response {
    Response {
        status: 200,
        body: format!(
            r#"{{"instances":[{{"id":"{INSTANCE_ID}","name":"nerpa-rc-instance","zoneId":"ru-central1-e","platformId":"standard-v3","resources":{{"coreFraction":"100","cores":"2","memory":"2147483648"}},"networkInterfaces":[{{"subnetId":"{SUBNET_ID}"}}],"status":"RUNNING","bootDisk":{{"diskId":"{DISK_ID}"}}}}]}}"#
        ),
    }
}

/// A list response with the instance present after the resources update (4 GB memory).
fn list_running_4gb() -> Response {
    Response {
        status: 200,
        body: format!(
            r#"{{"instances":[{{"id":"{INSTANCE_ID}","name":"nerpa-rc-instance","zoneId":"ru-central1-e","platformId":"standard-v3","resources":{{"coreFraction":"100","cores":"2","memory":"4294967296"}},"networkInterfaces":[{{"subnetId":"{SUBNET_ID}"}}],"status":"RUNNING","bootDisk":{{"diskId":"{DISK_ID}"}}}}]}}"#
        ),
    }
}

/// The disk at 10 GiB.
fn disk_10gib() -> Response {
    Response {
        status: 200,
        body: format!(r#"{{"id":"{DISK_ID}","size":"10737418240","sourceImageId":"{IMAGE_ID}"}}"#),
    }
}

/// The disk at 11 GiB (after the grow).
fn disk_11gib() -> Response {
    Response {
        status: 200,
        body: format!(r#"{{"id":"{DISK_ID}","size":"11811160064","sourceImageId":"{IMAGE_ID}"}}"#),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The provider creates an instance via the real Compute API wire.  The
/// recorded body pins the shape:
/// - `bootDiskSpec.diskSpec` (not `bootDiskSpec.disk`)
/// - `coreFraction` in `resourcesSpec`
/// - `primaryV4AddressSpec.oneToOneNatSpec` on the interface spec
///
/// A change to any of those fields makes the body mismatch and the test fails.
#[test]
fn create_instance_pins_the_real_wire() {
    let instance = address("yandex:instance.nerpa-rc-instance");

    // The body the provider serializes — field order follows struct
    // declarations in wire.rs.
    let create_body = format!(
        r#"{{"folderId":"{FOLDER}","name":"nerpa-rc-instance","zoneId":"ru-central1-e","platformId":"standard-v3","resourcesSpec":{{"coreFraction":"100","cores":"2","memory":"2147483648"}},"bootDiskSpec":{{"mode":"READ_WRITE","autoDelete":true,"diskSpec":{{"imageId":"{IMAGE_ID}","size":"10737418240"}}}},"networkInterfaceSpecs":[{{"subnetId":"{SUBNET_ID}","primaryV4AddressSpec":{{"oneToOneNatSpec":{{"ipVersion":"IPV4"}}}}}}]}}"#
    );

    // Exchange sequence for act_instance(Create):
    //  1. list_instances   — the existing instance is not there
    //  2. POST create      — returns pending operation
    //  3. GET op (pending)
    //  4. GET op (done)
    //  5. list_instances   — read_instance: find the new instance
    //  6. GET disk         — boot_disk_of
    let exchanges = vec![
        Exchange {
            method: "GET",
            url: format!("{COMPUTE}/instances?folderId={FOLDER}"),
            body: None,
            response: list_empty(),
        },
        Exchange {
            method: "POST",
            url: format!("{COMPUTE}/instances"),
            body: Some(create_body),
            response: Response {
                status: 200,
                body: format!(
                    r#"{{"id":"{OP_CREATE}","description":"Create instance","done":false,"metadata":{{"instanceId":"{INSTANCE_ID}"}}}}"#
                ),
            },
        },
        Exchange {
            method: "GET",
            url: format!("{OPERATIONS}/{OP_CREATE}"),
            body: None,
            response: op_pending(OP_CREATE),
        },
        Exchange {
            method: "GET",
            url: format!("{OPERATIONS}/{OP_CREATE}"),
            body: None,
            response: op_done(OP_CREATE),
        },
        Exchange {
            method: "GET",
            url: format!("{COMPUTE}/instances?folderId={FOLDER}"),
            body: None,
            response: list_running_2gb(),
        },
        Exchange {
            method: "GET",
            url: format!("{COMPUTE}/disks/{DISK_ID}"),
            body: None,
            response: disk_10gib(),
        },
    ];

    let provider = provider_over(exchanges);
    let desired = attrs(&[
        (
            "zone_id",
            nerpa_core::Value::Text("ru-central1-e".to_owned()),
        ),
        (
            "platform_id",
            nerpa_core::Value::Text("standard-v3".to_owned()),
        ),
        ("cores", nerpa_core::Value::Integer(2)),
        ("memory_gb", nerpa_core::Value::Integer(2)),
        ("subnet_id", nerpa_core::Value::Text(SUBNET_ID.to_owned())),
        (
            "boot_disk_image_id",
            nerpa_core::Value::Text(IMAGE_ID.to_owned()),
        ),
        ("boot_disk_gb", nerpa_core::Value::Integer(10)),
    ]);
    let secret: BTreeSet<String> = BTreeSet::new();
    let prepared = Prepared {
        address: &instance,
        kind: Sending::Create,
        roused: false,
        resent: false,
        desired: &desired,
        secret: &secret,
        recorded: None,
        facts: facts(),
    };
    let concluded = provider.act(&prepared).expect("the create succeeds");
    assert!(
        matches!(concluded, nerpa_core::provider::Concluded::Became(_)),
        "the create returns the new instance's attributes"
    );
}

/// The provider updates a running instance by stopping it, patching its
/// resources with `updateMask` in the body (not the query string), starting
/// it again, and growing the boot disk with its own patch that also carries
/// `updateMask` in the body.
///
/// The test pins:
/// - PATCH URL has no `?updateMask=...` query string
/// - `{"updateMask":"resourcesSpec"}` travels in the PATCH instance body
/// - `{"updateMask":"size"}` travels in the PATCH disk body
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "a recorded test: the lines are the recorded exchanges"
)]
fn update_instance_pins_stop_patch_start_and_disk_grow() {
    let instance = address("yandex:instance.nerpa-rc-instance");

    let patch_instance_body =
        r#"{"resourcesSpec":{"coreFraction":"100","cores":"2","memory":"4294967296"},"updateMask":"resourcesSpec"}"#
            .to_owned();
    let patch_disk_body = r#"{"size":"11811160064","updateMask":"size"}"#;

    // Exchange sequence for act_instance(Update) with resources + disk change
    // on a RUNNING instance:
    //  1. list_instances              — find the running instance
    //  2. GET disk                    — boot_disk_of (compare sizes)
    //  3. POST :stop                  — start the stop operation
    //  4. GET op/stop (pending)
    //  5. GET op/stop (done)
    //  6. PATCH instances/{id}        — update resources; mask in body
    //  7. GET op/patch-instance (pending)
    //  8. GET op/patch-instance (done)
    //  9. POST :start
    // 10. GET op/start (pending)
    // 11. GET op/start (done)
    // 12. PATCH disks/{id}            — grow disk; mask in body
    // 13. GET op/patch-disk (pending)
    // 14. GET op/patch-disk (done)
    // 15. list_instances              — read_instance
    // 16. GET disk                    — boot_disk_of (read grown disk)
    let exchanges = vec![
        Exchange {
            method: "GET",
            url: format!("{COMPUTE}/instances?folderId={FOLDER}"),
            body: None,
            response: list_running_2gb(),
        },
        Exchange {
            method: "GET",
            url: format!("{COMPUTE}/disks/{DISK_ID}"),
            body: None,
            response: disk_10gib(),
        },
        Exchange {
            method: "POST",
            url: format!("{COMPUTE}/instances/{INSTANCE_ID}:stop"),
            body: Some("{}".to_owned()),
            response: Response {
                status: 200,
                body: format!(r#"{{"id":"{OP_STOP}","description":"Stop instance","done":false}}"#),
            },
        },
        Exchange {
            method: "GET",
            url: format!("{OPERATIONS}/{OP_STOP}"),
            body: None,
            response: op_pending(OP_STOP),
        },
        Exchange {
            method: "GET",
            url: format!("{OPERATIONS}/{OP_STOP}"),
            body: None,
            response: op_done(OP_STOP),
        },
        // PATCH instance: URL has no query string; updateMask is in the body.
        Exchange {
            method: "PATCH",
            url: format!("{COMPUTE}/instances/{INSTANCE_ID}"),
            body: Some(patch_instance_body),
            response: Response {
                status: 200,
                body: format!(
                    r#"{{"id":"{OP_PATCH_INSTANCE}","description":"Update instance","done":false}}"#
                ),
            },
        },
        Exchange {
            method: "GET",
            url: format!("{OPERATIONS}/{OP_PATCH_INSTANCE}"),
            body: None,
            response: op_pending(OP_PATCH_INSTANCE),
        },
        Exchange {
            method: "GET",
            url: format!("{OPERATIONS}/{OP_PATCH_INSTANCE}"),
            body: None,
            response: op_done(OP_PATCH_INSTANCE),
        },
        Exchange {
            method: "POST",
            url: format!("{COMPUTE}/instances/{INSTANCE_ID}:start"),
            body: Some("{}".to_owned()),
            response: Response {
                status: 200,
                body: format!(
                    r#"{{"id":"{OP_START}","description":"Start instance","done":false}}"#
                ),
            },
        },
        Exchange {
            method: "GET",
            url: format!("{OPERATIONS}/{OP_START}"),
            body: None,
            response: op_pending(OP_START),
        },
        Exchange {
            method: "GET",
            url: format!("{OPERATIONS}/{OP_START}"),
            body: None,
            response: op_done(OP_START),
        },
        // PATCH disk: URL has no query string; updateMask is in the body.
        Exchange {
            method: "PATCH",
            url: format!("{COMPUTE}/disks/{DISK_ID}"),
            body: Some(patch_disk_body.to_owned()),
            response: Response {
                status: 200,
                body: format!(
                    r#"{{"id":"{OP_PATCH_DISK}","description":"Update disk","done":false}}"#
                ),
            },
        },
        Exchange {
            method: "GET",
            url: format!("{OPERATIONS}/{OP_PATCH_DISK}"),
            body: None,
            response: op_pending(OP_PATCH_DISK),
        },
        Exchange {
            method: "GET",
            url: format!("{OPERATIONS}/{OP_PATCH_DISK}"),
            body: None,
            response: op_done(OP_PATCH_DISK),
        },
        // read_instance after the update
        Exchange {
            method: "GET",
            url: format!("{COMPUTE}/instances?folderId={FOLDER}"),
            body: None,
            response: list_running_4gb(),
        },
        Exchange {
            method: "GET",
            url: format!("{COMPUTE}/disks/{DISK_ID}"),
            body: None,
            response: disk_11gib(),
        },
    ];

    let provider = provider_over(exchanges);
    let desired = attrs(&[
        (
            "zone_id",
            nerpa_core::Value::Text("ru-central1-e".to_owned()),
        ),
        (
            "platform_id",
            nerpa_core::Value::Text("standard-v3".to_owned()),
        ),
        ("cores", nerpa_core::Value::Integer(2)),
        ("memory_gb", nerpa_core::Value::Integer(4)),
        ("subnet_id", nerpa_core::Value::Text(SUBNET_ID.to_owned())),
        (
            "boot_disk_image_id",
            nerpa_core::Value::Text(IMAGE_ID.to_owned()),
        ),
        ("boot_disk_gb", nerpa_core::Value::Integer(11)),
    ]);
    let secret: BTreeSet<String> = BTreeSet::new();
    let prepared = Prepared {
        address: &instance,
        kind: Sending::Update,
        roused: false,
        resent: false,
        desired: &desired,
        secret: &secret,
        recorded: None,
        facts: facts(),
    };
    let concluded = provider.act(&prepared).expect("the update succeeds");
    assert!(
        matches!(concluded, nerpa_core::provider::Concluded::Became(_)),
        "the update returns the updated instance's attributes"
    );
}
