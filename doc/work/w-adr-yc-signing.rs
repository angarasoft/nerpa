use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-adr-yc-signing",
    title: NonEmptyStr::new("An ADR names how the Yandex key flow signs, and what it wipes"),
    slice: crate::slice::s_stage_3_provider,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("adr-2026-075 asks that the service-account key be zeroized once the JWT is signed and names no signer. The signer the tree already links, ring, keeps its parsed RSA key in memory it does not wipe, and the RustCrypto rsa crate carries an unpatched timing advisory (RUSTSEC-2023-0071): a decision names the signer, the seam that lets it be replaced, and what is and is not wiped, before w-yc-credential-model writes the code."),
    },
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("A decision supersedes adr-2026-075: ring signs PS256 behind a Signer seam, the key file is read with owner-only permissions and kept only for one signature, what nerpa wipes and what it cannot is stated and declared as a limitation, and the keyless paths are named, with federation in the backlog."),
);
