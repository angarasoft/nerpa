//! What a configuration may say, and what it is refused.
//!
//! The refusals are the point. Everything here that fails is a case where a
//! value which does not exist yet was about to decide something.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use nerpa_config::{ConfigError, evaluate};
use nerpa_core::Attribute;
use nerpa_core::address::address;

fn graph_of(source: &str) -> nerpa_core::Graph {
    evaluate("main.star", source).expect("the configuration should evaluate")
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
fn comparing_a_value_that_does_not_exist_yet_is_refused_before_anything_runs() {
    for body in [
        r#"x = db.ip == "10.0.0.1""#,
        r#"x = "10.0.0.1" == db.ip"#,
        r#"x = db.ip != "10.0.0.1""#,
        r#"x = db.ip in ["10.0.0.1"]"#,
        r#"x = db.ip not in ["10.0.0.1"]"#,
    ] {
        let source = format!("db = resource(\"yandex:instance.db\")\n{body}\n");
        let refusal =
            evaluate("main.star", &source).expect_err("this comparison should have been refused");

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
fn iterating_a_value_that_does_not_exist_yet_is_refused() {
    let refusal = evaluate(
        "main.star",
        r#"
db = resource("yandex:instance.db")
for disk in db.disks:
    resource("yandex:disk." + disk)
"#,
    )
    .expect_err("iteration over an unresolved value must fail");

    assert!(refusal.to_string().contains("iteration"), "{refusal}");
}

#[test]
fn indexing_a_value_that_does_not_exist_yet_is_refused() {
    let refusal = evaluate(
        "main.star",
        r#"
db = resource("yandex:instance.db")
x = db.addresses[0]
"#,
    )
    .expect_err("indexing an unresolved value must fail");

    assert!(refusal.to_string().contains("indexing"), "{refusal}");
}

/// The one refusal the toolchain will not let us make where it happens. The run
/// is refused afterwards instead, so the invariant holds and only the diagnostic
/// is weaker.
#[test]
fn branching_on_a_value_that_does_not_exist_yet_refuses_the_whole_run() {
    let refusal = evaluate(
        "main.star",
        r#"
db = resource("yandex:instance.db")
if db.ready:
    resource("yandex:instance.web")
"#,
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
    let refusal = evaluate("main.star", r#"resource("not-an-address")"#)
        .expect_err("a malformed address must fail");

    assert!(refusal.to_string().contains("provider"), "{refusal}");
}

#[test]
fn an_attribute_holding_something_a_resource_cannot_carry_is_reported() {
    let refusal = evaluate("main.star", r#"resource("yandex:instance.db", {"f": len})"#)
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

        let refusal = evaluate("main.star", &source)
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
        let refusal = evaluate("main.star", &source).expect_err("refused");
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
    assert!(evaluate("main.star", poisoned).is_err());

    let clean = graph_of(r#"resource("yandex:instance.db")"#);
    assert_eq!(
        clean.len(),
        1,
        "the previous run's branch must not carry over"
    );
}
