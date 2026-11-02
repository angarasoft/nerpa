use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-ali-api-signed",
    title: NonEmptyStr::new("Alibaba requests are signed with the V3 scheme through an HTTP seam"),
    slice: crate::slice::s_alibaba,
    origin: WorkOrigin::Decision(crate::adr::adr_2026_080),
    taxon: taxon!(Subsystem, Alibaba),
    radius: BlastRadius::Crate,
    outcome: NonEmptyStr::new("A crate nerpa-provider-alibaba carries an Api seam with the get, post and put the metadata service and the API need, a replayer for recorded exchanges, and the V3 signer — canonical request, string to sign and the ACS3-HMAC-SHA256 Authorization header — held to a known answer computed with OpenSSL; an RPC call returns the answer parsed, or the service's error named by its code and never its body."),
);
