use dacc_core::NonEmptyStr;

dacc_work::limitation!("l-changelog-is-convention",
    title: NonEmptyStr::new("The changelog entry the corpus promises is written by nobody and checked by nothing"),
    violated: NonEmptyStr::new("A change to the plan output of a recorded configuration fails until the golden is updated and a changelog entry is written."),
);
