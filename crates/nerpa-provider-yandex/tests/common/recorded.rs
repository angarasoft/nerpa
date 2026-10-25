//! The recorded-exchange machinery: a recording of real Yandex exchanges,
//! the tunnel that takes one, and the server that replays it.
//!
//! A recording holds the folder it was taken in and the exchanges — each one
//! the name of the call, as `Service/method`, and the protobuf bodies as
//! base64. Bodies only: authorization metadata is not stored, and the IAM
//! exchange is not recorded at all, because its answer is a token (ADR-071,
//! ADR-070).
//!
//! The tunnel forwards every call to the live API and writes the exchange as
//! it passes. The replay answers from the recording, matching each request by
//! its call name and its body bytes, and refuses what no recording covers.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unused_async,
    dead_code,
    reason = "the replay's shapes are fixed by the generated traits, and tests are allowed to be blunt"
)]

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, mpsc};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use nerpa_provider_yandex::client::Client;
use nerpa_provider_yandex::credentials::Credentials;
use nerpa_provider_yandex::yandex::cloud::access::{
    ListAccessBindingsRequest, ListAccessBindingsResponse, SetAccessBindingsRequest,
    UpdateAccessBindingsRequest,
};
use nerpa_provider_yandex::yandex::cloud::compute::v1::disk_service_server::{
    DiskService, DiskServiceServer,
};
use nerpa_provider_yandex::yandex::cloud::compute::v1::instance_service_server::{
    InstanceService, InstanceServiceServer,
};
use nerpa_provider_yandex::yandex::cloud::compute::v1::{
    AddInstanceOneToOneNatRequest, AttachInstanceDiskRequest, AttachInstanceFilesystemRequest,
    AttachInstanceNetworkInterfaceRequest, CreateDiskRequest, CreateInstanceRequest,
    DeleteDiskRequest, DeleteInstanceRequest, DetachInstanceDiskRequest,
    DetachInstanceFilesystemRequest, DetachInstanceNetworkInterfaceRequest, Disk, GetDiskRequest,
    GetInstanceRequest, GetInstanceSerialPortOutputRequest, GetInstanceSerialPortOutputResponse,
    Instance, ListDiskOperationsRequest, ListDiskOperationsResponse,
    ListDiskSnapshotSchedulesRequest, ListDiskSnapshotSchedulesResponse, ListDisksRequest,
    ListDisksResponse, ListInstanceOperationsRequest, ListInstanceOperationsResponse,
    ListInstancesRequest, ListInstancesResponse, MoveDiskRequest, MoveInstanceRequest,
    RelocateDiskRequest, RelocateInstanceRequest, RemoveInstanceOneToOneNatRequest,
    RestartInstanceRequest, SimulateInstanceMaintenanceEventRequest, StartInstanceRequest,
    StopInstanceRequest, UpdateDiskRequest, UpdateInstanceMetadataRequest,
    UpdateInstanceNetworkInterfaceRequest, UpdateInstanceRequest,
};
use nerpa_provider_yandex::yandex::cloud::iam::v1::create_iam_token_request::Identity;
use nerpa_provider_yandex::yandex::cloud::iam::v1::iam_token_service_server::{
    IamTokenService, IamTokenServiceServer,
};
use nerpa_provider_yandex::yandex::cloud::iam::v1::{
    CreateIamTokenForServiceAccountRequest, CreateIamTokenRequest, CreateIamTokenResponse,
    RevokeIamTokenRequest, RevokeIamTokenResponse,
};
use nerpa_provider_yandex::yandex::cloud::operation::operation_service_server::{
    OperationService, OperationServiceServer,
};
use nerpa_provider_yandex::yandex::cloud::operation::{
    CancelOperationRequest, GetOperationRequest, Operation,
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
use prost::Message;
use serde::{Deserialize, Serialize};
use tonic::{Request, Response, Status};

use super::authorized;

/// One recorded exchange: the name of the call, as `Service/method`, and the
/// protobuf request and response bodies as base64. Bodies only — no
/// authorization metadata, no token (ADR-071).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Exchange {
    /// The call, as `Service/method`, like `InstanceService/get`.
    pub(crate) call: String,
    /// The protobuf request body, base64.
    pub(crate) request: String,
    /// The protobuf response body, base64.
    pub(crate) response: String,
}

/// A scenario's recording: the folder it was taken in, and the exchanges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Recording {
    /// The folder the exchanges happened in; a replay test declares its
    /// resources there so its requests match the recorded ones.
    pub(crate) folder: String,
    /// What was sent and answered, in the order it happened.
    pub(crate) exchanges: Vec<Exchange>,
}

impl Recording {
    /// Reads a recording file written by [`Recording::save`].
    pub(crate) fn load(path: &std::path::Path) -> Self {
        let text = std::fs::read_to_string(path).expect("a recording file");
        serde_json::from_str(&text).expect("a recording")
    }

    /// Writes the recording the tunnel took, to be replayed by a test. The
    /// directory is made as well: a recording is the tool's output, not a
    /// hand-placed file.
    pub(crate) fn save(&self, path: &std::path::Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("a writable recording directory");
        }
        let text = serde_json::to_string_pretty(self).expect("a serializable recording");
        std::fs::write(path, text).expect("a writable recording file");
    }
}

/// The protobuf body a request carries, as base64.
fn encode<T: Message>(message: &T) -> String {
    STANDARD.encode(message.encode_to_vec())
}

/// The protobuf body a recording stored, decoded back.
fn decode<T: Message + Default>(text: &str) -> Result<T, Status> {
    let bytes = STANDARD
        .decode(text)
        .map_err(|e| Status::internal(format!("a recorded body is not base64: {e}")))?;
    T::decode(bytes.as_slice())
        .map_err(|e| Status::internal(format!("a recorded body is not a protobuf: {e}")))
}

/// The methods a replay answers from the recording, refused where the
/// recording covers nothing. Written in the desugared shape of the generated
/// trait's `async fn`, for the same reason `unserved!` is.
macro_rules! replayed {
    ($service:literal: $($method:ident($request:ty) -> $response:ty;)*) => {
        $(
            fn $method<'life0, 'async_trait>(
                &'life0 self,
                request: tonic::Request<$request>,
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
                    authorized(&request)?;
                    self.answer(concat!($service, "/", stringify!($method)), &request)
                })
            }
        )*
    };
}

/// The replay: a server answering from a recording, IAM locally — the same
/// JWT check the fake cloud does, so a recorded world carries no token.
#[derive(Debug, Clone)]
pub(crate) struct Replay {
    recording: Recording,
    /// How many times each recorded call has been answered: repeated
    /// identical requests are answered in the recorded order, so a polled
    /// operation progresses exactly as it did live.
    answered: Arc<Mutex<BTreeMap<(String, String), usize>>>,
}

impl Replay {
    /// Serves the recording on a free local port and hands back the handle
    /// and the endpoint it listens on.
    pub(crate) fn start(recording: Recording) -> (Self, SocketAddr) {
        let replay = Self {
            recording,
            answered: Arc::new(Mutex::new(BTreeMap::new())),
        };
        let server = replay.clone();
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
                    .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                    .await
                    .expect("served");
            });
        });
        let address = received.recv().expect("an address");
        (replay, address)
    }

    /// Answers a request from the recording: the call and the body bytes
    /// must match a recorded exchange exactly, which is what proves the
    /// provider sends exactly what was recorded (ADR-071). Repeated
    /// identical requests are answered in the recorded order, so a polled
    /// operation progresses exactly as it did live.
    fn answer<Q, A>(&self, call: &str, request: &Request<Q>) -> Result<Response<A>, Status>
    where
        Q: Message,
        A: Message + Default,
    {
        let body = encode(request.get_ref());
        let mut answered = self.answered.lock().expect("the replay's cursors");
        let at = answered.entry((call.to_owned(), body.clone())).or_insert(0);
        *at += 1;
        let at = *at - 1;
        drop(answered);
        let exchange = self
            .recording
            .exchanges
            .iter()
            .filter(|exchange| exchange.call == call && exchange.request == body)
            .nth(at)
            .ok_or_else(|| Status::failed_precondition(format!("no recording for {call}")))?;
        let response = decode::<A>(&exchange.response)?;
        Ok(Response::new(response))
    }
}

#[tonic::async_trait]
impl IamTokenService for Replay {
    async fn create(
        &self,
        request: Request<CreateIamTokenRequest>,
    ) -> Result<Response<CreateIamTokenResponse>, Status> {
        let Some(Identity::Jwt(jwt)) = request.into_inner().identity else {
            return Err(Status::invalid_argument("the identity must be a JWT"));
        };
        if !super::jwt_accepted(&jwt) {
            return Err(Status::unauthenticated("the JWT does not verify"));
        }
        Ok(Response::new(CreateIamTokenResponse {
            iam_token: super::TOKEN.to_owned(),
            expires_at: Some(super::an_hour_hence()),
        }))
    }

    super::unserved!(
        create_for_service_account(CreateIamTokenForServiceAccountRequest) -> CreateIamTokenResponse;
        revoke(RevokeIamTokenRequest) -> RevokeIamTokenResponse;
    );
}

#[tonic::async_trait]
impl InstanceService for Replay {
    replayed!("InstanceService":
        get(GetInstanceRequest) -> Instance;
        list(ListInstancesRequest) -> ListInstancesResponse;
        create(CreateInstanceRequest) -> Operation;
        update(UpdateInstanceRequest) -> Operation;
        delete(DeleteInstanceRequest) -> Operation;
        update_metadata(UpdateInstanceMetadataRequest) -> Operation;
        get_serial_port_output(GetInstanceSerialPortOutputRequest) -> GetInstanceSerialPortOutputResponse;
        stop(StopInstanceRequest) -> Operation;
        start(StartInstanceRequest) -> Operation;
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
impl DiskService for Replay {
    replayed!("DiskService":
        get(GetDiskRequest) -> Disk;
        list(ListDisksRequest) -> ListDisksResponse;
        create(CreateDiskRequest) -> Operation;
        update(UpdateDiskRequest) -> Operation;
        delete(DeleteDiskRequest) -> Operation;
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
impl NetworkService for Replay {
    replayed!("NetworkService":
        get(GetNetworkRequest) -> Network;
        list(ListNetworksRequest) -> ListNetworksResponse;
        create(CreateNetworkRequest) -> Operation;
        update(UpdateNetworkRequest) -> Operation;
        delete(DeleteNetworkRequest) -> Operation;
        list_subnets(ListNetworkSubnetsRequest) -> ListNetworkSubnetsResponse;
        list_security_groups(ListNetworkSecurityGroupsRequest) -> ListNetworkSecurityGroupsResponse;
        list_route_tables(ListNetworkRouteTablesRequest) -> ListNetworkRouteTablesResponse;
        list_operations(ListNetworkOperationsRequest) -> ListNetworkOperationsResponse;
        r#move(MoveNetworkRequest) -> Operation;
    );
}

#[tonic::async_trait]
impl SubnetService for Replay {
    replayed!("SubnetService":
        get(GetSubnetRequest) -> Subnet;
        list(ListSubnetsRequest) -> ListSubnetsResponse;
        create(CreateSubnetRequest) -> Operation;
        update(UpdateSubnetRequest) -> Operation;
        add_cidr_blocks(AddSubnetCidrBlocksRequest) -> Operation;
        remove_cidr_blocks(RemoveSubnetCidrBlocksRequest) -> Operation;
        relocate(RelocateSubnetRequest) -> Operation;
        delete(DeleteSubnetRequest) -> Operation;
        list_operations(ListSubnetOperationsRequest) -> ListSubnetOperationsResponse;
        r#move(MoveSubnetRequest) -> Operation;
        list_used_addresses(ListUsedAddressesRequest) -> ListUsedAddressesResponse;
    );
}

#[tonic::async_trait]
impl SecurityGroupService for Replay {
    replayed!("SecurityGroupService":
        get(GetSecurityGroupRequest) -> SecurityGroup;
        list(ListSecurityGroupsRequest) -> ListSecurityGroupsResponse;
        create(CreateSecurityGroupRequest) -> Operation;
        update(UpdateSecurityGroupRequest) -> Operation;
        update_rules(UpdateSecurityGroupRulesRequest) -> Operation;
        update_rule(UpdateSecurityGroupRuleRequest) -> Operation;
        delete(DeleteSecurityGroupRequest) -> Operation;
        r#move(MoveSecurityGroupRequest) -> Operation;
        list_operations(ListSecurityGroupOperationsRequest) -> ListSecurityGroupOperationsResponse;
    );
}

#[tonic::async_trait]
impl OperationService for Replay {
    replayed!("OperationService":
        get(GetOperationRequest) -> Operation;
        cancel(CancelOperationRequest) -> Operation;
    );
}

/// The live cloud serves its services on separate hosts: compute, vpc and
/// operation each answer on their own endpoint. The tunnel fans calls out per
/// service, which is the mapping a live client itself needs.
const COMPUTE_ENDPOINT: &str = "https://compute.api.cloud.yandex.net:443";
const VPC_ENDPOINT: &str = "https://vpc.api.cloud.yandex.net:443";
const OPERATION_ENDPOINT: &str = "https://operation.api.cloud.yandex.net:443";

/// The tunnel: a server that speaks the seven services and forwards every
/// call to the live API on the host that serves it, writing each exchange as
/// it passes. Its IAM answers locally — the live clients the tunnel forwards
/// through mint their own tokens against the real IAM — so the recording
/// never sees one (ADR-071).
#[derive(Debug, Clone)]
pub(crate) struct Recorder {
    compute: Arc<Client>,
    vpc: Arc<Client>,
    operation: Arc<Client>,
    recorded: Arc<Mutex<Vec<Exchange>>>,
}

impl Recorder {
    /// Connects the tunnel to the live API and serves it on a free local
    /// port, handing back the handle and the endpoint.
    pub(crate) fn start(credentials: &Credentials) -> (Self, SocketAddr) {
        let key = credentials.service_account_key();
        let recorder = Self {
            compute: Arc::new(
                Client::connect(COMPUTE_ENDPOINT, key).expect("the compute API is reachable"),
            ),
            vpc: Arc::new(Client::connect(VPC_ENDPOINT, key).expect("the vpc API is reachable")),
            operation: Arc::new(
                Client::connect(OPERATION_ENDPOINT, key).expect("the operation API is reachable"),
            ),
            recorded: Arc::new(Mutex::new(Vec::new())),
        };
        let server = recorder.clone();
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
                    .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                    .await
                    .expect("served");
            });
        });
        let address = received.recv().expect("an address");
        (recorder, address)
    }

    /// What the tunnel recorded, wrapped for a replay. The serving thread
    /// still holds its clone of the buffer, so the exchanges are taken out
    /// from under the lock rather than from the [`Arc`].
    pub(crate) fn into_recording(self, folder: String) -> Recording {
        Recording {
            folder,
            exchanges: std::mem::take(&mut *self.recorded.lock().expect("the recordings")),
        }
    }

    /// One forwarded call: writes the exchange and hands the live answer on.
    async fn forward<A>(
        &self,
        call: &str,
        request: &str,
        callable: impl Future<Output = Result<Response<A>, Status>>,
    ) -> Result<Response<A>, Status>
    where
        A: Message,
    {
        let response = callable.await?;
        self.recorded
            .lock()
            .expect("the recordings")
            .push(Exchange {
                call: call.to_owned(),
                request: request.to_owned(),
                response: encode(response.get_ref()),
            });
        Ok(response)
    }
}

/// The IAM the tunnel answers itself, so the live client mints its own token
/// against the real IAM and no recording sees one.
#[tonic::async_trait]
impl IamTokenService for Recorder {
    async fn create(
        &self,
        request: Request<CreateIamTokenRequest>,
    ) -> Result<Response<CreateIamTokenResponse>, Status> {
        let Some(Identity::Jwt(jwt)) = request.into_inner().identity else {
            return Err(Status::invalid_argument("the identity must be a JWT"));
        };
        if !super::jwt_accepted(&jwt) {
            return Err(Status::unauthenticated("the JWT does not verify"));
        }
        Ok(Response::new(CreateIamTokenResponse {
            iam_token: super::TOKEN.to_owned(),
            expires_at: Some(super::an_hour_hence()),
        }))
    }

    super::unserved!(
        create_for_service_account(CreateIamTokenForServiceAccountRequest) -> CreateIamTokenResponse;
        revoke(RevokeIamTokenRequest) -> RevokeIamTokenResponse;
    );
}

#[tonic::async_trait]
impl InstanceService for Recorder {
    async fn get(
        &self,
        request: Request<GetInstanceRequest>,
    ) -> Result<Response<Instance>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward(
            "InstanceService/get",
            &body,
            self.compute.instances().get(request),
        )
        .await
    }

    async fn list(
        &self,
        request: Request<ListInstancesRequest>,
    ) -> Result<Response<ListInstancesResponse>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward(
            "InstanceService/list",
            &body,
            self.compute.instances().list(request),
        )
        .await
    }

    async fn create(
        &self,
        request: Request<CreateInstanceRequest>,
    ) -> Result<Response<Operation>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward(
            "InstanceService/create",
            &body,
            self.compute.instances().create(request),
        )
        .await
    }

    async fn update(
        &self,
        request: Request<UpdateInstanceRequest>,
    ) -> Result<Response<Operation>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward(
            "InstanceService/update",
            &body,
            self.compute.instances().update(request),
        )
        .await
    }

    async fn delete(
        &self,
        request: Request<DeleteInstanceRequest>,
    ) -> Result<Response<Operation>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward(
            "InstanceService/delete",
            &body,
            self.compute.instances().delete(request),
        )
        .await
    }

    async fn stop(
        &self,
        request: Request<StopInstanceRequest>,
    ) -> Result<Response<Operation>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward(
            "InstanceService/stop",
            &body,
            self.compute.instances().stop(request),
        )
        .await
    }

    async fn start(
        &self,
        request: Request<StartInstanceRequest>,
    ) -> Result<Response<Operation>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward(
            "InstanceService/start",
            &body,
            self.compute.instances().start(request),
        )
        .await
    }

    super::unserved!(
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
impl DiskService for Recorder {
    async fn get(&self, request: Request<GetDiskRequest>) -> Result<Response<Disk>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward("DiskService/get", &body, self.compute.disks().get(request))
            .await
    }

    async fn list(
        &self,
        request: Request<ListDisksRequest>,
    ) -> Result<Response<ListDisksResponse>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward(
            "DiskService/list",
            &body,
            self.compute.disks().list(request),
        )
        .await
    }

    async fn create(
        &self,
        request: Request<CreateDiskRequest>,
    ) -> Result<Response<Operation>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward(
            "DiskService/create",
            &body,
            self.compute.disks().create(request),
        )
        .await
    }

    async fn update(
        &self,
        request: Request<UpdateDiskRequest>,
    ) -> Result<Response<Operation>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward(
            "DiskService/update",
            &body,
            self.compute.disks().update(request),
        )
        .await
    }

    async fn delete(
        &self,
        request: Request<DeleteDiskRequest>,
    ) -> Result<Response<Operation>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward(
            "DiskService/delete",
            &body,
            self.compute.disks().delete(request),
        )
        .await
    }

    super::unserved!(
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
impl NetworkService for Recorder {
    async fn list(
        &self,
        request: Request<ListNetworksRequest>,
    ) -> Result<Response<ListNetworksResponse>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward(
            "NetworkService/list",
            &body,
            self.vpc.networks().list(request),
        )
        .await
    }

    async fn create(
        &self,
        request: Request<CreateNetworkRequest>,
    ) -> Result<Response<Operation>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward(
            "NetworkService/create",
            &body,
            self.vpc.networks().create(request),
        )
        .await
    }

    async fn delete(
        &self,
        request: Request<DeleteNetworkRequest>,
    ) -> Result<Response<Operation>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward(
            "NetworkService/delete",
            &body,
            self.vpc.networks().delete(request),
        )
        .await
    }

    super::unserved!(
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
impl SubnetService for Recorder {
    async fn list(
        &self,
        request: Request<ListSubnetsRequest>,
    ) -> Result<Response<ListSubnetsResponse>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward(
            "SubnetService/list",
            &body,
            self.vpc.subnets().list(request),
        )
        .await
    }

    async fn create(
        &self,
        request: Request<CreateSubnetRequest>,
    ) -> Result<Response<Operation>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward(
            "SubnetService/create",
            &body,
            self.vpc.subnets().create(request),
        )
        .await
    }

    async fn delete(
        &self,
        request: Request<DeleteSubnetRequest>,
    ) -> Result<Response<Operation>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward(
            "SubnetService/delete",
            &body,
            self.vpc.subnets().delete(request),
        )
        .await
    }

    super::unserved!(
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
impl SecurityGroupService for Recorder {
    async fn list(
        &self,
        request: Request<ListSecurityGroupsRequest>,
    ) -> Result<Response<ListSecurityGroupsResponse>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward(
            "SecurityGroupService/list",
            &body,
            self.vpc.security_groups().list(request),
        )
        .await
    }

    async fn create(
        &self,
        request: Request<CreateSecurityGroupRequest>,
    ) -> Result<Response<Operation>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward(
            "SecurityGroupService/create",
            &body,
            self.vpc.security_groups().create(request),
        )
        .await
    }

    async fn delete(
        &self,
        request: Request<DeleteSecurityGroupRequest>,
    ) -> Result<Response<Operation>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward(
            "SecurityGroupService/delete",
            &body,
            self.vpc.security_groups().delete(request),
        )
        .await
    }

    super::unserved!(
        get(GetSecurityGroupRequest) -> SecurityGroup;
        update(UpdateSecurityGroupRequest) -> Operation;
        update_rules(UpdateSecurityGroupRulesRequest) -> Operation;
        update_rule(UpdateSecurityGroupRuleRequest) -> Operation;
        r#move(MoveSecurityGroupRequest) -> Operation;
        list_operations(ListSecurityGroupOperationsRequest) -> ListSecurityGroupOperationsResponse;
    );
}

#[tonic::async_trait]
impl OperationService for Recorder {
    async fn get(
        &self,
        request: Request<GetOperationRequest>,
    ) -> Result<Response<Operation>, Status> {
        let body = encode(request.get_ref());
        let request = request.into_inner();
        self.forward(
            "OperationService/get",
            &body,
            self.operation.operations().get(request),
        )
        .await
    }

    super::unserved!(cancel(CancelOperationRequest) -> Operation;);
}
