//! The invariant registry, checked in both directions.
//!
//! `docs/invariants.toml` names the guarantees Sarma must hold and, for the ones
//! already built, the code that enforces them and the tests that prove it. This
//! file is what stops that from becoming a document nobody maintains: a renamed
//! test, a deleted anchor or an invented identifier turns the suite red.
//!
//! The idea, and the `// ENFORCES:` anchor in the spirit of `// SAFETY:`, are
//! taken from AngaraBase's invariant registry. Deliberately without the rest of
//! that apparatus — no generated regions, no matrices, no baselines — because
//! this project has eleven invariants, not several hundred.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use toml::Value;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/<name> sits two levels below the repository root")
        .to_path_buf()
}

fn registry() -> Vec<toml::Table> {
    let text = fs::read_to_string(repo_root().join("docs/invariants.toml"))
        .expect("the registry is part of the repository");
    let parsed: toml::Table = toml::from_str(&text).expect("the registry is valid TOML");
    parsed
        .get("invariant")
        .and_then(Value::as_array)
        .expect("the registry holds an array of invariants")
        .iter()
        .map(|entry| entry.as_table().expect("each invariant is a table").clone())
        .collect()
}

fn field<'a>(entry: &'a toml::Table, name: &str) -> &'a Value {
    entry.get(name).unwrap_or_else(|| {
        panic!(
            "invariant {} has no {name}",
            entry.get("id").map_or("?".into(), Value::to_string)
        )
    })
}

fn strings(entry: &toml::Table, name: &str) -> Vec<String> {
    field(entry, name)
        .as_array()
        .expect("expected a list")
        .iter()
        .map(|value| value.as_str().expect("expected strings").to_owned())
        .collect()
}

/// Every source file under `crates/`, so anchors can be found wherever they sit.
fn sources() -> Vec<(PathBuf, String)> {
    fn walk(directory: &Path, found: &mut Vec<(PathBuf, String)>) {
        for entry in fs::read_dir(directory).expect("readable directory") {
            let path = entry.expect("readable entry").path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name == "target") {
                    continue;
                }
                walk(&path, found);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let text = fs::read_to_string(&path).expect("readable source");
                found.push((path, text));
            }
        }
    }

    let mut found = Vec::new();
    walk(&repo_root().join("crates"), &mut found);
    found
}

#[test]
fn identifiers_are_well_formed_and_unique() {
    let mut seen = BTreeSet::new();
    for entry in registry() {
        let id = field(&entry, "id")
            .as_str()
            .expect("id is a string")
            .to_owned();
        let shape: Vec<&str> = id.split('-').collect();
        assert_eq!(shape.len(), 3, "{id} should look like INV-AREA-001");
        assert_eq!(shape[0], "INV", "{id} should start with INV");
        assert!(
            shape[1].chars().all(|ch| ch.is_ascii_uppercase()),
            "{id} area is uppercase"
        );
        assert!(
            shape[2].len() == 3 && shape[2].chars().all(|ch| ch.is_ascii_digit()),
            "{id}"
        );
        assert!(seen.insert(id.clone()), "{id} is registered twice");
    }
}

#[test]
fn every_invariant_says_what_it_guarantees_and_why() {
    for entry in registry() {
        let id = field(&entry, "id");
        for name in ["statement", "rationale"] {
            let text = field(&entry, name).as_str().expect("prose");
            assert!(text.len() > 30, "{id} has a {name} too short to be one");
        }
        let status = field(&entry, "status")
            .as_str()
            .expect("status is a string");
        assert!(
            matches!(status, "enforced" | "planned"),
            "{id} has unknown status {status:?}"
        );
    }
}

/// An invariant claiming to be enforced must point at the code that does it and
/// the tests that prove it, and that code must carry the matching anchor.
#[test]
fn enforced_invariants_are_anchored_in_code() {
    let sources = sources();
    let root = repo_root();

    for entry in registry() {
        let id = field(&entry, "id").as_str().expect("id").to_owned();
        if field(&entry, "status").as_str() != Some("enforced") {
            continue;
        }

        let files = strings(&entry, "enforced_by");
        let tests = strings(&entry, "tests");
        assert!(
            !files.is_empty(),
            "{id} claims to be enforced but names no code"
        );
        assert!(
            !tests.is_empty(),
            "{id} claims to be enforced but names no test"
        );

        for file in files {
            let path = root.join(&file);
            assert!(path.is_file(), "{id} names {file}, which does not exist");
            let text = fs::read_to_string(&path).expect("readable source");
            assert!(
                text.contains(&format!("ENFORCES: {id}")),
                "{file} enforces {id} but carries no `// ENFORCES: {id}` anchor"
            );
        }

        for test in tests {
            let signature = format!("fn {test}(");
            assert!(
                sources.iter().any(|(_, text)| text.contains(&signature)),
                "{id} names test {test}, which no longer exists"
            );
        }
    }
}

/// The other direction: an anchor naming an identifier nobody registered is a
/// typo or a rename, and would otherwise sit there looking authoritative.
#[test]
fn every_anchor_names_a_registered_invariant() {
    let registered: BTreeSet<String> = registry()
        .iter()
        .map(|entry| field(entry, "id").as_str().expect("id").to_owned())
        .collect();

    for (path, text) in sources() {
        // This file defines the convention, so it necessarily spells the marker
        // out in its own format strings. Scanning it would find those.
        if path.ends_with("tests/invariants.rs") {
            continue;
        }
        for line in text.lines() {
            let Some((_, tail)) = line.split_once("ENFORCES: ") else {
                continue;
            };
            let id = tail.split_whitespace().next().unwrap_or_default();
            assert!(
                registered.contains(id),
                "{} anchors {id:?}, which is not in docs/invariants.toml",
                path.display()
            );
        }
    }
}
