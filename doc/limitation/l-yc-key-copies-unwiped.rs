use dacc_core::NonEmptyStr;

dacc_work::limitation!("l-yc-key-copies-unwiped",
    title: NonEmptyStr::new("A copy of the Yandex service-account key outlives the signature in memory nerpa does not own"),
    violated: NonEmptyStr::new("adr-2026-075 asked that the key's bytes be zeroized once the JWT is signed. nerpa wipes the buffers it owns, but ring 0.17.14 keeps the parsed RSA key until it is dropped and does not wipe it, and the JSON parser keeps a scratch buffer it does not wipe. Until the signer is replaced by one that wipes, or by one outside the process, a copy of the key can outlive the signature in freed memory."),
);
