//! A fake Yandex Cloud: the seven gRPC services the provider speaks, in one
//! process, backed by in-memory maps and a ledger of what happened.
//!
//! One process means a converge test drives the provider against a world it
//! can inspect: the ledger says what the provider did, and the maps say what
//! the world holds after. An operation finishes on its second poll, which is
//! about two hundred milliseconds of a run.

// Tests are allowed to be blunt. See docs/CODING_STANDARDS.md.
// The module is shared by test binaries that use different parts of it, so
// what one leaves unread another exercises; dead code is that sharing, not
// neglect.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unused_async,
    dead_code,
    reason = "the fake's shapes are fixed by the generated traits, and tests are allowed to be blunt"
)]

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use nerpa_core::address::address;
use nerpa_core::attribute::Reference;
use nerpa_core::provider::{Machines, Providers};
use nerpa_core::run;
use nerpa_core::schema::Schema;
use nerpa_core::secret::{Key, Remembered, Secrets};
use nerpa_core::state::State as EngineState;
use nerpa_core::{Actor, Graph, Observed, Plan, Resource, Value};
use nerpa_provider_yandex::Yandex;
use nerpa_provider_yandex::auth::ServiceAccountKey;
use nerpa_provider_yandex::credentials::Credentials;
use nerpa_provider_yandex::yandex::cloud::access::{
    ListAccessBindingsRequest, ListAccessBindingsResponse, SetAccessBindingsRequest,
    UpdateAccessBindingsRequest,
};
use nerpa_provider_yandex::yandex::cloud::compute::v1::disk_service_server::{
    DiskService, DiskServiceServer,
};
use nerpa_provider_yandex::yandex::cloud::compute::v1::instance::Status as InstanceStatus;
use nerpa_provider_yandex::yandex::cloud::compute::v1::instance_service_server::{
    InstanceService, InstanceServiceServer,
};
use nerpa_provider_yandex::yandex::cloud::compute::v1::{
    AddInstanceOneToOneNatRequest, AttachInstanceDiskRequest, AttachInstanceFilesystemRequest,
    AttachInstanceNetworkInterfaceRequest, AttachedDisk, CreateDiskRequest, CreateInstanceRequest,
    DeleteDiskRequest, DeleteInstanceRequest, DetachInstanceDiskRequest,
    DetachInstanceFilesystemRequest, DetachInstanceNetworkInterfaceRequest, Disk, GetDiskRequest,
    GetInstanceRequest, GetInstanceSerialPortOutputRequest, GetInstanceSerialPortOutputResponse,
    Instance, ListDiskOperationsRequest, ListDiskOperationsResponse,
    ListDiskSnapshotSchedulesRequest, ListDiskSnapshotSchedulesResponse, ListDisksRequest,
    ListDisksResponse, ListInstanceOperationsRequest, ListInstanceOperationsResponse,
    ListInstancesRequest, ListInstancesResponse, MoveDiskRequest, MoveInstanceRequest,
    NetworkInterface, RelocateDiskRequest, RelocateInstanceRequest,
    RemoveInstanceOneToOneNatRequest, Resources, RestartInstanceRequest,
    SimulateInstanceMaintenanceEventRequest, StartInstanceRequest, StopInstanceRequest,
    UpdateDiskRequest, UpdateInstanceMetadataRequest, UpdateInstanceNetworkInterfaceRequest,
    UpdateInstanceRequest, attached_disk_spec, create_disk_request, disk,
};
use nerpa_provider_yandex::yandex::cloud::iam::v1::create_iam_token_request::Identity;
use nerpa_provider_yandex::yandex::cloud::iam::v1::iam_token_service_server::{
    IamTokenService, IamTokenServiceServer,
};
use nerpa_provider_yandex::yandex::cloud::iam::v1::{
    CreateIamTokenForServiceAccountRequest, CreateIamTokenRequest, CreateIamTokenResponse,
    RevokeIamTokenRequest, RevokeIamTokenResponse,
};
use nerpa_provider_yandex::yandex::cloud::operation::operation::Result as OperationResult;
use nerpa_provider_yandex::yandex::cloud::operation::operation_service_server::{
    OperationService, OperationServiceServer,
};
use nerpa_provider_yandex::yandex::cloud::operation::{
    CancelOperationRequest, GetOperationRequest, Operation,
};
use nerpa_provider_yandex::yandex::cloud::storage::v1::bucket_service_server::{
    BucketService, BucketServiceServer,
};
use nerpa_provider_yandex::yandex::cloud::storage::v1::{
    Bucket, BucketStats, CreateBucketInventoryConfigurationRequest, CreateBucketRequest,
    DeleteBucketHttpsConfigRequest, DeleteBucketInventoryConfigurationRequest, DeleteBucketRequest,
    GetBucketHttpsConfigRequest, GetBucketInventoryConfigurationRequest, GetBucketRequest,
    GetBucketStatsRequest, HttpsConfig, InventoryConfiguration,
    ListBucketInventoryConfigurationsRequest, ListBucketInventoryConfigurationsResponse,
    ListBucketsRequest, ListBucketsResponse, SetBucketHttpsConfigRequest, UpdateBucketRequest,
};
use nerpa_provider_yandex::yandex::cloud::vpc::v1::network_service_server::{
    NetworkService, NetworkServiceServer,
};
use nerpa_provider_yandex::yandex::cloud::vpc::v1::security_group_service_server::{
    SecurityGroupService, SecurityGroupServiceServer,
};
use nerpa_provider_yandex::yandex::cloud::vpc::v1::subnet_service_server::{
    SubnetService, SubnetServiceServer,
};
use nerpa_provider_yandex::yandex::cloud::vpc::v1::{
    AddSubnetCidrBlocksRequest, CreateNetworkRequest, CreateSecurityGroupRequest,
    CreateSubnetRequest, DeleteNetworkRequest, DeleteSecurityGroupRequest, DeleteSubnetRequest,
    GetNetworkRequest, GetSecurityGroupRequest, GetSubnetRequest, ListNetworkOperationsRequest,
    ListNetworkOperationsResponse, ListNetworkRouteTablesRequest, ListNetworkRouteTablesResponse,
    ListNetworkSecurityGroupsRequest, ListNetworkSecurityGroupsResponse, ListNetworkSubnetsRequest,
    ListNetworkSubnetsResponse, ListNetworksRequest, ListNetworksResponse,
    ListSecurityGroupOperationsRequest, ListSecurityGroupOperationsResponse,
    ListSecurityGroupsRequest, ListSecurityGroupsResponse, ListSubnetOperationsRequest,
    ListSubnetOperationsResponse, ListSubnetsRequest, ListSubnetsResponse,
    ListUsedAddressesRequest, ListUsedAddressesResponse, MoveNetworkRequest,
    MoveSecurityGroupRequest, MoveSubnetRequest, Network, RelocateSubnetRequest,
    RemoveSubnetCidrBlocksRequest, SecurityGroup, Subnet, UpdateNetworkRequest,
    UpdateSecurityGroupRequest, UpdateSecurityGroupRuleRequest, UpdateSecurityGroupRulesRequest,
    UpdateSubnetRequest,
};
use prost_types::Timestamp;
use ring::signature::{RSA_PSS_2048_8192_SHA256, UnparsedPublicKey};
use tonic::{Request, Response, Status};

/// A gibibyte, the unit a declaration's sizes are given in.
pub(crate) const GIB: i64 = 1_073_741_824;

/// The default zone the scenarios declare their resources in: `d`, the zone
/// that stays available after the datacenter attack; the cloud refuses
/// allocation in the others.
pub(crate) const ZONE: &str = "ru-central1-d";

/// The image the scenarios make disks and boot disks from: a public Ubuntu
/// image, by id — a disk's create refuses a family name, so the scenario
/// names the image the way a disk can take it.
pub(crate) const IMAGE: &str = "fd8em71f8oo1hdpali1k";

/// The token the fake's IAM service hands out; every resource service checks
/// for it as a bearer.
pub(crate) const TOKEN: &str = "t1.test-token";

/// The service account key the fake's IAM service accepts: the test key whose
/// public half is checked into `src/rsa_public_key_test.der`.
pub(crate) const KEY_PEM: &str = "\
-----BEGIN PRIVATE KEY-----
MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQCNOxt1XCNROG3J
oNRCc0ehoX9URZU/rG/XVuEdV/pGQjmq1lcoG1b9nSky8mL9upT5SXcFTwZk97KW
yH4Tj54BLNmNhPwX38ggJuz/SjVMXTBAIeCoGIJorfZlrL132vBZvAusmllIZSft
5bsCBDOYKzh6djarqpAkc86jH/DtR0+wkTwsz2FiSu39o7v6/OWgmstyYvuq2CYK
f0yUKrF8TI7TbpDCjxGCHzl/D6SmuOwACwkebt1RbHkHaPgqua4oqpI7VBsp8/QR
h+xsb9PqsQNjxY1QShRO7B0XdvzrjLQkxZU7C3/sbvdA0zWKXWSvnnfUDEMV/O8U
R7XECdNhAgMBAAECggEADL+1LtbuaTqKzpSyFH2Ljc+iSt0tJ95NCkdRcvCqKwV3
ZzCKFZjCNYqmWW7/+J+LKyWylMwZa6vgdzQKDYH9ZWXDDb9fclhFHIHnCvSL8yTG
YkOsn//2ZbSudWcug09EOxV6iE42mQ5omFUQmJIAhlAAfS2GqeBvupqpoJgVi5Wu
C38awBc3g8T6NAbXRp6c6OcyW6+sHyz7jomwwIkvWwZLedm6UP+9xYCCYPOb44Bf
PYwiSx2wXpu1a7BlhJuyPvtNGUUqGHwBP4fsM50pp9kr9CQOxg7JtxjGwWE7Sahe
kVCW/5tj4WwkIQ+tP1e8OOYCBm2K11wrP5zu4/MPQQKBgQC/OUKhnYgiXPcGUAaA
GnIF/xIJbNulZHBwNon+5KKzJLlKhhq6TlZXIjduBYc16KGcbfD557RK7C1Nf3mP
7jOfc7+xwM4f2acaQFIJpI36/DjTX1MJFw/3amENWT7r47jekcC1EJGj0SeWC2Mk
IKzWPs9aLUn020xfpHuuiqrP5QKBgQC9EoTbVIQEj26mCJ1Dmlv8jtX866aAYr57
sMRZiFQ+jKR3tBc/v206O/olw7PXgB0TxYlc9q+wgBKC96wyMFrVpO0fobeDSn9R
cUScLh6MCS0TOkceLpnXZvH4SFhny/QE7ZTfdw0udlK8jFZZzo7+6/Pbgx8G4I17
tsGI1IxlzQKBgGGWsWCVarJOy2qXUJnz5RkWUXgp6/Z+SbuaudYxjK7sIPvdcKp/
7CRPvAYUICrs6/1PRlnjW+mYmo4gbkN8BzBq+/uVMBylEVieVr/BEQxzx/Rng5Ly
CB32Vk1frHLqJqC/tZ8yM4Fs7kzuxsvbORmluGCHGGUzvchZwx0Se5XZAoGAJGTX
K/zVuKb/TnhkBSpGQJQh/qBBmX7oWZjKAs3qIWwjEGObAgtP9CCtYu2LxZSEhFTG
vx1A7kGIzqrr+3ExWuR3PFp9n4kMftnFsiZyfNQbWBMks1uXA05ujzMlyT0KCJTO
zkD1SZc/D8oXqQvInKtAcmWH6GP69/BSyorXR5UCgYBUdRa1fb2nW2WYua8olY4+
36dAz/qOQnhw8XX40lCjupMpEMw5KHhHzzT9lrxUT3Uw1q4dH9VIy0gBg4WdtCGa
lvQlisiuEKZy3XkFO412zbEW9pJPRky+zi+sy+9VvbvRdUp62X9NgiZ7CD60iltA
NFOvnrJCq7mtBmQIvPZixQ==
-----END PRIVATE KEY-----
";

/// The test service account key the fake's IAM service accepts.
#[must_use]
pub(crate) fn a_key() -> ServiceAccountKey {
    ServiceAccountKey {
        id: "ajekey".to_owned(),
        service_account_id: "ajeservice".to_owned(),
        private_key: KEY_PEM.to_owned(),
    }
}

/// The credential file a run is configured with, spelling [`a_key`].
#[must_use]
pub(crate) fn a_credentials() -> Credentials {
    let json = serde_json::json!({
        "id": "ajekey",
        "service_account_id": "ajeservice",
        "private_key": KEY_PEM,
    });
    Credentials::from_json(&json.to_string()).expect("a usable credential")
}

/// The methods the fake does not need, refused with `unimplemented` exactly
/// like a service that never grew them.
///
/// Written in the desugared shape of the generated trait's `async fn`: the
/// `async_trait` attribute on the impl transforms only what it can see, and a
/// macro invocation is opaque to it.
macro_rules! unserved {
    ($($method:ident($request:ty) -> $response:ty;)*) => {
        $(
            fn $method<'life0, 'async_trait>(
                &'life0 self,
                _request: tonic::Request<$request>,
            ) -> ::core::pin::Pin<
                Box<
                    dyn ::core::future::Future<
                            Output = std::result::Result<
                                tonic::Response<$response>,
                                tonic::Status,
                            >,
                        > + Send
                        + 'async_trait,
                >,
            >
            where
                'life0: 'async_trait,
                Self: 'async_trait,
            {
                Box::pin(async move {
                    Err(tonic::Status::unimplemented(concat!(
                        "the fake does not serve ",
                        stringify!($method)
                    )))
                })
            }
        )*
    };
}

/// Re-exported so a sibling module can call the macro by path.
pub(crate) use unserved;

/// The fake's world: the resources it holds, the operations in flight, and
/// what happened. One mutex guards it all, so no two answers interleave.
#[derive(Debug, Default)]
struct State {
    instances: BTreeMap<String, Instance>,
    disks: BTreeMap<String, Disk>,
    networks: BTreeMap<String, Network>,
    subnets: BTreeMap<String, Subnet>,
    groups: BTreeMap<String, SecurityGroup>,
    buckets: BTreeMap<String, Bucket>,
    operations: BTreeMap<String, Operation>,
    polls: BTreeMap<String, u32>,
    ledger: Vec<String>,
    instances_next: u64,
    disks_next: u64,
    networks_next: u64,
    subnets_next: u64,
    groups_next: u64,
    buckets_next: u64,
    operations_next: u64,
}

impl State {
    /// Opens an operation and writes down what it does; it is not done until
    /// its second poll, which is what makes a wait visible.
    fn begin_operation(&mut self, note: String) -> Operation {
        self.operations_next += 1;
        let id = format!("op-{}", self.operations_next);
        let operation = Operation {
            id: id.clone(),
            done: false,
            ..Default::default()
        };
        self.operations.insert(id.clone(), operation.clone());
        self.polls.insert(id, 0);
        self.ledger.push(note);
        operation
    }
}

/// The fake, shared by value: the state is one mutex behind an [`Arc`].
#[derive(Debug, Clone, Default)]
pub(crate) struct FakeYandex {
    state: Arc<Mutex<State>>,
}

impl FakeYandex {
    /// Starts the fake on a free local port, serving all seven services, and
    /// hands back the handle and the endpoint it is listening on. The serving
    /// thread outlives the tests and dies with the process.
    pub(crate) fn start() -> (Self, SocketAddr) {
        let fake = Self::default();
        let server = fake.clone();
        let (sent, received) = mpsc::channel();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().expect("a runtime");
            runtime.block_on(async move {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                    .await
                    .expect("bound");
                let address = listener.local_addr().expect("an address");
                sent.send(address).expect("delivered");
                tonic::transport::Server::builder()
                    .add_service(IamTokenServiceServer::new(server.clone()))
                    .add_service(InstanceServiceServer::new(server.clone()))
                    .add_service(DiskServiceServer::new(server.clone()))
                    .add_service(NetworkServiceServer::new(server.clone()))
                    .add_service(SubnetServiceServer::new(server.clone()))
                    .add_service(SecurityGroupServiceServer::new(server.clone()))
                    .add_service(OperationServiceServer::new(server.clone()))
                    .add_service(BucketServiceServer::new(server.clone()))
                    .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                    .await
                    .expect("served");
            });
        });
        let address = received.recv().expect("an address");
        (fake, address)
    }

    /// What happened, in the order it happened.
    pub(crate) fn ledger(&self) -> Vec<String> {
        self.state.lock().expect("the fake's state").ledger.clone()
    }

    /// Whether the named instance is stopped, for asserting what the provider
    /// left behind.
    pub(crate) fn is_stopped(&self, instance_id: &str) -> bool {
        self.state
            .lock()
            .expect("the fake's state")
            .instances
            .get(instance_id)
            .is_some_and(|instance| instance.status == i32::from(InstanceStatus::Stopped))
    }

    /// Stops the named instance behind the API's back, the way a preemptible
    /// stop would: the provider's next read sees a stopped instance.
    pub(crate) fn set_stopped(&self, instance_id: &str) {
        let mut state = self.state.lock().expect("the fake's state");
        state
            .instances
            .get_mut(instance_id)
            .expect("a made instance")
            .status = i32::from(InstanceStatus::Stopped);
    }

    /// Writes one line of the ledger without opening an operation.
    fn note(&self, note: &str) {
        self.state
            .lock()
            .expect("the fake's state")
            .ledger
            .push(note.to_owned());
    }
}

/// Refuses a request that does not carry the fake's token, exactly like
/// every resource service on the real cloud.
pub(crate) fn authorized<T>(request: &Request<T>) -> Result<(), Status> {
    let bearer = format!("Bearer {TOKEN}");
    match request.metadata().get("authorization") {
        Some(value) if matches!(value.to_str(), Ok(value) if value == bearer) => Ok(()),
        _ => Err(Status::unauthenticated("not the fake's token")),
    }
}

/// The fake's IAM service: it accepts the test key's signature and hands out
/// [`TOKEN`], which every other service then checks for.
#[tonic::async_trait]
impl IamTokenService for FakeYandex {
    async fn create(
        &self,
        request: Request<CreateIamTokenRequest>,
    ) -> Result<Response<CreateIamTokenResponse>, Status> {
        let Some(Identity::Jwt(jwt)) = request.into_inner().identity else {
            return Err(Status::invalid_argument("the identity must be a JWT"));
        };
        if !jwt_accepted(&jwt) {
            return Err(Status::unauthenticated("the JWT does not verify"));
        }
        self.note("iam exchange");
        Ok(Response::new(CreateIamTokenResponse {
            iam_token: TOKEN.to_owned(),
            expires_at: Some(an_hour_hence()),
        }))
    }

    unserved!(
        create_for_service_account(CreateIamTokenForServiceAccountRequest) -> CreateIamTokenResponse;
        revoke(RevokeIamTokenRequest) -> RevokeIamTokenResponse;
    );
}

/// Whether the JWT was signed by the test key with the one algorithm the real
/// endpoint accepts.
pub(crate) fn jwt_accepted(jwt: &str) -> bool {
    let Some((signing, signature)) = jwt.rsplit_once('.') else {
        return false;
    };
    let Some((header, claims)) = signing.split_once('.') else {
        return false;
    };
    let Ok(decoded) = URL_SAFE_NO_PAD.decode(header) else {
        return false;
    };
    let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(&decoded) else {
        return false;
    };
    if parsed.get("alg").and_then(serde_json::Value::as_str) != Some("PS256") {
        return false;
    }
    let Ok(signature) = URL_SAFE_NO_PAD.decode(signature) else {
        return false;
    };
    let public = include_bytes!("../../src/rsa_public_key_test.der");
    UnparsedPublicKey::new(&RSA_PSS_2048_8192_SHA256, public)
        .verify(format!("{header}.{claims}").as_bytes(), &signature)
        .is_ok()
}

/// An expiry an hour away, so the provider's carried token never runs out
/// during a test.
pub(crate) fn an_hour_hence() -> Timestamp {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("after the epoch")
        .as_secs();
    Timestamp {
        seconds: i64::try_from(seconds).expect("a sane clock") + 3_600,
        nanos: 0,
    }
}

#[tonic::async_trait]
impl InstanceService for FakeYandex {
    async fn get(
        &self,
        request: Request<GetInstanceRequest>,
    ) -> Result<Response<Instance>, Status> {
        authorized(&request)?;
        let instance_id = request.into_inner().instance_id;
        let state = self.state.lock().expect("the fake's state");
        state
            .instances
            .get(&instance_id)
            .cloned()
            .ok_or_else(|| Status::not_found("no such instance"))
            .map(Response::new)
    }

    async fn list(
        &self,
        request: Request<ListInstancesRequest>,
    ) -> Result<Response<ListInstancesResponse>, Status> {
        authorized(&request)?;
        let folder_id = request.into_inner().folder_id;
        let state = self.state.lock().expect("the fake's state");
        let mut instances: Vec<Instance> = state
            .instances
            .values()
            .filter(|instance| instance.folder_id == folder_id)
            .cloned()
            .collect();
        // The real List omits metadata; the provider reads user data through
        // Get, exactly as a caller of the real API must.
        for instance in &mut instances {
            instance.metadata = HashMap::new();
        }
        Ok(Response::new(ListInstancesResponse {
            instances,
            ..Default::default()
        }))
    }

    async fn create(
        &self,
        request: Request<CreateInstanceRequest>,
    ) -> Result<Response<Operation>, Status> {
        authorized(&request)?;
        let CreateInstanceRequest {
            folder_id,
            name,
            zone_id,
            platform_id,
            resources_spec,
            boot_disk_spec,
            metadata,
            network_interface_specs,
            ..
        } = request.into_inner();
        let boot_disk_spec = boot_disk_spec
            .ok_or_else(|| Status::invalid_argument("an instance is made with a boot disk"))?;
        let Some(attached_disk_spec::Disk::DiskSpec(attached_disk_spec::DiskSpec {
            source: Some(attached_disk_spec::disk_spec::Source::ImageId(image)),
            ..
        })) = boot_disk_spec.disk
        else {
            return Err(Status::invalid_argument(
                "the boot disk must be made from an image",
            ));
        };
        let mut state = self.state.lock().expect("the fake's state");
        if state
            .instances
            .values()
            .any(|instance| instance.folder_id == folder_id && instance.name == name)
        {
            return Err(Status::already_exists("an instance with this name exists"));
        }
        state.instances_next += 1;
        let instance_id = format!("inst-{}", state.instances_next);
        let boot_disk_id = format!("{instance_id}-boot");
        // The boot disk the instance asked for, made beside it: the image's
        // own size and the default type, as the spec's zero promised.
        state.disks.insert(
            boot_disk_id.clone(),
            Disk {
                id: boot_disk_id.clone(),
                folder_id: folder_id.clone(),
                name: format!("{instance_id}-boot"),
                type_id: "network-ssd".to_owned(),
                zone_id: zone_id.clone(),
                size: 10 * GIB,
                source: Some(disk::Source::SourceImageId(image)),
                ..Default::default()
            },
        );
        state.instances.insert(
            instance_id.clone(),
            Instance {
                id: instance_id,
                folder_id: folder_id.clone(),
                name: name.clone(),
                zone_id,
                platform_id,
                resources: resources_spec.map(|spec| Resources {
                    memory: spec.memory,
                    cores: spec.cores,
                    core_fraction: spec.core_fraction,
                    gpus: spec.gpus,
                }),
                status: i32::from(InstanceStatus::Running),
                metadata,
                boot_disk: Some(AttachedDisk {
                    mode: i32::from(attached_disk_spec::Mode::ReadWrite),
                    device_name: "vda".to_owned(),
                    auto_delete: true,
                    disk_id: boot_disk_id,
                }),
                network_interfaces: network_interface_specs
                    .into_iter()
                    .enumerate()
                    .map(|(index, spec)| NetworkInterface {
                        index: index.to_string(),
                        mac_address: format!("fa:16:3e:00:00:{:02x}", index + 1),
                        subnet_id: spec.subnet_id,
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            },
        );
        let operation = state.begin_operation(format!("create-instance {name}"));
        Ok(Response::new(operation))
    }

    async fn update(
        &self,
        request: Request<UpdateInstanceRequest>,
    ) -> Result<Response<Operation>, Status> {
        authorized(&request)?;
        let request = request.into_inner();
        let instance_id = request.instance_id.clone();
        let mut state = self.state.lock().expect("the fake's state");
        match state.instances.get_mut(&request.instance_id) {
            Some(instance) => {
                // The real API refuses a resources change while running.
                if request.resources_spec.is_some()
                    && instance.status == i32::from(InstanceStatus::Running)
                {
                    return Err(Status::failed_precondition(
                        "the instance must be stopped to change its resources",
                    ));
                }
                if let Some(spec) = request.resources_spec {
                    instance.resources = Some(Resources {
                        memory: spec.memory,
                        cores: spec.cores,
                        core_fraction: spec.core_fraction,
                        gpus: spec.gpus,
                    });
                }
            }
            None => return Err(Status::not_found("no such instance")),
        }
        let operation = state.begin_operation(format!("update-instance {instance_id}"));
        Ok(Response::new(operation))
    }

    async fn delete(
        &self,
        request: Request<DeleteInstanceRequest>,
    ) -> Result<Response<Operation>, Status> {
        authorized(&request)?;
        let instance_id = request.into_inner().instance_id;
        let mut state = self.state.lock().expect("the fake's state");
        let Some(instance) = state.instances.remove(&instance_id) else {
            return Err(Status::not_found("no such instance"));
        };
        // The boot disk goes with the instance it boots.
        if let Some(attached) = &instance.boot_disk {
            state.disks.remove(&attached.disk_id);
        }
        let operation = state.begin_operation(format!("delete-instance {instance_id}"));
        Ok(Response::new(operation))
    }

    async fn stop(
        &self,
        request: Request<StopInstanceRequest>,
    ) -> Result<Response<Operation>, Status> {
        authorized(&request)?;
        let instance_id = request.into_inner().instance_id;
        let mut state = self.state.lock().expect("the fake's state");
        match state.instances.get_mut(&instance_id) {
            Some(instance) if instance.status == i32::from(InstanceStatus::Running) => {
                instance.status = i32::from(InstanceStatus::Stopped);
            }
            Some(_) => {
                return Err(Status::failed_precondition("the instance is not running"));
            }
            None => return Err(Status::not_found("no such instance")),
        }
        let operation = state.begin_operation(format!("stop {instance_id}"));
        Ok(Response::new(operation))
    }

    async fn start(
        &self,
        request: Request<StartInstanceRequest>,
    ) -> Result<Response<Operation>, Status> {
        authorized(&request)?;
        let instance_id = request.into_inner().instance_id;
        let mut state = self.state.lock().expect("the fake's state");
        match state.instances.get_mut(&instance_id) {
            Some(instance) if instance.status == i32::from(InstanceStatus::Stopped) => {
                instance.status = i32::from(InstanceStatus::Running);
            }
            Some(_) => {
                return Err(Status::failed_precondition("the instance is not stopped"));
            }
            None => return Err(Status::not_found("no such instance")),
        }
        let operation = state.begin_operation(format!("start {instance_id}"));
        Ok(Response::new(operation))
    }

    unserved!(
        update_metadata(UpdateInstanceMetadataRequest) -> Operation;
        get_serial_port_output(GetInstanceSerialPortOutputRequest) -> GetInstanceSerialPortOutputResponse;
        restart(RestartInstanceRequest) -> Operation;
        attach_disk(AttachInstanceDiskRequest) -> Operation;
        detach_disk(DetachInstanceDiskRequest) -> Operation;
        attach_filesystem(AttachInstanceFilesystemRequest) -> Operation;
        detach_filesystem(DetachInstanceFilesystemRequest) -> Operation;
        add_one_to_one_nat(AddInstanceOneToOneNatRequest) -> Operation;
        remove_one_to_one_nat(RemoveInstanceOneToOneNatRequest) -> Operation;
        update_network_interface(UpdateInstanceNetworkInterfaceRequest) -> Operation;
        attach_network_interface(AttachInstanceNetworkInterfaceRequest) -> Operation;
        detach_network_interface(DetachInstanceNetworkInterfaceRequest) -> Operation;
        list_operations(ListInstanceOperationsRequest) -> ListInstanceOperationsResponse;
        simulate_maintenance_event(SimulateInstanceMaintenanceEventRequest) -> Operation;
        r#move(MoveInstanceRequest) -> Operation;
        relocate(RelocateInstanceRequest) -> Operation;
        list_access_bindings(ListAccessBindingsRequest) -> ListAccessBindingsResponse;
        set_access_bindings(SetAccessBindingsRequest) -> Operation;
        update_access_bindings(UpdateAccessBindingsRequest) -> Operation;
    );
}

#[tonic::async_trait]
impl DiskService for FakeYandex {
    async fn get(&self, request: Request<GetDiskRequest>) -> Result<Response<Disk>, Status> {
        authorized(&request)?;
        let disk_id = request.into_inner().disk_id;
        let state = self.state.lock().expect("the fake's state");
        state
            .disks
            .get(&disk_id)
            .cloned()
            .ok_or_else(|| Status::not_found("no such disk"))
            .map(Response::new)
    }

    async fn list(
        &self,
        request: Request<ListDisksRequest>,
    ) -> Result<Response<ListDisksResponse>, Status> {
        authorized(&request)?;
        let folder_id = request.into_inner().folder_id;
        let state = self.state.lock().expect("the fake's state");
        let disks: Vec<Disk> = state
            .disks
            .values()
            .filter(|disk| disk.folder_id == folder_id)
            .cloned()
            .collect();
        Ok(Response::new(ListDisksResponse {
            disks,
            ..Default::default()
        }))
    }

    async fn create(
        &self,
        request: Request<CreateDiskRequest>,
    ) -> Result<Response<Operation>, Status> {
        authorized(&request)?;
        let CreateDiskRequest {
            folder_id,
            name,
            type_id,
            zone_id,
            size,
            source,
            ..
        } = request.into_inner();
        let source = source.map(|source| match source {
            create_disk_request::Source::ImageId(image) => disk::Source::SourceImageId(image),
            create_disk_request::Source::SnapshotId(snapshot) => {
                disk::Source::SourceSnapshotId(snapshot)
            }
        });
        let mut state = self.state.lock().expect("the fake's state");
        if state
            .disks
            .values()
            .any(|disk| disk.folder_id == folder_id && disk.name == name)
        {
            return Err(Status::already_exists("a disk with this name exists"));
        }
        state.disks_next += 1;
        let disk_id = format!("disk-{}", state.disks_next);
        state.disks.insert(
            disk_id.clone(),
            Disk {
                id: disk_id,
                folder_id,
                name: name.clone(),
                type_id,
                zone_id,
                size,
                source,
                ..Default::default()
            },
        );
        let operation = state.begin_operation(format!("create-disk {name}"));
        Ok(Response::new(operation))
    }

    async fn update(
        &self,
        request: Request<UpdateDiskRequest>,
    ) -> Result<Response<Operation>, Status> {
        authorized(&request)?;
        let request = request.into_inner();
        let disk_id = request.disk_id.clone();
        let mut state = self.state.lock().expect("the fake's state");
        match state.disks.get_mut(&request.disk_id) {
            Some(disk) => {
                // The real API refuses a shrink; the planner refuses before
                // the provider is ever asked.
                if request.size < disk.size {
                    return Err(Status::invalid_argument("a disk cannot shrink"));
                }
                disk.size = request.size;
            }
            None => return Err(Status::not_found("no such disk")),
        }
        let operation = state.begin_operation(format!("update-disk {disk_id}"));
        Ok(Response::new(operation))
    }

    async fn delete(
        &self,
        request: Request<DeleteDiskRequest>,
    ) -> Result<Response<Operation>, Status> {
        authorized(&request)?;
        let disk_id = request.into_inner().disk_id;
        let mut state = self.state.lock().expect("the fake's state");
        if state.disks.remove(&disk_id).is_none() {
            return Err(Status::not_found("no such disk"));
        }
        let operation = state.begin_operation(format!("delete-disk {disk_id}"));
        Ok(Response::new(operation))
    }

    unserved!(
        list_operations(ListDiskOperationsRequest) -> ListDiskOperationsResponse;
        r#move(MoveDiskRequest) -> Operation;
        relocate(RelocateDiskRequest) -> Operation;
        list_snapshot_schedules(ListDiskSnapshotSchedulesRequest) -> ListDiskSnapshotSchedulesResponse;
        list_access_bindings(ListAccessBindingsRequest) -> ListAccessBindingsResponse;
        set_access_bindings(SetAccessBindingsRequest) -> Operation;
        update_access_bindings(UpdateAccessBindingsRequest) -> Operation;
    );
}

#[tonic::async_trait]
impl NetworkService for FakeYandex {
    async fn list(
        &self,
        request: Request<ListNetworksRequest>,
    ) -> Result<Response<ListNetworksResponse>, Status> {
        authorized(&request)?;
        let folder_id = request.into_inner().folder_id;
        let state = self.state.lock().expect("the fake's state");
        let networks: Vec<Network> = state
            .networks
            .values()
            .filter(|network| network.folder_id == folder_id)
            .cloned()
            .collect();
        Ok(Response::new(ListNetworksResponse {
            networks,
            ..Default::default()
        }))
    }

    async fn create(
        &self,
        request: Request<CreateNetworkRequest>,
    ) -> Result<Response<Operation>, Status> {
        authorized(&request)?;
        let CreateNetworkRequest {
            folder_id, name, ..
        } = request.into_inner();
        let mut state = self.state.lock().expect("the fake's state");
        if state
            .networks
            .values()
            .any(|network| network.folder_id == folder_id && network.name == name)
        {
            return Err(Status::already_exists("a network with this name exists"));
        }
        state.networks_next += 1;
        let network_id = format!("net-{}", state.networks_next);
        state.networks.insert(
            network_id.clone(),
            Network {
                id: network_id,
                folder_id,
                name: name.clone(),
                ..Default::default()
            },
        );
        let operation = state.begin_operation(format!("create-network {name}"));
        Ok(Response::new(operation))
    }

    async fn delete(
        &self,
        request: Request<DeleteNetworkRequest>,
    ) -> Result<Response<Operation>, Status> {
        authorized(&request)?;
        let network_id = request.into_inner().network_id;
        let mut state = self.state.lock().expect("the fake's state");
        if state.networks.remove(&network_id).is_none() {
            return Err(Status::not_found("no such network"));
        }
        let operation = state.begin_operation(format!("delete-network {network_id}"));
        Ok(Response::new(operation))
    }

    unserved!(
        get(GetNetworkRequest) -> Network;
        update(UpdateNetworkRequest) -> Operation;
        list_subnets(ListNetworkSubnetsRequest) -> ListNetworkSubnetsResponse;
        list_security_groups(ListNetworkSecurityGroupsRequest) -> ListNetworkSecurityGroupsResponse;
        list_route_tables(ListNetworkRouteTablesRequest) -> ListNetworkRouteTablesResponse;
        list_operations(ListNetworkOperationsRequest) -> ListNetworkOperationsResponse;
        r#move(MoveNetworkRequest) -> Operation;
    );
}

#[tonic::async_trait]
impl SubnetService for FakeYandex {
    async fn list(
        &self,
        request: Request<ListSubnetsRequest>,
    ) -> Result<Response<ListSubnetsResponse>, Status> {
        authorized(&request)?;
        let folder_id = request.into_inner().folder_id;
        let state = self.state.lock().expect("the fake's state");
        let subnets: Vec<Subnet> = state
            .subnets
            .values()
            .filter(|subnet| subnet.folder_id == folder_id)
            .cloned()
            .collect();
        Ok(Response::new(ListSubnetsResponse {
            subnets,
            ..Default::default()
        }))
    }

    async fn create(
        &self,
        request: Request<CreateSubnetRequest>,
    ) -> Result<Response<Operation>, Status> {
        authorized(&request)?;
        let CreateSubnetRequest {
            folder_id,
            name,
            zone_id,
            network_id,
            v4_cidr_blocks,
            ..
        } = request.into_inner();
        let mut state = self.state.lock().expect("the fake's state");
        if state
            .subnets
            .values()
            .any(|subnet| subnet.folder_id == folder_id && subnet.name == name)
        {
            return Err(Status::already_exists("a subnet with this name exists"));
        }
        state.subnets_next += 1;
        let subnet_id = format!("subnet-{}", state.subnets_next);
        state.subnets.insert(
            subnet_id.clone(),
            Subnet {
                id: subnet_id,
                folder_id,
                name: name.clone(),
                zone_id,
                network_id,
                v4_cidr_blocks,
                ..Default::default()
            },
        );
        let operation = state.begin_operation(format!("create-subnet {name}"));
        Ok(Response::new(operation))
    }

    async fn delete(
        &self,
        request: Request<DeleteSubnetRequest>,
    ) -> Result<Response<Operation>, Status> {
        authorized(&request)?;
        let subnet_id = request.into_inner().subnet_id;
        let mut state = self.state.lock().expect("the fake's state");
        if state.subnets.remove(&subnet_id).is_none() {
            return Err(Status::not_found("no such subnet"));
        }
        let operation = state.begin_operation(format!("delete-subnet {subnet_id}"));
        Ok(Response::new(operation))
    }

    unserved!(
        get(GetSubnetRequest) -> Subnet;
        update(UpdateSubnetRequest) -> Operation;
        add_cidr_blocks(AddSubnetCidrBlocksRequest) -> Operation;
        remove_cidr_blocks(RemoveSubnetCidrBlocksRequest) -> Operation;
        relocate(RelocateSubnetRequest) -> Operation;
        list_operations(ListSubnetOperationsRequest) -> ListSubnetOperationsResponse;
        r#move(MoveSubnetRequest) -> Operation;
        list_used_addresses(ListUsedAddressesRequest) -> ListUsedAddressesResponse;
    );
}

#[tonic::async_trait]
impl SecurityGroupService for FakeYandex {
    async fn list(
        &self,
        request: Request<ListSecurityGroupsRequest>,
    ) -> Result<Response<ListSecurityGroupsResponse>, Status> {
        authorized(&request)?;
        let folder_id = request.into_inner().folder_id;
        let state = self.state.lock().expect("the fake's state");
        let security_groups: Vec<SecurityGroup> = state
            .groups
            .values()
            .filter(|group| group.folder_id == folder_id)
            .cloned()
            .collect();
        Ok(Response::new(ListSecurityGroupsResponse {
            security_groups,
            ..Default::default()
        }))
    }

    async fn create(
        &self,
        request: Request<CreateSecurityGroupRequest>,
    ) -> Result<Response<Operation>, Status> {
        authorized(&request)?;
        let CreateSecurityGroupRequest {
            folder_id,
            name,
            network_id,
            ..
        } = request.into_inner();
        let mut state = self.state.lock().expect("the fake's state");
        if state
            .groups
            .values()
            .any(|group| group.folder_id == folder_id && group.name == name)
        {
            return Err(Status::already_exists(
                "a security group with this name exists",
            ));
        }
        state.groups_next += 1;
        let group_id = format!("sg-{}", state.groups_next);
        state.groups.insert(
            group_id.clone(),
            SecurityGroup {
                id: group_id,
                folder_id,
                name: name.clone(),
                network_id,
                ..Default::default()
            },
        );
        let operation = state.begin_operation(format!("create-security-group {name}"));
        Ok(Response::new(operation))
    }

    async fn delete(
        &self,
        request: Request<DeleteSecurityGroupRequest>,
    ) -> Result<Response<Operation>, Status> {
        authorized(&request)?;
        let group_id = request.into_inner().security_group_id;
        let mut state = self.state.lock().expect("the fake's state");
        if state.groups.remove(&group_id).is_none() {
            return Err(Status::not_found("no such security group"));
        }
        let operation = state.begin_operation(format!("delete-security-group {group_id}"));
        Ok(Response::new(operation))
    }

    unserved!(
        get(GetSecurityGroupRequest) -> SecurityGroup;
        update(UpdateSecurityGroupRequest) -> Operation;
        update_rules(UpdateSecurityGroupRulesRequest) -> Operation;
        update_rule(UpdateSecurityGroupRuleRequest) -> Operation;
        r#move(MoveSecurityGroupRequest) -> Operation;
        list_operations(ListSecurityGroupOperationsRequest) -> ListSecurityGroupOperationsResponse;
    );
}

#[tonic::async_trait]
impl OperationService for FakeYandex {
    async fn get(
        &self,
        request: Request<GetOperationRequest>,
    ) -> Result<Response<Operation>, Status> {
        authorized(&request)?;
        let operation_id = request.into_inner().operation_id;
        let mut state = self.state.lock().expect("the fake's state");
        let operation = state
            .operations
            .get(&operation_id)
            .cloned()
            .ok_or_else(|| Status::not_found("no such operation"))?;
        let polls = state.polls.entry(operation_id).or_default();
        *polls += 1;
        if *polls < 2 {
            return Ok(Response::new(operation));
        }
        Ok(Response::new(Operation {
            done: true,
            result: Some(OperationResult::Response(prost_types::Any::default())),
            ..operation
        }))
    }

    unserved!(cancel(CancelOperationRequest) -> Operation;);
}

/// The Object Storage bucket service, as the live one behaves: a bucket is
/// unique by name across the whole platform, so a create answers
/// `already_exists` for any name a folder has seen before, whatever folder
/// it was made in.
#[tonic::async_trait]
impl BucketService for FakeYandex {
    async fn list(
        &self,
        request: Request<ListBucketsRequest>,
    ) -> Result<Response<ListBucketsResponse>, Status> {
        authorized(&request)?;
        let folder_id = request.into_inner().folder_id;
        let state = self.state.lock().expect("the fake's state");
        let buckets: Vec<Bucket> = state
            .buckets
            .values()
            .filter(|bucket| bucket.folder_id == folder_id)
            .cloned()
            .collect();
        Ok(Response::new(ListBucketsResponse {
            buckets,
            ..Default::default()
        }))
    }

    async fn create(
        &self,
        request: Request<CreateBucketRequest>,
    ) -> Result<Response<Operation>, Status> {
        authorized(&request)?;
        let request = request.into_inner();
        let mut state = self.state.lock().expect("the fake's state");
        // The name is the id: Object Storage makes them one and the same,
        // and a taken name is refused wherever the caller is.
        if state.buckets.contains_key(&request.name) {
            return Err(Status::already_exists(format!(
                "a bucket named {} already exists",
                request.name
            )));
        }
        state.buckets.insert(
            request.name.clone(),
            Bucket {
                id: request.name.clone(),
                name: request.name.clone(),
                folder_id: request.folder_id,
                ..Default::default()
            },
        );
        let operation = state.begin_operation(format!("create-bucket {}", request.name));
        Ok(Response::new(operation))
    }

    async fn delete(
        &self,
        request: Request<DeleteBucketRequest>,
    ) -> Result<Response<Operation>, Status> {
        authorized(&request)?;
        let name = request.into_inner().name;
        let mut state = self.state.lock().expect("the fake's state");
        if state.buckets.remove(&name).is_none() {
            return Err(Status::not_found("no such bucket"));
        }
        let operation = state.begin_operation(format!("delete-bucket {name}"));
        Ok(Response::new(operation))
    }

    unserved!(
        get(GetBucketRequest) -> Bucket;
        update(UpdateBucketRequest) -> Operation;
        get_stats(GetBucketStatsRequest) -> BucketStats;
        get_https_config(GetBucketHttpsConfigRequest) -> HttpsConfig;
        set_https_config(SetBucketHttpsConfigRequest) -> Operation;
        delete_https_config(DeleteBucketHttpsConfigRequest) -> Operation;
        set_access_bindings(SetAccessBindingsRequest) -> Operation;
        update_access_bindings(UpdateAccessBindingsRequest) -> Operation;
        list_access_bindings(ListAccessBindingsRequest) -> ListAccessBindingsResponse;
        create_inventory_configuration(CreateBucketInventoryConfigurationRequest) -> Operation;
        get_inventory_configuration(GetBucketInventoryConfigurationRequest)
            -> InventoryConfiguration;
        delete_inventory_configuration(DeleteBucketInventoryConfigurationRequest) -> Operation;
        list_inventory_configurations(ListBucketInventoryConfigurationsRequest)
            -> ListBucketInventoryConfigurationsResponse;
    );
}

/// The recorded-exchange machinery: the tunnel that takes a recording from
/// the live API and the replay that answers from it.
pub(crate) mod recorded;
/// The knobs a scenario turns; everything else is the declared set the
/// scenarios share.
#[derive(Clone, Copy)]
pub(crate) struct Declared {
    pub(crate) zone: &'static str,
    pub(crate) cores: i64,
    pub(crate) memory: i64,
    pub(crate) boot_disk: &'static str,
    pub(crate) size: i64,
}

impl Default for Declared {
    fn default() -> Self {
        Self {
            zone: ZONE,
            cores: 2,
            memory: 4 * GIB,
            boot_disk: IMAGE,
            size: 10 * GIB,
        }
    }
}

/// The declared set the scenarios share: a network, a subnet in it, a
/// security group on it, a data disk, and an instance whose boot disk is
/// made from an image and whose interface hangs off the subnet. The folder
/// is a parameter so a recording's replay declares in the folder the
/// recording was taken in.
pub(crate) fn declared(folder: &str, set: Declared) -> Graph {
    let mut graph = Graph::new();
    graph
        .insert(
            Resource::new(address("yandex:network.main"))
                .with("folder_id", folder)
                .with("name", "main"),
        )
        .expect("inserted");
    graph
        .insert(
            Resource::new(address("yandex:subnet.mgmt"))
                .with("folder_id", folder)
                .with("name", "mgmt")
                .with("zone", ZONE)
                .with(
                    "network_id",
                    Reference::to(address("yandex:network.main"), "id"),
                )
                .with(
                    "v4_cidr_blocks",
                    Value::List(vec![Value::Text("10.0.0.0/24".to_owned())]),
                ),
        )
        .expect("inserted");
    graph
        .insert(
            Resource::new(address("yandex:security_group.web"))
                .with("folder_id", folder)
                .with("name", "web")
                .with(
                    "network_id",
                    Reference::to(address("yandex:network.main"), "id"),
                ),
        )
        .expect("inserted");
    graph
        .insert(
            Resource::new(address("yandex:disk.data"))
                .with("folder_id", folder)
                .with("name", "data")
                // An hdd disk: the cloud's ssd quota in zone d is spent, so
                // the scenario's data disk is the kind the quota allows.
                .with("type", "network-hdd")
                .with("zone", ZONE)
                .with("size", set.size)
                .with("image_id", IMAGE),
        )
        .expect("inserted");
    graph
        .insert(
            Resource::new(address("yandex:instance.db"))
                .with("folder_id", folder)
                .with("name", "db")
                .with("zone", set.zone)
                .with("platform_id", "standard-v2")
                .with("cores", set.cores)
                .with("memory", set.memory)
                .with("boot_disk", set.boot_disk)
                .with("user_data", "#cloud-config\n")
                .with("subnet", Reference::to(address("yandex:subnet.mgmt"), "id")),
        )
        .expect("inserted");
    graph
}

/// The declared set a recording covers: a data disk, nothing else. The
/// cloud's vpc and core quotas are spent after the datacenter attack, so a
/// recording must be of what the cloud allows; the full five-resource set
/// stays in [`declared`] for the fake-cloud tests, and the recording is
/// re-taken when the quotas return.
pub(crate) fn declared_recordable(folder: &str, set: Declared) -> Graph {
    let mut graph = Graph::new();
    graph
        .insert(
            Resource::new(address("yandex:disk.data"))
                .with("folder_id", folder)
                .with("name", "data")
                .with("type", "network-hdd")
                .with("zone", ZONE)
                .with("size", set.size)
                .with("image_id", IMAGE),
        )
        .expect("inserted");
    graph
}

/// The declared set a bucket scenario covers: one bucket, by the name the
/// scenario gives it. The name is the id and must be unique across the
/// whole platform, so a recording picks its own.
pub(crate) fn declared_bucket(folder: &str, name: &str) -> Graph {
    let mut graph = Graph::new();
    graph
        .insert(
            Resource::new(address("yandex:bucket.data"))
                .with("folder_id", folder)
                .with("name", name),
        )
        .expect("inserted");
    graph
}

/// The digest key and the remembered-secrets store every converge run
/// shares.
static DIGEST_KEY: Key = Key::from_bytes([9; 32]);
static NO_STORE: Remembered = Remembered::EMPTY;

/// Secrets for a converge run.
pub(crate) fn secrets() -> Secrets<'static> {
    Secrets::new(&DIGEST_KEY, &NO_STORE)
}

/// The actor a plan is attributed to: `run::apply` proceeds only with an
/// actor identity, and a test is a run of a kind.
pub(crate) fn actor() -> Actor {
    Actor::new("tester", "run", None).expect("an actor")
}

/// The plan a test drives, attributed to a tester.
pub(crate) fn plan(
    graph: &Graph,
    observed: &Observed,
    state: &EngineState,
    schema: &Schema,
    secrets: Secrets<'_>,
) -> Result<Plan, nerpa_core::planner::PlanError> {
    nerpa_core::planner::plan(graph, observed, state, schema, secrets)
        .map(|planned| planned.attributed_to(actor()))
}

/// One full run against the cloud: observe, plan, apply.
pub(crate) fn converge(
    graph: &Graph,
    providers: &Providers,
    machines: &Machines,
    state: &mut EngineState,
) -> Plan {
    let observed = run::observe(graph, providers, machines, state).expect("observed");
    let planned = plan(graph, &observed, state, &providers.schema(), secrets()).expect("plannable");
    run::apply(
        &planned,
        providers,
        machines,
        &observed,
        state,
        secrets(),
        &mut |_: &EngineState| Ok(()),
    )
    .expect("applied");
    planned
}

/// The provider connected to the endpoint, which serves the API, the IAM
/// exchange, and Object Storage alike.
pub(crate) fn providers(endpoint: &SocketAddr) -> Providers {
    let endpoint = format!("http://{endpoint}");
    let provider =
        Yandex::connect_at_with_storage(&endpoint, &endpoint, &endpoint, &a_credentials())
            .expect("connected");
    Providers::new().with(Box::new(provider))
}
