//! The invariant registry, checked in both directions.
//!
//! `docs/invariants.toml` names the guarantees Sarma must hold and, for the ones
//! already built, the code that enforces them and the tests that prove it. This
//! is what stops that from becoming a document nobody maintains: a renamed test,
//! a deleted anchor or an invented identifier fails the gate.
//!
//! The idea, and the `// ENFORCES:` anchor in the spirit of `// SAFETY:`, are
//! taken from AngaraBase's invariant registry — without the generated regions,
//! matrices and baselines that a project with several hundred invariants needs
//! and one with eleven does not.

use std::collections::BTreeSet;
use std::fs;

use toml::Value;

use crate::lint;
use crate::shell;

/// Reads the registry, or explains why it could not.
fn registry() -> Result<Vec<toml::Table>, String> {
    let path = shell::root().join("docs/invariants.toml");
    let text = fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let parsed: toml::Table =
        toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;

    let entries = parsed
        .get("invariant")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{}: no [[invariant]] entries", path.display()))?;

    entries
        .iter()
        .map(|entry| {
            entry
                .as_table()
                .cloned()
                .ok_or_else(|| "each [[invariant]] must be a table".to_owned())
        })
        .collect()
}

fn text_field(entry: &toml::Table, name: &str) -> Result<String, String> {
    entry
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("an invariant is missing a textual {name}"))
}

fn list_field(entry: &toml::Table, name: &str, id: &str) -> Result<Vec<String>, String> {
    entry
        .get(name)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{id} is missing the list {name}"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{id}: {name} holds something that is not a string"))
        })
        .collect()
}

/// Checks the registry against the code, in both directions.
pub fn check() -> Result<(), String> {
    let entries = registry()?;
    let sources = lint::all_sources();
    let root = shell::root();
    let mut complaints = Vec::new();
    let mut registered: BTreeSet<String> = BTreeSet::new();

    for entry in &entries {
        let id = text_field(entry, "id")?;

        if !well_formed(&id) {
            complaints.push(format!("{id} does not look like INV-AREA-001"));
        }
        if !registered.insert(id.clone()) {
            complaints.push(format!("{id} is registered twice"));
        }

        for name in ["statement", "rationale"] {
            match text_field(entry, name) {
                Ok(prose) if prose.len() > 30 => {}
                Ok(_) => complaints.push(format!("{id} has a {name} too short to be one")),
                Err(report) => complaints.push(format!("{id}: {report}")),
            }
        }

        let status = text_field(entry, "status")?;
        if !matches!(status.as_str(), "enforced" | "planned") {
            complaints.push(format!("{id} has unknown status {status:?}"));
            continue;
        }
        if status != "enforced" {
            continue;
        }

        let files = list_field(entry, "enforced_by", &id)?;
        let tests = list_field(entry, "tests", &id)?;
        if files.is_empty() {
            complaints.push(format!("{id} claims to be enforced but names no code"));
        }
        if tests.is_empty() {
            complaints.push(format!("{id} claims to be enforced but names no test"));
        }

        for file in files {
            let path = root.join(&file);
            match fs::read_to_string(&path) {
                Err(_) => complaints.push(format!("{id} names {file}, which does not exist")),
                Ok(text) if !text.contains(&format!("ENFORCES: {id}")) => complaints.push(format!(
                    "{file} enforces {id} but carries no `// ENFORCES: {id}` anchor"
                )),
                Ok(_) => {}
            }
        }

        for test in tests {
            let signature = format!("fn {test}(");
            if !sources.iter().any(|(_, text)| text.contains(&signature)) {
                complaints.push(format!("{id} names test {test}, which no longer exists"));
            }
        }
    }

    // The other direction: an anchor naming an identifier nobody registered is a
    // typo or a rename, and would otherwise sit there looking authoritative.
    for (path, text) in &sources {
        // This file defines the convention, so it necessarily spells the marker
        // out in its own format strings. Scanning it would find those.
        if path.ends_with("xtask/src/invariants.rs") {
            continue;
        }
        for (index, line) in text.lines().enumerate() {
            let Some((_, tail)) = line.split_once("ENFORCES: ") else {
                continue;
            };
            let id = tail.split_whitespace().next().unwrap_or_default();
            if !registered.contains(id) {
                complaints.push(format!(
                    "{}:{}: anchors {id:?}, which is not in docs/invariants.toml",
                    lint::display(path),
                    index + 1
                ));
            }
        }
    }

    if complaints.is_empty() {
        let enforced = entries
            .iter()
            .filter(|entry| entry.get("status").and_then(Value::as_str) == Some("enforced"))
            .count();
        eprintln!(
            "  invariants: {} registered, {enforced} enforced and anchored",
            entries.len()
        );
        return Ok(());
    }
    Err(complaints.join("\n"))
}

fn well_formed(id: &str) -> bool {
    let parts: Vec<&str> = id.split('-').collect();
    parts.len() == 3
        && parts[0] == "INV"
        && !parts[1].is_empty()
        && parts[1].chars().all(|ch| ch.is_ascii_uppercase())
        && parts[2].len() == 3
        && parts[2].chars().all(|ch| ch.is_ascii_digit())
}
