//! A Terraform state reads into nerpa: the resources it knows map to addresses
//! and attributes, and every secret held in clear text is named.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use nerpa_core::Value;
use nerpa_import::{Import, TfState};

/// A small state: a network, a subnet, an instance, and a database user whose
/// password sits in the state in clear text.
fn state() -> TfState {
    serde_json::from_str(
        r#"{
            "version": 4,
            "terraform_version": "1.5.0",
            "serial": 1,
            "lineage": "lineage",
            "resources": [
                {
                    "mode": "managed",
                    "type": "yandex_vpc_network",
                    "name": "main",
                    "provider": "provider[\"registry.terraform.io/yandex-cloud/yandex\"]",
                    "instances": [
                        { "schema_version": 0, "attributes": { "id": "id-main", "name": "main" } }
                    ]
                },
                {
                    "mode": "managed",
                    "type": "yandex_vpc_subnet",
                    "name": "db",
                    "provider": "provider[\"registry.terraform.io/yandex-cloud/yandex\"]",
                    "instances": [
                        {
                            "schema_version": 0,
                            "attributes": {
                                "id": "id-subnet",
                                "name": "db",
                                "network_id": "id-main",
                                "zone": "ru-central1-b",
                                "v4_cidr_blocks.#": "1",
                                "v4_cidr_blocks.0": "10.0.0.0/24"
                            }
                        }
                    ]
                },
                {
                    "mode": "managed",
                    "type": "yandex_compute_instance",
                    "name": "db",
                    "provider": "provider[\"registry.terraform.io/yandex-cloud/yandex\"]",
                    "instances": [
                        {
                            "schema_version": 0,
                            "attributes": {
                                "id": "id-db",
                                "name": "db",
                                "zone": "ru-central1-b",
                                "platform_id": "standard-v3",
                                "resources.#": "1",
                                "resources.0.cores": "2",
                                "resources.0.memory": "4",
                                "resources.0.core_fraction": "100"
                            }
                        }
                    ]
                },
                {
                    "mode": "managed",
                    "type": "yandex_mdb_mysql_user",
                    "name": "app",
                    "provider": "provider[\"registry.terraform.io/yandex-cloud/yandex\"]",
                    "instances": [
                        {
                            "schema_version": 0,
                            "attributes": { "name": "app", "password": "hunter2" }
                        }
                    ]
                }
            ]
        }"#,
    )
    .unwrap()
}

#[test]
fn the_known_resources_map_to_addresses() {
    let import = Import::read(&state()).expect("reads");

    let instance = import
        .resources
        .iter()
        .find(|resource| resource.address == "yandex:instance.db")
        .expect("the instance");
    assert_eq!(
        instance.attributes.get("zone_id"),
        Some(&Value::Text("ru-central1-b".to_owned()))
    );
    assert_eq!(instance.attributes.get("cores"), Some(&Value::Integer(2)));
    assert_eq!(
        instance.attributes.get("memory_gb"),
        Some(&Value::Integer(4))
    );

    let subnet = import
        .resources
        .iter()
        .find(|resource| resource.address == "yandex:subnet.db")
        .expect("the subnet");
    assert_eq!(
        subnet.attributes.get("v4_cidr_blocks"),
        Some(&Value::List(vec![Value::Text("10.0.0.0/24".to_owned())]))
    );

    let network = import
        .resources
        .iter()
        .find(|resource| resource.address == "yandex:network.main")
        .expect("the network");
    assert!(
        network.attributes.is_empty(),
        "a network has no state beyond its name"
    );

    // The database user is not a resource nerpa maps, so it is not declared —
    // but its password is still named.
    assert_eq!(import.resources.len(), 3);
}

#[test]
fn every_secret_in_clear_text_is_named() {
    let import = Import::read(&state()).expect("reads");
    assert_eq!(
        import.secrets,
        vec![nerpa_import::Secret {
            resource: "yandex_mdb_mysql_user \"app\"".to_owned(),
            attribute: "password".to_owned(),
        }]
    );
}

#[test]
fn the_configuration_declares_what_was_imported() {
    let import = Import::read(&state()).expect("reads");
    let rendered = import.render();
    assert!(rendered.contains("resource(\"yandex:network.main\")\n"));
    assert!(rendered.contains("resource(\"yandex:instance.db\", {"));
    assert!(rendered.contains("\"zone_id\": \"ru-central1-b\","));
    assert!(rendered.contains("\"v4_cidr_blocks\": [\"10.0.0.0/24\"],"));
}
