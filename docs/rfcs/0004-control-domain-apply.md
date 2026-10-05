# RFC 0004. Control-domain apply: a provider that acts without a machine

- Status: draft
- Date: 2026-10-05
- Touches: 0001, 0004, 0006, 0034, 0041; the `Provider` trait, `run::apply`

## 1. Summary

A control-domain resource's state is read and changed through an API, from
wherever the run starts. Reading already works: `observe` hands a control
provider `&Nowhere` as its `reach`, and the provider ignores the handle and asks
the API itself inside `read`. Applying does not. The apply path is `prepare` →
`send` over a `Reach` → `conclude`, and all three are machine-shaped:
`Operation` is `Directory | File | Process | Remove`, `Reach` is the executor,
and a control resource's `reach` is `Nowhere`, which refuses every `ask` with
`NotOnAMachine`.

This RFC gives the control domain an apply path of its own: a `Provider::act`
method in which the provider performs the API calls itself and returns what
state records, called by `run::apply` for a resource with no target. The mark is
written before `act` and the conclusion after, so the durability that the
machine path gets from `mark → send → record` is preserved.

## 2. What the code says

- `provider.rs` — `Machines::reach(None)` returns `&Nowhere`; `Nowhere::ask`
  returns `Err(NotOnAMachine)` for every ask, empty operations included. A
  control provider cannot be made to apply by routing it through a machine.
- `run.rs` — `apply` takes `reach = machines.reach(address.target())`, and
  `send` does `perform(reach, operations, …)` → `reach.ask(…)`. For a control
  resource that is `Nowhere.ask`, which refuses.
- `nerpa-proto` — `Operation` is `Directory | File | Process | Remove`: the
  vocabulary of a machine, not of an API. There is nothing for a cloud call to
  be.
- `observe` already groups a control batch under `(provider, None)` and hands it
  `&Nowhere`; the provider ignores the handle and talks to the API. `read` is
  the proof that a provider can act on the world itself.

## 3. What is decided

- **A new trait method, `act`.** `fn act(&self, change: &Prepared<'_>) ->
  Result<Concluded, ProviderError>` is the control-domain counterpart of
  `prepare` + `conclude`. The provider performs the create, update or delete
  itself and returns `Concluded::Became(attributes)` or `Concluded::Gone`, the
  same two answers `conclude` returns, so everything downstream of the
  conclusion is unchanged. `act` has a default that refuses with a clear error,
  so the machine-side providers and the test fakes need not change; only a
  provider with control kinds overrides it.
- **`run::apply` branches on the domain.** For a change whose address has no
  target, the engine writes the mark, calls `act`, then records the conclusion,
  instead of `prepare` → `send` → `conclude`. The mark is written before `act`
  and the conclusion after, which is the control form of the machine path's
  `mark → send → record`.
- **The premise is the provider's own job.** The machine path checks a premise
  in the executor, right before the work. The control path has no executor, so
  the provider checks its own premise — the API's conditional-request version,
  or a re-read inside `act` — and refuses with the change stale rather than
  acting on a world that moved. This is weaker than the machine path, where the
  engine hands the premise to the far end and does not trust the provider to
  check it; the difference is stated, not hidden.
- **The credential is the provider's, never the resource's.** A cloud
  credential — service-account key, IAM token, or the instance metadata
  service — is resolved by the provider when it is built, from a `Secret` the
  engine hands it or from the metadata service, and never appears in a resource
  attribute, state or a plan.
- **`read` is unchanged.** It already works for the control domain; `act`
  mirrors it.

## 4. The design

The apply loop in `run::apply` today resolves, for every change:

```text
mark → prepare → send over Reach → conclude → record
```

The branch adds a second shape for `address.target() == None`:

```text
mark → act → record
```

`mark` and `record` are the same calls as today, which is what preserves the
durability story: a request never goes out without a mark written first, and the
mark is settled by the conclusion the same way the machine path settles a mark.
A run killed between `act` and `record` leaves the mark in flight, and the next
run's reading decides whether the change took — `Afterwards::Read` for a cloud
resource, whose state a read sees in full.

`act` is handed the same `Prepared` as `prepare`: the address, the kind, the
settled desired values (secrets included, by name), what state recorded, and the
facts. It returns `Concluded` directly, so `wrote`, `forget` and `settle`
downstream need no new arms.

The premise travels with the change as it does today, carried in `Observed`. A
machine premise is sealed by the engine and checked by the executor; a control
premise is the provider's to check, because there is no far end. The provider
decides what "the world I read" hashes to — an API version stamp, a resource
revision, or a re-read of the attributes it will change — and refuses `act`
with the change stale if it moved. The guarantee is as wide as what the provider
admits to having read, the same bound the machine path states in `0034`.

A control provider is built with its credential resolved, not stored: the CLI
hands it the `Secret` source, and the provider exchanges it for a token (or reads
the metadata service) when it is built. The token never enters `Attributes`, so
it cannot reach state or a plan; the `secret` names in a change are the resource
attributes a cloud call writes, not the credential the call is made with.

## 5. What is attacked

- **The premise is only as strong as the provider's honesty.** The machine path
  hands the premise to the far end and does not trust the provider to check it;
  the control path has no far end, so the check lives in the provider. A provider
  that skips it acts on a stale plan. This is the price of there being no
  executor on a cloud, and it is the same class of bound `0034` already names —
  the check is exactly as wide as what a provider admits to having read.
- **A credential in the provider is a credential in memory.** It is not in state
  or a plan, but it lives for the life of the process, like the ssh identity of
  the transport. The proposal does not move the credential into the typed plan;
  it keeps it where the transport keeps its own.
- **Batching applies to reads, not to acts.** A control read is one journey for
  many resources; a control act is one change at a time, because each change is
  its own API call and its own mark. The single-pass plan already orders control
  resources into stages, so this adds no new ordering question.
- **`Afterwards::SendAgain` is unlikely but not forbidden.** A cloud create whose
  read-back cannot be trusted to prove it happened can still say so, and the
  next run sends it again; nothing in this RFC rules it out.

## 6. Open findings

- The exact `ProviderError` a default `act` returns — a new variant versus
  reusing `UnknownKind` — is left to the implementation that lands this RFC.
- Whether the engine should hand `act` the credential (resolved) or the provider
  should hold it from construction is left to the implementation; the proposal
  fixes only that it never reaches state or a plan.
