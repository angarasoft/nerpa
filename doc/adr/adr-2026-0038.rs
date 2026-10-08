use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "Two package families, and a refusal by name for the rest",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Os)],
    context: r#"## Context

Packages are the first thing nerpa manages whose meaning differs by machine. A
file is a file on every system; a package is a transaction on one, a compile on
another, and the two disagree about what a name even is.

Three things already written down bear on this. `W-010` says the control machine
holds every distribution's semantics and that being wrong about one is discovered
remotely. [0032](0032-provider-logic-stays-on-the-control-machine.md) put that
logic here on purpose. And the roadmap opens providers to authors outside this
repository at stage 6 — so the breadth of the matrix is, by the project's own
plan, somebody else's work. Ours is the shape that lets them write it.

Ansible carries around thirty package modules and is still wrong in the corners
after a decade. Deriving an abstraction from two examples and calling it general
would harden it before anybody outside has written a line against it, which is
the one mistake that cannot be undone cheaply.

Measured on the two machines this was written against, because none of it is
guessable:

* `rpm -q --queryformat '%{NAME} %{EVR} %{ARCH}\n' bash` answers
  `bash 5.1.8-9.el9 x86_64`, and exits 1 saying "is not installed" when it is
  not. Exact, machine-readable, no parsing of prose.
* `qlist -ICv dev-lang/python` answers with `dev-lang/python-exec` among the
  matches — it matches by substring, so it is the wrong instrument.
  `portageq match / dev-lang/python` answers with exactly the two installed
  slots, and is the right one.
* `nginx` exists in three categories on a stock Gentoo tree — `acct-group/`,
  `acct-user/` and `www-servers/`. `portageq` refuses a bare name as an invalid
  atom, and it is right to: the name alone does not identify a package.
"#,
    decision: r#"
**Two families, chosen to stand as far apart as the space allows.** `rpm` with
`dnf`, and `portage` with `emerge`. One installs prebuilt packages inside a
transaction against a repository; the other compiles from source with build-time
flags, keeps several versions side by side in slots, and spells a name as
`category/name`. If the shape survives both, `apt` is another instance of a shape
already proven rather than a new problem. A third family added now would prove
nothing and would buy a false sense of coverage.

**A family the provider does not know is refused at plan time, by name.** The
`os_family` fact is read before any provider is asked (that is why the fact
journey goes first), and a machine that answers with something outside the closed
set produces a plan error naming the machine and what it said. It is not guessed
at, and no command is sent to a machine on the chance that it works.

**Presence is the declaration.** A package that is declared is installed; a
package whose declaration is removed is uninstalled, by the same abandonment
every other resource uses. There is no `state:` attribute, because there is
nothing for it to say that the presence of a line does not.

**`latest` is refused and will stay refused.** "Install the newest there is" is
not a state a plan can describe: what it does depends on when it runs, so a plan
carrying it cannot be checked twice, cannot carry a number that means anything,
and cannot be signed. This is a real ergonomic loss against the predecessor and
it is deliberate. A person who wants the newest version writes the version.

**Version comparison is asked of the machine and never implemented here.** `rpm`
has `rpmvercmp`, portage has its own rules with revisions and slots, `dpkg` has
`--compare-versions`, and the three disagree. A comparator written on the control
machine is exactly the failure `W-010` describes: distribution semantics living
where the distribution is not, and being wrong about it discovered on somebody
else's machine. So the question "is what is installed what was asked for" is put
to the tool that owns the answer, as an ordinary observation. This does not move
provider logic to the machine — deciding what to ask stays here; only the fact
being asked for lives there.

**A package's address must be able to name it.** In portage the identity of a
package is `category/name`: three different packages are called `nginx`, and an
address that cannot tell them apart would let two declarations collide in the
graph. The slot already in the grammar is the right home for `3.12` and the wrong
home for `www-servers`. So the name charset gains `/`.
"#,
    trade_offs: &[r#"
The provider is small enough to be right, and what it does not do is written
here rather than discovered. A machine running Debian gets a refusal naming
Debian rather than a `dnf` command that fails obscurely.

The costs, in the order they will be felt.

**Debian and Ubuntu get nothing**, and between them they are most of the
installed base anybody would try this on. That is the price of not deriving a
general abstraction from two examples, and it is paid until somebody writes the
third provider — which the roadmap says is stage 6 and somebody else. Until then
"does it support my distribution" is answered "no" for the common answer to that
question.

**Refusing `latest` costs a workflow people rely on**, and it will be the first
thing anybody coming from Ansible notices. The answer to them is in this record
rather than in a maintainer's memory.

**Asking the machine to compare versions costs a dependency on its tools.** A
machine whose `rpm` or `portageq` is missing or answers differently than measured
here reports a package it cannot describe rather than a wrong answer — which is
the right failure, and still a failure that a comparator written here would not
have had.

Widening the name charset is a change to the address grammar, which
`INV-ADDR-002` and `INV-ADDR-003` guard. It is a widening rather than a
narrowing, so no address that parsed before stops parsing; the round-trip test
covers the new shape and the digest is unaffected, since names have always been
length-prefixed.

What this record deliberately does not settle: how a version constraint other
than an exact one would be expressed, how a repository is configured, how USE
flags are set, and what happens to a package another package depends on. Each is
refused by name for now, and each is a decision somebody will have to take when
there is a reason to.
"#],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 7),
    breaking: Breaking::No,
    code_refs: &[],
    related_rfcs: &[],
    introduces_norm: false,
);
