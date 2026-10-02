use dacc_core::NonEmptyStr;

dacc_work::limitation!("l-replanning-is-empty",
    title: NonEmptyStr::new("A reference to an attribute changed in one plan resolves to the old value"),
    violated: NonEmptyStr::new("Applying a plan and planning again produces an empty plan."),
);
