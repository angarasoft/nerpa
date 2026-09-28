//! What a configuration may say, and what it is refused.
//!
//! The refusals are the point. Everything here that fails is a case where a
//! value which does not exist yet was about to decide something.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use nerpa_config::fleet::Fleet;
use nerpa_config::template::Held;
use nerpa_config::{ConfigError, evaluate};
use nerpa_core::Attribute;
use nerpa_core::address::address;

fn graph_of(source: &str) -> nerpa_core::Graph {
    evaluate("main.star", source, Box::new(Held::new()), Fleet::empty())
        .expect("the configuration should evaluate")
}

#[test]
fn a_program_declares_resources() {
    let graph = graph_of(
        r#"
resource("yandex:instance.db", {"cores": 8, "zone": "ru-central1-e"})
resource("yandex:disk.spare", {"size_gb": 100})
"#,
    );

    assert_eq!(graph.len(), 2);
    let db = graph.get(&address("yandex:instance.db")).expect("declared");
    assert_eq!(
        db.attribute("cores").and_then(Attribute::known),
        Some(&nerpa_core::Value::Integer(8))
    );
}

#[test]
fn reading_another_resource_leaves_the_value_unresolved_and_orders_the_two() {
    let graph = graph_of(
        r#"
db = resource("yandex:instance.db")
resource("yandex:instance.web", {"upstream": db.ip})
"#,
    );

    let web = graph
        .get(&address("yandex:instance.web"))
        .expect("declared");
    let waiting = web
        .attribute("upstream")
        .expect("declared")
        .reference()
        .expect("unresolved");
    assert_eq!(waiting.to_string(), "yandex:instance.db.ip");

    let stages = graph.stages().expect("acyclic");
    assert_eq!(stages.len(), 2, "the reader waits for what it reads");
}

/// Starlark answers equality between different types with `False` rather than an
/// error, and decides it without consulting either value. So `db.ip == "x"` is
/// silently false and the branch silently does not happen. Nothing at the value
/// level can intervene; this is caught before evaluation begins.
#[test]
#[dacc_derive::doc_anchor(
    id = "comparing-a-value-that-does-not-exist-yet-is-refused-before-anything-runs"
)]
fn comparing_a_value_that_does_not_exist_yet_is_refused_before_anything_runs() {
    for body in [
        r#"x = db.ip == "10.0.0.1""#,
        r#"x = "10.0.0.1" == db.ip"#,
        r#"x = db.ip != "10.0.0.1""#,
        r#"x = db.ip in ["10.0.0.1"]"#,
        r#"x = db.ip not in ["10.0.0.1"]"#,
    ] {
        let source = format!("db = resource(\"yandex:instance.db\")\n{body}\n");
        let refusal = evaluate("main.star", &source, Box::new(Held::new()), Fleet::empty())
            .expect_err("this comparison should have been refused");

        let message = refusal.to_string();
        assert!(message.contains("db.ip"), "{message}");
        assert!(message.contains("will not exist until apply"), "{message}");
        assert!(
            message.contains("main.star:2"),
            "the refusal names the place: {message}"
        );
    }
}

/// The rule is coarse on purpose, but not indiscriminate: a method call and an
/// item lookup are not bare attribute access, and neither is touched.
#[test]
fn ordinary_comparisons_are_left_alone() {
    let graph = graph_of(
        r#"
settings = {"zone": "ru-central1-e"}
if "ru-central1-e".upper() == "RU-CENTRAL1-E" and settings["zone"] == "ru-central1-e":
    resource("yandex:instance.db")
"#,
    );

    assert_eq!(graph.len(), 1);
}

#[test]
#[dacc_derive::doc_anchor(id = "iterating-a-value-that-does-not-exist-yet-is-refused")]
fn iterating_a_value_that_does_not_exist_yet_is_refused() {
    let refusal = evaluate(
        "main.star",
        r#"
db = resource("yandex:instance.db")
for disk in db.disks:
    resource("yandex:disk." + disk)
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("iteration over an unresolved value must fail");

    assert!(refusal.to_string().contains("iteration"), "{refusal}");
}

#[test]
#[dacc_derive::doc_anchor(id = "indexing-a-value-that-does-not-exist-yet-is-refused")]
fn indexing_a_value_that_does_not_exist_yet_is_refused() {
    let refusal = evaluate(
        "main.star",
        r#"
db = resource("yandex:instance.db")
x = db.addresses[0]
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("indexing an unresolved value must fail");

    assert!(refusal.to_string().contains("indexing"), "{refusal}");
}

/// The one refusal the toolchain will not let us make where it happens. The run
/// is refused afterwards instead, so the invariant holds and only the diagnostic
/// is weaker.
#[test]
#[dacc_derive::doc_anchor(
    id = "branching-on-a-value-that-does-not-exist-yet-refuses-the-whole-run"
)]
fn branching_on_a_value_that_does_not_exist_yet_refuses_the_whole_run() {
    let refusal = evaluate(
        "main.star",
        r#"
db = resource("yandex:instance.db")
if db.ready:
    resource("yandex:instance.web")
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("branching on an unresolved value must not produce a graph");

    let ConfigError::BranchedOnUnresolved(named) = refusal else {
        panic!("expected the branch to be reported, got {refusal}");
    };
    assert_eq!(named, "yandex:instance.db.ready");
}

#[test]
fn everything_on_a_node_waits_for_whatever_the_inventory_says_creates_it() {
    let graph = graph_of(
        r#"
db = resource("yandex:instance.db")
node("db-01", provided_by = db)
resource("os:package.postgresql@db-01")
"#,
    );

    let stages = graph.stages().expect("acyclic");
    assert_eq!(stages.len(), 2);
    assert_eq!(stages[0].addresses(), [address("yandex:instance.db")]);
    assert_eq!(
        stages[1].addresses(),
        [address("os:package.postgresql@db-01")]
    );
}

#[test]
fn a_machine_that_already_exists_is_declared_without_a_provider() {
    let graph = graph_of(
        r#"
node("web-01")
resource("os:package.nginx@web-01", {"state": "present"})
"#,
    );

    assert_eq!(graph.stages().expect("acyclic").len(), 1);
}

#[test]
fn a_resource_on_a_node_the_configuration_never_mentioned_is_refused() {
    let graph = graph_of(r#"resource("os:package.nginx@web-01")"#);

    assert!(matches!(
        graph.stages(),
        Err(nerpa_core::GraphError::UnknownNode { .. })
    ));
}

#[test]
fn an_address_the_model_refuses_is_reported_rather_than_guessed_at() {
    let refusal = evaluate(
        "main.star",
        r#"resource("not-an-address")"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("a malformed address must fail");

    assert!(refusal.to_string().contains("provider"), "{refusal}");
}

#[test]
fn an_attribute_holding_something_a_resource_cannot_carry_is_reported() {
    let refusal = evaluate(
        "main.star",
        r#"resource("yandex:instance.db", {"f": len})"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("a function is not a value a resource can carry");

    assert!(
        refusal
            .to_string()
            .contains("not a value a resource can carry"),
        "{refusal}"
    );
}

/// Starlark is deliberately not Turing complete: iteration is over finite
/// collections and there is no `while`. A configuration therefore always
/// terminates, which is what lets a plan be computed before anything is touched.
#[test]
fn ordinary_starlark_still_works() {
    let graph = graph_of(
        r#"
def web_server(index):
    return resource("yandex:instance.web:%d" % index, {"cores": 2})

for i in range(3):
    web_server(i)
"#,
    );

    assert_eq!(graph.len(), 3);
    assert!(graph.get(&address("yandex:instance.web:2")).is_some());
}

/// Two mechanisms, and it takes both. Starlark folds `x == <literal>` into a
/// type check that never consults the value, so the direct form is caught before
/// evaluation. Behind a function parameter the literal is no longer constant, the
/// general path runs, and the handle refuses it with a traceback.
#[test]
fn a_comparison_hidden_behind_a_function_is_refused_by_the_value_itself() {
    for arguments in ["db.ip, db.port", r#"db.ip, "10.0.0.1""#] {
        let source = format!(
            "def same(a, b):\n    return a == b\n\n\
             db = resource(\"yandex:instance.db\")\n\
             if same({arguments}):\n    resource(\"yandex:instance.web\")\n"
        );

        let refusal = evaluate("main.star", &source, Box::new(Held::new()), Fleet::empty())
            .expect_err("a comparison the syntax pass cannot see must still be refused");
        let message = refusal.to_string();
        assert!(
            message.contains("cannot be used with =="),
            "{arguments}: {message}"
        );
        assert!(
            message.contains("main.star") && message.contains("a == b"),
            "the traceback should point at the comparison: {message}"
        );
    }
}

#[test]
fn the_refusal_names_the_operator_that_was_used() {
    // Matched with the surrounding words on purpose: `not in` contains ` in `,
    // so a looser assertion passes even when the two are confused.
    for (body, operator) in [
        (r#"x = db.ip == "a""#, "with == is"),
        (r#"x = db.ip != "a""#, "with != is"),
        (r#"x = db.ip in ["a"]"#, "with in is"),
        (r#"x = db.ip not in ["a"]"#, "with not in is"),
    ] {
        let source = format!("db = resource(\"yandex:instance.db\")\n{body}\n");
        let refusal = evaluate("main.star", &source, Box::new(Held::new()), Fleet::empty())
            .expect_err("refused");
        assert!(
            refusal.to_string().contains(operator),
            "{operator}: {refusal}"
        );
    }
}

#[test]
fn a_nested_attribute_is_recognised_too() {
    let refusal = evaluate(
        "main.star",
        r#"
db = resource("yandex:instance.db")
x = db.network.address == "10.0.0.1"
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("a nested attribute access is still an attribute access");

    // Matched on the variant, not on the text. A runtime failure echoes the
    // source line back, so any assertion on message content would pass for the
    // wrong reason — which is what mutation testing caught here.
    let ConfigError::ComparedUnresolved(message) = refusal else {
        panic!("the syntax pass should have refused this before evaluation");
    };
    assert!(message.contains("db.network.address"), "{message}");
}

#[test]
fn asking_whether_a_resource_has_an_attribute_is_always_yes() {
    let graph = graph_of(
        r#"
db = resource("yandex:instance.db")
if hasattr(db, "anything_at_all"):
    resource("yandex:instance.web")
"#,
    );

    assert_eq!(
        graph.len(),
        2,
        "what a resource exposes is the provider's business, not ours"
    );
}

/// State left behind by one evaluation must not decide the next one.
#[test]
fn a_refused_run_does_not_poison_the_next_one() {
    let poisoned = r#"
db = resource("yandex:instance.db")
if db.ready:
    resource("yandex:instance.web")
"#;
    assert!(evaluate("main.star", poisoned, Box::new(Held::new()), Fleet::empty()).is_err());

    let clean = graph_of(r#"resource("yandex:instance.db")"#);
    assert_eq!(
        clean.len(),
        1,
        "the previous run's branch must not carry over"
    );
}

/// A secret is named, not read: the configuration says where it lives and the
/// value never enters evaluation.
#[test]
fn a_secret_becomes_an_attribute_that_says_where_to_look() {
    let graph = graph_of(
        r#"
resource("os:file.credentials@web-01", {
    "path": "/etc/app/credentials",
    "content": secret("lockbox", "prod/db/password"),
})
"#,
    );

    let resource = graph
        .get(&address("os:file.credentials@web-01"))
        .expect("declared");
    let content = resource.attribute("content").expect("an attribute");
    let source = content.secret().expect("a secret");
    assert_eq!(source.to_string(), "lockbox:prod/db/password");
    assert!(content.known().is_none(), "there is no value to read here");
}

#[test]
fn a_secret_can_be_pinned_to_a_version() {
    let graph = graph_of(
        r#"
resource("os:file.credentials@web-01", {
    "content": secret("lockbox", "prod/db/password", version = "v3"),
})
"#,
    );

    let resource = graph
        .get(&address("os:file.credentials@web-01"))
        .expect("declared");
    let source = resource
        .attribute("content")
        .and_then(Attribute::secret)
        .expect("a secret");
    assert_eq!(source.to_string(), "lockbox:prod/db/password@v3");
}

/// Comparing a secret is not a mistake about ordering. It is an oracle: a
/// configuration that can ask "is the password admin?" can ask it in a loop.
#[test]
#[dacc_derive::doc_anchor(id = "a-secret-refuses-to-be-compared")]
fn a_secret_refuses_to_be_compared() {
    let refusal = evaluate(
        "main.star",
        r#"
if secret("lockbox", "prod/db/password") == "admin":
    resource("os:file.a@web-01", {})
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("comparing a secret is not a configuration")
    .to_string();
    assert!(
        refusal.contains("is a secret") && refusal.contains("main.star"),
        "the refusal should name the secret and the line: {refusal}"
    );
}

/// The same through a name, because that is how anybody would actually write it.
#[test]
#[dacc_derive::doc_anchor(id = "a-secret-held-in-a-variable-refuses-to-be-compared-too")]
fn a_secret_held_in_a_variable_refuses_to_be_compared_too() {
    let refusal = evaluate(
        "main.star",
        r#"
password = secret("lockbox", "prod/db/password")
if password == "admin":
    resource("os:file.a@web-01", {})
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("comparing a secret is not a configuration")
    .to_string();
    assert!(
        refusal.contains("password is a secret"),
        "the refusal should name the variable: {refusal}"
    );
}

#[test]
#[dacc_derive::doc_anchor(id = "a-secret-refuses-to-be-a-condition")]
fn a_secret_refuses_to_be_a_condition() {
    let refusal = evaluate(
        "main.star",
        r#"
if secret("lockbox", "prod/db/password"):
    resource("os:file.a@web-01", {})
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("a secret is not a condition")
    .to_string();
    assert!(
        refusal.contains("secret from lockbox:prod/db/password"),
        "the refusal should name the secret: {refusal}"
    );
}

/// The refusals the syntax pass cannot reach.
///
/// The pass follows opacity through names, containers and function returns, so
/// almost every route is closed before evaluation. A function *parameter* is
/// not followed — that would need the call sites of every function — and this is
/// the route through it. The value itself has to refuse, and does.
#[test]
#[dacc_derive::doc_anchor(id = "a-secret-passed-into-a-function-still-refuses-to-be-compared")]
fn a_secret_passed_into_a_function_still_refuses_to_be_compared() {
    let refusal = evaluate(
        "main.star",
        r#"
def same(left, right):
    return left == right

if same(secret("lockbox", "prod/db/password"), secret("lockbox", "other")):
    resource("os:file.a@web-01", {})
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("comparing two secrets is not a configuration")
    .to_string();
    assert!(
        refusal.contains("secret from lockbox:prod/db/password") && refusal.contains("=="),
        "the value itself should refuse, naming the operation: {refusal}"
    );
}

#[test]
fn a_secret_refuses_to_be_indexed() {
    let refusal = evaluate(
        "main.star",
        r#"resource("os:file.a@web-01", {"content": secret("lockbox", "db")[0]})"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("a secret has no first character to take")
    .to_string();
    assert!(
        refusal.contains("indexing"),
        "the refusal should name the operation: {refusal}"
    );
}

#[test]
fn a_secret_refuses_to_be_iterated() {
    let refusal = evaluate(
        "main.star",
        r#"
for part in secret("lockbox", "db"):
    resource("os:file.a@web-01", {"content": part})
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("a secret is not a sequence")
    .to_string();
    assert!(
        refusal.contains("iteration"),
        "the refusal should name the operation: {refusal}"
    );
}

#[test]
fn a_secret_refuses_to_be_ordered() {
    let refusal = evaluate(
        "main.star",
        r#"
def held():
    return secret("lockbox", "db")

if held() < held():
    resource("os:file.a@web-01", {})
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("secrets have no order")
    .to_string();
    assert!(
        refusal.contains("ordering comparison"),
        "the refusal should name the operation: {refusal}"
    );
}

// Every test below was a way past this check. Each is written the way somebody
// evading it would write it, not the way somebody demonstrating it would.

/// The one an independent review found first: put the handle in a dictionary and
/// read it back, and the pattern that matched `x.y` sees an index instead.
#[test]
#[dacc_derive::doc_anchor(
    id = "a-value-that-does-not-exist-yet-cannot-be-laundered-through-a-dictionary"
)]
fn a_value_that_does_not_exist_yet_cannot_be_laundered_through_a_dictionary() {
    let refusal = evaluate(
        "main.star",
        r#"
db = resource("yandex:instance.db")
held = {"address": db.ip}
if held["address"] == "10.0.0.1":
    resource("yandex:instance.yes", {})
else:
    resource("yandex:instance.no", {})
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("a dictionary is not a way around it");
    let ConfigError::ComparedUnresolved(message) = refusal else {
        panic!("the comparison should be refused before evaluation");
    };
    assert!(message.contains("held[...]"), "{message}");
}

#[test]
#[dacc_derive::doc_anchor(
    id = "a-value-that-does-not-exist-yet-cannot-be-laundered-through-a-list"
)]
fn a_value_that_does_not_exist_yet_cannot_be_laundered_through_a_list() {
    let refusal = evaluate(
        "main.star",
        r#"
db = resource("yandex:instance.db")
addresses = [db.ip]
for address in addresses:
    if address == "10.0.0.1":
        resource("yandex:instance.yes", {})
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("iterating a list of them is not a way around it either");
    assert!(
        matches!(refusal, ConfigError::ComparedUnresolved(_)),
        "{refusal}"
    );
}

#[test]
#[dacc_derive::doc_anchor(id = "a-secret-cannot-be-laundered-through-a-dictionary")]
fn a_secret_cannot_be_laundered_through_a_dictionary() {
    let refusal = evaluate(
        "main.star",
        r#"
held = {"password": secret("vault", "db/password")}
if held["password"] == "changeme":
    resource("os:file.weak@web-01", {"path": "/tmp/weak"})
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("a dictionary is not a way around it");
    let ConfigError::ComparedUnresolved(message) = refusal else {
        panic!("the comparison should be refused before evaluation");
    };
    assert!(message.contains("is a secret"), "{message}");
}

#[test]
#[dacc_derive::doc_anchor(
    id = "a-value-that-does-not-exist-yet-cannot-be-handed-out-of-a-function"
)]
fn a_value_that_does_not_exist_yet_cannot_be_handed_out_of_a_function() {
    let refusal = evaluate(
        "main.star",
        r#"
db = resource("yandex:instance.db")

def address():
    return db.ip

if address() == "10.0.0.1":
    resource("yandex:instance.yes", {})
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("a function is not a way around it");
    let ConfigError::ComparedUnresolved(message) = refusal else {
        panic!("the comparison should be refused before evaluation");
    };
    assert!(message.contains("address(...)"), "{message}");
}

/// Containment reads as `False` for the same reason equality does.
#[test]
#[dacc_derive::doc_anchor(id = "a-value-that-does-not-exist-yet-cannot-be-tested-for-membership")]
fn a_value_that_does_not_exist_yet_cannot_be_tested_for_membership() {
    let refusal = evaluate(
        "main.star",
        r#"
db = resource("yandex:instance.db")
if db.ip in ["10.0.0.1", "10.0.0.2"]:
    resource("yandex:instance.yes", {})
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("membership is a comparison");
    assert!(
        matches!(refusal, ConfigError::ComparedUnresolved(_)),
        "{refusal}"
    );
}

/// A comprehension is a loop with the same hole in it.
#[test]
#[dacc_derive::doc_anchor(id = "a-comprehension-over-a-deferred-list-is-refused")]
fn a_comprehension_over_a_deferred_list_is_refused() {
    let refusal = evaluate(
        "main.star",
        r#"
db = resource("yandex:instance.db")
addresses = [db.ip]
matching = [a for a in addresses if a == "10.0.0.1"]
resource("yandex:instance.web", {"count": len(matching)})
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("a comprehension is a loop");
    assert!(
        matches!(refusal, ConfigError::ComparedUnresolved(_)),
        "{refusal}"
    );
}

/// The check must still let an ordinary configuration through, or it has traded
/// one silent wrongness for a tool nobody can use.
#[test]
#[dacc_derive::doc_anchor(id = "comparing-ordinary-values-is-still-allowed")]
fn comparing_ordinary_values_is_still_allowed() {
    let graph = graph_of(
        r#"
environment = "production"
sizes = {"production": 8, "staging": 2}
if environment == "production":
    resource("yandex:instance.db", {"cores": sizes[environment]})
if "prod" in environment:
    resource("yandex:instance.web", {})
"#,
    );
    assert_eq!(graph.len(), 2, "nothing here is deferred or secret");
}

/// A path that differs by distribution is a *value*, and every branch is written
/// down — so the graph is the same whichever way it goes. That is the whole of
/// what decision 0029 allows a fact to do.
#[test]
#[dacc_derive::doc_anchor(id = "a-value-may-be-chosen-by-what-a-machine-turns-out-to-be")]
fn a_value_may_be_chosen_by_what_a_machine_turns_out_to_be() {
    let graph = graph_of(
        r#"
web = node("web-01")
resource("os:file.conf@web-01", {
    "path": select(web.os_family, {
        "debian": "/etc/nginx/sites-enabled/default",
        "gentoo": "/etc/nginx/nginx.conf",
    }),
})
"#,
    );

    let choice = graph
        .get(&address("os:file.conf@web-01"))
        .and_then(|resource| resource.attribute("path"))
        .and_then(Attribute::chosen)
        .expect("a choice");

    assert_eq!(choice.fact().name(), "os_family");
    assert_eq!(choice.fact().node().as_str(), "web-01");
    assert_eq!(choice.options().len(), 2);
    assert!(choice.given("gentoo").is_some());
    assert!(choice.given("alpine").is_none());
}

/// A fact may decide what an attribute says and never whether a resource exists.
/// Every route to the second is refused, and this is the one the language would
/// otherwise make easiest.
#[test]
#[dacc_derive::doc_anchor(id = "a-fact-cannot-decide-whether-a-resource-exists")]
fn a_fact_cannot_decide_whether_a_resource_exists() {
    let refusal = evaluate(
        "main.star",
        r#"
web = node("web-01")
if web.os_family == "debian":
    resource("os:file.a@web-01", {})
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("a fact is not a condition");
    assert!(
        matches!(refusal, ConfigError::ComparedUnresolved(_)),
        "{refusal}"
    );
}

#[test]
#[dacc_derive::doc_anchor(id = "a-fact-refuses-to-be-a-condition-on-its-own")]
fn a_fact_refuses_to_be_a_condition_on_its_own() {
    let refusal = evaluate(
        "main.star",
        r#"
web = node("web-01")

def held():
    return web.os_family

if held():
    resource("os:file.a@web-01", {})
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("a fact is not a condition");
    assert!(
        matches!(refusal, ConfigError::BranchedOnUnresolved(_)),
        "{refusal}"
    );
}

#[test]
fn select_needs_a_fact_and_something_to_choose_between() {
    for (source, expected) in [
        (
            r#"resource("os:file.a@web-01", {"path": select("debian", {"a": "b"})})"#,
            "select() chooses by something read from a machine",
        ),
        (
            r#"
web = node("web-01")
resource("os:file.a@web-01", {"path": select(web.os_family, {})})
"#,
            "nothing to choose between",
        ),
        (
            r#"
web = node("web-01")
resource("os:file.a@web-01", {"path": select(web.os_family, "debian")})
"#,
            "needs a dictionary",
        ),
    ] {
        let refusal = evaluate("main.star", source, Box::new(Held::new()), Fleet::empty())
            .expect_err("this is not a choice")
            .to_string();
        assert!(refusal.contains(expected), "{refusal}");
    }
}

/// The second independent review found this one: the same dictionary, read back
/// with `.get` instead of `[...]`. The analysis followed the index and stopped
/// at the method call, and a secret compared against a guess answered silently.
/// That is an oracle — the resource appears if and only if the guess is right.
#[test]
#[dacc_derive::doc_anchor(id = "a-secret-cannot-be-laundered-through-a-method-call")]
fn a_secret_cannot_be_laundered_through_a_method_call() {
    let refusal = evaluate(
        "main.star",
        r#"
held = {"password": secret("vault", "db/password")}
if held.get("password") == "changeme":
    resource("os:file.weak@web-01", {"path": "/tmp/weak"})
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("reading it back by method is not a way around it");
    let ConfigError::ComparedUnresolved(message) = refusal else {
        panic!("the comparison should be refused before evaluation");
    };
    assert!(message.contains("is a secret"), "{message}");
}

#[test]
#[dacc_derive::doc_anchor(
    id = "a-value-that-does-not-exist-yet-cannot-be-laundered-through-a-method-call"
)]
fn a_value_that_does_not_exist_yet_cannot_be_laundered_through_a_method_call() {
    let refusal = evaluate(
        "main.star",
        r#"
db = resource("yandex:instance.db")
held = {"address": db.ip}
if held.get("address") == "10.0.0.1":
    resource("yandex:instance.yes", {})
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("nor for a value that does not exist yet");
    assert!(
        matches!(refusal, ConfigError::ComparedUnresolved(_)),
        "{refusal}"
    );
}

/// A method that hands back something it was given rather than something it
/// held: the receiver is clean and the argument is not.
#[test]
#[dacc_derive::doc_anchor(id = "a-secret-cannot-be-laundered-through-a-method-it-was-handed-to")]
fn a_secret_cannot_be_laundered_through_a_method_it_was_handed_to() {
    let refusal = evaluate(
        "main.star",
        r#"
empty = {}
if empty.get("password", secret("vault", "db/password")) == "changeme":
    resource("os:file.weak@web-01", {"path": "/tmp/weak"})
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("handing it in is not a way around it either");
    let ConfigError::ComparedUnresolved(message) = refusal else {
        panic!("the comparison should be refused before evaluation");
    };
    assert!(message.contains("is a secret"), "{message}");
}

/// A `def` body is walked for what it returns. A lambda's was not walked at
/// all, so the shortest way to write the same thing went straight through.
#[test]
#[dacc_derive::doc_anchor(
    id = "a-value-that-does-not-exist-yet-cannot-be-laundered-through-a-lambda"
)]
fn a_value_that_does_not_exist_yet_cannot_be_laundered_through_a_lambda() {
    let refusal = evaluate(
        "main.star",
        r#"
db = resource("yandex:instance.db")
address = lambda: db.ip
if address() == "10.0.0.1":
    resource("yandex:instance.yes", {})
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("a lambda is not a way around it");
    assert!(
        matches!(refusal, ConfigError::ComparedUnresolved(_)),
        "{refusal}"
    );
}

/// The other direction. Taint follows a method call's *receiver*, not the whole
/// `x.y` — a bare `x.y` is a resource attribute and always opaque, so asking
/// about it here would make every method call on every string opaque too, and
/// refuse a configuration that does nothing wrong.
#[test]
fn an_ordinary_method_call_is_not_refused_for_looking_like_one() {
    let graph = evaluate(
        "main.star",
        r#"
names = {"kind": "web"}
if names.get("kind") == "web" and "WEB".lower() == "web":
    resource("yandex:instance.yes", {})
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect("nothing here is opaque");
    assert_eq!(
        graph.len(),
        1,
        "the branch was taken and the resource exists"
    );
}

/// A reaction is declared by reference, so a typo is this error with a line
/// number rather than the silence a global handler name gives.
#[test]
fn a_reaction_to_something_that_is_not_a_resource_is_refused() {
    let refusal = evaluate(
        "main.star",
        r#"
resource("os:service.nginx@web-01", {}, reacts_to = ["restart nginx"])
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("a name is not a reference");
    assert!(
        refusal.to_string().contains("reacts_to"),
        "the refusal should name the field: {refusal}"
    );
}

#[test]
fn a_resource_cannot_react_to_itself() {
    let refusal = evaluate(
        "main.star",
        r#"
svc = resource("os:service.nginx@web-01", {})
resource("os:service.nginx@web-01", {}, reacts_to = [svc])
"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("that is a cycle of one");
    assert!(
        refusal.to_string().contains("itself"),
        "the refusal should say so: {refusal}"
    );
}
