use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "The lifecycle rules are carried through every update, and an empty list is the word for none",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Yandex)],
    context: r"
        A backup bucket wants its versions moved to cold storage and expired
        on a schedule, and Yandex names that schedule the lifecycle rules.
        Probes against the owner's folder wrote the rules the wire answers
        with: the rules apply at creation — unlike the website document, which
        the create ignores — and apply in an update; an update that omits them
        replaces them with nothing, so a change to a tag wipes the schedule;
        an empty list clears what is there, which is a word the website
        vocabulary lacks; and the read materializes the zero subfields of the
        wrapped values — a nil date as `0001-01-01`, the days as text — which
        the report has to fold back into the words a declaration speaks.
    ",
    decision: r"
        The declared vocabulary gains `lifecycle`: a list of rule maps. A rule
        carries an `id` — required, for the rule is compared as part of the
        whole list and a generated id would never converge — an `enabled`
        that defaults to true, an optional `prefix` the filter applies, a
        `transitions` list of `{days, storage_class}` for the current
        versions, a `noncurrent_transitions` list of the same shape for the
        versions that were replaced, and `expire_after_days` and
        `noncurrent_expire_after_days` for the deletions; a rule with none of
        the four actions is refused. The rules are carried from the read into
        every update body, declared or not, for the same reason the website
        document is: the wire replaces what the body omits, and what it omits
        here is the schedule. Declaring an empty list is the word for no
        rules: the body carries nothing, and what is there is cleared. The
        create carries the declared rules — the wire takes them there. The
        report folds the wire's zero subfields away, so what is read back is
        the shape the declaration speaks.
    ",
    trade_offs: &[
        "The cost of the carry is a body that grows by the schedule on every bucket update, read back before it is written; the alternative is a tag change that silently wipes a retention schedule, which the probe showed is exactly what the wire does.",
        "The rule is the whole document: the filter speaks one prefix, and the richer filters — sizes, tags, the and-operator — are out of the vocabulary for now, reported by nobody and refused by nothing; a limitation names them.",
        "The id is required where the wire would generate one, and the comparison is over the whole list in order: the cost is a declaration that must name what it means, the gain is a rule that converges.",
    ],
    constraints: &[
        "The lifecycle rules are carried from the read into every management update a bucket takes.",
        "An empty declared list is the word for no rules, and it clears them.",
        "No S3-door XML is opened for the rules; the management door speaks them.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 11, 6),
    breaking: Breaking::No,
    introduces_norm: false,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_002],
);
