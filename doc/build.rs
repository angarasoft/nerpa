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

#![allow(clippy::unwrap_used, clippy::print_stdout)]

use slipway_scan::ScanError;
use slipway_scan::plan::{SLICES, THRUSTS, WORK, emit_plan, emit_work_checks, scan_plan};
use std::{env, fs, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());

    let decisions = unwrap_scan(slipway_scan::scan_decisions(&manifest.join("adr")));
    fs::write(out.join("adr.rs"), slipway_scan::emit_refs(&decisions)).unwrap();

    let specs = unwrap_scan(slipway_scan::scan_specs(&manifest.join("rfc")));
    fs::write(out.join("rfc.rs"), slipway_scan::emit_spec_refs(&specs)).unwrap();

    let thrusts = unwrap_scan(scan_plan(&manifest.join("thrust"), &THRUSTS));
    let slices = unwrap_scan(scan_plan(&manifest.join("slice"), &SLICES));
    let work = unwrap_scan(scan_plan(&manifest.join("work"), &WORK));
    let mut plan = emit_plan(&thrusts, &THRUSTS);
    plan.push_str(&emit_plan(&slices, &SLICES));
    plan.push_str(&emit_plan(&work, &WORK));
    plan.push_str(&emit_work_checks(&work));
    fs::write(out.join("plan.rs"), plan).unwrap();

    // The journal folds into states at build time: a machine violation does not
    // compile and names the event file.
    let journal = unwrap_scan(slipway_scan::journal::scan_journal(
        &manifest.join("journal"),
        &work,
    ));
    fs::write(out.join("journal.rs"), journal).unwrap();

    for dir in ["adr", "rfc", "thrust", "slice", "work", "journal"] {
        println!("cargo::rerun-if-changed={dir}");
    }
}

fn unwrap_scan<T>(r: Result<T, ScanError>) -> T {
    match r {
        Ok(v) => v,
        Err(e) => {
            println!("cargo::error=slipway: {e}");
            std::process::exit(1);
        }
    }
}
