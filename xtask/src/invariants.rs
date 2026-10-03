//! The invariant registry, read from the compiled records.
//!
//! `doc/invariant/` holds the invariants as typed records whose `enforced_by`,
//! `tests` and `adversarial` are anchors the scan checks at compile time: a
//! renamed test or a deleted enforcement no longer resolves. What is left for
//! this check is the prose that no type holds — the `SECURITY.md` list of which
//! guarantees are still believed rather than demonstrated.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::shell;

/// Reads the id and status from each invariant record in `dir`.
fn read_records(dir: &Path) -> Result<Vec<(String, String)>, String> {
    let mut paths: Vec<_> = fs::read_dir(dir)
        .map_err(|error| format!("{}: {error}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension().and_then(|ext| ext.to_str()) == Some("rs")
                && path.file_name().and_then(|name| name.to_str()) != Some("mod.rs")
        })
        .collect();
    paths.sort();

    let mut records = Vec::new();
    for path in paths {
        let text =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        // The INV- key rides in the statement before its first colon, and the
        // status is the upstream name (adr-2026-068).
        let id = text
            .lines()
            .find_map(|line| line.trim().strip_prefix("statement: NonEmptyStr::new(\""))
            .and_then(|rest| rest.split('"').next())
            .and_then(|statement| statement.split(": ").next())
            .unwrap_or_default();
        let status = if text.contains("Status::Planned") {
            "planned"
        } else if text.contains("Status::Claimed") {
            "claimed"
        } else {
            "enforced"
        };
        records.push((id.to_owned(), status.to_owned()));
    }
    Ok(records)
}

/// Every invariant `SECURITY.md` names is one that is still `claimed`.
///
/// The policy tells a researcher which guarantees are believed rather than
/// demonstrated, so that reporting one is not mistaken for news. That list is
/// prose beside a registry, which is the shape drift takes: promote an entry and
/// the policy goes on saying a known weakness is known, about something the
/// project has started claiming is proven. Checked both ways, like an anchor.
fn security_policy(policy: &str, claimed: &BTreeSet<String>) -> Vec<String> {
    let mut complaints = Vec::new();

    let mut named: BTreeSet<String> = BTreeSet::new();
    let mut rest = policy;
    while let Some(at) = rest.find("INV-") {
        let tail = rest.get(at..).unwrap_or_default();
        let id: String = tail
            .chars()
            .take_while(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || *ch == '-')
            .collect();
        named.insert(id);
        rest = tail.get(4..).unwrap_or_default();
    }

    for id in named.difference(claimed) {
        complaints.push(format!(
            "SECURITY.md names {id} among what is already known, and it is not claimed"
        ));
    }
    for id in claimed.difference(&named) {
        complaints.push(format!(
            "{id} is claimed and SECURITY.md does not say so — a researcher would \
             report it as news"
        ));
    }

    complaints
}

/// Checks the registry's statuses against `SECURITY.md`.
pub(crate) fn check() -> Result<(), String> {
    let root = shell::root();
    let records = read_records(&root.join("doc/invariant"))?;

    let claimed: BTreeSet<String> = records
        .iter()
        .filter(|(_, status)| status == "claimed")
        .map(|(id, _)| id.clone())
        .collect();

    let policy_path = root.join("SECURITY.md");
    let policy =
        fs::read_to_string(&policy_path).map_err(|error| format!("SECURITY.md: {error}"))?;
    let complaints = security_policy(&policy, &claimed);
    if !complaints.is_empty() {
        return Err(complaints.join("\n"));
    }

    let count = |wanted: &str| {
        records
            .iter()
            .filter(|(_, status)| status == wanted)
            .count()
    };
    eprintln!(
        "  invariants: {} registered, {} enforced, {} claimed, {} planned",
        records.len(),
        count("enforced"),
        count("claimed"),
        count("planned"),
    );
    Ok(())
}
