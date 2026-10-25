//! The gRPC client: a channel, a short-lived token, and the runtime that
//! bridges the synchronous [`Provider`](nerpa_core::provider::Provider) trait
//! to the async tonic client.

use std::future::Future;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

use tonic::service::Interceptor;
use tonic::transport::{Channel, Endpoint};

use crate::auth::{self, ServiceAccountKey};
use crate::yandex::cloud::compute::v1::disk_service_client::DiskServiceClient;
use crate::yandex::cloud::compute::v1::instance_service_client::InstanceServiceClient;
use crate::yandex::cloud::operation::operation_service_client::OperationServiceClient;
use crate::yandex::cloud::storage::v1::bucket_service_client::BucketServiceClient;
use crate::yandex::cloud::vpc::v1::network_service_client::NetworkServiceClient;
use crate::yandex::cloud::vpc::v1::security_group_service_client::SecurityGroupServiceClient;
use crate::yandex::cloud::vpc::v1::subnet_service_client::SubnetServiceClient;

/// The most the channel may take to connect.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// The most one call may take: no answer is an error, not a hang
/// (`INV-REACH-001`).
const CALL_TIMEOUT: Duration = Duration::from_secs(60);

/// How long a token is asked to live. The endpoint's own ceiling is an hour.
const TOKEN_TTL: u64 = 3_600;

/// The Object Storage bucket service lives on its own host (ADR-072).
const STORAGE_ENDPOINT: &str = "https://storage.api.cloud.yandex.net:443";

/// The bearer token every request carries, and when it stops working.
type Carried = Arc<RwLock<(Arc<str>, SystemTime)>>;

/// Adds the bearer token to every request.
///
/// Reads the token when a request is made, so a refresh between two calls is
/// carried by the next one. The `Debug` output names nothing of the token.
pub struct Bearer {
    carried: Carried,
}

impl Interceptor for Bearer {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, tonic::Status> {
        let (token, _) = &*self
            .carried
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let value = format!("Bearer {token}");
        let value = value
            .parse()
            .map_err(|_| tonic::Status::internal("a bad bearer token"))?;
        request.metadata_mut().insert("authorization", value);
        Ok(request)
    }
}

impl std::fmt::Debug for Bearer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bearer").finish_non_exhaustive()
    }
}

/// Why the client could not be built or used.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// The channel could not be opened.
    #[error("the Yandex endpoint could not be reached: {0}")]
    Connect(String),
    /// The token the client carries has expired.
    #[error("the IAM token expired at {at:?}; exchange a new one before the next call")]
    Expired {
        /// When the carried token stopped working.
        at: SystemTime,
    },
}

/// A Yandex Cloud gRPC client, holding a channel and a short-lived token.
///
/// The token is carried in a cell the requests read, so it can be refreshed
/// without rebuilding the client; every call refuses with
/// [`ClientError::Expired`] once it is past its expiry (070).
pub struct Client {
    runtime: tokio::runtime::Runtime,
    channel: Channel,
    /// The bucket service's own channel: Object Storage answers on its own
    /// host, and the bearer is the same token (ADR-072).
    storage: Channel,
    /// Where a refreshed token is exchanged: the same service the first one
    /// came from, so a refresh never moves the credential somewhere else.
    iam_endpoint: Arc<str>,
    carried: Carried,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("channel", &self.channel)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Signs one JWT, exchanges it for a short-lived IAM token at the
    /// default IAM endpoint, and connects to the endpoint the token will be
    /// carried to.
    ///
    /// The channel speaks TLS — an `https` endpoint dialled plain would be
    /// refused — and both the connect and each call are bounded, so a silent
    /// endpoint is an error rather than a hung run.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::Connect`] when the key is unusable, the token
    /// exchange fails, or the endpoint cannot be reached.
    pub fn connect(endpoint: &str, key: &ServiceAccountKey) -> Result<Self, ClientError> {
        Self::connect_at(endpoint, auth::IAM_ENDPOINT, key)
    }

    /// Signs one JWT, exchanges it for a short-lived IAM token at the named
    /// IAM endpoint, and connects to the endpoint the token will be carried
    /// to. The IAM endpoint is a parameter rather than a constant so a test
    /// can point both at one in-process fake.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::Connect`] when the key is unusable, the token
    /// exchange fails, or the endpoint cannot be reached.
    pub fn connect_at(
        endpoint: &str,
        iam_endpoint: &str,
        key: &ServiceAccountKey,
    ) -> Result<Self, ClientError> {
        Self::connect_at_with_storage(endpoint, iam_endpoint, STORAGE_ENDPOINT, key)
    }

    /// Signs one JWT, exchanges it for a short-lived IAM token at the named
    /// IAM endpoint, and connects to the endpoint the token will be carried
    /// to, plus the named Object Storage endpoint. The endpoints are
    /// parameters rather than constants so a test can point all of them at
    /// one in-process fake.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::Connect`] when the key is unusable, the token
    /// exchange fails, or an endpoint cannot be reached.
    pub fn connect_at_with_storage(
        endpoint: &str,
        iam_endpoint: &str,
        storage_endpoint: &str,
        key: &ServiceAccountKey,
    ) -> Result<Self, ClientError> {
        let runtime =
            tokio::runtime::Runtime::new().map_err(|e| ClientError::Connect(e.to_string()))?;
        let carried = Arc::new(RwLock::new(
            runtime
                .block_on(exchange(key, iam_endpoint))
                .map_err(|e| ClientError::Connect(e.to_string()))?,
        ));
        let endpoint = Endpoint::from_shared(endpoint.to_owned())
            .map_err(|e| ClientError::Connect(e.to_string()))?
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(CALL_TIMEOUT)
            .tls_config(tonic::transport::ClientTlsConfig::new().with_native_roots())
            .map_err(|e| ClientError::Connect(e.to_string()))?;
        let channel = runtime
            .block_on(endpoint.connect())
            .map_err(|e| ClientError::Connect(e.to_string()))?;
        let storage = runtime
            .block_on(
                Endpoint::from_shared(storage_endpoint.to_owned())
                    .map_err(|e| ClientError::Connect(e.to_string()))?
                    .connect_timeout(CONNECT_TIMEOUT)
                    .timeout(CALL_TIMEOUT)
                    .tls_config(tonic::transport::ClientTlsConfig::new().with_native_roots())
                    .map_err(|e| ClientError::Connect(e.to_string()))?
                    .connect(),
            )
            .map_err(|e| ClientError::Connect(e.to_string()))?;
        Ok(Self {
            runtime,
            channel,
            storage,
            iam_endpoint: Arc::from(iam_endpoint),
            carried,
        })
    }

    /// Runs an async future on the client's runtime, refusing to send a
    /// request under a token that has expired.
    ///
    /// This is the sync-to-async bridge: the
    /// [`Provider`](nerpa_core::provider::Provider) trait is synchronous, and
    /// tonic is not, so the provider's methods run their gRPC calls through
    /// here.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::Expired`] once the carried token is past its
    /// expiry, which is the point of carrying one (070).
    pub(crate) fn call<F: Future>(&self, future: F) -> Result<F::Output, ClientError> {
        let (_, expires_at) = *self
            .carried
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if SystemTime::now() >= expires_at {
            return Err(ClientError::Expired { at: expires_at });
        }
        Ok(self.runtime.block_on(future))
    }

    /// Signs a fresh JWT and exchanges it for a fresh token, before the old
    /// one expires.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::Connect`] when the key is unusable or the
    /// exchange fails.
    pub fn refresh(&self, key: &ServiceAccountKey) -> Result<(), ClientError> {
        let token = self
            .runtime
            .block_on(exchange(key, &self.iam_endpoint))
            .map_err(|e| ClientError::Connect(e.to_string()))?;
        let mut carried = self
            .carried
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *carried = token;
        Ok(())
    }

    fn bearer(&self) -> Bearer {
        Bearer {
            carried: self.carried.clone(),
        }
    }

    /// The instance service, with the token injected.
    pub fn instances(
        &self,
    ) -> InstanceServiceClient<tonic::service::interceptor::InterceptedService<Channel, Bearer>>
    {
        InstanceServiceClient::with_interceptor(self.channel.clone(), self.bearer())
    }

    /// The disk service, with the token injected.
    pub fn disks(
        &self,
    ) -> DiskServiceClient<tonic::service::interceptor::InterceptedService<Channel, Bearer>> {
        DiskServiceClient::with_interceptor(self.channel.clone(), self.bearer())
    }

    /// The network service, with the token injected.
    pub fn networks(
        &self,
    ) -> NetworkServiceClient<tonic::service::interceptor::InterceptedService<Channel, Bearer>>
    {
        NetworkServiceClient::with_interceptor(self.channel.clone(), self.bearer())
    }

    /// The subnet service, with the token injected.
    pub fn subnets(
        &self,
    ) -> SubnetServiceClient<tonic::service::interceptor::InterceptedService<Channel, Bearer>> {
        SubnetServiceClient::with_interceptor(self.channel.clone(), self.bearer())
    }

    /// The security group service, with the token injected.
    pub fn security_groups(
        &self,
    ) -> SecurityGroupServiceClient<tonic::service::interceptor::InterceptedService<Channel, Bearer>>
    {
        SecurityGroupServiceClient::with_interceptor(self.channel.clone(), self.bearer())
    }

    /// The operation service, with the token injected: the endpoint the
    /// provider polls while a create, update or delete completes.
    pub fn operations(
        &self,
    ) -> OperationServiceClient<tonic::service::interceptor::InterceptedService<Channel, Bearer>>
    {
        OperationServiceClient::with_interceptor(self.channel.clone(), self.bearer())
    }

    /// The bucket service, with the token injected: Object Storage's own
    /// host, reached through the channel made for it (ADR-072).
    pub fn buckets(
        &self,
    ) -> BucketServiceClient<tonic::service::interceptor::InterceptedService<Channel, Bearer>> {
        BucketServiceClient::with_interceptor(self.storage.clone(), self.bearer())
    }
}

/// One JWT, one exchange: the key signs once and is forgotten, and what is
/// carried afterwards is the short-lived token (070).
async fn exchange(
    key: &ServiceAccountKey,
    iam_endpoint: &str,
) -> Result<(Arc<str>, SystemTime), auth::AuthError> {
    let jwt = auth::signed_jwt(key, auth::now_seconds()?, TOKEN_TTL)?;
    let token = auth::token_for(&jwt, iam_endpoint).await?;
    Ok((Arc::from(token.token()), token.expires_at()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_bearer() -> Bearer {
        Bearer {
            carried: Arc::new(RwLock::new((
                Arc::from("abc"),
                SystemTime::now() + Duration::from_secs(3_600),
            ))),
        }
    }

    /// A client whose token is already past its expiry. The channel is a
    /// literal tonic never dials — `call` refuses before anything is sent —
    /// so no network exists in this test.
    fn a_client_with_expired_token() -> Client {
        let runtime = tokio::runtime::Runtime::new().expect("a runtime");
        let channel =
            runtime.block_on(async { Endpoint::from_static("http://[::1]:1").connect_lazy() });
        let storage =
            runtime.block_on(async { Endpoint::from_static("http://[::1]:1").connect_lazy() });
        Client {
            runtime,
            channel,
            storage,
            iam_endpoint: Arc::from("http://[::1]:1"),
            carried: Arc::new(RwLock::new((
                Arc::from("abc"),
                SystemTime::now() - Duration::from_secs(1),
            ))),
        }
    }

    #[test]
    fn a_call_under_an_expired_token_is_refused() {
        let client = a_client_with_expired_token();
        assert!(matches!(
            client.call(std::future::ready(())),
            Err(ClientError::Expired { .. })
        ));
    }

    #[test]
    fn the_bearer_puts_the_token_in_the_header() {
        let mut bearer = a_bearer();
        let request = bearer.call(tonic::Request::new(())).expect("intercepted");
        let header = request.metadata().get("authorization").expect("a header");
        assert_eq!(header.to_str().expect("ascii"), "Bearer abc");
    }

    #[test]
    fn no_debug_prints_the_token() {
        let debug = format!("{:?}", a_bearer());
        assert!(!debug.contains("abc"), "the token is redacted: {debug}");
    }
}
