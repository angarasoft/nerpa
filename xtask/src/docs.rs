//! Checks on the written documentation, so that it decays visibly rather than quietly.
//!
//! The decision records are the part worth guarding. They are immutable once
//! accepted and superseded rather than edited, which only works if the numbering
//! is sound and every supersession points both ways — otherwise the trail that
//! explains a reversal breaks, and the reversal looks like an accident.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::shell;

/// One Markdown page: a repository-relative path, and its text.
type Page = (String, String);

/// The decision numbers in the registry, from the file names in `doc/adr/`.
///
/// The registry's own build (the scan) checks supersessions and references, so
/// this is the one check left to the gate: a record whose consequences never
/// name a cost reads as advocacy rather than a decision.
pub(crate) fn decision_numbers() -> Result<Vec<String>, String> {
    let directory = shell::root().join("doc/adr");
    let entries =
        fs::read_dir(&directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    let mut numbers = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(".rs") else {
            continue;
        };
        // "adr-2026-0001" → "0001"
        let Some(rest) = stem.strip_prefix("adr-") else {
            continue;
        };
        if let Some((_, number)) = rest.split_once('-') {
            numbers.push(number.to_owned());
        }
    }
    numbers.sort();
    Ok(numbers)
}

/// The template asks for the downside by name; a record without one is refused.
fn cost_is_named(numbers: &[String]) -> Vec<String> {
    let mut complaints = Vec::new();
    for number in numbers {
        let path = shell::root().join(format!("doc/adr/adr-2026-{number}.rs"));
        let Ok(text) = fs::read_to_string(&path) else {
            complaints.push(format!("doc/adr/adr-2026-{number}.rs: unreadable"));
            continue;
        };
        if !text.to_lowercase().contains("cost") {
            complaints.push(format!(
                "doc/adr/adr-2026-{number}.rs: consequences never name a cost"
            ));
        }
    }
    complaints
}

/// The numbers in a `field: &[crate::kind::ref_NNNN, ...]` list, without the
/// prefix, so that a decision and a specification can name each other.
fn refs(text: &str, field: &str, prefix: &str) -> Vec<String> {
    let Some(at) = text.find(field) else {
        return Vec::new();
    };
    let rest = text.get(at + field.len()..).unwrap_or_default();
    let Some(end) = rest.find(']') else {
        return Vec::new();
    };
    rest.get(..end)
        .unwrap_or_default()
        .split(',')
        .filter_map(|part| part.trim().strip_prefix(prefix))
        .map(str::to_owned)
        .collect()
}

/// The decisions a specification was decided by, read from its record.
fn spec_decided_by(text: &str) -> Vec<String> {
    refs(text, "decided_by: &[", "crate::adr::adr_2026_")
}

/// The specifications a decision names, read from its record.
fn decision_related_rfcs(text: &str) -> Vec<String> {
    refs(text, "related_rfcs: &[", "crate::rfc::rfc_2026_")
}

/// The numbers in a directory of `prefix-2026-NNNN.rs` records.
fn numbers(directory: &Path, prefix: &str) -> Result<Vec<String>, String> {
    let entries =
        fs::read_dir(directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(".rs") else {
            continue;
        };
        let Some(rest) = stem.strip_prefix(prefix) else {
            continue;
        };
        if let Some((_, number)) = rest.split_once('-') {
            out.push(number.to_owned());
        }
    }
    out.sort();
    Ok(out)
}

/// Both directions of a specification's deciding decisions, held so a reader
/// can follow the trail either way: a spec names the decisions that decided it,
/// and each of those decisions names the spec back.
fn audit_links(
    decided_by: &BTreeMap<String, Vec<String>>,
    related: &BTreeMap<String, Vec<String>>,
) -> Vec<String> {
    let mut complaints = Vec::new();

    for (spec, decisions) in decided_by {
        for number in decisions {
            if !related
                .get(number)
                .is_some_and(|specs| specs.contains(spec))
            {
                complaints.push(format!(
                    "rfc-2026-{spec} is decided by {number}, but that decision does not name it in related_rfcs"
                ));
            }
        }
    }
    for (number, specs) in related {
        for spec in specs {
            if !decided_by
                .get(spec)
                .is_some_and(|decisions| decisions.contains(number))
            {
                complaints.push(format!(
                    "{number} names rfc-2026-{spec} in related_rfcs, but that specification is not decided by it"
                ));
            }
        }
    }

    complaints
}

/// Reads the specifications and decisions, and checks the links both ways.
fn spec_decision_links() -> Result<Vec<String>, String> {
    let root = shell::root();

    let mut decided_by: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for number in numbers(&root.join("doc/rfc"), "rfc-2026-")? {
        let path = root.join(format!("doc/rfc/rfc-2026-{number}.rs"));
        let text =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        decided_by.insert(number, spec_decided_by(&text));
    }

    let mut related: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for number in numbers(&root.join("doc/adr"), "adr-2026-")? {
        let path = root.join(format!("doc/adr/adr-2026-{number}.rs"));
        let text =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        related.insert(number, decision_related_rfcs(&text));
    }

    Ok(audit_links(&decided_by, &related))
}

/// The three words a section of the architecture may use about itself.
///
/// Ordered longest-first nowhere on purpose: none is a prefix of another after
/// the separator, so a plain `contains` cannot confuse them.
const BUILD_MARKERS: [&str; 3] = ["· built", "· partly built", "· planned"];

/// Every section of the architecture says whether it exists yet.
///
/// The document is written in the present tense throughout, because that is how
/// a contract reads. Without a marker there is nothing on the page telling a
/// reader that a section describes something nobody has built, and the file
/// becomes indistinguishable from a description of a working program.
fn architecture(text: &str) -> Vec<String> {
    let mut complaints = Vec::new();
    let mut sections = 0usize;

    for (index, line) in text.lines().enumerate() {
        let Some(heading) = line.strip_prefix("## ") else {
            continue;
        };
        sections += 1;

        // At the end, not merely present. `contains` let a marker sit anywhere
        // on the line, so a heading could carry one and then contradict it in
        // its own trailing words — and the message said "does not end in" while
        // the code meant something weaker. An independent review found both.
        let Some(before) = BUILD_MARKERS
            .iter()
            .find_map(|marker| heading.strip_suffix(marker))
        else {
            complaints.push(format!(
                "docs/ARCHITECTURE.md:{}: {heading:?} does not end in `\u{b7} built`, \
                 `\u{b7} partly built` or `\u{b7} planned`",
                index + 1
            ));
            continue;
        };

        if before.trim().is_empty() {
            complaints.push(format!(
                "docs/ARCHITECTURE.md:{}: a marker with no section in front of it",
                index + 1
            ));
        }
    }

    if sections == 0 {
        complaints.push("docs/ARCHITECTURE.md: no sections to judge".to_owned());
    }
    complaints
}

/// Where a link in `page` points, as a repository-relative path.
///
/// `None` for anything that does not name a file in this repository: an external
/// address, or a bare fragment pointing within the page itself.
fn resolve(page: &str, target: &str) -> Option<String> {
    let target = target.split('#').next().unwrap_or_default();
    if target.is_empty()
        || target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("mailto:")
    {
        return None;
    }

    let mut parts: Vec<&str> = page.split('/').collect();
    parts.pop();
    for step in target.split('/') {
        match step {
            "." | "" => {}
            ".." => {
                parts.pop()?;
            }
            name => parts.push(name),
        }
    }
    Some(parts.join("/"))
}

/// Every link between written documents points at something that is there.
///
/// The failure this exists to prevent is specific rather than theoretical: a
/// route that goes on looking plausible after the file it names has moved, so a
/// reader follows it and believes whatever they land on. A broken link is
/// noticed; a link into the wrong place is not.
///
/// Three things it deliberately does not treat as links, each found by an
/// independent review rather than by imagination: anything inside a fenced code
/// block, which is an example rather than a route; a bracket escaped as `\\[`,
/// which is markdown for "these are literal brackets"; and neither of those was
/// hypothetical, because this very file documents link syntax. Reference-style
/// definitions (`[label]: target`) are the opposite case — they were invisible
/// to the scanner, and are now checked like any other route.
fn routes(pages: &[Page], present: &BTreeSet<String>) -> Vec<String> {
    let mut complaints = Vec::new();

    for (page, text) in pages {
        let mut fenced = false;
        for (number, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                fenced = !fenced;
                continue;
            }
            if fenced {
                continue;
            }

            let mut complain = |target: &str| {
                if let Some(path) = resolve(page, target)
                    && !present.contains(&path)
                {
                    complaints.push(format!(
                        "{page}:{}: links to {target:?}, which is not there",
                        number + 1
                    ));
                }
            };

            // `[label]: target` — a reference-style definition, which carries a
            // route without ever spelling `](`.
            if let Some(rest) = trimmed.strip_prefix('[')
                && let Some((_, target)) = rest.split_once("]: ")
                && !target.trim().is_empty()
            {
                complain(target.trim());
                continue;
            }

            for target in inline_targets(line) {
                complain(target);
            }
        }
    }

    complaints
}

/// Every `](target)` on one line, skipping brackets the author escaped.
fn inline_targets(line: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut at = 0usize;

    while let Some(offset) = line.get(at..).and_then(|rest| rest.find("](")) {
        let open = at + offset;
        let after = open + 2;

        // `\](` is a literal bracket, not the seam of a link.
        let escaped = line
            .get(..open)
            .is_some_and(|before| before.ends_with('\\'));

        match line.get(after..).and_then(|rest| rest.find(')')) {
            Some(close) => {
                if !escaped && let Some(target) = line.get(after..after + close) {
                    found.push(target);
                }
                at = after + close + 1;
            }
            None => break,
        }
    }

    found
}

/// What the written documentation is made of, as far as a link is concerned.
#[derive(Debug)]
struct Written {
    /// Every Markdown page, as a repository-relative path and its text.
    pages: Vec<Page>,
    /// Every path in the repository, which is what a link may point at.
    present: BTreeSet<String>,
}

/// Whether a directory holds something other than this repository's writing.
fn skipped(name: &str) -> bool {
    name.starts_with('.') || matches!(name, "target" | "mutants.out" | "node_modules")
}

fn written() -> Result<Written, String> {
    let root = shell::root();
    let mut pages = Vec::new();
    let mut present = BTreeSet::new();
    let mut pending = vec![root.clone()];

    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?;
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(relative) = path.strip_prefix(&root) else {
                continue;
            };
            let shown = relative.to_string_lossy().replace('\\', "/");
            // Build output, version control and installed dependencies are not
            // this repository's documentation. A dependency ships its own README
            // with its own broken links, and reporting those is noise that
            // teaches somebody to stop reading the report.
            if skipped(&shown) {
                continue;
            }

            present.insert(shown.clone());
            if path.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
                && let Ok(text) = fs::read_to_string(&path)
            {
                pages.push((shown, text));
            }
        }
    }

    Ok(Written { pages, present })
}

/// Verifies the decision records and the written documentation, then that the
/// API documentation builds clean.
pub(crate) fn check() -> Result<(), String> {
    let numbers = decision_numbers()?;
    let complaints = cost_is_named(&numbers);
    if !complaints.is_empty() {
        return Err(complaints.join("\n"));
    }
    eprintln!("  decisions: {} records, every cost named", numbers.len());

    let complaints = spec_decision_links()?;
    if !complaints.is_empty() {
        return Err(complaints.join("\n"));
    }
    eprintln!("  specifications: every deciding decision points both ways");

    let architecture_path = shell::root().join("docs/ARCHITECTURE.md");
    let text = fs::read_to_string(&architecture_path)
        .map_err(|error| format!("{}: {error}", architecture_path.display()))?;
    let complaints = architecture(&text);
    if !complaints.is_empty() {
        return Err(complaints.join("\n"));
    }

    let Written { pages, present } = written()?;
    let complaints = routes(&pages, &present);
    if !complaints.is_empty() {
        return Err(complaints.join("\n"));
    }
    eprintln!(
        "  written: {} pages, every section placed and every route resolving",
        pages.len()
    );

    shell::cargo_with_doc_warnings_denied(&["doc", "--no-deps", "--all-features"])
}

// The gate is only worth what its failures are worth, so each rule is shown a
// record that breaks it.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests may be blunt: an unwrap is an expectation and an index is a claim about shape"
)]
mod tests {
    use super::{
        architecture, audit_links, decision_related_rfcs, resolve, routes, skipped, spec_decided_by,
    };
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn a_section_that_does_not_say_whether_it_is_built_is_reported() {
        let complaints = architecture("# Architecture\n\n## Secrets — [0027](x.md)\n\nProse.\n");
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(complaints[0].contains("does not end in"), "{complaints:?}");
    }

    #[test]
    fn each_of_the_three_words_is_accepted() {
        for marker in ["built", "partly built", "planned"] {
            let text = format!("## A section — [0001](x.md) · {marker}\n");
            assert_eq!(architecture(&text), Vec::<String>::new(), "{marker}");
        }
    }

    #[test]
    fn partly_built_is_not_read_as_built() {
        // The two markers share a word, and a careless `contains` would let a
        // section claiming half of itself pass as a section claiming all of it.
        let complaints = architecture("## A section — [0001](x.md) · partly\n");
        assert_eq!(complaints.len(), 1, "{complaints:?}");
    }

    #[test]
    fn an_architecture_with_no_sections_is_itself_a_complaint() {
        assert_eq!(architecture("# Architecture\n\nProse only.\n").len(), 1);
    }

    #[test]
    fn a_link_is_resolved_against_the_page_that_holds_it() {
        assert_eq!(
            resolve("docs/MAP.md", "decisions/0005-a.md").as_deref(),
            Some("docs/decisions/0005-a.md")
        );
        assert_eq!(
            resolve("docs/decisions/0005-a.md", "../ARCHITECTURE.md").as_deref(),
            Some("docs/ARCHITECTURE.md")
        );
        assert_eq!(
            resolve("README.md", "ROADMAP.md").as_deref(),
            Some("ROADMAP.md")
        );
    }

    #[test]
    fn an_anchor_names_the_page_it_sits_on() {
        assert_eq!(
            resolve("docs/MAP.md", "ARCHITECTURE.md#addresses").as_deref(),
            Some("docs/ARCHITECTURE.md")
        );
    }

    #[test]
    fn what_is_not_a_file_in_this_repository_is_left_alone() {
        assert_eq!(resolve("docs/MAP.md", "https://example.invalid/a"), None);
        assert_eq!(resolve("docs/MAP.md", "#a-heading-here"), None);
        assert_eq!(
            resolve("docs/MAP.md", "mailto:someone@example.invalid"),
            None
        );
    }

    #[test]
    fn a_route_to_something_that_moved_is_reported_with_its_line() {
        let pages = vec![(
            "docs/MAP.md".to_owned(),
            "One line.\nSee [the plan](decisions/0099-gone.md).\n".to_owned(),
        )];
        let present = BTreeSet::from(["docs/MAP.md".to_owned()]);

        let complaints = routes(&pages, &present);
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(complaints[0].contains("docs/MAP.md:2"), "{complaints:?}");
    }

    #[test]
    fn several_routes_on_one_line_are_each_checked() {
        let pages = vec![(
            "docs/MAP.md".to_owned(),
            "[a](a.md) and [b](b.md) and [c](c.md)\n".to_owned(),
        )];
        let present = BTreeSet::from(["docs/b.md".to_owned()]);

        let complaints = routes(&pages, &present);
        assert_eq!(complaints.len(), 2, "{complaints:?}");
    }

    #[test]
    fn a_route_that_resolves_is_quiet() {
        let pages = vec![(
            "docs/MAP.md".to_owned(),
            "See [the record](decisions/0005-a.md).\n".to_owned(),
        )];
        let present = BTreeSet::from(["docs/decisions/0005-a.md".to_owned()]);

        assert_eq!(routes(&pages, &present), Vec::<String>::new());
    }

    #[test]
    fn a_marker_that_is_not_at_the_end_is_refused() {
        // Found by review: `contains` accepted a heading that carried a marker
        // and then contradicted it in its own trailing words.
        let complaints =
            architecture("## Fully built — [0001](x.md) · planned, actually not built at all\n");
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(complaints[0].contains("does not end in"), "{complaints:?}");
    }

    #[test]
    fn a_heading_that_is_nothing_but_a_marker_is_refused() {
        let complaints = architecture("## · built\n");
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(
            complaints[0].contains("no section in front"),
            "{complaints:?}"
        );
    }

    #[test]
    fn a_link_shown_as_an_example_in_a_code_block_is_not_a_route() {
        let pages = vec![(
            "docs/COMMITS.md".to_owned(),
            "Prose.\n```markdown\n[an example](decisions/0099-invented.md)\n```\nMore prose.\n"
                .to_owned(),
        )];
        assert_eq!(routes(&pages, &BTreeSet::new()), Vec::<String>::new());
    }

    #[test]
    fn a_fence_that_closes_lets_checking_resume() {
        let pages = vec![(
            "docs/a.md".to_owned(),
            "```\n[x](inside.md)\n```\n[y](outside.md)\n".to_owned(),
        )];
        let complaints = routes(&pages, &BTreeSet::new());
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(complaints[0].contains("outside.md"), "{complaints:?}");
    }

    #[test]
    fn an_escaped_bracket_is_not_a_link() {
        let pages = vec![(
            "docs/a.md".to_owned(),
            r"a literal \[thing\](not-a-path) in prose".to_owned() + "\n",
        )];
        assert_eq!(routes(&pages, &BTreeSet::new()), Vec::<String>::new());
    }

    #[test]
    fn a_reference_style_definition_is_a_route_like_any_other() {
        let pages = vec![(
            "docs/a.md".to_owned(),
            "See [the record][rec].\n\n[rec]: decisions/0099-gone.md\n".to_owned(),
        )];
        let complaints = routes(&pages, &BTreeSet::new());
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(complaints[0].contains("0099-gone.md"), "{complaints:?}");
    }

    #[test]
    fn a_reference_style_definition_that_resolves_is_quiet() {
        let pages = vec![(
            "docs/a.md".to_owned(),
            "[rec]: decisions/0005-a.md\n".to_owned(),
        )];
        let present = BTreeSet::from(["docs/decisions/0005-a.md".to_owned()]);
        assert_eq!(routes(&pages, &present), Vec::<String>::new());
    }

    #[test]
    fn what_is_not_this_repositorys_writing_is_not_walked() {
        // A virtual environment ships hundreds of READMEs with links of their
        // own, and this gate reported two of them the day the book arrived.
        for name in [".venv-docs", ".git", ".cargo", "target", "node_modules"] {
            assert!(skipped(name), "{name} should not be walked");
        }
        assert!(!skipped("docs"), "docs must still be walked");
        assert!(!skipped("crates"), "crates must still be walked");
    }

    #[test]
    fn a_spec_and_its_decision_hold_each_other_both_ways() {
        let decided_by = BTreeMap::from([("003".to_owned(), vec!["0010".to_owned()])]);
        let related = BTreeMap::from([("0010".to_owned(), vec!["003".to_owned()])]);
        assert_eq!(audit_links(&decided_by, &related), Vec::<String>::new());
    }

    #[test]
    fn a_spec_naming_a_decision_that_does_not_point_back_is_reported() {
        let decided_by = BTreeMap::from([("003".to_owned(), vec!["0010".to_owned()])]);
        let related = BTreeMap::new();
        let complaints = audit_links(&decided_by, &related);
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(complaints[0].contains("does not name it"), "{complaints:?}");
    }

    #[test]
    fn a_decision_naming_a_spec_that_does_not_point_back_is_reported() {
        let decided_by = BTreeMap::new();
        let related = BTreeMap::from([("0010".to_owned(), vec!["003".to_owned()])]);
        let complaints = audit_links(&decided_by, &related);
        assert_eq!(complaints.len(), 1, "{complaints:?}");
        assert!(
            complaints[0].contains("not decided by it"),
            "{complaints:?}"
        );
    }

    #[test]
    fn the_reference_lists_are_read_without_the_prefix() {
        assert_eq!(
            spec_decided_by("decided_by: &[crate::adr::adr_2026_0010, crate::adr::adr_2026_0041]"),
            vec!["0010", "0041"]
        );
        assert_eq!(
            decision_related_rfcs("related_rfcs: &[crate::rfc::rfc_2026_003]"),
            vec!["003"]
        );
    }
}
