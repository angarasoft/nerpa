# 06. Hypotheses, stop criteria and the verdict

Written before the start, as Slipway's thrust t0004 requires. Each hypothesis says
what measures it, where the number comes from, and which result stops or narrows the
pilot. A criterion written after the data is a description, not a test.

| H | Hypothesis | Measure | Stops or narrows the pilot if |
| --- | --- | --- | --- |
| H1 | Compiled references catch disagreements between a record and the code that nerpa's Markdown checks miss | Disagreements caught by building the registry, against those found by review or later, inside anchored code | None caught by the build by the end of wave 3, while review found at least one |
| H2 | Writing a record costs little enough (Slipway's decision 5) | Minutes to write a new work item or decision, and the share needing a second build, over the first ten of each | A median over three minutes, or more than 30 % needing a second build |
| H3 | Nobody works around the gate | Median time from `cargo slipway commit` to `COMMIT OK`, against the baseline; uses of `--no-verify` | A median over twice the baseline, or any bypass |
| H4 | Origins stay meaningful | The share of `Toil` among work items, per slice | Over 20 % in two consecutive slices |
| H5 | Done is proven | `landed` events without a gate event on the same tree, counted from the first commit for which the gate recorded a proof | Any |
| H6 | Radius is labelled honestly | Work items whose radius a review raised | A `Persistent` change landed as `Local` or `Crate` |
| H7 | Book-keeping does not grow | The records a change has to touch, over 20 changes, against the baseline | Two or more added without one removed |
| H8 | Conversion loses nothing | Sections that differ between a Markdown record and its registry record; routes that stop resolving | Any, at the end of wave 1 |
| H9 | The local types earn their place | Mistakes the typed form refused or caught — a supersession, a missing cost, an invariant's status — against the time the types took | None by the end of wave 3, and the types are not offered to Slipway |
| — | Agents find their way by `where` and `map` | Not measured: the access layer is not built | Stated as outside the pilot, so that silence is not read as a result |

## How long

Until `Core` reaches L2 in wave 4, or eight weeks after the switch, whichever comes
first.

## The verdict (D6)

- **Given by** Anishchuk Sergei.
- **Read first by** a model other than the one that did most of the pilot's work. Its
  reading is a review record — its angle, its findings, and what became of each — and
  its author writes no other pilot record.
- **One of three:**
  - continue — L3 per subsystem, and Slipway's crates published;
  - narrow — keep what H1 to H9 justified, and supersede the rest by decision;
  - stop — the rollback in [08](08-risks.md).
- **Recorded twice:** by name in `doc/`, under the thrust `slipway_pilot`, and
  anonymised in Slipway as a decision that closes s0008 or opens the next slice of
  t0004.

Becomes: the pilot's first decision in `doc/`; its anonymised copy lands Slipway's
w0027.
