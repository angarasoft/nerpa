use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "A secret taints what it touches, and masks only its own span",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Transport)],
    context: r"## Context

The architecture says state holds references to secrets rather than their values.
Today that is true only because nothing has put a secret in one yet. A password
written into a configuration file would reach state in clear text, which is
precisely the complaint made against Terraform — where a `terraform.tfstate` is a
plaintext dump of every credential the configuration touched, and the answer has
always been to encrypt the whole file at rest and hope.

The complaint is not that state is unencrypted. It is that a value nobody needed
to keep was kept anyway.

Two things make this urgent rather than tidy. Retrofitting a state format after
anyone's state exists is a migration. And [0023](0023-templates-that-look-like-the-file.md)
just promised that a plan shows what a file will contain — which, for a file with
a password in it, is a promise to print the password.
",
    decision: r"
**Secrecy is a taint that propagates.** A value read from a secret source is
secret; anything derived from it — concatenated, formatted, rendered through a
template — is secret. This is carried by the value model, not by the author
remembering to mark things.

**State holds a reference and a digest, never a value.** The reference is what
the secret provider needs to fetch it again — provider, path, version. The digest
answers the only question state has to answer, which is whether it changed. It is
keyed, so that a digest which escapes into a log or a diff is not a target for a
dictionary of likely passwords; against somebody holding the state file itself it
offers nothing, and no claim is made that it does.

**A plan reports change without reporting value.** `password: (secret) → (secret,
changed)` says everything a person needs to approve, and nothing they do not.

**A rendered file masks the substituted spans, not the file.** The renderer runs
twice — once with real values, once with a random sentinel per secret — and the
difference between the two renderings locates exactly where each secret landed.
The diff shows every other line as usual and blanks those spans.
",
    trade_offs: &[r"
The diff for a configuration file with a credential in it is still a readable
diff — the whole point of rendering early survives contact with a password, which
it would not if the answer were to suppress the file.

The costs are three. Span location fails when a template transforms a secret
rather than inserting it, because the sentinel comes out the other side unlike
itself; the fallback is to mask the whole line, which is correct but blunt, and
whoever hits it will find the behaviour inconsistent until they know why.
Rendering twice doubles the work for every templated file, which is cheap in
absolute terms and is nonetheless a cost paid on every plan for the benefit of
the minority of files that hold secrets. And taint spreads further than people
expect: a filename computed from a secret makes the filename secret, and the plan
will decline to print it. That will look like a bug the first time and be correct
every time.
"],
    constraints: &[],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 9, 5),
    breaking: Breaking::No,
    introduces_norm: false,
    code_refs: &[],
    related_rfcs: &[],
);
