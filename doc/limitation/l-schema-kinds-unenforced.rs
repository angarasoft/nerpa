use dacc_core::NonEmptyStr;

dacc_work::limitation!("l-schema-kinds-unenforced",
    title: NonEmptyStr::new("The planner never consults the monotonic and stop-to-change kinds"),
    violated: NonEmptyStr::new("monotonic refuses a shrink while planning, and stop-to-change marks a change that needs the resource stopped first. The planner calls neither grows_only nor requires_stop: only the Schema's own tests do."),
);
