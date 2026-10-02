use dacc_core::NonEmptyStr;

dacc_work::limitation!("l-state-keeps-no-values",
    title: NonEmptyStr::new("The state file keeps what it must not keep"),
    violated: NonEmptyStr::new("State holds references to secrets and digests of their values, never the values."),
);
