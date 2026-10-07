use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-wasm-credential",
    title: NonEmptyStr::new("A credential is a handle, never a value"),
    slice: crate::slice::s_stage_6_others,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_0004),
    taxon: taxon!(Subsystem, Core),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "A WASM component holds a credential as an opaque handle and spends it through a `request` import the host answers with the value it alone holds; a request to a host outside the manifest is refused, so the component demonstrably cannot read the credential."
    ),
);
