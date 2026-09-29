use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "Separate what a resource is born with from what it must remain",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Core)],
    context: r"## Context

A fleet is built from an image, configured, and left running. Later the image
changes — a security fix, a new base, a rebuild. In Terraform this makes every
machine want to be destroyed and made again, because `image_id` is marked as
forcing replacement, and that marking is physically honest: the boot disk of a
running machine cannot be swapped underneath it.

The question is asked in the wrong place. **An image is not a property the
machine must keep matching. It is a property of how the machine was made.** Once
it is up and configuration has been applied to it, the image it was born from is
history.

Terraform cannot express that distinction, because everything in its state is one
kind of attribute: declared is compared with actual, they differ, so something
must happen. Its answer is `lifecycle { ignore_changes = [...] }` — a per-resource
escape hatch somebody has to remember, easy to over-apply, and which silences
drift detection for that attribute entirely. That is how a fleet ends up three
years behind with nobody aware of it.
",
    decision: r#"
A third kind of attribute, alongside the two that exist:

| Kind | What a difference means |
| --- | --- |
| converged | drift, corrected in place |
| immutable | corrected by replacing the resource |
| **at birth** | **not corrected, and reported every time** |

The reporting is the whole difference from `ignore_changes`. A plan never
proposes to act on a birth attribute, and it always says how many resources were
made from something that is no longer declared. Silent ignoring and a stated "I
know, not now" are not the same thing.

Moving a fleet onto a new image is therefore a **deliberate act**, not a
consequence of a plan run for some other reason. Two ways, and both already
exist in the model: replace on demand by selector, or run the two side by side
using the slot from `0002` — `yandex:instance.web:v2` beside `v1`, both real,
both configured the same way, and `v1` retired when `v2` has shown itself.

This works here in a way it cannot in a Terraform and Ansible pair, because the
software on the machine is converged by the same graph. **A new image changes
where a machine starts, not where it has to end up.** A machine on an older image
is not unmanaged; it is older.
"#,
    trade_offs: &[r"
The common operation stops being dangerous. Editing an image reference no longer
means every plan until the end of time proposes to destroy the fleet, and nobody
has to reach for an escape hatch to make ordinary work possible.

The costs are three.

A provider author now classifies every attribute into three kinds rather than
two, and getting one wrong in the birth direction means a change that should have
happened never does. So converged is the default and birth is opt-in and
explicit: silence has to be asked for.

A deliberate replacement is something a person has to decide to do. The plan says
so on every run, which is both the mitigation and the irritation.

And there is a shape this does not cover: an attribute that must be corrected but
only during a window. That is scheduling, it is not the model's business, and
pretending otherwise here would be the beginning of a second tool.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 5),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
