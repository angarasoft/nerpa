//! Generates the Yandex Cloud gRPC client from the vendored protos.
//!
//! The protos live in `proto/`, fetched from the Yandex `cloudapi` repository
//! and the `googleapis` repository; the `google/protobuf` well-known types come
//! from the `protoc` installation and are mapped by `prost-types`, not
//! regenerated.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure().compile_protos(
        &[
            "proto/yandex/cloud/compute/v1/instance_service.proto",
            "proto/yandex/cloud/compute/v1/disk_service.proto",
            "proto/yandex/cloud/vpc/v1/network_service.proto",
            "proto/yandex/cloud/vpc/v1/subnet_service.proto",
            "proto/yandex/cloud/vpc/v1/security_group_service.proto",
        ],
        &["proto/", "/usr/include"],
    )?;
    Ok(())
}
