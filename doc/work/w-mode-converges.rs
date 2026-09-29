use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-mode-converges",
    title: NonEmptyStr::new("A text file mode converges"),
    slice: crate::slice::s_os_converges,
    origin: WorkOrigin::Divergence {
        specification: crate::rfc::rfc_2026_002,
        violated: NonEmptyStr::new("A file mode is written as text and compared as the number it spells; a number, or text that spells no mode a file can have, is refused while planning."),
    },
    taxon: taxon!(Subsystem, Os),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("mode = \"644\" converges: the second run is empty, and a mode that spells no file permission is refused while planning."),
);
