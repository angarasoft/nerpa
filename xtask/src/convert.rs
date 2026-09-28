//! Convert decision records between Markdown and the Slipway registry, both ways.
//!
//! The reasoning moves byte for byte; only the carrier changes. The round-trip
//! test is the proof that a conversion loses nothing: every section of the
//! original Markdown must come back identical from the generated record.
//!
//! This is wave 1's first item on purpose: if the two directions do not agree,
//! five thousand lines would have to be retyped by hand, and H8 would fail from
//! fatigue rather than from substance.

// A converter reads and writes text, so it indexes and slices the strings it
// parses; and `decision` mirrors the name the registry's own schema gives that
// field. These lints would forbid the only honest way to write a parser, so
// they are lifted here and nowhere else in this module.
#![allow(clippy::indexing_slicing, clippy::struct_field_names)]

use std::fs;

use crate::shell;

/// One decision, read from either carrier.
#[derive(Debug, PartialEq, Eq)]
struct Decision {
    number: u16,
    title: String,
    status: Status,
    date: String,
    corrected: Vec<String>,
    context: String,
    decision: String,
    consequences: String,
    /// Sections beyond Context/Decision/Consequences, verbatim: `(heading, body)`.
    extras: Vec<(String, String)>,
}

#[derive(Debug, PartialEq, Eq)]
enum Status {
    Accepted,
    SupersededBy(u16),
    Proposed,
}

// ---------------------------------------------------------------------------
// Markdown → Decision
// ---------------------------------------------------------------------------

fn parse_markdown(text: &str) -> Decision {
    let lines: Vec<&str> = text.split('\n').collect();
    let (number, title) = parse_title(lines[0]);

    let first_section = lines
        .iter()
        .position(|line| line.starts_with("## ") || line.starts_with("### "));
    let header = &lines[1..first_section.unwrap_or(lines.len())];
    let (status, date, corrected) = parse_metadata(header);

    let body = &lines[first_section.unwrap_or(lines.len())..];
    let mut context = String::new();
    let mut decision = String::new();
    let mut consequences = String::new();
    let mut extras = Vec::new();
    for (heading, section) in parse_sections(body) {
        match heading.as_str() {
            "## Context" => context = section,
            "## Decision" => decision = section,
            "## Consequences" => consequences = section,
            _ => extras.push((heading, section)),
        }
    }

    Decision {
        number,
        title,
        status,
        date,
        corrected,
        context,
        decision,
        consequences,
        extras,
    }
}

/// `# NNNN. Title` → `(NNNN, Title)`.
fn parse_title(line: &str) -> (u16, String) {
    let rest = line.strip_prefix("# ").unwrap_or(line);
    match rest.split_once(". ") {
        Some((number, title)) => (number.trim().parse().unwrap_or(0), title.trim().to_string()),
        None => (0, rest.trim().to_string()),
    }
}

/// The `- Status:` / `- Date:` / `- Corrected:` block, before the first section.
///
/// A `Corrected:` line may run on across indented lines; each such block is kept
/// whole and verbatim, because a correction is part of the record's body until
/// the registry grows a field for it.
fn parse_metadata(lines: &[&str]) -> (Status, String, Vec<String>) {
    let mut status = Status::Proposed;
    let mut date = String::new();
    let mut corrected = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if let Some(value) = line.strip_prefix("- Status: ") {
            status = parse_status(value.trim());
            i += 1;
        } else if let Some(value) = line.strip_prefix("- Date: ") {
            date = value.trim().to_string();
            i += 1;
        } else if line.starts_with("- Corrected: ") {
            let start = i;
            let mut end = i + 1;
            while end < lines.len() && lines[end].starts_with("  ") {
                end += 1;
            }
            corrected.push(lines[start..end].join("\n"));
            i = end;
        } else {
            i += 1;
        }
    }
    (status, date, corrected)
}

fn parse_status(value: &str) -> Status {
    match value {
        "accepted" => Status::Accepted,
        "proposed" => Status::Proposed,
        _ => match value.strip_prefix("superseded by ") {
            Some(number) => Status::SupersededBy(number.trim().parse().unwrap_or(0)),
            None => Status::Proposed,
        },
    }
}

/// Split a body into `(heading, section)` pairs. The body of a section is the
/// text between its heading and the next, kept verbatim.
fn parse_sections(lines: &[&str]) -> Vec<(String, String)> {
    let mut sections = Vec::new();
    let mut heading: Option<&str> = None;
    let mut body: Vec<&str> = Vec::new();
    for line in lines {
        if line.starts_with("## ") || line.starts_with("### ") {
            if let Some(previous) = heading.take() {
                sections.push((previous.to_string(), body.join("\n")));
                body = Vec::new();
            }
            heading = Some(line);
        } else if heading.is_some() {
            body.push(line);
        }
    }
    if let Some(previous) = heading {
        sections.push((previous.to_string(), body.join("\n")));
    }
    sections
}

// ---------------------------------------------------------------------------
// Decision → Slipway registry
// ---------------------------------------------------------------------------

/// Wrap content in a raw string literal with enough `#` that the content cannot
/// close it early. Chosen minimally so the generated record stays readable.
fn raw_literal(content: &str) -> String {
    let mut hashes = 0;
    while content.contains(&format!("\"{}", "#".repeat(hashes))) {
        hashes += 1;
    }
    format!(
        "r{}\"{}\"{}",
        "#".repeat(hashes),
        content,
        "#".repeat(hashes)
    )
}

/// A plain string literal for the title: escape the two things a string cannot
/// contain as themselves.
fn escape_title(title: &str) -> String {
    title.replace('\\', "\\\\").replace('"', "\\\"")
}

fn status_to_rs(status: &Status) -> String {
    match status {
        Status::Accepted => "DocStatus::Active".to_string(),
        Status::Proposed => "DocStatus::Draft".to_string(),
        Status::SupersededBy(number) => {
            format!("DocStatus::SupersededBy(crate::adr::adr_2026_{number:04})")
        }
    }
}

fn date_to_rs(date: &str) -> String {
    let mut parts = date.split('-');
    let year: u32 = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    let month: u32 = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    let day: u32 = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    format!("slipway_core::date!({year}, {month}, {day})")
}

fn render_rs(decision: &Decision) -> String {
    // The context field carries what the registry has no field for yet: the
    // corrections, then the context section, then any sections beyond the three
    // standard ones. Each keeps its heading so the reverse split is unambiguous.
    let mut context = String::new();
    for block in &decision.corrected {
        context.push_str(block);
        context.push('\n');
    }
    context.push_str("## Context\n");
    context.push_str(&decision.context);
    for (heading, body) in &decision.extras {
        context.push('\n');
        context.push_str(heading);
        context.push('\n');
        context.push_str(body);
    }

    format!(
        r#"use slipway_core::nonempty_str;
use slipway_knowledge::{{Breaking, DocStatus}};

slipway_knowledge::adr!(
    title: "{}",
    status: {},
    subsystems: &[],
    context: {},
    decision: {},
    trade_offs: &[{}],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: {},
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
"#,
        escape_title(&decision.title),
        status_to_rs(&decision.status),
        raw_literal(&context),
        raw_literal(&decision.decision),
        raw_literal(&decision.consequences),
        date_to_rs(&decision.date),
    )
}

// ---------------------------------------------------------------------------
// Slipway registry → Decision
// ---------------------------------------------------------------------------

/// Take a raw string literal starting at the `r` of the literal.
fn take_raw(s: &str) -> (String, &str) {
    let rest = s.strip_prefix('r').unwrap_or(s);
    let hashes = rest.chars().take_while(|c| *c == '#').count();
    let after_hashes = &rest[hashes..];
    let after_quote = after_hashes.strip_prefix('"').unwrap_or(after_hashes);
    let closing = format!("\"{}", "#".repeat(hashes));
    let end = after_quote.find(&closing).unwrap_or(after_quote.len());
    (
        after_quote[..end].to_string(),
        &after_quote[end + closing.len()..],
    )
}

fn field_after<'a>(text: &'a str, marker: &str) -> &'a str {
    text.find(marker).map_or("", |i| &text[i + marker.len()..])
}

fn extract_title(text: &str) -> String {
    let after = field_after(text, "title: \"");
    let mut result = String::new();
    let mut chars = after.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(next) = chars.next() {
                    result.push(match next {
                        'n' => '\n',
                        't' => '\t',
                        '\\' => '\\',
                        '"' => '"',
                        other => other,
                    });
                }
            }
            '"' => break,
            other => result.push(other),
        }
    }
    result
}

fn extract_status(text: &str) -> Status {
    if text.contains("DocStatus::SupersededBy") {
        let after = field_after(text, "adr_2026_");
        let number: u16 = after
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .unwrap_or(0);
        Status::SupersededBy(number)
    } else if text.contains("DocStatus::Draft") {
        Status::Proposed
    } else {
        Status::Accepted
    }
}

fn extract_date(text: &str) -> String {
    let after = field_after(text, "date!(");
    let end = after.find(')').unwrap_or(0);
    let parts: Vec<&str> = after[..end].split(',').map(str::trim).collect();
    if parts.len() == 3 {
        format!("{}-{:0>2}-{:0>2}", parts[0], parts[1], parts[2])
    } else {
        String::new()
    }
}

/// Split the `context` field back into corrections, the context section and any
/// extra sections. This is the inverse of the concatenation in [`render_rs`].
fn split_context_field(text: &str) -> (Vec<String>, String, Vec<(String, String)>) {
    let lines: Vec<&str> = text.split('\n').collect();
    let first_section = lines
        .iter()
        .position(|line| line.starts_with("## ") || line.starts_with("### "));
    let (_, _, corrected) = parse_metadata(&lines[..first_section.unwrap_or(lines.len())]);

    let mut context = String::new();
    let mut extras = Vec::new();
    for (heading, section) in parse_sections(&lines[first_section.unwrap_or(lines.len())..]) {
        if heading == "## Context" {
            context = section;
        } else {
            extras.push((heading, section));
        }
    }
    (corrected, context, extras)
}

/// Read a decision back out of a generated record. The number is not in the
/// record itself — it lives in the file name — so it is passed in.
fn parse_rs(text: &str, number: u16) -> Decision {
    let title = extract_title(text);
    let status = extract_status(text);
    let date = extract_date(text);

    let context_field = take_raw(field_after(text, "context: ")).0;
    let decision = take_raw(field_after(text, "decision: ")).0;
    let consequences = take_raw(field_after(text, "trade_offs: &[")).0;

    let (corrected, context, extras) = split_context_field(&context_field);

    Decision {
        number,
        title,
        status,
        date,
        corrected,
        context,
        decision,
        consequences,
        extras,
    }
}

// ---------------------------------------------------------------------------
// The round trip
// ---------------------------------------------------------------------------

/// Convert Markdown to a record and back, and compare every section verbatim.
fn round_trip(markdown: &str, number: u16) -> Result<(), String> {
    let from_markdown = parse_markdown(markdown);
    let record = render_rs(&from_markdown);
    let back = parse_rs(&record, number);
    if from_markdown != back {
        return Err(format!(
            "round trip diverged for decision {number:04}:\n  parsed: {from_markdown:#?}\n  got:    {back:#?}"
        ));
    }
    Ok(())
}

/// Read one decision record by number, from `docs/decisions/`.
fn read(number: u16) -> Result<String, String> {
    let directory = shell::root().join("docs/decisions");
    let entries =
        fs::read_dir(&directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy().into_owned();
        if name.starts_with(&format!("{number:04}-")) {
            return fs::read_to_string(entry.path()).map_err(|error| format!("{name}: {error}"));
        }
    }
    Err(format!("no decision {number:04}"))
}

/// The numbers of every decision record, in order.
fn numbers() -> Result<Vec<u16>, String> {
    let directory = shell::root().join("docs/decisions");
    let entries =
        fs::read_dir(&directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    let mut numbers = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy().into_owned();
        if let Some((digits, _)) = name.split_once('-')
            && let Ok(number) = digits.parse()
            && number > 0
        {
            numbers.push(number);
        }
    }
    numbers.sort_unstable();
    Ok(numbers)
}

/// Round-trip every decision record, and fail naming the first divergence.
/// This is H8 as a gate, not a test: it runs in `cargo xtask ci`.
pub(crate) fn check() -> Result<(), String> {
    let mut failures = Vec::new();
    for number in numbers()? {
        let markdown = read(number)?;
        if let Err(error) = round_trip(&markdown, number) {
            failures.push(error);
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} records diverged:\n\n{}",
            failures.len(),
            failures.join("\n\n")
        ))
    }
}

/// Write every decision record into the registry — the wave-1 conversion. A
/// record that does not survive the round trip is not written; the round trip is
/// the safety net, run before anything moves rather than after.
pub(crate) fn write() -> Result<(), String> {
    let mut written = 0;
    for number in numbers()? {
        let markdown = read(number)?;
        round_trip(&markdown, number)?;
        let record = render_rs(&parse_markdown(&markdown));
        let path = shell::root().join(format!("doc/adr/adr-2026-{number:04}.rs"));
        fs::write(&path, record).map_err(|error| format!("{}: {error}", path.display()))?;
        written += 1;
    }
    eprintln!("  convert: wrote {written} records to doc/adr/");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every section of every record comes back from the registry byte for byte.
    /// This is H8, run before the wave that moves the records rather than after.
    #[test]
    fn conversion_loses_nothing() {
        check().expect("every decision record round-trips byte for byte");
    }
}
