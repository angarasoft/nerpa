use dacc_core::{BlastRadius, NonEmptyStr};

dacc_work::slice!("s-fleet-closed",
    title: NonEmptyStr::new("The fleet document is a closed set"),
    thrust: crate::thrust::t_converges_to_invariants,
    outcome: NonEmptyStr::new(
        "A table the fleet does not know, and a known table written as something that is not a table, is refused by name, so a misspelt [approval] cannot ask for no signature; and a cloud the build has no provider for is named, not skipped."
    ),
    specification: crate::rfc::rfc_2026_004,
    max_radius: BlastRadius::Crate,
);
