use dacc_core::NonEmptyStr;

dacc_work::limitation!("l-yc-cannot-create",
    title: NonEmptyStr::new("The Yandex provider cannot create or resize an instance in a real cloud"),
    violated: NonEmptyStr::new("A control-domain provider creates and changes the resources a configuration declares, in the cloud it reaches. The Yandex provider's create request carries no boot disk and no network interface, no operation it starts is awaited, and a running instance is patched without being stopped first, so it cannot create or resize an instance in a real cloud."),
);
