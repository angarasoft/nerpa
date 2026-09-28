//! Templates: what they may say, and what the sandbox will not let them do.
//!
//! The compatibility half is the point of choosing Jinja at all. These twenty
//! idioms were taken out of real Ansible templates and measured against the
//! candidates before `0023` chose between them; they live here now so that the
//! choice keeps being true rather than having been true once.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use nerpa_config::evaluate;
use nerpa_config::fleet::Fleet;
use nerpa_config::template::Held;
use nerpa_core::Attribute;
use nerpa_core::address::address;

/// Renders one template with a fixed set of values and returns what came out.
fn rendered(template: &str) -> Result<String, String> {
    let source = r#"
resource("os:file.subject@web-01", {
    "content": template("t.j2", {
        "name": "example.com",
        "workers": 4,
        "tls": True,
        "protocols": ["TLSv1.2", "TLSv1.3"],
        "opts": {"keepalive": 65, "gzip": "on"},
        "sites": [{"name": "a", "tls": True}, {"name": "b", "tls": False}],
    }),
})
"#;
    let graph = evaluate(
        "main.star",
        source,
        Box::new(Held::new().holding("t.j2", template)),
        Fleet::empty(),
    )
    .map_err(|error| error.to_string())?;

    match graph
        .get(&address("os:file.subject@web-01"))
        .and_then(|resource| resource.attribute("content"))
        .and_then(Attribute::known)
    {
        Some(nerpa_core::Value::Text(text)) => Ok(text.clone()),
        other => panic!("content should be text, and is {other:?}"),
    }
}

/// Twenty idioms, written the way somebody writes them for Ansible.
#[test]
#[dacc_derive::doc_anchor(id = "templates-written-for-ansible-render-here")]
fn templates_written_for_ansible_render_here() {
    let idioms: [(&str, &str); 20] = [
        ("{{ missing | default('www-data') }}", "www-data"),
        ("{{ protocols | join(' ') }}", "TLSv1.2 TLSv1.3"),
        ("{% if sites is defined %}yes{% endif %}", "yes"),
        ("{% if nope is not defined %}yes{% endif %}", "yes"),
        ("{{ name | replace('.', '_') }}", "example_com"),
        ("{{ 'on' if tls else 'off' }}", "on"),
        ("{{ sites | map(attribute='name') | join(',') }}", "a,b"),
        (
            "{% for k, v in opts.items() %}{{ k }}={{ v }};{% endfor %}",
            "gzip=on;keepalive=65;",
        ),
        ("{% for s in protocols %}{{ loop.index }}{% endfor %}", "12"),
        ("{% set n = workers + 1 %}{{ n }}", "5"),
        ("{{ opts['keepalive'] }}", "65"),
        ("{{ sites | selectattr('tls') | list | length }}", "1"),
        ("a\n{%- if tls %}b{% endif %}", "ab"),
        ("{{ sites[0].name }}", "a"),
        ("{{ name ~ '.conf' }}", "example.com.conf"),
        ("{% filter upper %}quiet{% endfilter %}", "QUIET"),
        (
            "{% macro row(k) %}[{{ k }}]{% endmacro %}{{ row('x') }}",
            "[x]",
        ),
        ("{{ protocols | length }}", "2"),
        ("{% if 'TLSv1.3' in protocols %}yes{% endif %}", "yes"),
        ("{{ protocols | batch(1) | list | length }}", "2"),
    ];

    for (template, expected) in idioms {
        assert_eq!(rendered(template).as_deref(), Ok(expected), "{template}");
    }
}

/// A value that is not there is a mistake, not an empty string. For a
/// configuration file the difference is a directive that silently disappears.
#[test]
#[dacc_derive::doc_anchor(id = "a-value-that-was-not-handed-over-is-a-refusal-rather-than-a-blank")]
fn a_value_that_was_not_handed_over_is_a_refusal_rather_than_a_blank() {
    let refusal = rendered("listen {{ port }};").expect_err("undefined is not empty");
    assert!(refusal.contains("undefined"), "{refusal}");
}

/// There is no ambient scope. A value declared beside the call is not in it.
#[test]
#[dacc_derive::doc_anchor(id = "nothing-is-in-scope-that-was-not-handed-over")]
fn nothing_is_in_scope_that_was_not_handed_over() {
    let refusal = evaluate(
        "main.star",
        r#"
port = 8080
resource("os:file.a@web-01", {"content": template("t.j2", {})})
"#,
        Box::new(Held::new().holding("t.j2", "listen {{ port }};")),
        Fleet::empty(),
    )
    .expect_err("a variable beside the call is not a variable in the template");
    assert!(refusal.to_string().contains("undefined"), "{refusal}");
}

/// The filters are a list this project keeps, not whatever the library ships,
/// because every one of them is part of what a configuration may rely on.
#[test]
#[dacc_derive::doc_anchor(id = "a-filter-that-is-not-on-the-list-is-refused-by-name")]
fn a_filter_that_is_not_on_the_list_is_refused_by_name() {
    let refusal = rendered("{{ name | urlencode }}").expect_err("not on the list");
    assert!(
        refusal.contains("urlencode"),
        "the refusal should name it: {refusal}"
    );
}

/// A template arriving from somebody's configuration repository is exactly where
/// an unbounded loop should be impossible.
#[test]
#[dacc_derive::doc_anchor(id = "a-template-that-would-not-stop-is-stopped")]
fn a_template_that_would_not_stop_is_stopped() {
    let refusal =
        rendered("{% for a in range(2000) %}{% for b in range(2000) %}x{% endfor %}{% endfor %}")
            .expect_err("four million iterations is not a configuration file");
    assert!(refusal.contains("fuel"), "{refusal}");
}

/// Methods arrive by a different door than filters. `pycompat` was taken whole,
/// which put around twenty-five of them inside the sandbox unexamined — and one,
/// `str.count("")`, never returns: it advances by the length of what it searched
/// for, and searching for nothing advances nothing. Fuel cannot stop it, because
/// fuel is charged per instruction of the template's own machine and that loop
/// runs inside a call the machine is waiting on. Measured before the list
/// existed: a plan over this template ran at a full core until it was killed.
#[test]
fn a_method_that_is_not_on_the_list_is_refused_by_name() {
    let refusal = rendered(r#"{{ "x".count("") }}"#).expect_err("not on the list");
    assert!(
        refusal.contains("count"),
        "the refusal should name it: {refusal}"
    );
}

/// And the ones that are on it still work, which is why `pycompat` is here.
#[test]
fn the_methods_ansible_templates_use_still_answer() {
    assert_eq!(
        rendered(r#"{{ name.upper() }} {{ name.split(".")[0] }}"#).expect("both are on the list"),
        "EXAMPLE.COM example"
    );
}

#[test]
fn a_template_that_is_not_there_says_so() {
    let refusal = evaluate(
        "main.star",
        r#"resource("os:file.a@web-01", {"content": template("absent.j2", {})})"#,
        Box::new(Held::new()),
        Fleet::empty(),
    )
    .expect_err("there is no such template");
    assert!(refusal.to_string().contains("absent.j2"), "{refusal}");
}

/// The one place a template may read from is the configuration's own directory.
#[test]
#[dacc_derive::doc_anchor(id = "a-template-cannot-be-named-outside-the-configuration")]
fn a_template_cannot_be_named_outside_the_configuration() {
    use nerpa_config::template::{Directory, Templates};

    let source = Directory::at("/etc/nerpa");
    for name in ["../secrets.j2", "a/../../b.j2", "/etc/passwd"] {
        let refusal = source
            .get(name)
            .expect_err("a template outside the configuration is not part of it");
        assert!(
            refusal.contains(name),
            "the refusal should name what was asked for: {refusal}"
        );
    }
}

/// A template's last newline is part of the file it looks like. Jinja drops it
/// by default and Ansible keeps it, so the two rendered one template into two
/// different files and each restarted `chronyd` after the other ran. Cron ignores
/// a last line with no newline after it, which is what dropping one costs.
#[test]
fn a_templates_last_newline_is_kept_and_none_is_invented() {
    assert_eq!(
        rendered("server {{ name }}\n").unwrap(),
        "server example.com\n"
    );
    assert_eq!(
        rendered("server {{ name }}").unwrap(),
        "server example.com",
        "a template that ends without one renders without one"
    );
    assert_eq!(rendered("a\nb\n\n").unwrap(), "a\nb\n\n");
}
