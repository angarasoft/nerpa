use dacc_core::NonEmptyStr;

dacc_work::thrust!("t-stage-4-access",
    title: NonEmptyStr::new("Stage 4 — who is allowed to do what"),
    outcome: NonEmptyStr::new(
        "A run where one party applies and another signs, on the live fleet; an apply with a signature taken before an out-of-band change is refused, naming what moved; the log verifies from the first entry."
    ),
);
