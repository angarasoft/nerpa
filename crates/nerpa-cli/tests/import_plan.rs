//! The Stage 3 exit, proven without a live cloud: a configuration imported from
//! Terraform plans to nothing against a recorded exchange of the same instances.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::path::PathBuf;

use nerpa_core::provider::{Machines, Providers};
use nerpa_core::run::observe;
use nerpa_core::secret::{Key, Remembered, Secrets};
use nerpa_provider_yandex::{Auth, Exchange, Recorded, Response, Yandex};

/// The state of two instances, as Terraform 1.15 writes it: nested lists, a
/// data-source subnet the import skips, and each instance naming its subnet and
/// its boot-disk image.
fn state() -> nerpa_import::TfState {
    serde_json::from_str(
        r#"{
            "version": 4,
            "terraform_version": "1.15.1",
            "serial": 1,
            "lineage": "lineage",
            "resources": [
                {
                    "mode": "data",
                    "type": "yandex_vpc_subnet",
                    "name": "this",
                    "instances": [
                        {
                            "schema_version": 0,
                            "attributes": {
                                "name": "subnet",
                                "network_id": "enp",
                                "zone": "ru-central1-e",
                                "v4_cidr_blocks": ["10.0.61.0/24"]
                            }
                        }
                    ]
                },
                {
                    "mode": "managed",
                    "type": "yandex_compute_instance",
                    "name": "alpine",
                    "instances": [
                        {
                            "schema_version": 0,
                            "attributes": {
                                "name": "nerpa-alpine",
                                "zone": "ru-central1-e",
                                "platform_id": "standard-v3",
                                "resources": [{ "cores": 2, "memory": 2 }],
                                "network_interface": [{ "subnet_id": "ajcjde8o8cphncu85tuq" }],
                                "boot_disk": [{ "initialize_params": [{ "image_id": "fd8vocqhuvdv841pnkhf" }] }]
                            }
                        }
                    ]
                },
                {
                    "mode": "managed",
                    "type": "yandex_compute_instance",
                    "name": "test",
                    "instances": [
                        {
                            "schema_version": 0,
                            "attributes": {
                                "name": "nerpa-test",
                                "zone": "ru-central1-e",
                                "platform_id": "standard-v3",
                                "resources": [{ "cores": 2, "memory": 2 }],
                                "network_interface": [{ "subnet_id": "ajcjde8o8cphncu85tuq" }],
                                "boot_disk": [{ "initialize_params": [{ "image_id": "fd872p5u8u4bm06um4bj" }] }]
                            }
                        }
                    ]
                }
            ]
        }"#,
    )
    .unwrap()
}

static DIGEST_KEY: Key = Key::from_bytes([5; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

fn secrets() -> Secrets<'static> {
    Secrets::new(&DIGEST_KEY, &NO_STORE)
}

/// The instances read back exactly as the import declared them, except the
/// boot-disk image, which a read never carries back (it is a birth attribute).
fn provider() -> Yandex {
    Yandex::new(
        "folder",
        "https://compute.api.cloud.yandex.net/compute/v1",
        "https://vpc.api.cloud.yandex.net/vpc/v1",
        Auth::IamToken("token".to_owned()),
        Box::new(Recorded::new(vec![Exchange {
            method: "GET",
            url: "https://compute.api.cloud.yandex.net/compute/v1/instances?folderId=folder"
                .to_owned(),
            body: None,
            response: Response {
                status: 200,
                body: r#"{"instances":[
                    {"id":"id-alpine","name":"nerpa-alpine","zoneId":"ru-central1-e","platformId":"standard-v3","resources":{"cores":2,"memory":2147483648},"networkInterfaces":[{"subnetId":"ajcjde8o8cphncu85tuq"}]},
                    {"id":"id-test","name":"nerpa-test","zoneId":"ru-central1-e","platformId":"standard-v3","resources":{"cores":2,"memory":2147483648},"networkInterfaces":[{"subnetId":"ajcjde8o8cphncu85tuq"}]}
                ]}"#
                .to_owned(),
            },
        }])),
    )
}

#[test]
fn an_imported_configuration_plans_to_nothing() {
    let import = nerpa_import::Import::read(&state()).expect("reads");
    let rendered = import.render();
    let state = import.state().expect("a state");

    let providers = Providers::new().with(Box::new(provider()));
    let machines = Machines::new();

    let graph = nerpa_config::evaluate_with(
        "main.star",
        &rendered,
        Box::new(nerpa_config::template::Directory::at(PathBuf::from("."))),
        nerpa_config::fleet::Fleet::empty(),
        providers.schema(),
    )
    .expect("evaluates");

    let observed = observe(&graph, &providers, &machines, &state).expect("observed");
    let planned =
        nerpa_core::planner::plan(&graph, &observed, &state, &providers.schema(), secrets())
            .expect("plannable");
    assert_eq!(
        planned.changes().count(),
        0,
        "an imported configuration plans to nothing"
    );
}
