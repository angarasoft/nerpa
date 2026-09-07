//! Printing a plan so that somebody can read it at three in the morning.
//!
//! The shape is taken from `emerge -pv`, for the reasons in
//! [0013](../../../docs/decisions): a fixed action column so a long plan can be
//! scanned rather than parsed, deltas rather than values so the eye goes to what
//! moves, the summary before the detail so the first question — how bad is this
//! — is answered without scrolling, and the cause of a replacement marked where
//! it happens instead of left to be inferred.
//!
//! This is a rendering of the typed plan, not a second way of describing one.
//! `--json` prints the same object; the moment these become two code paths they
//! start disagreeing.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use nerpa_core::plan::{Outdated, Unreachable};
use nerpa_core::secret::Recorded;
use nerpa_core::value::Value;
use nerpa_core::{BlastRadius, Change, ChangeKind, Confidence, Plan, StageKind, Target};

/// The whole plan, as text.
// ENFORCES: INV-AGENT-001
pub(crate) fn plan(plan: &Plan) -> String {
    if plan.is_empty() && plan.is_complete() && plan.outdated().is_empty() {
        return "nothing to do: the world already matches the configuration\n".to_owned();
    }

    let mut out = if plan.is_empty() {
        "nothing to do about what could be read\n".to_owned()
    } else {
        summary(plan.blast_radius())
    };
    out.push_str(&out_of_reach(plan.unreachable()));
    out.push_str(&behind(plan.outdated()));

    for (number, stage) in plan.stages().iter().enumerate() {
        let _ = writeln!(out, "\nstage {} · {}", number + 1, describe(stage.kind()));
        for change in stage.changes() {
            out.push_str(&one(change));
        }
    }

    if plan.blast_radius().has_irreversible() {
        out.push_str("\n! marks a change that destroys what is there now\n");
    }

    // Last, because it is about everything above it. A plan that could not look
    // at everything has none, and says so where the number would have been
    // rather than leaving a person to wonder.
    let _ = match plan.digest() {
        Ok(digest) => writeln!(out, "\nplan {digest}"),
        Err(problem) => writeln!(out, "\nno plan number: {problem}"),
    };
    out
}

/// What is running on something no longer declared.
///
/// Never among the stages, because it is not work and never will be until
/// somebody decides it is. Printed anyway, on every plan, because the failure
/// this exists to prevent is a fleet quietly years behind. See decision 0021.
// ENFORCES: INV-BIRTH-001
fn behind(outdated: &[Outdated]) -> String {
    if outdated.is_empty() {
        return String::new();
    }

    let mut out = format!(
        "\n  {} resources were made from something no longer declared:\n",
        outdated.len()
    );
    for entry in outdated {
        let born = entry
            .born_with()
            .map_or_else(|| "—".to_owned(), ToString::to_string);
        let declared = shown(entry.declared());
        let _ = writeln!(
            out,
            "    {} · {} was {born}, declared {declared}",
            entry.address(),
            entry.attribute()
        );
    }
    out.push_str(
        "  nothing is proposed: a running machine cannot be rebuilt underneath itself.\n  \
         replace them deliberately, or run a new slot beside the old one\n",
    );
    out
}

// ENFORCES: INV-REACH-003
/// What could not be looked at, and how old the last look was.
///
/// Printed with the summary rather than among the stages: it is not work, and a
/// reader scanning the action column must not find it there. This is also where
/// a plan says it cannot be signed, which is the half of the invariant that
/// lives here — the anchor used to sit on the function above, which renders
/// something else entirely.
fn out_of_reach(unreachable: &[Unreachable]) -> String {
    if unreachable.is_empty() {
        return String::new();
    }

    let mut nodes: BTreeMap<&str, Option<u64>> = BTreeMap::new();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for entry in unreachable {
        let node = entry.node().as_str();
        *counts.entry(node).or_default() += 1;
        nodes.entry(node).or_insert_with(|| entry.known_for());
    }

    let mut out = format!(
        "\n  {} resources could not be looked at, and nothing is claimed about them:\n",
        unreachable.len()
    );
    for (node, age) in nodes {
        let count = counts.get(node).copied().unwrap_or_default();
        let known = match age {
            Some(seconds) => format!("last seen {}", ago(seconds)),
            None => "never seen".to_owned(),
        };
        let _ = writeln!(
            out,
            "    {node} did not answer · {known} · {count} resources"
        );
    }
    out.push_str("  this plan cannot be signed: it describes a world nobody could look at\n");
    out
}

/// An age a person reads, not a number of seconds they have to divide.
fn ago(seconds: u64) -> String {
    match seconds {
        0..=90 => format!("{seconds}s ago"),
        91..=5400 => format!("{}m ago", seconds / 60),
        5401..=172_800 => format!("{}h ago", seconds / 3600),
        _ => format!("{}d ago", seconds / 86400),
    }
}

fn summary(radius: &BlastRadius) -> String {
    let mut parts = Vec::new();
    for (count, word) in [
        (radius.creates(), "to create"),
        (radius.updates(), "to update"),
        (radius.replaces(), "to replace"),
        (radius.deletes(), "to remove"),
        (radius.reacts(), "to rouse"),
    ] {
        if count > 0 {
            parts.push(format!("{count} {word}"));
        }
    }

    let mut out = format!("  {} changes: {}", radius.total(), parts.join(", "));
    let irreversible = radius.replaces() + radius.deletes();
    if irreversible > 0 {
        let _ = write!(out, " · {irreversible} irreversible");
    }
    out.push('\n');

    if !radius.nodes().is_empty() {
        let names: Vec<&str> = radius.nodes().iter().map(Target::as_str).collect();
        let count = radius.nodes().len();
        let noun = if count == 1 { "node" } else { "nodes" };
        let _ = writeln!(out, "  touches {count} {noun}: {}", names.join(", "));
    }

    match radius.unresolved() {
        0 => {}
        1 => out.push_str("  1 value will only be known once an earlier stage has run\n"),
        many => {
            let _ = writeln!(
                out,
                "  {many} values will only be known once earlier stages have run"
            );
        }
    }
    out
}

fn describe(kind: &StageKind) -> String {
    match kind {
        StageKind::Control => "control".to_owned(),
        StageKind::Target(nodes) => {
            let names: Vec<&str> = nodes.iter().map(Target::as_str).collect();
            format!("target: {}", names.join(", "))
        }
        StageKind::Mixed => "mixed".to_owned(),
        // These types are non-exhaustive on purpose, so a core that knows a kind
        // this renderer does not is a thing that can happen. Saying so beats
        // picking a label at random.
        _ => "unrecognised".to_owned(),
    }
}

/// The action column is fixed width on purpose: a plan is scanned down a page,
/// and a column that moves cannot be.
fn action(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Create => "[create   ]",
        ChangeKind::Update => "[update   ]",
        ChangeKind::Replace => "[replace !]",
        ChangeKind::Delete => "[remove  !]",
        // Not marked: `!` means the change destroys what is there, and a
        // restart destroys nothing. What makes it visible is the line under it
        // naming what roused it, and the count in the summary — diluting a mark
        // that means one thing would cost more than it bought. See 0026.
        ChangeKind::React => "[rouse    ]",
        _ => "[unknown !]",
    }
}

/// What a plan may say about a value it is about to write.
///
/// A secret says where it comes from and nothing about what it is. That is not a
/// diminished diff: which store a credential is read from is the part a person
/// approving the change actually needs.
fn shown(after: &Confidence) -> String {
    match after {
        Confidence::Known(value) => value.to_string(),
        Confidence::Unknown(reference) => format!("unknown until apply, from {reference}"),
        Confidence::Secret(sealed) => format!("(secret from {})", sealed.source()),
        // Every value it could take, because a person can approve a set and
        // cannot approve "unknown". See decision 0029.
        Confidence::Bounded(choice) => choice.to_string(),
        _ => "unrecognised".to_owned(),
    }
}

fn one(change: &Change) -> String {
    let mut out = format!("  {} {}\n", action(change.kind()), change.address());

    // Why, before what. A person reading a plan over two hundred machines is
    // entitled to know which file is about to bounce which service, and that
    // this is the question Ansible answers only by running. See decision 0026.
    for cause in change.roused_by() {
        let _ = writeln!(out, "               ↺ because {cause} changes");
    }

    for delta in change.deltas() {
        let before = delta
            .before()
            .map_or_else(|| "—".to_owned(), ToString::to_string);
        // A secret that was already there and is changing: the fact is the whole
        // report, because the value is the thing being withheld. See 0027.
        let after = match (delta.before(), delta.after()) {
            (Some(Recorded::Secret(_)), Confidence::Secret(_)) => "(secret, changed)".to_owned(),
            (_, after) => shown(after),
        };
        let cause = if delta.forces_replacement() {
            "   ← cause of the replacement"
        } else {
            ""
        };
        // A file's contents on one escaped line defeat the reason they are
        // rendered before anything happens. Anything with a newline in it goes
        // underneath, as itself.
        let blocks: Vec<(&str, &str)> = [
            ("was", block_of(delta.before())),
            ("becomes", after_block(delta)),
        ]
        .into_iter()
        .filter_map(|(label, text)| text.map(|text| (label, text)))
        .collect();

        if blocks.is_empty() {
            let _ = writeln!(
                out,
                "               ~ {:<14} {before} → {after}{cause}",
                delta.name()
            );
            continue;
        }

        let _ = writeln!(out, "               ~ {}{cause}", delta.name());
        for (label, text) in blocks {
            let _ = writeln!(out, "                 {label}:");
            out.push_str(&lines_of(text));
        }
    }
    out
}

/// How much of a file a plan prints before it stops being something anybody
/// reads.
const SHOWN_LINES: usize = 40;

/// The text of a value worth showing on its own lines, if it is one.
fn multiline(value: Option<&Value>) -> Option<&str> {
    match value {
        Some(Value::Text(text)) if text.contains('\n') => Some(text.as_str()),
        _ => None,
    }
}

fn block_of(before: Option<&Recorded>) -> Option<&str> {
    multiline(before.and_then(Recorded::plain))
}

fn after_block(delta: &nerpa_core::plan::AttributeDelta) -> Option<&str> {
    match delta.after() {
        Confidence::Known(value) => multiline(Some(value)),
        _ => None,
    }
}

/// A file's contents, indented, and cut off before they stop being readable.
fn lines_of(text: &str) -> String {
    let mut out = String::new();
    let all: Vec<&str> = text.lines().collect();
    for line in all.iter().take(SHOWN_LINES) {
        let _ = writeln!(out, "                   │ {line}");
    }
    if let Some(rest) = all.len().checked_sub(SHOWN_LINES).filter(|rest| *rest > 0) {
        let _ = writeln!(out, "                   │ … and {rest} more lines");
    }
    out
}

// The output is the product here, so it is asserted rather than eyeballed. Not
// character-for-character — that pins formatting nobody promised — but every
// claim the reader is entitled to make from it.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests may be blunt: see docs/CODING_STANDARDS.md"
)]
mod tests {
    use std::collections::BTreeMap;

    use nerpa_core::address::address;
    use nerpa_core::planner;
    use nerpa_core::provider::Attributes;
    use nerpa_core::state::{Record, State, Written};
    use nerpa_core::{
        ExecutionDomain, Graph, Kind, Observed, Provider, Reference, Resource, Schema, Target,
        Value,
    };

    use nerpa_core::secret::{Key, Recorded, Remembered, Secrets};

    use super::plan as render;

    /// A fixed key and a store holding nothing: rendering is what is under test,
    /// not where a secret comes from.
    static DIGEST_KEY: Key = Key::from_bytes([7; 32]);
    static NO_STORE: Remembered = Remembered::EMPTY;

    fn secrets() -> Secrets<'static> {
        Secrets::new(&DIGEST_KEY, &NO_STORE)
    }

    fn graph_of(resources: Vec<Resource>) -> Graph {
        let mut graph = Graph::new();
        for resource in resources {
            if let ExecutionDomain::Target(target) = resource.address().domain() {
                graph.preexisting(target);
            }
            graph.insert(resource).unwrap();
        }
        graph
    }

    fn observed_with(address_text: &str, attributes: &[(&str, Value)]) -> Observed {
        let mut observed = Observed::new();
        let mut recorded = BTreeMap::new();
        for (name, value) in attributes {
            recorded.insert((*name).to_owned(), value.clone());
        }
        observed.record(address(address_text), recorded);
        observed
    }

    fn rendered(graph: &Graph, observed: &Observed, state: &State, schema: &Schema) -> String {
        render(&planner::plan(graph, observed, state, schema, secrets()).unwrap())
    }

    /// The credential these two tests go looking for in the output.
    const PASSWORD: &str = "hunter2";

    static WITH_PASSWORD: std::sync::LazyLock<Remembered> =
        std::sync::LazyLock::new(|| Remembered::new().holding("db/password", PASSWORD));

    fn holding_a_secret() -> Secrets<'static> {
        Secrets::new(&DIGEST_KEY, &*WITH_PASSWORD)
    }

    fn secret_at(path: &str) -> nerpa_core::secret::Source {
        nerpa_core::secret::Source::Stored {
            provider: "remembered".to_owned(),
            path: path.to_owned(),
            version: None,
        }
    }

    fn with_a_secret() -> Graph {
        graph_of(vec![
            Resource::new(address("os:file.credentials@web-01"))
                .with("path", "/etc/app/credentials")
                .with("content", secret_at("db/password")),
        ])
    }

    /// The plan says which store a credential comes from, because that is what a
    /// person approving the change needs, and never what it is.
    #[test]
    fn a_secret_is_shown_as_where_it_comes_from() {
        let text = render(
            &planner::plan(
                &with_a_secret(),
                &Observed::new(),
                &State::new(),
                &Schema::new(),
                holding_a_secret(),
            )
            .unwrap(),
        );

        assert!(
            text.contains("(secret from remembered:db/password)"),
            "the plan should name the store: {text}"
        );
        assert!(!text.contains(PASSWORD), "the plan carries it: {text}");
    }

    /// A credential that was rotated is a change worth approving, and the fact of
    /// it is the whole report.
    #[test]
    fn a_secret_that_changed_says_so_and_no_more() {
        let observed = observed_with(
            "os:file.credentials@web-01",
            &[
                ("path", Value::Text("/etc/app/credentials".to_owned())),
                ("content", Value::Text("the-old-one".to_owned())),
            ],
        );

        let text = render(
            &planner::plan(
                &with_a_secret(),
                &observed,
                &State::new(),
                &Schema::new(),
                holding_a_secret(),
            )
            .unwrap(),
        );

        assert!(
            text.contains("(secret) → (secret, changed)"),
            "a rotation reads as one: {text}"
        );
        assert!(
            !text.contains(PASSWORD),
            "the plan carries the new one: {text}"
        );
        assert!(
            !text.contains("the-old-one"),
            "the plan carries the old one: {text}"
        );
    }

    #[test]
    fn a_settled_world_says_so_in_one_line() {
        let graph = graph_of(vec![
            Resource::new(address("yandex:instance.db")).with("cores", 8),
        ]);
        let observed = observed_with("yandex:instance.db", &[("cores", Value::Integer(8))]);

        let text = rendered(&graph, &observed, &State::new(), &Schema::new());
        assert!(text.contains("nothing to do"), "{text}");
        assert!(!text.contains("stage"), "there is nothing to stage: {text}");
    }

    #[test]
    fn the_summary_comes_before_the_detail_and_counts_each_kind() {
        let graph = graph_of(vec![
            Resource::new(address("yandex:instance.new")),
            Resource::new(address("yandex:instance.changed")).with("cores", 8),
        ]);
        let observed = observed_with("yandex:instance.changed", &[("cores", Value::Integer(4))]);

        let text = rendered(&graph, &observed, &State::new(), &Schema::new());
        let summary = text.lines().next().unwrap();

        assert!(summary.contains("2 changes"), "{summary}");
        assert!(summary.contains("1 to create"), "{summary}");
        assert!(summary.contains("1 to update"), "{summary}");
        assert!(
            !summary.contains("0 to"),
            "kinds with nothing in them are left out: {summary}"
        );
        assert!(
            text.contains("[update   ]"),
            "the action column names an update: {text}"
        );
        assert!(
            !summary.contains("irreversible"),
            "nothing here is: {summary}"
        );
        assert!(text.find("2 changes").unwrap() < text.find("stage").unwrap());
    }

    #[test]
    fn the_action_column_names_the_kind_and_marks_what_cannot_be_undone() {
        let graph = graph_of(vec![
            Resource::new(address("yandex:instance.db")).with("zone", "e"),
            Resource::new(address("yandex:disk.new")),
        ]);
        let observed = observed_with(
            "yandex:instance.db",
            &[("zone", Value::Text("a".to_owned()))],
        );
        let schema = Schema::new().immutable(
            Provider::new("yandex").unwrap(),
            Kind::new("instance").unwrap(),
            "zone",
        );

        let text = rendered(&graph, &observed, &State::new(), &schema);

        assert!(text.contains("[create   ]"), "{text}");
        assert!(text.contains("[replace !]"), "{text}");
        assert!(text.contains("1 irreversible"), "{text}");
        assert!(text.contains("! marks a change that destroys"), "{text}");
        assert!(
            text.contains("← cause of the replacement"),
            "the reason is named: {text}"
        );
        assert!(
            text.contains("\"a\" → \"e\""),
            "the delta shows both sides: {text}"
        );
    }

    #[test]
    fn a_removal_is_reported_as_one_and_counted_as_irreversible() {
        let mut state = State::new();
        state.record(
            address("yandex:instance.gone"),
            Record::new(
                Recorded::all(Attributes::new()),
                std::collections::BTreeSet::new(),
            ),
        );

        let text = rendered(&Graph::new(), &Observed::new(), &state, &Schema::new());
        assert!(text.contains("[remove  !]"), "{text}");
        assert!(
            text.contains("stage 1 · mixed"),
            "removals are not tied to one domain: {text}"
        );
        assert!(text.contains("1 to remove"), "{text}");
        assert!(text.contains("1 irreversible"), "{text}");
    }

    #[test]
    fn a_stage_says_which_machines_it_touches() {
        let graph = graph_of(vec![
            Resource::new(address("os:file.motd@web-01")).with("path", "/etc/motd"),
        ]);

        let text = rendered(&graph, &Observed::new(), &State::new(), &Schema::new());
        assert!(text.contains("stage 1 · target: web-01"), "{text}");
        assert!(text.contains("touches 1 node: web-01"), "{text}");
    }

    #[test]
    fn a_control_stage_says_control_rather_than_naming_a_machine() {
        let graph = graph_of(vec![Resource::new(address("yandex:instance.db"))]);

        let text = rendered(&graph, &Observed::new(), &State::new(), &Schema::new());
        assert!(text.contains("stage 1 · control"), "{text}");
        assert!(!text.contains("touches"), "nothing on a machine: {text}");
    }

    #[test]
    fn a_value_that_does_not_exist_yet_says_where_it_will_come_from() {
        let graph = graph_of(vec![
            Resource::new(address("yandex:instance.db")),
            Resource::new(address("yandex:instance.web")).with(
                "upstream",
                Reference::to(address("yandex:instance.db"), "ip"),
            ),
        ]);

        let text = rendered(&graph, &Observed::new(), &State::new(), &Schema::new());
        assert!(
            text.contains("unknown until apply, from yandex:instance.db.ip"),
            "{text}"
        );
        assert!(
            text.contains("1 value will only be known"),
            "singular: {text}"
        );
    }

    #[test]
    fn several_unknown_values_are_counted_in_the_plural() {
        let graph = graph_of(vec![
            Resource::new(address("yandex:instance.db")),
            Resource::new(address("yandex:instance.web"))
                .with("host", Reference::to(address("yandex:instance.db"), "ip"))
                .with("port", Reference::to(address("yandex:instance.db"), "port")),
        ]);

        let text = rendered(&graph, &Observed::new(), &State::new(), &Schema::new());
        assert!(text.contains("2 values will only be known"), "{text}");
    }

    #[test]
    fn what_could_not_be_looked_at_is_reported_with_its_age_and_never_as_work() {
        let graph = graph_of(vec![
            Resource::new(address("os:file.motd@web-01")).with("path", "/etc/motd"),
            Resource::new(address("yandex:instance.db")).with("cores", 8),
        ]);
        let mut observed = Observed::new();
        observed.unreachable(Target::new("web-01").unwrap());

        let mut state = State::new();
        state.record(
            address("os:file.motd@web-01"),
            Record::written_at(
                Recorded::all(Attributes::new()),
                std::collections::BTreeSet::new(),
                Written::at(1_700_000_000 - 7200),
            ),
        );

        let text = render(
            &planner::plan_at(
                &graph,
                &observed,
                &state,
                &Schema::new(),
                secrets(),
                Written::at(1_700_000_000),
            )
            .unwrap(),
        );

        assert!(text.contains("could not be looked at"), "{text}");
        assert!(text.contains("web-01 did not answer"), "{text}");
        assert!(
            text.contains("last seen 2h ago"),
            "the age is readable: {text}"
        );
        assert!(
            text.contains("cannot be signed"),
            "and the consequence is stated: {text}"
        );
        assert!(
            !text.contains("[create   ] os:file.motd"),
            "it is not work: {text}"
        );
        assert!(
            text.contains("[create   ] yandex:instance.db"),
            "the rest still is: {text}"
        );
    }

    /// Every band of the scale, and two resources on one machine so that counting
    /// them wrongly cannot pass unnoticed.
    #[test]
    fn ages_are_reported_in_units_a_person_reads() {
        for (seconds, expected) in [
            (45_u64, "45s ago"),
            (600, "10m ago"),
            (7200, "2h ago"),
            (400_000, "4d ago"),
        ] {
            let graph = graph_of(vec![
                Resource::new(address("os:file.one@web-01")).with("path", "/one"),
                Resource::new(address("os:file.two@web-01")).with("path", "/two"),
            ]);
            let mut observed = Observed::new();
            observed.unreachable(Target::new("web-01").unwrap());

            let mut state = State::new();
            state.record(
                address("os:file.one@web-01"),
                Record::written_at(
                    Recorded::all(Attributes::new()),
                    std::collections::BTreeSet::new(),
                    Written::at(1_700_000_000 - seconds),
                ),
            );

            let text = render(
                &planner::plan_at(
                    &graph,
                    &observed,
                    &state,
                    &Schema::new(),
                    secrets(),
                    Written::at(1_700_000_000),
                )
                .unwrap(),
            );

            assert!(
                text.contains(expected),
                "{seconds}s should read as {expected}: {text}"
            );
            // Asserted on the machine's own line, not on the total: the total
            // counts entries directly, so a broken per-machine counter slips
            // past a looser check. Mutation testing found exactly that.
            assert!(
                text.contains(&format!("{expected} · 2 resources")),
                "the machine's own line counts both: {text}"
            );
            assert!(
                text.contains("2 resources could not"),
                "and so does the total: {text}"
            );
        }
    }

    #[test]
    fn a_fleet_behind_its_declaration_is_told_about_and_never_planned() {
        let graph = graph_of(vec![
            Resource::new(address("yandex:instance.web")).with("image", "gentoo-2026-09"),
        ]);
        let mut observed = Observed::new();
        let mut attributes = BTreeMap::new();
        attributes.insert("image".to_owned(), Value::Text("gentoo-2026-01".to_owned()));
        observed.record(address("yandex:instance.web"), attributes);

        let schema = Schema::new().at_birth(
            Provider::new("yandex").unwrap(),
            Kind::new("instance").unwrap(),
            "image",
        );

        let text =
            render(&planner::plan(&graph, &observed, &State::new(), &schema, secrets()).unwrap());

        assert!(text.contains("no longer declared"), "{text}");
        assert!(text.contains("image was \"gentoo-2026-01\""), "{text}");
        assert!(text.contains("declared \"gentoo-2026-09\""), "{text}");
        assert!(text.contains("nothing is proposed"), "{text}");
        assert!(
            text.contains("run a new slot beside the old one"),
            "and says the way out: {text}"
        );
        assert!(
            !text.contains("[replace"),
            "nothing is being destroyed: {text}"
        );
        assert!(
            !text.contains("nothing to do:"),
            "it is not silence either: {text}"
        );
    }

    /// The image a machine should have been born from can itself be something
    /// that does not exist yet — built by an earlier stage. Being behind is still
    /// reported, and the report says where the answer will come from.
    #[test]
    fn a_fleet_behind_a_value_that_does_not_exist_yet_still_says_so() {
        let graph = graph_of(vec![
            Resource::new(address("yandex:image.base")),
            Resource::new(address("yandex:instance.web"))
                .with("image", Reference::to(address("yandex:image.base"), "id")),
        ]);

        let mut observed = Observed::new();
        let mut attributes = BTreeMap::new();
        attributes.insert("image".to_owned(), Value::Text("gentoo-2026-01".to_owned()));
        observed.record(address("yandex:instance.web"), attributes);

        let schema = Schema::new().at_birth(
            Provider::new("yandex").unwrap(),
            Kind::new("instance").unwrap(),
            "image",
        );

        let text =
            render(&planner::plan(&graph, &observed, &State::new(), &schema, secrets()).unwrap());

        assert!(text.contains("no longer declared"), "{text}");
        assert!(
            text.contains("declared unknown until apply, from yandex:image.base.id"),
            "the report says where the answer will come from: {text}"
        );
        assert!(
            !text.contains("[replace"),
            "and still proposes nothing: {text}"
        );
    }

    #[test]
    fn a_machine_never_seen_is_reported_as_never_seen() {
        let graph = graph_of(vec![Resource::new(address("os:file.motd@web-09"))]);
        let mut observed = Observed::new();
        observed.unreachable(Target::new("web-09").unwrap());

        let text = render(
            &planner::plan_at(
                &graph,
                &observed,
                &State::new(),
                &Schema::new(),
                secrets(),
                Written::at(1_700_000_000),
            )
            .unwrap(),
        );

        assert!(text.contains("never seen"), "{text}");
        assert!(
            text.contains("nothing to do about what could be read"),
            "{text}"
        );
    }

    #[test]
    fn two_machines_are_both_named() {
        let graph = graph_of(vec![
            Resource::new(address("os:file.motd@web-01")).with("path", "/etc/motd"),
            Resource::new(address("os:file.motd@web-02")).with("path", "/etc/motd"),
        ]);

        let text = rendered(&graph, &Observed::new(), &State::new(), &Schema::new());
        assert!(text.contains("touches 2 nodes: web-01, web-02"), "{text}");
    }
}
