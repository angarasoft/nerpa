use dacc_core::NonEmptyStr;

dacc_work::thrust!(3,
    title: NonEmptyStr::new("Stage 3 — the cloud, and a way in from Terraform"),
    outcome: NonEmptyStr::new(
        "One real environment moves across without recreating a single resource; nerpa plan immediately after import is empty; import reports every secret it found in clear text in the old state; recorded-exchange tests cover the provider."
    ),
);
