use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-brand-assets",
    title: NonEmptyStr::new("The project has a logo and a favicon"),
    slice: crate::slice::s_branding,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("the repository had no logo, icon or favicon, and neither the README nor the book carried one"),
    },
    taxon: taxon!(Subsystem, Docs),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("The README and the Sphinx book show the logo, and the built book serves the favicon and the generated icon set."),
);
