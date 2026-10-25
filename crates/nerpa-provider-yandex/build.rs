//! Generates the Yandex Cloud gRPC client from the vendored protos.
//!
//! The protos live in `proto/`, fetched from the Yandex `cloudapi` repository
//! and the `googleapis` repository; their revisions and licences are recorded
//! in `proto/README.md`. The protoc binary comes from `protoc-bin-vendored`,
//! so the build needs nothing from the system: no binary in `PATH`, no
//! headers under `/usr/include`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    unsafe_code,
    reason = "a build script reports and stops, and sets its own PROTOC before anything parallel reads it"
)]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // SAFETY: a build script runs before anything parallel; setting PROTOC
    // here only tells prost-build which binary to invoke, and the variable is
    // never read back into this process for anything else.
    unsafe {
        std::env::set_var("PROTOC", protoc_bin_vendored::protoc_bin_path()?);
    }
    let include = protoc_bin_vendored::include_path()?;
    tonic_prost_build::configure().compile_protos(
        &[
            "proto/yandex/cloud/compute/v1/instance_service.proto",
            "proto/yandex/cloud/compute/v1/disk_service.proto",
            "proto/yandex/cloud/operation/operation_service.proto",
            "proto/yandex/cloud/vpc/v1/network_service.proto",
            "proto/yandex/cloud/vpc/v1/subnet_service.proto",
            "proto/yandex/cloud/vpc/v1/security_group_service.proto",
            "proto/yandex/cloud/storage/v1/bucket_service.proto",
            "proto/yandex/cloud/iam/v1/iam_token_service.proto",
        ],
        &[
            "proto/",
            include.to_str().expect("the vendored include path is text"),
        ],
    )?;
    Ok(())
}
