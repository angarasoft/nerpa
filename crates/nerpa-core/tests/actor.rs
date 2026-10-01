//! Who acts in a run: `INV-TRUST-001`.
//!
//! The invariant names its bypass — a run that proceeds with nobody attached —
//! and this attempts it at the engine's door, which is the one the command line
//! does not stand in front of.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use nerpa_core::planner::plan;
use nerpa_core::provider::{Machines, Providers};
use nerpa_core::run;
use nerpa_core::secret::{Key, Remembered, Secrets};
use nerpa_core::state::State;
use nerpa_core::{Graph, Observed, Schema};

static DIGEST_KEY: Key = Key::from_bytes([7; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

/// `INV-TRUST-001` names its bypass: a run that proceeds without an actor
/// identity. The CLI refuses one by name; this is the other door, and it slams
/// before a single operation is sent.
///
/// The gate also runs without contracts (`--no-default-features`), where there
/// is no precondition to trip; the refusal is covered there too, by
/// `a_run_without_an_actor_is_refused_naming_the_flag` at the command line.
#[test]
#[cfg_attr(
    feature = "strict-contracts",
    should_panic(expected = "precondition violated")
)]
#[dacc_derive::doc_anchor(id = "a-run-proceeds-only-with-an-actor-identity")]
fn a_run_proceeds_only_with_an_actor_identity() {
    if !cfg!(feature = "strict-contracts") {
        return;
    }
    let nobody = plan(
        &Graph::new(),
        &Observed::new(),
        &State::new(),
        &Schema::new(),
        Secrets::new(&DIGEST_KEY, &NO_STORE),
    )
    .expect("an empty plan is plannable");
    let mut state = State::new();
    let _ = run::apply(
        &nobody,
        &Providers::new(),
        &Machines::new(),
        &Observed::new(),
        &mut state,
        Secrets::new(&DIGEST_KEY, &NO_STORE),
        &mut |_| Ok(()),
    );
}
