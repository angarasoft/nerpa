use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "The Yandex provider stays on REST, and the develop line's gRPC client is not carried forward",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Yandex), taxon!(Subsystem, Pilot)],
    context: r"
        Two lines of this tree built a Yandex Cloud provider apart from 2026-10-05.
        The main line's crate speaks REST over ureq (adr-2026-070), implements
        Provider::act and shares its attribute vocabulary with nerpa-import, but
        cannot yet create an instance in a real cloud: its create request carries
        no boot disk or network interface, it does not wait for an operation, and
        it patches a running instance without stopping it. The develop line's crate
        speaks gRPC over tonic, was exercised against a real folder and holds a
        service-account key flow, but its calls live in prepare, where the engine's
        control path, which calls act, does not reach them, and it brings tokio,
        tonic, prost and prebuilt protoc binaries to a tool that 0031 made
        synchronous.
    ",
    decision: r"
        The main line's crate is the base and adr-2026-070 stands. The develop
        line's client, protos, resources, credentials module and recorded gRPC
        exchanges leave the tree at the rejoining merge and remain in its history.
        adr-2026-076 and adr-2026-077 are superseded by this decision. What the
        develop line proved is ported over REST as declared works: the
        service-account key exchange and the refusal of long-lived credentials that
        adr-2026-075 asks for, redacted Debug, the full create request, operation
        polling, stop-update-start, the disk, security group and bucket kinds, and
        exchanges recorded again from a real folder.
    ",
    trade_offs: &[
        "Keeping REST keeps the dependency tree small and leaves the engine and nerpa-import as they are; the cost is that the only live recordings are gRPC-shaped, cannot be replayed over REST, and must be recorded again against a real cloud.",
        "The develop line's kinds and behaviours leave the tree until they are ported; its history stays as the reference.",
        "Until the ported works land the provider still cannot create an instance in a real cloud, as on the main line before the merge; a limitation declares it.",
    ],
    constraints: &[
        "No gRPC, tokio or protoc dependency enters the Yandex provider without a decision that supersedes this one.",
        "Until w-yc-credential-model lands, a limitation declares that adr-2026-075's refusal is held by no test.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 11, 2),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_002],
    introduces_norm: false,
);
