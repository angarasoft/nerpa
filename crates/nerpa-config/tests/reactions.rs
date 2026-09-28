//! What a provider says cannot react is refused where the configuration says it.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use nerpa_config::evaluate_with;
use nerpa_config::fleet::Fleet;
use nerpa_config::template::Directory;
use nerpa_core::{Kind, Provider, Schema};

fn evaluated(source: &str) -> Result<nerpa_core::Graph, String> {
    let packages_cannot_react =
        Schema::new().cannot_react(Provider::new("os").unwrap(), Kind::new("package").unwrap());
    evaluate_with(
        "main.star",
        source,
        Box::new(Directory::at(std::env::temp_dir())),
        Fleet::empty(),
        packages_cannot_react,
    )
    .map_err(|refusal| refusal.to_string())
}

/// A package cannot react: on rhel a reaction did nothing, and on Portage it merged
/// the package again. Its provider says so, and a configuration saying otherwise is
/// refused at the line that says it. Decided after the third review of RFC 0001.
#[test]
#[dacc_derive::doc_anchor(id = "a-kind-that-cannot-react-is-refused-where-the-reaction-is-written")]
fn a_kind_that_cannot_react_is_refused_where_the_reaction_is_written() {
    let refusal = evaluated(concat!(
        "node(\"web-01\")\n",
        "conf = resource(\"os:file.conf@web-01\", {\"path\": \"/etc/app.conf\"})\n",
        "resource(\"os:package.app-misc/jq@web-01\", {\"version\": \"1.8.1\"}, reacts_to = [conf])\n",
    ))
    .expect_err("a package cannot react");
    assert!(
        refusal.contains("os:package.app-misc/jq@web-01"),
        "{refusal}"
    );
    assert!(
        refusal.contains("main.star:3"),
        "at the line that says it: {refusal}"
    );
}

/// A service still reacts, and a package that reacts to nothing is a package.
#[test]
#[dacc_derive::doc_anchor(id = "what-can-react-still-does")]
fn what_can_react_still_does() {
    evaluated(concat!(
        "node(\"web-01\")\n",
        "conf = resource(\"os:file.conf@web-01\", {\"path\": \"/etc/app.conf\"})\n",
        "resource(\"os:service.nginx@web-01\", {}, reacts_to = [conf])\n",
        "resource(\"os:package.app-misc/jq@web-01\", {\"version\": \"1.8.1\"})\n",
    ))
    .expect("a service reacts, and a package is declared");
}
