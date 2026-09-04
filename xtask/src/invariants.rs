//! The invariant registry, checked in both directions.
//!
//! `docs/invariants.toml` names the guarantees Sarma must hold and, for the ones
//! already built, the code that enforces them and the tests that prove it. This
//! is what stops that from becoming a document nobody maintains: a renamed test,
//! a deleted anchor or an invented identifier fails the gate.
//!
//! The idea, and the `// ENFORCES:` anchor in the spirit of `// SAFETY:`, are
//! taken from `AngaraBase`'s invariant registry — without the generated regions,
//! matrices and baselines that a project with several hundred invariants needs
//! and one with eleven does not.

use std::collections::BTreeSet;
use std::fs;

use toml::Value;

use crate::lint;
use crate::shell;

/// One source file, as a repository-relative path and its text.
type Source = (String, String);

/// Reads the registry, or explains why it could not.
fn parse(text: &str) -> Result<Vec<toml::Table>, String> {
    let parsed: toml::Table =
        toml::from_str(text).map_err(|error| format!("docs/invariants.toml: {error}"))?;

    let entries = parsed
        .get("invariant")
        .and_then(Value::as_array)
        .ok_or_else(|| "docs/invariants.toml: no [[invariant]] entries".to_owned())?;

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

fn well_formed(id: &str) -> bool {
    let mut parts = id.split('-');
    let (Some(prefix), Some(area), Some(number), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };

    prefix == "INV"
        && !area.is_empty()
        && area.chars().all(|ch| ch.is_ascii_uppercase())
        && number.len() == 3
        && number.chars().all(|ch| ch.is_ascii_digit())
}

/// The whole check, with no filesystem in sight, so that it can be handed
/// fixtures and asked to go red.
///
/// Returns one line per complaint. Empty means the registry and the code agree.
fn audit(registry: &str, sources: &[Source]) -> Result<Vec<String>, String> {
    let entries = parse(registry)?;
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
            match sources.iter().find(|(path, _)| path == &file) {
                None => complaints.push(format!("{id} names {file}, which does not exist")),
                Some((_, text)) if !text.contains(&format!("ENFORCES: {id}")) => complaints.push(
                    format!("{file} enforces {id} but carries no `// ENFORCES: {id}` anchor"),
                ),
                Some(_) => {}
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
    for (path, text) in sources {
        // This file defines the convention, so it necessarily spells the marker
        // out in its own strings. Scanning it would find those.
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
                    "{path}:{}: anchors {id:?}, which is not in docs/invariants.toml",
                    index + 1
                ));
            }
        }
    }

    Ok(complaints)
}

/// Checks the registry against the code, in both directions.
pub(crate) fn check() -> Result<(), String> {
    let root = shell::root();
    let path = root.join("docs/invariants.toml");
    let registry =
        fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;

    let sources: Vec<Source> = lint::all_sources()
        .into_iter()
        .map(|(path, text)| (lint::display(&path), text))
        .collect();

    let complaints = audit(&registry, &sources)?;
    if !complaints.is_empty() {
        return Err(complaints.join("\n"));
    }

    let entries = parse(&registry)?;
    let enforced = entries
        .iter()
        .filter(|entry| entry.get("status").and_then(Value::as_str) == Some("enforced"))
        .count();
    eprintln!(
        "  invariants: {} registered, {enforced} enforced and anchored",
        entries.len()
    );
    Ok(())
}

// A check nobody has watched fail is not a check. Every rule below is shown a
// fixture that breaks it, and asked to say so.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests may be blunt: an unwrap is an expectation and an index is a claim about shape"
)]
mod tests {
    use super::{Source, audit};

    const SOUND: &str = r#"
[[invariant]]
id = "INV-TEST-001"
status = "enforced"
statement = "A statement long enough to count as one."
rationale = "A rationale that is also long enough to count as one."
enforced_by = ["crates/thing/src/lib.rs"]
tests = ["the_thing_holds"]
"#;

    fn sources() -> Vec<Source> {
        vec![
            (
                "crates/thing/src/lib.rs".to_owned(),
                "// ENFORCES: INV-TEST-001\n".to_owned(),
            ),
            (
                "crates/thing/tests/thing.rs".to_owned(),
                "fn the_thing_holds() {}\n".to_owned(),
            ),
        ]
    }

    #[test]
    fn a_registry_that_agrees_with_the_code_is_quiet() {
        assert_eq!(audit(SOUND, &sources()).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn a_missing_anchor_is_reported() {
        let mut sources = sources();
        sources[0].1 = "// nothing here\n".to_owned();

        let complaints = audit(SOUND, &sources).unwrap();
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(complaints[0].contains("carries no"), "{complaints:?}");
    }

    #[test]
    fn a_renamed_test_is_reported() {
        let mut sources = sources();
        sources[1].1 = "fn the_thing_still_holds() {}\n".to_owned();

        let complaints = audit(SOUND, &sources).unwrap();
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(complaints[0].contains("no longer exists"), "{complaints:?}");
    }

    #[test]
    fn an_anchor_nobody_registered_is_reported() {
        let mut sources = sources();
        sources.push((
            "crates/thing/src/other.rs".to_owned(),
            "// ENFORCES: INV-MADE-UP\n".to_owned(),
        ));

        let complaints = audit(SOUND, &sources).unwrap();
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(
            complaints[0].contains("not in docs/invariants.toml"),
            "{complaints:?}"
        );
    }

    #[test]
    fn an_enforced_invariant_with_no_evidence_is_reported() {
        let registry = SOUND.replace("tests = [\"the_thing_holds\"]", "tests = []");

        let complaints = audit(&registry, &sources()).unwrap();
        assert!(
            complaints.iter().any(|line| line.contains("names no test")),
            "{complaints:?}"
        );
    }

    /// One case per condition, each failing exactly that one. A single fixture
    /// that breaks several at once passes even when the rule has been widened to
    /// check almost nothing — which is what mutation testing found here.
    #[test]
    fn each_part_of_an_identifier_is_checked_on_its_own() {
        for wrong in [
            "inv-TEST-001",
            "INV-test-001",
            "INV--001",
            "INV-TEST-01",
            "INV-TEST-0a1",
        ] {
            let registry = SOUND.replace("INV-TEST-001", wrong);
            let complaints = audit(&registry, &sources()).unwrap();
            assert!(
                complaints
                    .iter()
                    .any(|line| line.contains("does not look like")),
                "{wrong} should have been rejected, got {complaints:?}"
            );
        }
    }

    #[test]
    fn prose_is_measured_at_the_boundary_not_near_it() {
        let exactly_thirty = "x".repeat(30);
        let one_more = "x".repeat(31);

        let short = SOUND.replace("A statement long enough to count as one.", &exactly_thirty);
        assert!(
            audit(&short, &sources())
                .unwrap()
                .iter()
                .any(|line| line.contains("too short")),
            "thirty characters is not a statement"
        );

        let long = SOUND.replace("A statement long enough to count as one.", &one_more);
        assert_eq!(audit(&long, &sources()).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn a_malformed_identifier_is_reported() {
        let registry = SOUND.replace("INV-TEST-001", "INVALID");

        let complaints = audit(&registry, &sources()).unwrap();
        assert!(
            complaints
                .iter()
                .any(|line| line.contains("does not look like")),
            "{complaints:?}"
        );
    }

    #[test]
    fn prose_too_short_to_be_a_statement_is_reported() {
        let registry = SOUND.replace("A statement long enough to count as one.", "Short.");

        let complaints = audit(&registry, &sources()).unwrap();
        assert!(
            complaints.iter().any(|line| line.contains("too short")),
            "{complaints:?}"
        );
    }

    #[test]
    fn an_unknown_status_is_reported() {
        let registry = SOUND.replace(r#"status = "enforced""#, r#"status = "probably""#);

        let complaints = audit(&registry, &sources()).unwrap();
        assert!(
            complaints
                .iter()
                .any(|line| line.contains("unknown status")),
            "{complaints:?}"
        );
    }

    #[test]
    fn a_registry_that_is_not_toml_is_an_error_not_a_complaint() {
        assert!(audit("this is not toml", &sources()).is_err());
    }
}
