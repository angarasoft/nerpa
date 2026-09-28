use dacc_core::NonEmptyStr;

dacc_work::thrust!(6,
    title: NonEmptyStr::new("Stage 6 — other people's work"),
    outcome: NonEmptyStr::new(
        "A provider built outside this repository loads, creates a resource, and demonstrably cannot read a credential value or reach a host outside its manifest."
    ),
);
