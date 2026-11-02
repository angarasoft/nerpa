use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-rustls-advisories",
    title: NonEmptyStr::new("The dependency advisories and the yanked clipboard-win leave the lock and the wasmtime features"),
    slice: crate::slice::s_develop_rejoined,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("the advisories RUSTSEC-2026-0346 to -0349 on rustls 0.23.45, RUSTSEC-2026-0382 on rustls-webpki, RUSTSEC-2026-0381 on webpki-roots, RUSTSEC-2026-0353 to -0364, -0378, -0379 and -0422 to -0424 on wasmtime 49.0.2, RUSTSEC-2026-0391, -0398 and -0403 on zstd and zstd-safe, RUSTSEC-2026-0410 on zstd-sys, and the yanked clipboard-win 5.4.1 turn cargo deny red, so the gate cannot pass on any tree that carries this lock; the lock moves to the patched releases, and the zstd and zstd-safe advisories, which have no fix in the 0.13 line that wasmtime requires, are removed by building wasmtime without the compile cache nothing here uses; upkeep, not a decision"),
    },
    taxon: taxon!(Subsystem, Gate),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("Cargo.lock names no crate with an open advisory and no yanked crate, wasmtime is built without its compile cache so no zstd crate is in the tree, and cargo deny check passes."),
);
