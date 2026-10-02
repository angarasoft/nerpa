use dacc_core::NonEmptyStr;

dacc_work::limitation!("l-waiting-says-waiting",
    title: NonEmptyStr::new("A resource that waits reads as unreachable"),
    violated: NonEmptyStr::new("What waits for something that cannot be planned is said to wait for it, whatever its domain, and only a machine that did not answer is reported as one."),
);
