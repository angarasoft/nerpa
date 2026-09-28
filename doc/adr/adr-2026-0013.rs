use crate::taxonomy::Subsystem;
use slipway_core::{nonempty_str, taxon};
use slipway_knowledge::{Breaking, DocStatus};

slipway_knowledge::adr!(
    title: "Borrow from Portage what neither Terraform nor Ansible has",
    status: DocStatus::Active,
    subsystems: &[taxon!(Subsystem, Os)],
    context: r"## Context

Portage is the only widely used convergent state manager that people are fond of.
Terraform manages state in one domain; Ansible is not a state manager at all but
a task runner that resembles one. Portage has spent twenty-five years on exactly
the problem the `target` domain (`0001`) poses: reconciling a real, mutable,
hand-edited machine with a declaration. Nix answered the same question by
refusing mutability, which puts it out of scope here.
",
    decision: r"
Take five things, and leave the apparatus around them.

**`CONFIG_PROTECT`** becomes `Protection::Merge`: a resource may be marked so that
drift stops the stage and produces a three-way merge, instead of being silently
corrected. **`pkg_pretend`** becomes a declared validation phase that runs during
`plan`, changes nothing, and can fail the plan early — this is the mechanism
behind the honest dry run in `0012`. **USE flags and `REQUIRED_USE`** become
variability as a declared, constrained dimension with checkable conflicts, rather
than conditionals scattered through the configuration. **`SLOT`** is already in
`0002`. **The build sandbox** becomes a declared filesystem and network scope for
each executor operation, where writing outside it fails loudly.

The plan output follows `emerge -pv`: a fixed action column, deltas rather than
values, the origin of each value, and a summary before the detail.
",
    trade_offs: &[r"
Several of these have no equivalent anywhere in this space. Refusing to overwrite
a hand-edited file is the clearest — every other tool's answer to drift is a
claim about operational reality that is untrue.

What is left behind matters as much: profile sprawl is Ansible's twenty-two
precedence levels in another costume, a directory tree is not a state store, and
Portage's conflict messages are a lesson in intent rather than in execution. The
cost of the parts taken is that variability, protection and sandbox scope all
become schema the provider author has to get right.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: slipway_core::date!(2026, 9, 4),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
