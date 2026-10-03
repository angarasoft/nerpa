use dacc_knowledge::{Enforced, InvariantStatus};
use dacc_core::NonEmptyStr;

dacc_knowledge::invariant!("i0036",
    specification: crate::rfc::rfc_2026_002,
    status: InvariantStatus::Enforced(Enforced::Adversarial { bypass: NonEmptyStr::new("A removal that reaches for `remove_dir_all` because a non-empty directory is inconvenient, or a write that lands on a symlink and follows it into a file the declaration never named — and the same for a directory, where `create_dir_all` calls a link to one already done and the mode then lands on whatever it pointed at. All three read as correct against the in-memory fake, which has no directory that can be non-empty and no link to follow."), tests: &[crate::anchor::deleting_a_directory_does_not_take_what_nerpa_did_not_make, crate::anchor::writing_where_a_symlink_is_replaces_the_link_and_leaves_its_target_alone, crate::anchor::making_a_directory_where_a_symlink_is_leaves_what_it_pointed_at_alone] }),
    statement: NonEmptyStr::new("INV-DELETE-001: Removing a resource removes that resource and nothing that was never declared."),
    rationale: r"A directory nerpa made may since have been filled by somebody else, and a recursive delete of what it never created is not a removal but a second, unannounced change — the plan showed one line and the run destroyed a tree. The same reasoning refuses to write through a symlink: the declaration names a path, and the file on the other side of a pointer is not that path. A directory with something in it fails the removal and says so, which is a person's decision to make rather than the tool's.",
    enforced_by: &[crate::anchor::inv_delete_001],
    tests: &[crate::anchor::an_empty_directory_is_still_removed, crate::anchor::a_resource_removed_from_the_configuration_is_removed_from_the_machine],
);
