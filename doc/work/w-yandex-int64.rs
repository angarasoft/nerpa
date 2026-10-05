use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-yandex-int64",
    title: NonEmptyStr::new("The yandex provider reads an instance's int64 the way the API writes it, as text"),
    slice: crate::slice::s_stage_3_cloud,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new(
            "A live plan failed reading a real instance: Yandex Cloud writes an int64 — an instance's cores and memory — as text in JSON, and the wire struct said it was a number. The recorded exchange had carried the same wrong number. This is a wire-format detail with no architectural trace; the fix reads either form and writes text."
        ),
    },
    taxon: taxon!(Subsystem, Yandex),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new(
        "An instance's cores and memory read whether they cross JSON as text or a number, and are written as text; a recorded exchange pins the text form."
    ),
);
