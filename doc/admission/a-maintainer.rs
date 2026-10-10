use dacc_core::NonEmptyStr;

dacc_work::admission!("a-maintainer",
    identity: dacc_work::Identity::Human {
        name: NonEmptyStr::new("Anishchuk Sergei"),
    },
    rights: dacc_work::Rights {
        authorship: true,
        landing: true,
        irreversible: true,
    },
);
