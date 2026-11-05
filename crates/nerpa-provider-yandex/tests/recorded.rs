//! The provider's wire format is pinned against recorded exchanges: a request
//! is asked exactly as recorded, and the answer is what the real API answered.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeMap;

use nerpa_core::address::address;
use nerpa_core::observed::Observed;
use nerpa_core::provider::{Machines, Provider, Read, Reading};
use nerpa_core::value::Value;
use nerpa_provider_yandex::{Auth, Exchange, Recorded, Response, Yandex};

/// A provider that replays one recorded exchange.
fn provider_with(exchange: Exchange) -> Yandex {
    Yandex::new(
        "folder",
        "https://compute.api.cloud.yandex.net/compute/v1",
        "https://vpc.api.cloud.yandex.net/vpc/v1",
        "https://storage.api.cloud.yandex.net/storage/v1",
        Auth::IamToken("token".to_owned()),
        Box::new(Recorded::new(vec![exchange])),
    )
}

/// Reads one address through the provider.
fn read(provider: &Yandex, address: nerpa_core::Address) -> Read {
    let observed = Observed::new();
    let facts = observed.facts_of(None);
    let machines = Machines::new();
    let reach = machines
        .reach(None)
        .expect("a control resource has nowhere");
    provider
        .read(&[Reading::new(address, BTreeMap::new())], facts, reach)
        .expect("read")
}

#[test]
fn an_instance_reads_a_recorded_list() {
    let instance = address("yandex:instance.db");
    let provider = provider_with(Exchange {
        method: "GET",
        url: "https://compute.api.cloud.yandex.net/compute/v1/instances?folderId=folder".to_owned(),
        body: None,
        response: Response {
            status: 200,
            body: r#"{"instances":[{"id":"id-db","name":"db","zoneId":"ru-central1-b","platformId":"standard-v3","resources":{"cores":"2","memory":"2147483648"},"networkInterfaces":[{"subnetId":"id-subnet"}]}]}"#.to_owned(),
        },
    });

    let found = read(&provider, instance.clone());
    let attributes = found
        .attributes()
        .get(&instance)
        .expect("the instance was found");
    assert_eq!(
        attributes.get("zone_id"),
        Some(&Value::Text("ru-central1-b".to_owned()))
    );
    assert_eq!(
        attributes.get("platform_id"),
        Some(&Value::Text("standard-v3".to_owned()))
    );
    assert_eq!(attributes.get("cores"), Some(&Value::Integer(2)));
    assert_eq!(attributes.get("memory_gb"), Some(&Value::Integer(2)));
    assert_eq!(
        attributes.get("subnet_id"),
        Some(&Value::Text("id-subnet".to_owned()))
    );
}

#[test]
fn a_network_reads_a_recorded_list() {
    let network = address("yandex:network.main");
    let provider = provider_with(Exchange {
        method: "GET",
        url: "https://vpc.api.cloud.yandex.net/vpc/v1/networks?folderId=folder".to_owned(),
        body: None,
        response: Response {
            status: 200,
            body: r#"{"networks":[{"id":"id-main","name":"main"}]}"#.to_owned(),
        },
    });

    let found = read(&provider, network.clone());
    assert!(
        found.attributes().contains_key(&network),
        "the network exists, with no state beyond its name"
    );
}

#[test]
fn a_subnet_reads_a_recorded_list() {
    let subnet = address("yandex:subnet.db");
    let provider = provider_with(Exchange {
        method: "GET",
        url: "https://vpc.api.cloud.yandex.net/vpc/v1/subnets?folderId=folder".to_owned(),
        body: None,
        response: Response {
            status: 200,
            body: r#"{"subnets":[{"id":"id-subnet","name":"db","networkId":"id-main","zoneId":"ru-central1-b","v4CidrBlocks":["10.0.0.0/24"]}]}"#.to_owned(),
        },
    });

    let found = read(&provider, subnet.clone());
    let attributes = found
        .attributes()
        .get(&subnet)
        .expect("the subnet was found");
    assert_eq!(
        attributes.get("network_id"),
        Some(&Value::Text("id-main".to_owned()))
    );
    assert_eq!(
        attributes.get("zone_id"),
        Some(&Value::Text("ru-central1-b".to_owned()))
    );
    assert_eq!(
        attributes.get("v4_cidr_blocks"),
        Some(&Value::List(vec![Value::Text("10.0.0.0/24".to_owned())]))
    );
}

#[test]
fn a_bucket_reads_a_recorded_list() {
    let bucket = address("yandex:bucket.backup");
    let provider = provider_with(Exchange {
        method: "GET",
        url: "https://storage.api.cloud.yandex.net/storage/v1/buckets?folderId=folder".to_owned(),
        body: None,
        response: Response {
            status: 200,
            // Versioning is omitted by the API where it is disabled, the
            // zero value; the read reports it as disabled all the same.
            body: r#"{"buckets":[{"name":"backup","defaultStorageClass":"COLD","maxSize":"1073741824","tags":[{"key":"team","value":"nerpa"}]}]}"#.to_owned(),
        },
    });

    let found = read(&provider, bucket.clone());
    let attributes = found
        .attributes()
        .get(&bucket)
        .expect("the bucket was found");
    assert_eq!(
        attributes.get("default_storage_class"),
        Some(&Value::Text("COLD".to_owned()))
    );
    assert_eq!(
        attributes.get("versioning"),
        Some(&Value::Text("disabled".to_owned()))
    );
    assert_eq!(
        attributes.get("max_size"),
        Some(&Value::Integer(1_073_741_824))
    );
    assert_eq!(
        attributes.get("tags"),
        Some(&Value::Map(BTreeMap::from([(
            "team".to_owned(),
            Value::Text("nerpa".to_owned())
        )])))
    );
}
