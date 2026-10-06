use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-budget-target-dir",
    title: NonEmptyStr::new("The budget and live runs read built binaries from the configured target directory"),
    slice: crate::slice::s_gate_upkeep,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "moving the build output to .cargo-target left the gate's readers pointing at target/, which broke the budget run, and the budget's plan run gained an actor requirement; fixing both is routine gate maintenance with no product-code trace"
        ),
    },
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new(
        "xtask reads built binaries from the directory `.cargo/config.toml` names rather than a hardcoded `target/`, so `cargo xtask budget` and `cargo xtask live` find what they built; the budget passes `--as budget@example.invalid` so the plan run starts (INV-TRUST-001)."
    ),
);
