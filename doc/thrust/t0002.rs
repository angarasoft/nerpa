use dacc_core::NonEmptyStr;

dacc_work::thrust!(2,
    title: NonEmptyStr::new("Stage 2 — what cannot be changed later"),
    outcome: NonEmptyStr::new(
        "A password in a configuration appears in neither the state file nor the plan, and the resource that uses it still converges; a resource renamed in the configuration keeps its identity and is not rebuilt; a value derived from a fact works, and an attempt to decide a resource's existence from one is refused with a line number."
    ),
);
