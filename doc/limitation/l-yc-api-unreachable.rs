use dacc_core::NonEmptyStr;

dacc_work::limitation!("l-yc-api-unreachable",
    title: NonEmptyStr::new("The Yandex client cannot reach the API it was landed for"),
    violated: NonEmptyStr::new("The compute and vpc resources are read and changed through the Yandex gRPC API, authenticated by a short-lived token. The JWT the client signs carries RS256, which the IAM endpoint refuses — only PS256 is accepted — and the channel is built without a TLS feature, so an https endpoint is dialled plaintext. No exchange succeeds."),
);
