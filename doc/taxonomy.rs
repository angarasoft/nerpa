//! The subsystems of nerpa and the team that owns them.
//!
//! The subsystem axis is the unit of ownership, conformance level and radius; its
//! values, lower-cased, are the scopes of commit subjects (`[FEAT](core): …`), and
//! the gate checks them against this file in the commit's tree.
//!
//! The split follows the crate layout: a subsystem is the boundary behind which a
//! separate decision about how a layer is built stands. `Pilot` is the thrust
//! `dacc_pilot` — the pilot's own protocol, measurements and findings — kept
//! apart so that a commit scope for it exists without touching a product crate.

dacc_core::declare_taxonomy! {
    Subsystem => [Core, Config, Proto, Executor, Transport, Os, Cli, Gate, Docs, Pilot],
    Team => [Maintainers],
}

// nerpa has one publication channel: everything is public-facing.
dacc_core::declare_channels! {
    Public,
}
