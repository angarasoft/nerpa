use crate::taxonomy::Subsystem;
use dacc_core::{nonempty_str, taxon};
use dacc_knowledge::{Breaking, DocStatus};

dacc_knowledge::adr!(
    title: "Versioning is changed through the S3 door, which takes the IAM token without a signature",
    status: DocStatus::Active,
    channel: crate::taxonomy::channel::Public,
    subsystems: &[taxon!(Subsystem, Yandex)],
    context: r"
        The management API's update cannot carry a bucket's versioning: its
        validation accepts the enabled word alone, and answers any other with
        a refusal about an object lock. The bucket work therefore declared the
        attribute immutable — a change replaces the bucket — and refused to
        correct a bucket whose versioning is not enabled, because the enabled
        word the update body must carry would flip it silently. The owner
        needs to enable versioning on buckets that already hold data, which a
        replacement cannot do at all: the cloud refuses to delete a bucket
        that is not empty.

        Object Storage answers through a second door, the S3 API on
        storage.yandexcloud.net, where a sub-resource carries its document as
        a PUT. Yandex's own documentation names the IAM token the recommended
        authentication there — more secure than a static key, and requiring no
        request signature — so the short-lived credential the provider already
        resolves is the credential this door takes. Probes against the owner's
        folder confirmed all of it: the token works as a bearer on the S3
        endpoint, a PUT of the versioning document with Enabled or Suspended
        is accepted on an unversioned, enabled or suspended bucket alike, and
        the management read carries what the S3 door wrote.
    ",
    decision: r"
        A declared change to a bucket's versioning is applied in place through
        the S3 door: a PUT of the versioning document to
        /{bucket}?versioning, carrying Enabled or Suspended. The declared
        vocabulary is those two words — a bucket that was never versioned
        reads as disabled and stays so until it is declared, because no door
        can unversion a bucket: the S3 API has no such state and the
        management update cannot carry the word. The management update still
        runs first for the attributes it manages, carrying the enabled word
        its validation demands, and the S3 door is the final word on
        versioning. An update whose declaration does not manage versioning,
        on a bucket whose versioning is not enabled, is refused by name: the
        management body would flip it. The read stays where it is — the
        management door's full view carries the state both doors wrote.
    ",
    trade_offs: &[
        "The update order means a suspension passes through the enabled word the management body forces: the cost is a window of one request in which the bucket takes versions. Accepted because the end state is what was declared, and the alternative — refusing every suspension alongside a class change — would close a change the S3 door plainly allows.",
        "The vocabulary loses 'disabled' as a declarable word. Unversioning a bucket is impossible in the cloud, and a word for a state no door can reach would be a promise the plan cannot keep; the absence of the declaration says it instead, and a never-versioned bucket reads as disabled either way.",
        "The S3 door is a second transport shape in the crate — an XML document over the object endpoint rather than JSON over the management one — held to one fixed document and one verb, so the shape does not become a second everything.",
    ],
    constraints: &[
        "No SigV4 code and no static access key enter the tree: the S3 door takes the short-lived IAM token the provider already resolves, as a bearer.",
        "The S3 door speaks PUT /{bucket}?versioning and nothing else; every other attribute stays on the management door.",
        "Versioning is read from the management door's full view; the S3 door is written, not read.",
    ],
    authors: nonempty_str!["Anishchuk Sergei"],
    decided_at: dacc_core::date!(2026, 11, 6),
    breaking: Breaking::No,
    introduces_norm: false,
    code_refs: &[],
    related_rfcs: &[crate::rfc::rfc_2026_002],
);
