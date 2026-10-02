use dacc_core::NonEmptyStr;

dacc_work::limitation!("l-born-attributes-never-work",
    title: NonEmptyStr::new("An attribute a resource was born with becomes work"),
    violated: NonEmptyStr::new("An attribute a resource was born with is reported, never turned into work."),
);
