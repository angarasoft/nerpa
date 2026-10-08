use dacc_core::NonEmptyStr;
use dacc_work::{Identity, Rights};

dacc_work::admission!("a-agent",
    identity: Identity::Agent {
        name: NonEmptyStr::new("Qwen Code"),
        model: NonEmptyStr::new("qwen-code"),
    },
    rights: Rights {
        authorship: true,
        landing: true,
        irreversible: false,
    },
);
