//! Which machines there are, and what is written down about them.
//!
//! A hundred machines cannot be a hundred declarations, and the reason a group
//! may be iterated where a fact may not is that the fleet is settled before the
//! program runs. See decision 0025.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use nerpa_config::evaluate;
use nerpa_config::fleet::Fleet;
use nerpa_config::template::Held;
use nerpa_core::Attribute;
use nerpa_core::address::address;

const FLEET: &str = r#"
[groups.web]
workers = 4
port = 443

[groups.production]
retention = 30

[nodes.web-01]
groups = ["web", "production"]

[nodes.web-02]
groups = ["web", "production"]
values = { workers = 16 }

[nodes.db-01]
groups = ["production"]
"#;

fn fleet() -> Fleet {
    Fleet::read(FLEET).expect("a fleet")
}

fn graph_of(source: &str) -> nerpa_core::Graph {
    evaluate("main.star", source, Box::new(Held::new()), fleet()).expect("it should evaluate")
}

/// The whole point: one declaration, every machine in the group.
#[test]
fn a_group_is_iterated_and_a_hundred_machines_are_not_a_hundred_declarations() {
    let graph = graph_of(
        r#"
for host in group("web"):
    resource("os:file.conf@%s" % host, {"workers": host["workers"]})
"#,
    );

    assert_eq!(graph.len(), 2, "one resource per machine in the group");
    for (node, workers) in [("web-01", 4), ("web-02", 16)] {
        let found = graph
            .get(&address(&format!("os:file.conf@{node}")))
            .and_then(|resource| resource.attribute("workers"))
            .and_then(Attribute::known)
            .expect("a value");
        assert_eq!(found, &nerpa_core::Value::Integer(workers));
    }
}

/// A node saying what it is is a statement, not a rank in a table of twenty-two
/// levels nobody can remember.
#[test]
fn a_node_may_say_what_its_groups_do_not() {
    let graph = graph_of(
        r#"
for host in group("production"):
    resource("os:file.conf@%s" % host, {"retention": host["retention"]})
"#,
    );
    assert_eq!(graph.len(), 3, "web-01, web-02 and db-01");
}

/// Ansible answers "where did this value come from" with a precedence table.
/// Here the question has one answer per value, and a collision is something
/// somebody decides rather than something a rule decides for them.
#[test]
fn two_groups_giving_one_node_the_same_key_is_an_error_naming_both() {
    let refusal = Fleet::read(
        r#"
[groups.web]
workers = 4

[groups.production]
workers = 8

[nodes.web-01]
groups = ["web", "production"]
"#,
    )
    .expect_err("a precedence rule is not the answer");

    assert!(refusal.contains("web-01"), "{refusal}");
    assert!(refusal.contains("\"web\""), "{refusal}");
    assert!(refusal.contains("\"production\""), "{refusal}");
    assert!(refusal.contains("workers"), "{refusal}");
}

#[test]
fn a_value_the_fleet_does_not_give_is_a_refusal_rather_than_a_default() {
    let refusal = evaluate(
        "main.star",
        r#"
for host in group("web"):
    resource("os:file.conf@%s" % host, {"nothing": host["nowhere"]})
"#,
        Box::new(Held::new()),
        fleet(),
    )
    .expect_err("an absent value is not an empty one");
    assert!(refusal.to_string().contains("nowhere"), "{refusal}");
}

#[test]
fn a_group_nothing_describes_is_a_refusal_rather_than_an_empty_run() {
    let refusal = evaluate(
        "main.star",
        r#"
for host in group("nonesuch"):
    resource("os:file.conf@%s" % host, {})
"#,
        Box::new(Held::new()),
        fleet(),
    )
    .expect_err("a group that is not there is a mistake, not nothing to do");
    assert!(refusal.to_string().contains("nonesuch"), "{refusal}");
}

/// Only what a run touches, because a document describing two hundred machines
/// should not make a run that touches three open two hundred connections.
#[test]
fn only_the_machines_a_run_reaches_for_join_its_inventory() {
    let graph = graph_of(
        r#"
for host in group("web"):
    resource("os:file.conf@%s" % host, {})
"#,
    );
    let known: Vec<&str> = graph
        .inventory()
        .nodes()
        .map(nerpa_core::Target::as_str)
        .collect();
    assert_eq!(known, vec!["web-01", "web-02"], "db-01 was never asked for");
}

/// A fleet is data by the time the program sees it, so it may be compared and
/// branched on — which is exactly what a fact may not do.
#[test]
fn what_the_fleet_says_may_decide_which_resources_exist() {
    let graph = graph_of(
        r#"
for host in group("web"):
    if host["workers"] > 8:
        resource("os:file.big@%s" % host, {})
"#,
    );
    assert_eq!(graph.len(), 1, "only web-02 has sixteen");
    assert!(graph.get(&address("os:file.big@web-02")).is_some());
}

/// How a machine is reached is a property of the node, not of the protocol.
/// Escalation is an argument and appears nowhere on the wire — decision 0035.
#[test]
fn a_node_says_how_it_is_reached_and_what_it_runs_under() {
    use nerpa_config::fleet::Route;

    let fleet = Fleet::read(
        r#"
[groups.test]
kept = 1

[nodes.here]
groups = ["test"]

[nodes.plain]
groups = ["test"]
ssh = "10.0.61.34"

[nodes.privileged]
groups = ["test"]
ssh = "web-01"
executor = "/opt/nerpa/nerpa-executor"
become = "sudo -n"

[nodes.proxied]
groups = ["test"]
ssh = "nerpa@web-02.example.internal"
ssh_config = "/home/somebody/.tbot/nerpa/ssh_config"
"#,
    )
    .expect("a fleet");

    let node = |name: &str| nerpa_core::Target::new(name).expect("a node name");

    assert_eq!(
        fleet.route(&node("here")),
        Route::Here,
        "a machine the document says nothing about is this one"
    );
    assert_eq!(
        fleet.route(&node("plain")),
        Route::Ssh {
            host: "10.0.61.34".to_owned(),
            config: None,
            executor: None,
            escalate: Vec::new(),
        },
        "and one that names no executor gets one put there"
    );
    assert_eq!(
        fleet.route(&node("privileged")),
        Route::Ssh {
            host: "web-01".to_owned(),
            config: None,
            executor: Some("/opt/nerpa/nerpa-executor".to_owned()),
            escalate: vec!["sudo".to_owned(), "-n".to_owned()],
        },
        "escalation is words, because a shell string is where somebody else's \
         quoting becomes somebody else's command"
    );
    assert_eq!(
        fleet.route(&node("proxied")),
        Route::Ssh {
            host: "nerpa@web-02.example.internal".to_owned(),
            config: Some("/home/somebody/.tbot/nerpa/ssh_config".to_owned()),
            executor: None,
            escalate: Vec::new(),
        },
        "a machine behind an identity-aware proxy names the generated \
         configuration and needs nothing else: it is a route, not a transport"
    );
}
