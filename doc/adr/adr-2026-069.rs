use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "The first cloud is Yandex Cloud",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Yandex), taxon!(Subsystem, Docs)],
    context: r"
        Stage 3 is the cloud: a control-domain provider, and a way in from
        Terraform. Its outcome is a real environment moved across without
        recreating a single resource, an empty plan right after import, and an
        import that names every secret it found in clear text in the old state.
        None of that can be demonstrated without a real cloud, because the stage
        is declared finished only when its exit criterion passes on real
        hardware.

        The choice of cloud is not among the things this project can leave open:
        a provider is built against one API, and its recorded-exchange tests are
        recorded against one account. The cloud named here is the one the
        project tests on.
    ",
    decision: r"
        The first control-domain provider is `yandex`, reaching the Yandex Cloud
        REST API. It owns `instance`, `network` and `subnet` to begin with,
        because those are what a Terraform environment on Yandex Cloud is made
        of, and the import path has to see them all to move one across without
        recreating it.

        The provider reaches the API synchronously, from where the engine runs,
        because a control-domain resource has no machine and the engine is
        synchronous. It ships a fake, and its tests are recorded HTTP exchanges,
        as the stage's outcome demands.
    ",
    trade_offs: &[
        "Yandex Cloud is one cloud, and the first provider is shaped by it; the provider interface stays the same for the next cloud, so the cost is one kind of resource, not one interface.",
        "Reaching the API synchronously keeps the engine single-threaded and a provider a plain read/prepare/conclude call; the cost is that a slow API call blocks the run, accepted because control reads are batched like any other.",
    ],
    constraints: &[
        "A credential for Yandex Cloud travels as a secret reference, never as a value in state or a plan.",
        "The provider ships a fake, and its tests are recorded exchanges, not live calls.",
        "The address is `yandex:instance.<name>`, `yandex:network.<name>`, `yandex:subnet.<name>`; the `yandex` name already appears in the address tests.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 10, 5),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
    introduces_norm: false,
);
