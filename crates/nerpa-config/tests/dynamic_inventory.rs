//! A resource that creates a node registers that node, so a resource on it is
//! ordered after it without an explicit `node()` declaration.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use nerpa_config::evaluate_with;
use nerpa_config::fleet::Fleet;
use nerpa_config::template::Held;
use nerpa_core::Provenance;
use nerpa_core::address::address;

#[test]
fn an_instance_registers_the_node_it_creates() {
    let schema = nerpa_core::Schema::new().creates_node(
        nerpa_core::Provider::new("yandex").unwrap(),
        nerpa_core::Kind::new("instance").unwrap(),
    );
    let graph = evaluate_with(
        "main.star",
        r#"
resource("yandex:instance.db", {"zone_id": "ru-central1-b", "platform_id": "standard-v3", "cores": 2, "memory_gb": 4})
resource("os:file.motd@db", {"path": "/etc/motd", "content": "hi\n"})
"#,
        Box::new(Held::new()),
        Fleet::empty(),
        schema,
    )
    .expect("the configuration evaluates");

    let db = nerpa_core::Target::new("db").unwrap();
    assert_eq!(
        graph.inventory().provenance(&db),
        Some(&Provenance::Managed(address("yandex:instance.db")))
    );

    // The resource on the node is ordered after the instance, not refused as an
    // unknown node.
    graph.dependencies().expect("the graph orders");
}
