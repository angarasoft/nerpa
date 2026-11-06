use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "A bucket's website is carried through every update, and the anonymous flags open it to the public",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Yandex)],
    context: r"
        A static site served from a bucket needs two settings: the website
        document — an index page and an error page — and public access, which
        Yandex spells as anonymous access flags rather than an S3 policy. Both
        live on the management door the provider already speaks, and both
        read back in the full view. Probes against the owner's folder showed
        the shapes and three findings: the pair applies in a single patch; the
        anonymous flags survive an update that omits them, while the website
        document does not — the wire materializes an absent message and
        replaces the document whole, so a change to a tag wipes the site's
        index and error pages; the document cannot be emptied on purpose, for
        the validation demands an index when no redirect is named; and the
        create ignores the website document altogether — the flags it takes,
        the pages it silently drops.
    ",
    decision: r"
        The declared vocabulary gains two attributes, both managed through the
        management door. `website` is a map of `index` — required, the wire
        refuses a document without one — and `error`; `anonymous_access` is
        the map of `read`, `list` and `config_read` flags, and it is the whole
        document: a flag the declaration does not name reads as false. The
        website document is carried from the read into every update body,
        declared or not, for the same reason the object lock is: the wire
        applies what the body carries and replaces what it omits with nothing,
        and what is omitted here is the site. The anonymous flags are carried
        only where declared — the wire leaves them alone when they are absent.
        A site with a redirect and neither page is neither carried nor
        managed; a limitation names it. The create does not carry the site —
        the wire ignores the document there — so a site joins a bucket
        through its first update, which carries the enabled word the
        validation demands: a site declared on a bucket whose versioning is
        neither enabled nor declared meets the guard that refuses the silent
        flip, by name.
    ",
    trade_offs: &[
        "The cost of the carry is a body that grows by one document on every bucket update, read back before it is written; the alternative is a tag change that silently kills a production site, which the probe showed is exactly what the wire does.",
        "The flags are the whole document, so a declaration that names one flag turns the unnamed ones off. Accepted because the wire's message is atomic — there is no way to send `keep the rest as it is` — and a half-named policy is a trap the vocabulary refuses to hide.",
        "There is no word for `no site`: the door cannot clear a document on purpose (an empty one is refused), and the carry protects what is there. Removing the declaration stops the managing and keeps the site, the same shape as the versioning vocabulary.",
        "The cost of the create's silence is a coupling: a site on a bucket without versioning is refused until versioning is declared or enabled, because the update that carries the site carries the enabled word too. Accepted because the refusal is named and the alternative is the silent flip the guard exists to prevent.",
    ],
    constraints: &[
        "The website document is carried from the read into every management update a bucket takes.",
        "The anonymous flags are written only where declared; the S3 door is not opened for the pair.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 11, 6),
    breaking: Breaking::No,
    introduces_norm: false,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_002],
);
