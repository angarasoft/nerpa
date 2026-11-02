use dacc_core::NonEmptyStr;

dacc_work::limitation!("l-yc-credential-model-absent",
    title: NonEmptyStr::new("The Yandex provider holds no credential model, and adr-2026-075 asks for one"),
    violated: NonEmptyStr::new("adr-2026-075 asks that a long-lived credential is refused by name, held by a test, and that no Debug output prints key or token material. The provider has no service-account key flow, no test that refuses a long-lived credential, and Auth::IamToken and Resolved derive Debug with the token in them."),
);
