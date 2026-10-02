use dacc_core::NonEmptyStr;

dacc_work::limitation!("l-refusal-per-resource",
    title: NonEmptyStr::new("A machine that stops answering fails a whole read"),
    violated: NonEmptyStr::new("A machine that did not answer is refused per resource, not per run."),
);
