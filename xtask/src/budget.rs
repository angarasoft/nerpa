//! What a plan is allowed to cost, measured rather than assumed.
//!
//! Planning is arithmetic over declarations: no network, no disk, nothing that
//! could excuse it. So its cost is a property of the code, and a property of the
//! code is something a gate can hold.
//!
//! # Why a baseline as well as a budget
//!
//! The number this ought to cost and the number it costs today are different,
//! and writing down only one of them loses something. A budget alone fails on
//! the first run and stays red until somebody has time, at which point people
//! learn to ignore it. A baseline alone quietly blesses whatever the code does
//! now, which is how a cost becomes permanent.
//!
//! So both are recorded. The **baseline** blocks: a measurement above it is a
//! new regression, and it is refused. The **budget** reports: the gap between
//! the two is the work outstanding, printed on every run so that it is a number
//! somebody is looking at rather than an intention. Closing the gap means
//! lowering both, in the same commit that earns it.
//!
//! # Why this is not on the commit path
//!
//! It needs a release build, which is minutes on this dependency tree, and it
//! measures wall-clock time on whatever machine happens to be running it. That
//! belongs beside `mutants` — nightly, and by hand when the planner is touched —
//! for the reason given there: a gate people learn to skip is worse than no gate.

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use toml::Value;

use crate::shell;

/// How many times the measurement is taken before the fastest is believed.
///
/// Three rather than one because an independent review pointed out that a single
/// wall-clock sample on a shared machine reports the machine's mood as a
/// regression — the same false-positive failure this file's own header warns
/// about.
const SAMPLES: usize = 3;

/// What the registry records about one measurement.
#[derive(Debug, Clone, Copy)]
struct Recorded {
    resources: usize,
    budget: f64,
    baseline: f64,
}

/// Reads `docs/budgets.toml`.
fn read(text: &str) -> Result<Recorded, String> {
    let parsed: toml::Table =
        toml::from_str(text).map_err(|error| format!("docs/budgets.toml: {error}"))?;
    let plan = parsed
        .get("plan")
        .and_then(Value::as_table)
        .ok_or_else(|| "docs/budgets.toml: no [plan] section".to_owned())?;

    let number = |name: &str| -> Result<f64, String> {
        plan.get(name)
            .and_then(Value::as_float)
            .ok_or_else(|| format!("docs/budgets.toml: [plan] has no float {name}"))
    };
    let resources = plan
        .get("resources")
        .and_then(Value::as_integer)
        .and_then(|count| usize::try_from(count).ok())
        .ok_or_else(|| "docs/budgets.toml: [plan] has no integer resources".to_owned())?;

    let recorded = Recorded {
        resources,
        budget: number("budget_seconds")?,
        baseline: number("baseline_seconds")?,
    };

    if recorded.budget > recorded.baseline {
        return Err(
            "docs/budgets.toml: the budget is above the baseline, which would make \
             the baseline unreachable rather than a thing being burned down"
                .to_owned(),
        );
    }
    Ok(recorded)
}

/// The verdict on one measurement, with no clock and no filesystem in sight.
///
/// `Err` is a regression against what the code already did. `Ok` carries the
/// line to print, which says how far the gap still is when there is one.
fn judge(measured: f64, recorded: Recorded) -> Result<String, String> {
    let Recorded {
        resources,
        budget,
        baseline,
    } = recorded;

    if measured > baseline {
        return Err(format!(
            "planning {resources} resources took {measured:.2}s, and the recorded \
             baseline is {baseline:.2}s.\nThis is a regression, not a threshold to \
             raise: something made the planner slower. Raise the baseline only with \
             a changelog entry saying what was bought with it."
        ));
    }

    if measured > budget {
        let times = measured / budget;
        return Ok(format!(
            "  budget: {resources} resources planned in {measured:.2}s \
             — within the {baseline:.2}s baseline, {times:.1}x over the {budget:.2}s budget"
        ));
    }

    Ok(format!(
        "  budget: {resources} resources planned in {measured:.2}s, inside the \
         {budget:.2}s budget — lower both numbers"
    ))
}

/// A configuration of `resources` files, all reading one value from one
/// directory.
///
/// Deliberately the cheapest shape there is: one edge per resource and nothing
/// to resolve. Anything this costs is the planner walking its own graph, which
/// is what the budget is about.
fn configuration(resources: usize, root: &str) -> String {
    let mut text = String::from("node(\"localhost\")\n");
    let _ = writeln!(
        text,
        "root = resource(\"os:directory.root@localhost\", {{\"path\": \"{root}/out\"}})"
    );
    for index in 0..resources {
        let _ = writeln!(
            text,
            "resource(\"os:file.f{index}@localhost\", \
             {{\"path\": \"{root}/out/f{index}\", \"content\": root.path}})"
        );
    }
    text
}

/// Builds the release binary and times one plan over a generated configuration.
fn measure(resources: usize) -> Result<f64, String> {
    shell::cargo(&[
        "build",
        "--release",
        "--bin",
        "nerpa",
        "--bin",
        "nerpa-executor",
    ])?;

    let directory = shell::root().join("target/tmp/budget");
    fs::create_dir_all(&directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    let shown = directory.to_string_lossy().replace('\\', "/");

    let source = directory.join("budget.star");
    fs::write(&source, configuration(resources, &shown))
        .map_err(|error| format!("{}: {error}", source.display()))?;
    let state = directory.join("budget.state.json");
    let _ = fs::remove_file(&state);

    let binary: PathBuf = shell::target_dir().join("release/nerpa");
    let mut command = std::process::Command::new(&binary);
    command
        .arg("-f")
        .arg(&source)
        .arg("--state")
        .arg(&state)
        // A run needs an actor identity (`INV-TRUST-001`), and this one is the
        // harness measuring itself, so the name says what it is.
        .arg("--as")
        .arg("budget@example.invalid")
        .arg("plan")
        .stdout(std::process::Stdio::null());

    // Three runs, and the fastest is the answer. Noise on a shared machine only
    // ever adds time — another process took the core, the file cache was cold —
    // so the lowest sample is the closest thing to what the code actually costs.
    // One sample would make this gate report the machine's mood as a regression,
    // which is how a gate teaches people to ignore it.
    let mut best = f64::INFINITY;
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let status = command
            .status()
            .map_err(|error| format!("could not run {}: {error}", binary.display()))?;
        let elapsed = started.elapsed().as_secs_f64();

        if !status.success() {
            return Err(format!(
                "{} could not plan the generated configuration",
                binary.display()
            ));
        }
        best = best.min(elapsed);
    }
    Ok(best)
}

/// Measures the planner against what `docs/budgets.toml` records.
pub(crate) fn check() -> Result<(), String> {
    let path = shell::root().join("docs/budgets.toml");
    let text = fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let recorded = read(&text)?;

    let measured = measure(recorded.resources)?;
    eprintln!("{}", judge(measured, recorded)?);
    Ok(())
}

// Both directions of the verdict are shown a fixture. A gate nobody has watched
// refuse anything is a gate nobody has grounds to trust.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "tests may be blunt: see docs/CODING_STANDARDS.md"
)]
mod tests {
    use super::{Recorded, configuration, judge, read};

    const RECORDED: Recorded = Recorded {
        resources: 4000,
        budget: 0.5,
        baseline: 4.0,
    };

    #[test]
    fn a_measurement_above_the_baseline_is_a_regression() {
        let report = judge(4.01, RECORDED).unwrap_err();
        assert!(report.contains("regression"), "{report}");
        assert!(report.contains("4.01s"), "{report}");
    }

    #[test]
    fn the_baseline_is_a_ceiling_rather_than_a_target() {
        // Exactly at the recorded number is not yet worse than it. Mutation
        // testing cannot tell `>` from `>=` anywhere except here.
        assert!(judge(4.0, RECORDED).is_ok());
    }

    #[test]
    fn the_gap_to_the_budget_is_reported_rather_than_refused() {
        let report = judge(3.5, RECORDED).unwrap();
        assert!(report.contains("7.0x over"), "{report}");
    }

    #[test]
    fn inside_the_budget_asks_for_both_numbers_to_come_down() {
        let report = judge(0.4, RECORDED).unwrap();
        assert!(report.contains("lower both"), "{report}");
    }

    #[test]
    fn a_budget_above_its_own_baseline_is_refused_as_unreachable() {
        let registry = "[plan]\nresources = 10\nbudget_seconds = 9.0\nbaseline_seconds = 1.0\n";
        let report = read(registry).unwrap_err();
        assert!(report.contains("unreachable"), "{report}");
    }

    #[test]
    fn a_registry_that_is_sound_reads_back_what_it_says() {
        let registry = "[plan]\nresources = 40\nbudget_seconds = 0.1\nbaseline_seconds = 1.0\n";
        let recorded = read(registry).unwrap();
        assert_eq!(recorded.resources, 40);
    }

    #[test]
    fn the_generated_configuration_declares_what_it_promises() {
        let text = configuration(3, "/tmp/x");
        assert_eq!(text.matches("os:file.f").count(), 3, "{text}");
        assert!(text.contains("root.path"), "{text}");
    }
}
