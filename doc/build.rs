//! Scans the registry — decisions, specifications, thrusts, slices, work items
//! and the journal — and writes the reference modules to `OUT_DIR`.
//!
//! The document types are named here explicitly. A missing directory for a named
//! type is a build error, not an empty registry: a typo in a path must not turn
//! into a registry with no records.
//!
//! A build script has one job and no fallback: if it cannot read the registry or
//! write `OUT_DIR` it must stop loudly, so `unwrap` is the right tool here and a
//! `cargo::` line is the only channel to cargo.
//!
//! The pilot's promises are held here as well (adr-2026-068): the reference
//! constants, the journal fold and the radius ceiling are the sites the
//! invariant records name, so the site marks carry their anchors.

#![allow(
    clippy::unwrap_used,
    clippy::print_stdout,
    reason = "a build script reports and stops"
)]

use dacc_scan::ScanError;
use dacc_scan::anchors::ScannedAnchor;
use dacc_scan::plan::{
    INVARIANTS, LIMITATIONS, REVIEWS, SLICES, THRUSTS, WORK, emit_limitation_checks, emit_plan,
    emit_work_checks, scan_plan,
};
use std::{env, fs, path::PathBuf};

/// The reference constants of the registry: a record that names a document
/// that no longer exists does not compile (INV-PILOT-001).
#[dacc_derive::doc_anchor(id = "a-reference-is-a-path-to-a-generated-constant")]
fn decision_references(decisions: &[dacc_scan::ScannedSlug]) -> String {
    dacc_scan::emit_refs(decisions)
}

/// The journal fold: a landing is refused without the gate proof recorded on
/// the same tree (INV-PILOT-002).
#[dacc_derive::doc_anchor(id = "the-journal-fold-refuses-a-landing-without-its-gate")]
fn journal_fold(
    journal_toml: &std::path::Path,
    journal_dir: &std::path::Path,
    work: &[dacc_scan::ScannedSlug],
) -> Result<String, ScanError> {
    dacc_scan::journal::scan_journal(journal_toml, journal_dir, work)
}

/// The radius ceiling, asserted over every work item (INV-PILOT-005).
#[dacc_derive::doc_anchor(id = "the-radius-ceiling-is-asserted-over-every-work-item")]
fn radius_ceiling(work: &[dacc_scan::ScannedSlug]) -> String {
    emit_work_checks(work)
}

/// The anchors of the registry: `#[doc_anchor]` markup in the product code, the
/// gate and this build, each id a constant a record can name.
fn scanned_anchors(
    root: &std::path::Path,
    this: &std::path::Path,
) -> Result<Vec<ScannedAnchor>, ScanError> {
    dacc_scan::anchors::scan_anchors(&[&root.join("crates"), &root.join("xtask"), this])
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());

    let decisions = unwrap_scan(dacc_scan::scan_decisions(&manifest.join("adr")));
    fs::write(out.join("adr.rs"), decision_references(&decisions)).unwrap();

    let specs = unwrap_scan(dacc_scan::scan_specs(&manifest.join("rfc")));
    fs::write(out.join("rfc.rs"), dacc_scan::emit_spec_refs(&specs)).unwrap();

    let thrusts = unwrap_scan(scan_plan(&manifest.join("thrust"), &THRUSTS));
    let slices = unwrap_scan(scan_plan(&manifest.join("slice"), &SLICES));
    let work = unwrap_scan(scan_plan(&manifest.join("work"), &WORK));
    let limitations = unwrap_scan(scan_plan(&manifest.join("limitation"), &LIMITATIONS));
    let invariants = unwrap_scan(scan_plan(&manifest.join("invariant"), &INVARIANTS));
    let reviews = unwrap_scan(scan_plan(&manifest.join("review"), &REVIEWS));
    let mut plan = emit_plan(&thrusts, &THRUSTS);
    plan.push_str(&emit_plan(&slices, &SLICES));
    plan.push_str(&emit_plan(&work, &WORK));
    plan.push_str(&emit_plan(&limitations, &LIMITATIONS));
    plan.push_str(&emit_plan(&invariants, &INVARIANTS));
    plan.push_str(&emit_plan(&reviews, &REVIEWS));
    plan.push_str(&radius_ceiling(&work));
    plan.push_str(&emit_limitation_checks(&limitations));
    // Deliberately not generated: `emit_toil_checks`, dacc 0.5.0's norm that a
    // Toil work cannot declare a radius above Local. Six work items landed
    // before the norm existed declaring exactly that pair, and their records
    // say what they did rather than what a later norm would have allowed.
    // The norm holds for everything registered from here on — see W-032.
    fs::write(out.join("plan.rs"), plan).unwrap();

    // The journal folds into states at build time: a machine violation does not
    // compile and names the event file. Both shapes are read since 0.5.0 — the
    // single file the CLI writes and the directories older events sit in — so
    // the fold sees one history rather than two (W-031).
    let journal = unwrap_scan(journal_fold(
        &manifest.join("journal.toml"),
        &manifest.join("journal"),
        &work,
    ));
    fs::write(out.join("journal.rs"), journal).unwrap();

    // Code anchors: a decision names a code fragment, and the scan turns the
    // marker in the product's source into a reference constant.
    let anchors = unwrap_scan(scanned_anchors(manifest.parent().unwrap(), &manifest));
    fs::write(
        out.join("anchor.rs"),
        dacc_scan::anchors::emit_anchor_refs(&anchors),
    )
    .unwrap();

    for dir in [
        "adr",
        "rfc",
        "thrust",
        "slice",
        "work",
        "journal",
        "limitation",
        "invariant",
        "review",
    ] {
        println!("cargo::rerun-if-changed={dir}");
    }
    println!("cargo::rerun-if-changed=journal.toml");
    println!("cargo::rerun-if-changed=lib.rs");
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=../crates");
    println!("cargo::rerun-if-changed=../xtask");
}

fn unwrap_scan<T>(r: Result<T, ScanError>) -> T {
    match r {
        Ok(v) => v,
        Err(e) => {
            println!("cargo::error=dacc: {e}");
            std::process::exit(1);
        }
    }
}
