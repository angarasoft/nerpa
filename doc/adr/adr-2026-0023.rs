use crate::taxonomy::Subsystem;
use slipway_core::{nonempty_str, taxon};
use slipway_knowledge::{Breaking, DocStatus};

slipway_knowledge::adr!(
    title: "Render configuration files from Jinja templates, sandboxed",
    status: DocStatus::Active,
    subsystems: &[taxon!(Subsystem, Config)],
    context: r"## Context

Writing configuration files is most of what configuration management does, and
Ansible is genuinely good at it. The reason is one property: **a template looks
like the file it produces.** Somebody opens `nginx.conf.j2` and sees nginx.conf,
with holes in it. Building the same file by concatenating strings in a program
loses that entirely, and with it most of the reason a person can read the
configuration at all.

So this is the one place where matching a predecessor exactly is the feature
rather than a failure of nerve.

What cannot be inherited is how Ansible renders. A Jinja template there can reach
anything: every variable in an ambient scope of twenty-two precedence levels,
facts gathered from the machine, `lookup('file', ...)`, `lookup('env', ...)`,
`lookup('pipe', ...)` — which runs a command. A template that can run a command
renders differently every time, and a plan computed from it cannot be signed,
compared, or trusted.

## Alternatives considered

Measured rather than assumed, by running twenty idioms taken from the way Ansible
templates are actually written against each candidate.

**Tera 2.3** is the serious alternative and is better than its reputation from
version 1: the built-in functions that used to reach the world — `get_env`,
`now`, `get_random` — are gone, leaving only `range` and `throw`, and every
default filter is pure. Hermeticity is not what separates it.

Two things do. **Tera requires filter arguments to be keyword arguments**:
`join(sep=' ')`, not `join(' ')`; `default(value='x')`, not `default('x')`. That
single divergence rejects most real templates at parse time, and Tera 2 also
removed `{% macro %}`. Of the twenty idioms it accepted twelve unmodified; the
eight it refused were not exotic — `default`, `join`, `replace`, `map`,
`selectattr`, `batch`, `macro`. minijinja accepted nineteen, and twenty with
`minijinja-contrib`'s `pycompat`, which exists to answer exactly this by mapping
Python method syntax such as `.items()` onto filters.

The second is that Tera has **no bound on execution** — no fuel, no recursion
limit, nothing in its public API. minijinja has `set_fuel`, `set_recursion_limit`
and a built-in cap on `range()`, and a template arriving from a configuration
repository is precisely where an unbounded loop should not be possible. It also
offers `Environment::empty()`, which starts with no filters at all rather than
with a set to be pruned, and a loader callback through which every `include` is
resolved — an attempt at `../../etc/shadow` is refused by our own code.

One knob must not be forgotten: Tera errors on an undefined variable by default
and minijinja does not, so `UndefinedBehavior::Strict` is set explicitly. For a
configuration file a missing value must fail, never render as empty.

**Askama** compiles templates into the binary, so a template from somebody's
configuration directory could not be rendered at all. **Handlebars** and
**Liquid** are competent and use a different syntax, which forfeits the one
property this decision is about.
",
    decision: r#"
Templates are Jinja, rendered by `minijinja` — Jinja2's own author's
implementation, deliberately faithful to its semantics — with three limits that
Ansible does not have.

**Nothing reaches out.** No clock, no environment, no filesystem, no process, no
network. The renderer is given exactly the functions and filters it is allowed
and nothing else. Same reasoning as `0005` for Starlark: the plan has to be
reproducible before it can be worth signing, and a template that reads the world
makes it not.

**Values are handed over, not ambient.** `template(path, values)` takes the
mapping it is rendered with. There is no scope to inherit from and no precedence
to memorise — the twenty-two levels are refused here as they are elsewhere.

**Templates live in the configuration, not on the machine.** They are read
relative to the configuration's own root, with no absolute paths and no escaping
upwards. That makes the configuration a directory rather than a file, and the
plan's notion of "which revision" has to cover the tree.

**Rendering happens at plan time** when every value is settled. A template whose
input is still unresolved is rendered during apply from what the earlier stage
produced, and the plan says so — the same `Confidence` the rest of the model
already uses.
"#,
    trade_offs: &[r"
The plan can show what a file will contain, line by line, **before anything
happens**. Ansible shows a diff of a rendered file only with `--diff`, during the
run, after it has started. For a configuration file that is the single most useful
thing a plan can say, and it falls out of rendering early rather than late.

The costs are three, and the first is the sharpest.

Some existing templates will not port. Anything using `lookup` or a filter that
touches the world has to become a value passed in — which is usually the right
shape anyway, and occasionally a real loss.

Ambient variables are convenient, and handing values over explicitly is more
typing. That is the same trade already made against precedence levels, taken
again, and it will be felt more here than anywhere else because templates are
where people write most.

And the configuration stops being one file. Everything that computes a revision —
the plan hash above all — has to take the tree, which is more work than hashing a
program and is not optional: a template changing while the program does not is
exactly the change somebody would otherwise sign without seeing.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: slipway_core::date!(2026, 9, 5),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
);
