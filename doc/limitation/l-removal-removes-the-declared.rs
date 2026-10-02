use dacc_core::NonEmptyStr;

dacc_work::limitation!("l-removal-removes-the-declared",
    title: NonEmptyStr::new("A removal takes what was never declared"),
    violated: NonEmptyStr::new("Removing a resource removes that resource and nothing that was never declared."),
);
