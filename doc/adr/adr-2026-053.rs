use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "The gate's composition on nerpa: dacc's steps around cargo xtask ci",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Gate), taxon!(Subsystem, Pilot)],
    context: r"
        The gate is thirteen steps: external names, markdown links, the journal,
        the project command, fmt, clippy, build and tests, the error-code probe,
        the attacks, docs, MSRV, MSRV attacks, and deny. Measured on nerpa at
        dacc 40e9905: external names not run (no list); markdown links failed
        on a gitignored .venv-docs until it moved outside the tree; attacks
        failed at the default floor 36 because nerpa has no compile_fail attacks;
        MSRV failed while rust-version was 1.98 and only 1.98.1 was installed;
        deny failed until the git dependencies carried a version field.
    ",
    decision: r"
        The gate runs cargo xtask ci as the delegated project step, with
        doctest_floor = 0 until the local invariant type lands, the commit types
        FEAT FIX PERF CHANGE DOCS TEST CI CHORE ADR PLAN, a subject limit of 90,
        and rust-version 1.98.1 so the MSRV step builds on the pinned toolchain.
        The documentation venv lives outside the tree, and the git dependencies
        carry a version field so deny sees a version rather than a wildcard.

        After the fixes the gate reports GATE OK (11 of 12; doctest 0; not run:
        external names).
    ",
    trade_offs: &[
        "gate_command duplicates fmt, clippy and the build the gate's own steps also run; the duplication is a finding for dacc, accepted rather than left unstated.",
        "doctest_floor 0 records honestly that nothing is attack-tested yet; the floor rises only when the local invariant type lands in wave 3.",
        "A git dependency without a version is a wildcard to cargo-deny; the version field is the cost of the pin.",
    ],
    constraints: &[
        "The documentation venv stays outside the tree; the gate walks everything except target/ and .git/.",
        "doctest_floor is zero until the local invariant type produces compile_fail attacks.",
        "rust-version and rust-toolchain.toml name the same version.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 28),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_001],
);
