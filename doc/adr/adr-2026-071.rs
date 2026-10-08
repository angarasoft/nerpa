use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "The yandex provider takes its credential from the yc profile, not a script",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Yandex), taxon!(Subsystem, Docs)],
    context: r"
        A run against Yandex Cloud needs a short-lived IAM token, twelve hours at
        most. Today the fleet names it as a literal `iam_token`, so a person
        obtains it first — `yc iam create-token`, wrapped in a script — and the
        wrapper is the part nobody wants: the machine already holds the credential.

        The yc CLI keeps a long-lived OAuth token in
        `~/.config/yandex-cloud/config.yaml`, and `yc iam create-token` is nothing
        but: read that token from the active profile, POST it to
        `https://iam.api.cloud.yandex.net/iam/v1/tokens`, and return the IAM token
        the answer carries. The config file is YAML on disk; the CLI can also print
        JSON (`--format json`), but that is the shell-out path — it keeps the CLI as
        a runtime dependency and merely moves the script inside nerpa.
    ",
    decision: r"
        The provider derives its own token. A new `Auth::YcProfile` reads the OAuth
        token from the yc config file's active profile and exchanges it for an IAM
        token over the IAM API — the same journey the wrapper script made, now
        native and without a `yc` dependency.

        `iam_token` becomes optional in the fleet: present, it is the static IAM
        token for CI and tests; absent, the credential is the yc profile. The config
        file is read with a full YAML parser — `serde-saphyr`, the pure-Rust serde
        serializer over the `saphyr` parser — taken as a workspace dependency and
        reused by later YAML readers rather than a one-off line extractor.
    ",
    trade_offs: &[
        "Native exchange against shelling out to `yc iam create-token --format json`: the shell-out parses JSON and reuses yc's profile logic, but keeps the CLI as a runtime dependency and spawns a process; the native path parses YAML and does one POST. The native path wins for a tool that runs where yc may not be installed.",
        "A personal OAuth token against a service-account key: the OAuth token is what yc already holds, so it costs nothing new but is a personal, long-lived credential; a service-account key is the automation-grade shape but needs JWT signing (a new dependency) and is a separate later work, already marked in the code.",
        "YAML parser choice: `serde-saphyr` is pure Rust, panic-free and serde-compatible, and the tree already favours dependencies without a C compiler (ureq over rustls, webpki-roots for cert data); `serde_yaml_ng` is the maintained serde-YAML fork but pulls `unsafe-libyaml`, a C library. The pure-Rust crate is chosen, and nothing needs the serde_yaml API, so there is no compatibility to keep.",
    ],
    constraints: &[
        "The OAuth token is read and exchanged at once, and never stored; only the short-lived IAM token is used for the run.",
        "`iam_token` remains the explicit static credential for CI and tests; nothing forces a personal profile on a machine that has none.",
        "The YAML parser is a workspace dependency, so the next YAML reader — Ansible inventory, for the dynamic inventory work — reuses it rather than re-deciding.",
        "Credential resolution stays inside the provider crate; no HTTP or credential type crosses into nerpa-core.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 10, 5),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
    introduces_norm: false,
);
