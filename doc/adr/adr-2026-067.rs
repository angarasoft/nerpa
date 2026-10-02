use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "Sign a plan with OpenSSH's signature format, and verify with ssh-keygen",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Cli)],
    context: r"
        0009 requires that where it is configured, `apply` holds a detached
        signature over the plan hash from somebody other than whoever proposed
        it, and names no mechanism. 0010's correction records what the tree
        actually has: there is no signing primitive in the dependency tree, and
        nothing signs anything. The mechanism is therefore open, and the two
        shapes are a signature scheme brought into the tree — a crate, key
        formats, key generation, and a second place keys live — or a signature
        format somebody already holds keys for and already trusts.

        The identity story decided the tie. 0009 rides OIDC, Teleport or
        workload identity for who acts, and the fleet's key belongs to ssh: a
        second key ecosystem beside ssh is a second thing every deployment has
        to provision, and a second place a long-lived key could hide from the
        half of `INV-TRUST-001` that says there is no code path that reads one.
        Git's `allowed_signers` is the prior art for exactly this shape: who may
        sign is a file of the deployment's, and what is signed is chosen by the
        verifier.
    ",
    decision: r"
        The detached signature is an OpenSSH signature over the plan's number:
        made with `ssh-keygen -Y sign`, verified with `ssh-keygen -Y verify` in
        the namespace `plan@nerpa.run` against an `allowed_signers` file the
        fleet document names. The approval document wraps it — the signer's
        principal, the plan it approves, and the signature block — so a refusal
        can name who signed, and an apply whose plan moved since the signature
        can be refused by comparing the plan that was signed with the one the
        run computed, naming what moved.

        What 0009 called the plan hash is, since `INV-PLAN-006`, two numbers
        rather than one: the plan's number, which covers what the plan intends
        and whose plan it is and is what gets signed; and the premise, which
        covers what the plan was computed from and is checked where the work
        happens. The signature cannot be replayed against a changed world
        because a changed world moves one of the two, and the premise is what
        refuses it naming what moved.

        nerpa signs nothing and reads no private key. `nerpa sign` hands the
        approver's key to `ssh-keygen` — which may hold the private half in
        `ssh-agent`, the key file being only a public one — exactly as the
        transport hands hosts to `ssh`.
    ",
    trade_offs: &[
        "Verification needs ssh-keygen beside ssh: the same OpenSSH suite the transport already requires, so nothing new is provisioned. What is given up is control of the signature format — it is OpenSSH's to change and this project's to follow.",
        "Against a signing crate in the tree, which would make verification self-contained and add a cryptographic dependency tree, key formats and key generation to a dependency list kept deliberately shallow, plus a second place keys live.",
        "The approvers file is a trust root somebody maintains by hand, which is the same cost allowed_signers has everywhere and the same first five minutes 0009 already accepted for identities. And 0009's one hash is now honestly two numbers, which splits the claim across the signature and the premise check — a reader has to know both halves to know what an approval covers.",
    ],
    constraints: &[
        "nerpa reads no private key, in any code path: the static-key half of INV-TRUST-001 holds (0009).",
        "The signed message is the plan's number as lowercase hexadecimal and nothing else, in the namespace `plan@nerpa.run`.",
        "Whoever proposed a plan cannot sign it, and `apply` refuses before anything is applied (`INV-TRUST-002`).",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 10, 2),
    breaking: Breaking::No,
    code_refs: &[crate::anchor::plan_approval],
    related_rfcs: &[],
);
