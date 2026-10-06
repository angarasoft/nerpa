use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-machine-identity",
    title: NonEmptyStr::new("A machine acts over a grant the fleet issued it"),
    slice: crate::slice::s_stage_5_surface,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "The grant is wired into the run but is still typed on the command line (`--as-scope`), so an agent's grant is whatever it says rather than what it was issued. adr-2026-009 says an agent acts over a grant it was issued; the fleet names the grant, and a run whose actor names that identity carries it."
        ),
    },
    taxon: taxon!(Subsystem, Cli),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new(
        "The fleet names a grant for an identity (`[grants.<who>] = \"kind=…;target=…;op=…\"`), a run whose `--as` names that identity carries the issued grant, and one that also gives `--as-scope` is refused as typing what was issued."
    ),
);
