# Coding standards

Rules, and next to each one whether a machine checks it. A standard nobody
enforces decays into a preference within a quarter, so the split is deliberate:
anything that can be checked is checked by `cargo xtask ci`, and the rest is what
review is actually for.

Moving a rule from the second table to the first is always a welcome change.

## Enforced by the gate

| Rule | Checked by |
| --- | --- |
| Formatting, 100 columns | `cargo fmt --check` |
| No clippy warnings, at all | `cargo clippy` with `-D warnings` |
| Tests pass with contracts compiled in, and without them | `cargo xtask test` |
| No bare `unwrap()` outside tests | `cargo xtask lint` |
| Every module opens with `//!` documentation | `cargo xtask lint` |
| Every enforced invariant names code and tests that exist | `cargo xtask invariants` |
| Every `// ENFORCES:` anchor names a registered invariant | `cargo xtask invariants` |
| Dependency licences are on the allowlist | `cargo deny` |

## Enforced by review

### Correctness, in this order

State it in the type system if you can, because that costs nothing at runtime
and cannot be forgotten. `Address` refuses to hold a component starting with
`-`, so no interpolation mistake anywhere else can turn a resource name into a
command line option.

State it in a contract if you cannot. `requires!` is a promise the caller broke,
`ensures!` is one this function broke, `invariant!` is a property of a value at
rest. They stay compiled into release builds because the runs that matter are
unattended, from release binaries, against production — exactly the builds where
`debug_assert!` is gone.

A contract failure panics, and that is the safe direction: the process holds a
belief about a machine that reality does not share, and the next thing it would
do is act on it. This is why the release profile must never set
`panic = "abort"` — a panic has to unwind so that the state lock and the open
transport run their destructors.

### Errors

Library crates return typed errors. `anyhow` belongs in binaries, if anywhere;
a caller that gets a string back cannot decide anything with it, and this tool's
callers include agents.

An error message names the thing that is wrong and, where it can, what to do
about it. Errors are read at three in the morning by somebody who did not write
the code.

`expect("...")` carries a reason. Not "should not happen" — the reason the caller
already knows it cannot.

### Determinism

Anything that reaches a plan, a hash or a report is built from `BTree*` and comes
out sorted. Two runs over one configuration have to produce identical bytes,
because those bytes are signed. This is not tidiness; hash map iteration order
would break the product's central claim on alternate Tuesdays.

### Comments

Explain the reasoning and what breaks without it — not what the line does. The
comment that earns its place is the one that stops somebody from simplifying a
decision back out six months later. Comments that restate the code are worse than
no comment, because they rot silently.

Module documentation says how the pieces fit before it says what the pieces are.
Where a diagram helps, draw it in a `text` block.

### Dependencies

Every new dependency is an argument to be made in the pull request: what it does,
what it would cost to not have it, and who maintains it. The tool is a static
binary that people run against production; every crate in the tree is a crate
somebody could compromise.

### Tests

A test name is a sentence about a guarantee, not a label for a function.
`cycle_reports_do_not_depend_on_insertion_order` says what breaks if it fails;
`test_graph_2` says nothing.

A test that exists only to raise coverage is noise. A test that pins a guarantee
from `docs/invariants.toml` is the point, and the registry should name it.
