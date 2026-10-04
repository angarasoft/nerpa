use dacc_core::NonEmptyStr;

dacc_work::limitation!("l-refusal-names-nothing",
    title: NonEmptyStr::new("A refusal that promises to name what moved can come out naming nothing"),
    violated: NonEmptyStr::new("A signature taken before an out-of-band change is refused, naming what moved."),
);
