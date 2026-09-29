use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-deb-packages",
    title: NonEmptyStr::new("The os provider manages packages on Debian-like systems"),
    slice: crate::slice::s_deb_packages,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("the os provider refuses Debian-like systems by name, and apt is the most common package manager; the commands must be measured on a real machine, like the two families already taught"),
    },
    taxon: taxon!(Subsystem, Os),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("The os provider knows a third family — dpkg to read, apt-get to act — and a Debian or Ubuntu machine converges, with an empty second run."),
);
