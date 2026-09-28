use crate::invariant::{Enforced, Status};
use dacc_core::NonEmptyStr;

crate::invariant!(
    id: NonEmptyStr::new("INV-PLAN-005"),
    status: Status::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A provider that echoes every attribute it was given without acting on any of them satisfies this and converges on a lie. The check catches an attribute nobody owns, not an attribute somebody claims and drops — for that there is only reading the world back, which is what `apply` already does everywhere it can."), tests: &[crate::anchor::an_attribute_that_is_not_reported_back_stops_the_run_rather_than_never_settling, crate::anchor::what_was_made_before_the_refusal_is_still_recorded] }),
    statement: NonEmptyStr::new("An attribute handed to a provider comes back from it, or the run stops."),
    rationale: r"Reporting an attribute is the provider's claim that it took it. One that goes in and does not come out never settles: the next plan finds it missing from the reading, proposes the same change, and every run after that does the same for ever. The tool stops converging and says nothing — which is worse than failing, because a second run that is not empty is the one signal this whole design rests on.

Measured on a file declared with an attribute the `os` provider does not own. It applied cleanly and re-planned indefinitely.

The check is generic rather than schema-driven, so a provider written outside this repository gets it whether its author thought about it or not. A provider that cannot read a value back — a credential the cloud will not return — echoes it, and the echo is the claim. What it may not do is say nothing.",
    enforced_by: &[crate::anchor::run_wrote],
    tests: &[crate::anchor::a_provider_that_echoes_what_it_cannot_read_back_is_accepted, crate::anchor::a_file_is_written_and_the_second_run_has_nothing_to_do],
);
