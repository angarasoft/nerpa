use dacc_core::NonEmptyStr;

dacc_work::thrust!("t-converges-to-invariants",
    title: NonEmptyStr::new("The built system converges to its declared invariants"),
    outcome: NonEmptyStr::new(
        "The seven open divergences — three in the os provider, four in the core — are closed, each against the baseline specification that names the invariant it violated."
    ),
);
