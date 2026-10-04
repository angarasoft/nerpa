use dacc_core::NonEmptyStr;

dacc_work::limitation!("l-expiry-is-decorative",
    title: NonEmptyStr::new("The actor's expiry is read, carried, and checked by nothing"),
    violated: NonEmptyStr::new("An identity is valid until the expiry it names: `--as-expiry` says when this identity stops being valid, and a run does not proceed under an actor whose time has passed."),
);
