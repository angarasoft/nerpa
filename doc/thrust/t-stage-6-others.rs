use dacc_core::NonEmptyStr;

dacc_work::thrust!("t-stage-6-others",
    title: NonEmptyStr::new("Stage 6 — other people's work"),
    outcome: NonEmptyStr::new(
        "A provider built outside this repository loads, creates a resource, and demonstrably cannot read a credential value or reach a host outside its manifest."
    ),
);
