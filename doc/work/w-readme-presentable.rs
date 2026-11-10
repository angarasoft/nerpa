use crate::taxonomy::Subsystem;
use dacc_core::{taxon, BlastRadius, NonEmptyStr};
use dacc_work::WorkOrigin;

dacc_work::work!("w-readme-presentable",
    title: NonEmptyStr::new("The landing page reads for a newcomer"),
    slice: crate::slice::s_maintenance,
    origin: WorkOrigin::Toil {
        justification: NonEmptyStr::new("the landing page was dense prose a newcomer had to read through; it needed a shorter, scannable shape, an example that both creates a resource and configures it, the core ideas as links, and a comparison with the tools a reader arrives from"),
    },
    taxon: taxon!(Subsystem, Docs),
    radius: BlastRadius::Local,
    outcome: NonEmptyStr::new("The README leads with an example that creates a VM and configures the file on it, names its core ideas as links into the book, and places itself next to Terraform and Ansible."),
);
