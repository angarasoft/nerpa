use dacc_core::NonEmptyStr;
use dacc_work::{Identity, Rights};

dacc_work::admission!("a-maintainer",
    identity: Identity::Human {
        name: NonEmptyStr::new("Anishchuk Sergei"),
    },
    rights: Rights {
        authorship: true,
        landing: true,
        irreversible: true,
    },
);
