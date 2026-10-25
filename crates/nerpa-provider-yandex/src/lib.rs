//! A control-domain provider for Yandex Cloud.
//!
//! Resources in the `control` domain have no machine: their state lives behind
//! the Yandex Cloud API, reached from where the engine already runs. This
//! provider owns `yandex:instance`, `yandex:disk`, `yandex:network`,
//! `yandex:subnet` and `yandex:security_group`, and the schema that says which
//! of their attributes change in place, replace the resource, were born with
//! it, only grow, or need a stop to change.

use std::collections::BTreeMap;
use std::future::Future;
use std::time::{Duration, Instant};

use nerpa_core::address::{Address, Kind, Provider as ProviderName};
use nerpa_core::observed::Facts;
use nerpa_core::provider::{
    Afterwards, Answered, Attributes, Concluded, Concluding, Planned, Prepared, Provider,
    ProviderError, Reach, Read, Reading, Sending,
};
use nerpa_core::schema::Schema;
use nerpa_core::value::Value;

use crate::auth::ServiceAccountKey;
use crate::client::{Client, ClientError};
use crate::credentials::Credentials;
use crate::yandex::cloud::compute::v1::instance::Status as InstanceStatus;
use crate::yandex::cloud::compute::v1::{
    DeleteDiskRequest, DeleteInstanceRequest, GetDiskRequest, GetInstanceRequest, InstanceView,
    ListDisksRequest, ListInstancesRequest, StartInstanceRequest, StopInstanceRequest,
};
use crate::yandex::cloud::operation::operation::Result as OperationResult;
use crate::yandex::cloud::operation::{GetOperationRequest, Operation};
use crate::yandex::cloud::storage::v1::{DeleteBucketRequest, ListBucketsRequest};
use crate::yandex::cloud::vpc::v1::{
    DeleteNetworkRequest, DeleteSecurityGroupRequest, DeleteSubnetRequest, ListNetworksRequest,
    ListSecurityGroupsRequest, ListSubnetsRequest,
};

pub mod auth;
pub mod client;
pub mod credentials;
mod resources;

/// The generated Yandex Cloud proto types and service clients.
#[allow(
    clippy::all,
    clippy::as_conversions,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::pedantic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unwrap_used,
    elided_lifetimes_in_paths,
    missing_debug_implementations,
    missing_docs,
    single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unsafe_code,
    unused_lifetimes,
    unused_qualifications,
    rustdoc::broken_intra_doc_links,
    rustdoc::invalid_html_tags,
    rustdoc::private_intra_doc_links,
    rustdoc::unescaped_backticks
)]
pub mod yandex {
    pub mod cloud {
        include!(concat!(env!("OUT_DIR"), "/yandex.cloud.rs"));
        pub mod access {
            include!(concat!(env!("OUT_DIR"), "/yandex.cloud.access.rs"));
        }
        pub mod api {
            include!(concat!(env!("OUT_DIR"), "/yandex.cloud.api.rs"));
        }
        pub mod compute {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/yandex.cloud.compute.v1.rs"));
            }
        }
        pub mod iam {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/yandex.cloud.iam.v1.rs"));
            }
        }
        pub mod operation {
            include!(concat!(env!("OUT_DIR"), "/yandex.cloud.operation.rs"));
        }
        pub mod reference {
            include!(concat!(env!("OUT_DIR"), "/yandex.cloud.reference.rs"));
        }
        pub mod storage {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/yandex.cloud.storage.v1.rs"));
            }
        }
        pub mod vpc {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/yandex.cloud.vpc.v1.rs"));
            }
        }
    }
}

/// The generated google proto types the Yandex protos import.
#[allow(
    clippy::all,
    clippy::as_conversions,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::pedantic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unwrap_used,
    elided_lifetimes_in_paths,
    missing_debug_implementations,
    missing_docs,
    single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unsafe_code,
    unused_lifetimes,
    unused_qualifications,
    rustdoc::broken_intra_doc_links,
    rustdoc::invalid_html_tags,
    rustdoc::private_intra_doc_links,
    rustdoc::unescaped_backticks
)]
pub mod google {
    pub mod api {
        include!(concat!(env!("OUT_DIR"), "/google.api.rs"));
    }
    pub mod rpc {
        include!(concat!(env!("OUT_DIR"), "/google.rpc.rs"));
    }
}

/// The Yandex Cloud provider, answering to `yandex` in an address.
///
/// Holds the API client once connected; without one it reads nothing and
/// refuses every change, which is what [`Provider::fake`] is built from. The
/// key is kept beside the client so an expired token can be refreshed in
/// place, one retry, instead of ending the run.
#[derive(Debug)]
pub struct Yandex {
    name: ProviderName,
    client: Option<Client>,
    key: Option<ServiceAccountKey>,
}

impl Yandex {
    /// The provider, answering to `yandex` in an address.
    ///
    /// # Panics
    ///
    /// Panics if `yandex` is somehow not a valid provider name, which would mean
    /// the address grammar had changed underneath this crate.
    #[must_use]
    #[allow(
        clippy::expect_used,
        reason = "a literal that stopped parsing is a bug here"
    )]
    pub fn new() -> Self {
        Self {
            name: Self::name(),
            client: None,
            key: None,
        }
    }

    /// Connects to the Yandex Cloud API with the given credentials, exchanging
    /// the service account key for a short-lived token at the default IAM
    /// endpoint, and holds both.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::Connect`] when the key is unusable, the exchange
    /// fails, or the endpoint cannot be reached.
    pub fn connect(endpoint: &str, credentials: &Credentials) -> Result<Self, ClientError> {
        Self::connect_at(endpoint, auth::IAM_ENDPOINT, credentials)
    }

    /// Connects to the Yandex Cloud API with the given credentials, exchanging
    /// the key at the named IAM endpoint. The IAM endpoint is a parameter
    /// rather than a constant so a test can point both at one in-process fake.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::Connect`] when the key is unusable, the exchange
    /// fails, or the endpoint cannot be reached.
    pub fn connect_at(
        endpoint: &str,
        iam_endpoint: &str,
        credentials: &Credentials,
    ) -> Result<Self, ClientError> {
        let key = credentials.service_account_key().clone();
        let client = Client::connect_at(endpoint, iam_endpoint, &key)?;
        Ok(Self {
            name: Self::name(),
            client: Some(client),
            key: Some(key),
        })
    }

    /// Connects to the Yandex Cloud API with the given credentials, exchanging
    /// the key at the named IAM endpoint and reaching Object Storage at the
    /// named storage endpoint. The endpoints are parameters rather than
    /// constants so a test can point all of them at one in-process fake.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::Connect`] when the key is unusable, the exchange
    /// fails, or an endpoint cannot be reached.
    pub fn connect_at_with_storage(
        endpoint: &str,
        iam_endpoint: &str,
        storage_endpoint: &str,
        credentials: &Credentials,
    ) -> Result<Self, ClientError> {
        let key = credentials.service_account_key().clone();
        let client =
            Client::connect_at_with_storage(endpoint, iam_endpoint, storage_endpoint, &key)?;
        Ok(Self {
            name: Self::name(),
            client: Some(client),
            key: Some(key),
        })
    }

    /// The name this provider answers to, made once and shared.
    #[must_use]
    #[allow(
        clippy::expect_used,
        reason = "a literal that stopped parsing is a bug here"
    )]
    fn name() -> ProviderName {
        ProviderName::new("yandex").expect("`yandex` is a valid provider name")
    }

    /// The schema for the five kinds this provider owns.
    #[must_use]
    fn schema_of(name: ProviderName) -> Schema {
        #[allow(
            clippy::expect_used,
            reason = "literals that stopped parsing are a bug here"
        )]
        let kind = |n: &str| Kind::new(n).expect("a valid kind");
        Schema::new()
            // An instance's zone, platform and subnet cannot move; its boot
            // disk and user data describe how it was made; its cores and
            // memory change only while it is stopped.
            .immutable(name.clone(), kind("instance"), "zone")
            .immutable(name.clone(), kind("instance"), "platform_id")
            .immutable(name.clone(), kind("instance"), "subnet")
            .at_birth(name.clone(), kind("instance"), "boot_disk")
            .at_birth(name.clone(), kind("instance"), "user_data")
            .identity(name.clone(), kind("instance"), "name", "name")
            .stop_to_change(name.clone(), kind("instance"), "cores")
            .stop_to_change(name.clone(), kind("instance"), "memory")
            // A disk's type and zone cannot move; its size can only grow; its
            // image describes how it was made.
            .immutable(name.clone(), kind("disk"), "type")
            .immutable(name.clone(), kind("disk"), "zone")
            .monotonic(name.clone(), kind("disk"), "size")
            .at_birth(name.clone(), kind("disk"), "image_id")
            .identity(name.clone(), kind("disk"), "name", "name")
            // A network is only a name and a scope.
            .identity(name.clone(), kind("network"), "name", "name")
            // A subnet's zone, network and CIDR cannot move.
            .immutable(name.clone(), kind("subnet"), "zone")
            .immutable(name.clone(), kind("subnet"), "network_id")
            .immutable(name.clone(), kind("subnet"), "v4_cidr_blocks")
            .identity(name.clone(), kind("subnet"), "name", "name")
            // A bucket is a name and a folder.
            .identity(name.clone(), kind("bucket"), "name", "name")
            // A security group belongs to one network.
            .immutable(name.clone(), kind("security_group"), "network_id")
            .identity(name, kind("security_group"), "name", "name")
    }

    /// The held client, or the refusal a provider with no client owes a
    /// change.
    fn client(&self, address: &Address) -> Result<&Client, ProviderError> {
        self.client
            .as_ref()
            .ok_or_else(|| world(address, "the provider holds no API client"))
    }

    /// One call, mapped into the provider's error shape.
    fn api<T, F>(&self, address: &Address, make: impl Fn() -> F) -> Result<T, ProviderError>
    where
        F: Future<Output = Result<tonic::Response<T>, tonic::Status>>,
    {
        self.api_status(make)
            .map_err(|status| world(address, status.to_string()))
    }

    /// One call against the held client, and under an expired token one
    /// refresh and one retry. The retry is exactly once: a token the IAM
    /// service keeps handing out expired is a broken credential, not a reason
    /// to loop.
    fn api_status<T, F>(&self, make: impl Fn() -> F) -> Result<T, tonic::Status>
    where
        F: Future<Output = Result<tonic::Response<T>, tonic::Status>>,
    {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| tonic::Status::internal("the provider holds no API client"))?;
        let attempt = |client: &Client| client.call(make());
        match attempt(client) {
            Ok(response) => response.map(tonic::Response::into_inner),
            Err(ClientError::Expired { .. }) => {
                let key = self.key.as_ref().ok_or_else(|| {
                    tonic::Status::internal("the token expired and no key is held to refresh it")
                })?;
                client
                    .refresh(key)
                    .map_err(|error| tonic::Status::internal(error.to_string()))?;
                let response =
                    attempt(client).map_err(|error| tonic::Status::internal(error.to_string()))?;
                response.map(tonic::Response::into_inner)
            }
            Err(error) => Err(tonic::Status::internal(error.to_string())),
        }
    }

    /// Waits for one Yandex operation to finish: polls until `done`, refusing
    /// a failed operation and a hang. A poll sleeps rather than spins, and
    /// the deadline bounds a silent endpoint.
    fn wait_for_operation(
        &self,
        client: &Client,
        address: &Address,
        operation: Operation,
    ) -> Result<(), ProviderError> {
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut operation = operation;
        loop {
            if operation.done {
                return match operation.result {
                    Some(OperationResult::Error(status)) => Err(world(
                        address,
                        format!("the operation failed: {}", status.message),
                    )),
                    Some(OperationResult::Response(_)) | None => Ok(()),
                };
            }
            if Instant::now() >= deadline {
                return Err(world(address, "the operation did not finish in time"));
            }
            std::thread::sleep(Duration::from_millis(100));
            let operation_id = operation.id.clone();
            operation = self.api(address, move || {
                let operation_id = operation_id.clone();
                async move {
                    client
                        .operations()
                        .get(GetOperationRequest { operation_id })
                        .await
                }
            })?;
        }
    }

    /// Reads one kind in one folder: one listing, then what each reading's
    /// name matches. The batch is grouped so the world is asked once, not
    /// once per reading.
    fn read_kind(
        &self,
        client: &Client,
        kind: &str,
        folder_id: &str,
        at: &Address,
        readings: &[&Reading],
    ) -> Result<Vec<(Address, Attributes)>, ProviderError> {
        match kind {
            "instance" => self.read_instances(client, at, folder_id, readings),
            "disk" => self.read_disks(client, at, folder_id, readings),
            "network" => self.read_networks(client, at, folder_id, readings),
            "subnet" => self.read_subnets(client, at, folder_id, readings),
            "security_group" => self.read_groups(client, at, folder_id, readings),
            "bucket" => self.read_buckets(client, at, folder_id, readings),
            _ => Err(ProviderError::UnknownKind {
                provider: self.name.clone(),
                address: at.clone(),
            }),
        }
    }

    fn read_instances(
        &self,
        client: &Client,
        at: &Address,
        folder_id: &str,
        readings: &[&Reading],
    ) -> Result<Vec<(Address, Attributes)>, ProviderError> {
        let listed = self.api(at, move || {
            let folder_id = folder_id.to_owned();
            async move {
                client
                    .instances()
                    .list(ListInstancesRequest {
                        folder_id,
                        ..Default::default()
                    })
                    .await
            }
        })?;
        let mut found = Vec::new();
        for (address, instance) in found_by_name(readings, &listed.instances, |instance| {
            instance.name.as_str()
        })? {
            // The real List omits metadata, and an instance names its boot
            // disk by id while the declaration names it by image: read the
            // instance and its boot disk through Get.
            let instance_id = instance.id.clone();
            let instance = self.api(address, move || {
                let instance_id = instance_id.clone();
                async move {
                    client
                        .instances()
                        .get(GetInstanceRequest {
                            instance_id,
                            view: i32::from(InstanceView::Full),
                        })
                        .await
                }
            })?;
            let boot_disk_id = instance
                .boot_disk
                .as_ref()
                .map(|attached| attached.disk_id.clone())
                .ok_or_else(|| world(address, "the instance names no boot disk"))?;
            let boot_disk = self.api(address, move || {
                let boot_disk_id = boot_disk_id.clone();
                async move {
                    client
                        .disks()
                        .get(GetDiskRequest {
                            disk_id: boot_disk_id,
                        })
                        .await
                }
            })?;
            found.push((
                address.clone(),
                resources::instance_attributes(&instance, &boot_disk),
            ));
        }
        Ok(found)
    }

    fn read_disks(
        &self,
        client: &Client,
        at: &Address,
        folder_id: &str,
        readings: &[&Reading],
    ) -> Result<Vec<(Address, Attributes)>, ProviderError> {
        let listed = self.api(at, move || {
            let folder_id = folder_id.to_owned();
            async move {
                client
                    .disks()
                    .list(ListDisksRequest {
                        folder_id,
                        ..Default::default()
                    })
                    .await
            }
        })?;
        Ok(
            found_by_name(readings, &listed.disks, |disk| disk.name.as_str())?
                .into_iter()
                .map(|(address, disk)| (address.clone(), resources::disk_attributes(disk)))
                .collect(),
        )
    }

    fn read_networks(
        &self,
        client: &Client,
        at: &Address,
        folder_id: &str,
        readings: &[&Reading],
    ) -> Result<Vec<(Address, Attributes)>, ProviderError> {
        let listed = self.api(at, move || {
            let folder_id = folder_id.to_owned();
            async move {
                client
                    .networks()
                    .list(ListNetworksRequest {
                        folder_id,
                        ..Default::default()
                    })
                    .await
            }
        })?;
        Ok(
            found_by_name(readings, &listed.networks, |network| network.name.as_str())?
                .into_iter()
                .map(|(address, network)| (address.clone(), resources::network_attributes(network)))
                .collect(),
        )
    }

    fn read_subnets(
        &self,
        client: &Client,
        at: &Address,
        folder_id: &str,
        readings: &[&Reading],
    ) -> Result<Vec<(Address, Attributes)>, ProviderError> {
        let listed = self.api(at, move || {
            let folder_id = folder_id.to_owned();
            async move {
                client
                    .subnets()
                    .list(ListSubnetsRequest {
                        folder_id,
                        ..Default::default()
                    })
                    .await
            }
        })?;
        Ok(
            found_by_name(readings, &listed.subnets, |subnet| subnet.name.as_str())?
                .into_iter()
                .map(|(address, subnet)| (address.clone(), resources::subnet_attributes(subnet)))
                .collect(),
        )
    }

    fn read_groups(
        &self,
        client: &Client,
        at: &Address,
        folder_id: &str,
        readings: &[&Reading],
    ) -> Result<Vec<(Address, Attributes)>, ProviderError> {
        let listed = self.api(at, move || {
            let folder_id = folder_id.to_owned();
            async move {
                client
                    .security_groups()
                    .list(ListSecurityGroupsRequest {
                        folder_id,
                        ..Default::default()
                    })
                    .await
            }
        })?;
        Ok(found_by_name(readings, &listed.security_groups, |group| {
            group.name.as_str()
        })?
        .into_iter()
        .map(|(address, group)| (address.clone(), resources::security_group_attributes(group)))
        .collect())
    }

    fn read_buckets(
        &self,
        client: &Client,
        at: &Address,
        folder_id: &str,
        readings: &[&Reading],
    ) -> Result<Vec<(Address, Attributes)>, ProviderError> {
        let listed = self.api(at, move || {
            let folder_id = folder_id.to_owned();
            async move {
                client
                    .buckets()
                    .list(ListBucketsRequest {
                        folder_id,
                        ..Default::default()
                    })
                    .await
            }
        })?;
        Ok(
            found_by_name(readings, &listed.buckets, |bucket| bucket.name.as_str())?
                .into_iter()
                .map(|(address, bucket)| (address.clone(), resources::bucket_attributes(bucket)))
                .collect(),
        )
    }

    /// Reads one resource by its identity, or nothing where the world holds
    /// no such thing. A create's existence check, an update's current state,
    /// and a conclude's read-back all run through here.
    fn read_one(
        &self,
        client: &Client,
        kind: &str,
        folder_id: &str,
        name: &str,
        at: &Address,
    ) -> Result<Option<Attributes>, ProviderError> {
        let mut locator = Attributes::new();
        locator.insert("folder_id".to_owned(), Value::Text(folder_id.to_owned()));
        locator.insert("name".to_owned(), Value::Text(name.to_owned()));
        let reading = Reading::new(at.clone(), locator);
        let readings = [&reading];
        let mut found = self.read_kind(client, kind, folder_id, at, &readings)?;
        Ok(found.pop().map(|(_, attributes)| attributes))
    }

    fn prepare_create(&self, change: &Prepared<'_>) -> Result<Planned, ProviderError> {
        let client = self.client(change.address)?;
        let kind = change.address.kind().as_str();
        let (folder_id, name) = resources::lookup(change.desired, change.address)?;
        // The plan read nothing, but the world may already hold one: a resent
        // create can arrive after its earlier self already made the resource.
        if self
            .read_one(client, kind, &folder_id, &name, change.address)?
            .is_some()
        {
            return if change.resent {
                Ok(Planned::default())
            } else {
                Err(world(
                    change.address,
                    format!("a {kind} named `{name}` already exists"),
                ))
            };
        }
        let operation = match kind {
            "instance" => {
                let request = resources::create_instance(change.desired, change.address)?;
                self.api(change.address, move || {
                    let request = request.clone();
                    async move { client.instances().create(request).await }
                })?
            }
            "disk" => {
                let request = resources::create_disk(change.desired, change.address)?;
                self.api(change.address, move || {
                    let request = request.clone();
                    async move { client.disks().create(request).await }
                })?
            }
            "network" => {
                let request = resources::create_network(change.desired, change.address)?;
                self.api(change.address, move || {
                    let request = request.clone();
                    async move { client.networks().create(request).await }
                })?
            }
            "subnet" => {
                let request = resources::create_subnet(change.desired, change.address)?;
                self.api(change.address, move || {
                    let request = request.clone();
                    async move { client.subnets().create(request).await }
                })?
            }
            "security_group" => {
                let request = resources::create_security_group(change.desired, change.address)?;
                self.api(change.address, move || {
                    let request = request.clone();
                    async move { client.security_groups().create(request).await }
                })?
            }
            "bucket" => {
                let request = resources::create_bucket(change.desired, change.address)?;
                self.api(change.address, move || {
                    let request = request.clone();
                    async move { client.buckets().create(request).await }
                })?
            }
            _ => {
                return Err(ProviderError::UnknownKind {
                    provider: self.name.clone(),
                    address: change.address.clone(),
                });
            }
        };
        self.wait_for_operation(client, change.address, operation)?;
        Ok(Planned::default())
    }

    fn prepare_update(&self, change: &Prepared<'_>) -> Result<Planned, ProviderError> {
        let client = self.client(change.address)?;
        let kind = change.address.kind().as_str();
        let (folder_id, name) = resources::lookup(change.desired, change.address)?;
        let current = self
            .read_one(client, kind, &folder_id, &name, change.address)?
            .ok_or_else(|| {
                world(
                    change.address,
                    format!("a {kind} named `{name}` is not there to update"),
                )
            })?;
        if resources::folder_moved(&current, change.desired) {
            return Err(world(change.address, "a folder move is refused"));
        }
        match resources::differing(&current, change.desired) {
            None => Ok(Planned::default()),
            Some(attribute) if resources::in_place_attributes(kind).contains(&attribute) => {
                match kind {
                    "instance" => self.update_instance(client, change, &current),
                    "disk" => self.update_disk(client, change, &current),
                    _ => Err(world(
                        change.address,
                        format!("nothing of a {kind} changes in place"),
                    )),
                }
            }
            Some(attribute) => Err(world(
                change.address,
                format!("`{attribute}` cannot change in place"),
            )),
        }
    }

    /// Changes an instance's resources in place: stop, update, start. An
    /// instance already stopped is changed where it stands and is never
    /// started — a preemptible stop is a state the declaration did not ask to
    /// leave, and a start under preemption would lose the instance.
    fn update_instance(
        &self,
        client: &Client,
        change: &Prepared<'_>,
        current: &Attributes,
    ) -> Result<Planned, ProviderError> {
        let instance_id = resources::required_text(current, "id", change.address)?;
        let request =
            resources::update_instance(instance_id.clone(), change.desired, change.address)?;
        let instance = self.api(change.address, || {
            let instance_id = instance_id.clone();
            async move {
                client
                    .instances()
                    .get(GetInstanceRequest {
                        instance_id,
                        view: i32::from(InstanceView::Full),
                    })
                    .await
            }
        })?;
        let running = match InstanceStatus::try_from(instance.status) {
            Ok(InstanceStatus::Running) => true,
            Ok(InstanceStatus::Stopped) => false,
            Ok(other) => {
                return Err(world(
                    change.address,
                    format!("an instance in {other:?} cannot change its resources"),
                ));
            }
            Err(_) => {
                return Err(world(
                    change.address,
                    format!("the instance's status is unknown: {}", instance.status),
                ));
            }
        };
        if running {
            self.stop_and_wait(client, change.address, &instance_id)?;
        }
        let operation = self.api(change.address, move || {
            let request = request.clone();
            async move { client.instances().update(request).await }
        })?;
        self.wait_for_operation(client, change.address, operation)?;
        if running {
            self.start_and_wait(client, change.address, &instance_id)?;
        }
        Ok(Planned::default())
    }

    /// Grows a disk in place; the planner refuses a shrink before this is
    /// ever built, and the API would refuse it too.
    fn update_disk(
        &self,
        client: &Client,
        change: &Prepared<'_>,
        current: &Attributes,
    ) -> Result<Planned, ProviderError> {
        let disk_id = resources::required_text(current, "id", change.address)?;
        let request = resources::update_disk(disk_id, change.desired, change.address)?;
        let operation = self.api(change.address, move || {
            let request = request.clone();
            async move { client.disks().update(request).await }
        })?;
        self.wait_for_operation(client, change.address, operation)?;
        Ok(Planned::default())
    }

    fn stop_and_wait(
        &self,
        client: &Client,
        address: &Address,
        instance_id: &str,
    ) -> Result<(), ProviderError> {
        let operation = self.api(address, move || {
            let instance_id = instance_id.to_owned();
            async move {
                client
                    .instances()
                    .stop(StopInstanceRequest { instance_id })
                    .await
            }
        })?;
        self.wait_for_operation(client, address, operation)
    }

    fn start_and_wait(
        &self,
        client: &Client,
        address: &Address,
        instance_id: &str,
    ) -> Result<(), ProviderError> {
        let operation = self.api(address, move || {
            let instance_id = instance_id.to_owned();
            async move {
                client
                    .instances()
                    .start(StartInstanceRequest { instance_id })
                    .await
            }
        })?;
        self.wait_for_operation(client, address, operation)
    }

    fn prepare_replace(&self, change: &Prepared<'_>) -> Result<Planned, ProviderError> {
        let client = self.client(change.address)?;
        self.delete_recorded(client, change)?;
        self.prepare_create(change)
    }

    fn prepare_delete(&self, change: &Prepared<'_>) -> Result<Planned, ProviderError> {
        let client = self.client(change.address)?;
        self.delete_recorded(client, change)?;
        Ok(Planned::default())
    }

    /// Removes what state recorded, by its recorded id. A resent removal can
    /// arrive after its earlier self already deleted it; a fresh one cannot
    /// have.
    fn delete_recorded(&self, client: &Client, change: &Prepared<'_>) -> Result<(), ProviderError> {
        let recorded = change.recorded.ok_or_else(|| ProviderError::Declaration {
            address: change.address.clone(),
            problem: "a removal must know the recorded id".to_owned(),
        })?;
        let id = resources::required_text(recorded, "id", change.address)?;
        match self.delete_by_id(client, change.address.kind().as_str(), &id) {
            Ok(operation) => self.wait_for_operation(client, change.address, operation),
            Err(status) if status.code() == tonic::Code::NotFound => {
                if change.resent {
                    Ok(())
                } else {
                    Err(world(change.address, "already gone"))
                }
            }
            Err(status) => Err(world(change.address, status.to_string())),
        }
    }

    fn delete_by_id(
        &self,
        client: &Client,
        kind: &str,
        id: &str,
    ) -> Result<Operation, tonic::Status> {
        match kind {
            "instance" => {
                let request = DeleteInstanceRequest {
                    instance_id: id.to_owned(),
                };
                self.api_status(move || {
                    let request = request.clone();
                    async move { client.instances().delete(request).await }
                })
            }
            "disk" => {
                let request = DeleteDiskRequest {
                    disk_id: id.to_owned(),
                };
                self.api_status(move || {
                    let request = request.clone();
                    async move { client.disks().delete(request).await }
                })
            }
            "network" => {
                let request = DeleteNetworkRequest {
                    network_id: id.to_owned(),
                };
                self.api_status(move || {
                    let request = request.clone();
                    async move { client.networks().delete(request).await }
                })
            }
            "subnet" => {
                let request = DeleteSubnetRequest {
                    subnet_id: id.to_owned(),
                };
                self.api_status(move || {
                    let request = request.clone();
                    async move { client.subnets().delete(request).await }
                })
            }
            "security_group" => {
                let request = DeleteSecurityGroupRequest {
                    security_group_id: id.to_owned(),
                };
                self.api_status(move || {
                    let request = request.clone();
                    async move { client.security_groups().delete(request).await }
                })
            }
            "bucket" => {
                // A bucket's id is its name; the delete names it.
                let request = DeleteBucketRequest {
                    name: id.to_owned(),
                };
                self.api_status(move || {
                    let request = request.clone();
                    async move { client.buckets().delete(request).await }
                })
            }
            _ => Err(tonic::Status::internal("the provider owns no such kind")),
        }
    }
}

/// The world refused the change, with a problem named for the person reading
/// the error at three in the morning.
fn world(address: &Address, problem: impl Into<String>) -> ProviderError {
    ProviderError::World {
        address: address.clone(),
        problem: problem.into(),
    }
}

/// Pairs the readings with the listed resources that share their name, so a
/// read reports only what is actually there. `name_of` reads the name off a
/// listed resource; the listing was fetched once per folder above.
fn found_by_name<'a, T>(
    readings: &[&'a Reading],
    listed: &'a [T],
    name_of: impl Fn(&'a T) -> &'a str,
) -> Result<Vec<(&'a Address, &'a T)>, ProviderError> {
    let mut found = Vec::new();
    for reading in readings {
        let (_, name) = resources::lookup(reading.locator(), reading.address())?;
        if let Some(item) = listed.iter().find(|item| name_of(*item) == name) {
            found.push((reading.address(), item));
        }
    }
    Ok(found)
}

impl Default for Yandex {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for Yandex {
    fn name(&self) -> &ProviderName {
        &self.name
    }

    fn schema(&self) -> Schema {
        Self::schema_of(self.name.clone())
    }

    fn read(
        &self,
        batch: &[Reading],
        _facts: Facts<'_>,
        _reach: &dyn Reach,
    ) -> Result<Read, ProviderError> {
        let Some(client) = &self.client else {
            return Ok(Read::new());
        };
        // One listing per folder per kind; a provider with no client reads
        // nothing, and the world holds nothing the run can see.
        let mut by_folder: BTreeMap<(String, String), (Address, Vec<&Reading>)> = BTreeMap::new();
        for reading in batch {
            let (folder_id, _) = resources::lookup(reading.locator(), reading.address())?;
            by_folder
                .entry((reading.address().kind().as_str().to_owned(), folder_id))
                .or_insert_with(|| (reading.address().clone(), Vec::new()))
                .1
                .push(reading);
        }
        let mut read = Read::new();
        for ((kind, folder_id), (at, readings)) in by_folder {
            for (address, attributes) in
                self.read_kind(client, &kind, &folder_id, &at, &readings)?
            {
                read = read.with(address, attributes, Vec::new());
            }
        }
        Ok(read)
    }

    fn prepare(&self, change: &Prepared<'_>) -> Result<Planned, ProviderError> {
        match &change.kind {
            Sending::Create => self.prepare_create(change),
            Sending::Update => self.prepare_update(change),
            Sending::Replace => self.prepare_replace(change),
            Sending::Delete => self.prepare_delete(change),
            Sending::React => Err(world(change.address, "nothing of a Yandex resource reacts")),
            _ => Err(world(
                change.address,
                "a change kind this provider does not handle",
            )),
        }
    }

    fn conclude(
        &self,
        change: &Concluding<'_>,
        _became: &[Answered],
    ) -> Result<Concluded, ProviderError> {
        if change.kind == Sending::Delete {
            return Ok(Concluded::Gone);
        }
        let client = self.client(change.address)?;
        let kind = change.address.kind().as_str();
        let (folder_id, name) = resources::lookup(change.plain, change.address)?;
        match self.read_one(client, kind, &folder_id, &name, change.address)? {
            Some(attributes) => Ok(Concluded::Became(attributes)),
            None => Err(world(change.address, "not there after the change")),
        }
    }

    fn afterwards(&self, _address: &Address, _kind: Sending) -> Afterwards {
        Afterwards::Read
    }

    /// The provider without a client: reads nothing and refuses every change,
    /// which is exactly what a fake owes a plan that must not act.
    fn fake() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nerpa_core::address::address;

    #[test]
    fn the_schema_names_what_changes_in_place_and_what_does_not() {
        let schema = Yandex::new().schema();
        let instance = address("yandex:instance.db");
        let disk = address("yandex:disk.data");
        let subnet = address("yandex:subnet.mgmt");
        let group = address("yandex:security_group.web");

        // An instance's zone and subnet cannot move; its boot disk was born;
        // its cores need a stop to change.
        assert!(schema.forces_replacement(&instance, "zone"));
        assert!(schema.forces_replacement(&instance, "subnet"));
        assert!(schema.is_at_birth(&instance, "boot_disk"));
        assert!(schema.requires_stop(&instance, "cores"));
        assert!(!schema.forces_replacement(&instance, "cores"));

        // A disk's size only grows; its type cannot move.
        assert!(schema.grows_only(&disk, "size"));
        assert!(schema.forces_replacement(&disk, "type"));

        // A subnet's zone and network cannot move.
        assert!(schema.forces_replacement(&subnet, "zone"));
        assert!(schema.forces_replacement(&subnet, "network_id"));

        // A security group belongs to one network.
        assert!(schema.forces_replacement(&group, "network_id"));
    }
}
