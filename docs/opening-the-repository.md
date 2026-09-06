# Opening the repository

**This file deletes itself.** It is a runbook for one day, and step 9 is
removing it. It is deliberately not in [MAP.md](MAP.md): that map names kinds of
knowledge, and wiring a temporary thing into a permanent index is the drift the
map exists to prevent.

The order matters. Everything in step 1 is impossible afterwards, and several
settings are easier to get right while nobody is watching.

---

## What was already checked

Run on 2026-09-06, against the whole history. Re-run rather than re-decide:

```console
$ git log --format='%an <%ae>' | sort -u
Anishchuk Sergei <anishukserg@gmail.com>            # one author, one address

$ git count-objects -vH | grep size-pack
size-pack: 486.56 KiB                               # nothing large hiding

$ git rev-list --objects --all \
    | git cat-file --batch-check='%(objecttype) %(objectname) %(objectsize) %(rest)' \
    | awk '$1=="blob" && $3>100000 {print $3, $4}'
                                                    # no blob over 100 KiB, ever

$ git log --all --diff-filter=A --name-only --format='' | sort -u \
    | grep -iE '\.pem$|\.key$|id_rsa|\.env|credential'
                                                    # no key-shaped file ever added

$ git log --all -p --format='' | grep -E '^\+' \
    | grep -iEo 'AKIA[0-9A-Z]{16}|gh[pousr]_[A-Za-z0-9]{20,}|-----BEGIN [A-Z ]*PRIVATE KEY-----'
                                                    # no credential-shaped string
```

The secret fixtures in the test suite are `db/password` and `digest_key` — plainly
placeholders. The one hexadecimal string in `README.md` is a plan hash in example
output, not a commit.

There is nothing to scrub. That is the answer this file exists to record, so
nobody re-derives it under time pressure.

## 1. Decide about the history — the only step with no later

Publishing exposes **every commit, branch and tag**, not the tip. Anything to be
changed about the past is changed now or never.

**The address.** All commits carry `anishukserg@gmail.com`. It is already in
`Cargo.toml` and would be on crates.io regardless, so publishing it is not new —
but in a public history it is permanently scrapeable.

Rewriting it costs something specific to this repository: every commit SHA
changes, and the commit messages here **cite each other** — `4b5cc21`, `3b4317b`
and a dozen more. Those citations become dangling, and they are some of the most
useful writing in the project.

> **Recommendation: keep it.** The gain is small, the loss is the cross-references.
> If the answer is otherwise, it happens here, with `git filter-repo` (not
> installed; `pipx install git-filter-repo`) and a mailmap, before anything else.

**The messages.** They are frank about mistakes — invariants demoted, a gate
defeated by a review, a miscount corrected. That is a feature and worth keeping,
but it is a decision to take deliberately rather than discover.

- [ ] Address settled
- [ ] Messages read once, as a stranger would

## 2. Settings, before the flip

- [ ] **Branch protection on `main`**: no force-push, no deletion, require the
      gate to pass. `CONTRIBUTING.md` says `main` is merged into by hand after a
      change has been tried on something real — the protection should say the
      same thing.
- [ ] **Actions → General**: workflow permissions **read-only**, and require
      approval for workflows from fork pull requests. A public repository runs
      whatever a stranger's branch contains otherwise.
- [ ] **Turn off what is not used**: wiki, projects. Less surface, fewer places
      for something to rot unwatched.
- [ ] **Description and topics**: `rust`, `infrastructure-as-code`,
      `configuration-management`. This is how anybody finds it at all.

## 3. Flip it

Settings → General → Danger Zone → Change visibility → Make public.

## 4. Immediately after — the things only a public repository has

- [ ] **Private vulnerability reporting** (Settings → Advanced Security). The
      *Report a vulnerability* button is a public-repository feature, and
      [SECURITY.md](../SECURITY.md) links straight at it. Until this is on, that
      link goes nowhere and the policy is a promise with no channel behind it.
- [ ] **Dependabot alerts**, and security updates. `cargo xtask deny` already
      catches advisories on every run of the gate; this catches them on the days
      nobody runs it.
- [ ] Open `SECURITY.md` and follow its link. Open the issue forms and check all
      four render.

## 5. Know what cannot be undone

Making it private again **does not delete forks**. They stay public, and every
commit they hold stays reachable by SHA. Within days there are copies in Software
Heritage, GHArchive and every crawler that indexes public code.

Public is a one-way door. Private is not.

## 6. crates.io is a separate door, also one-way

Publishing a crate is what actually guards the name — [0019](decisions/0019-name-the-tool-nerpa.md)
says so and flags that the name is unguarded until then. A published version can
be yanked but never deleted, and `Cargo.toml` pins `=0.0.1` path dependencies
that have to be published in order. Not part of opening the repository; do not
let the two run together.

## 7. Reconcile the writing that becomes untrue

Two sentences stop being true the moment this succeeds, and both are load-bearing:

- `CONTRIBUTING.md` — *"The repository is private while the early stages are in
  progress."*
- `ROADMAP.md`, end of stage 6 — *"At this point the repository opens."*

If the repository is public before stage 6, that is a reversal of a recorded
decision and rule 1 wants a record saying why — not an edit to either file.
If it is public **because** stage 6 arrived, both sentences simply go.

- [ ] Decided which of the two happened, and written it down

## 8. Watch the first week

- [ ] The nightly schedule in `.github/workflows/ci.yml` actually fires. It never
      has: `mutants` and `budget` only run `on: schedule`, and a schedule on a
      private repository does not run at all.
- [ ] Actions minutes. Free and unlimited on public repositories, which is why
      the nightly job lives there rather than on GitLab.

## 9. Delete this file

```console
$ git rm docs/opening-the-repository.md
```

Nothing above is worth keeping once it has happened. A runbook left behind is a
document describing a world that no longer exists, and the next person reads it
as though it does.
