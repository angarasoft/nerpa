//! A Terraform state reads into nerpa: the resources it knows map to addresses
//! and attributes, and every secret held in clear text is named.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use nerpa_core::Value;
use nerpa_core::secret::Recorded;
use nerpa_import::{Import, TfState};

/// A small state in the nested format Terraform writes: a data source subnet,
/// a network, a subnet, an instance, and a database user whose password sits in
/// the state in clear text.
fn state() -> TfState {
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
                    "provider": "provider[\"registry.terraform.io/yandex-cloud/yandex\"]",
                    "instances": [
                        {
                            "schema_version": 0,
                            "attributes": {
                                "name": "bosshub-benchmark-e-subnet",
                                "network_id": "enp",
                                "zone": "ru-central1-e",
                                "v4_cidr_blocks": ["10.0.61.0/24"]
                            }
                        }
                    ]
                },
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
                                "v4_cidr_blocks": ["10.0.0.0/24"]
                            }
                        }
                    ]
                },
                {
                    "mode": "managed",
                    "type": "yandex_compute_instance",
                    "name": "this",
                    "provider": "provider[\"registry.terraform.io/yandex-cloud/yandex\"]",
                    "instances": [
                        {
                            "schema_version": 0,
                            "attributes": {
                                "id": "id-db",
                                "name": "db",
                                "zone": "ru-central1-b",
                                "platform_id": "standard-v3",
                                "resources": [{ "cores": 2, "memory": 4 }],
                                "network_interface": [{ "subnet_id": "id-subnet" }],
                                "boot_disk": [{ "initialize_params": [{ "image_id": "img-db" }] }]
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
    assert_eq!(
        instance.attributes.get("subnet_id"),
        Some(&Value::Text("id-subnet".to_owned()))
    );
    assert_eq!(
        instance.attributes.get("boot_disk_image_id"),
        Some(&Value::Text("img-db".to_owned()))
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

#[test]
fn the_state_records_what_was_imported() {
    let import = Import::read(&state()).expect("reads");
    let state = import.state().expect("a state");

    let instance: nerpa_core::Address = "yandex:instance.db".parse().unwrap();
    let record = state.get(&instance).expect("the instance is recorded");
    assert_eq!(
        record.attributes().get("zone_id"),
        Some(&Recorded::Plain(Value::Text("ru-central1-b".to_owned())))
    );
    assert_eq!(
        record.attributes().get("cores"),
        Some(&Recorded::Plain(Value::Integer(2)))
    );

    let network: nerpa_core::Address = "yandex:network.main".parse().unwrap();
    assert_eq!(
        state.get(&network).expect("the network").attributes().len(),
        0,
        "a network records no attributes"
    );

    // No secret among the recorded values: the import names them, it does not
    // carry them into state.
    for address in state.addresses() {
        for recorded in state.get(address).expect("recorded").attributes().values() {
            assert!(!recorded.is_secret(), "{address}: a secret reached state");
        }
    }
}
