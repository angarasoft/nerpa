# 0023. Render configuration files from Jinja templates, sandboxed

- Status: accepted
- Date: 2026-09-05

## Context

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

## Decision

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

## Consequences

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
