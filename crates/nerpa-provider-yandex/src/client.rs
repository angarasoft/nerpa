//! The gRPC client: a channel, a short-lived token, and the runtime that
//! bridges the synchronous [`Provider`](nerpa_core::provider::Provider) trait
//! to the async tonic client.

use std::future::Future;
use std::sync::Arc;

use tonic::service::Interceptor;
use tonic::transport::{Channel, Endpoint};

use crate::auth::IamToken;
use crate::yandex::cloud::compute::v1::disk_service_client::DiskServiceClient;
use crate::yandex::cloud::compute::v1::instance_service_client::InstanceServiceClient;
use crate::yandex::cloud::vpc::v1::network_service_client::NetworkServiceClient;
use crate::yandex::cloud::vpc::v1::security_group_service_client::SecurityGroupServiceClient;
use crate::yandex::cloud::vpc::v1::subnet_service_client::SubnetServiceClient;

/// Adds the bearer token to every request.
#[derive(Debug, Clone)]
pub struct Bearer {
    token: Arc<str>,
}

impl Interceptor for Bearer {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, tonic::Status> {
        let value = format!("Bearer {}", self.token);
        let value = value
            .parse()
            .map_err(|_| tonic::Status::internal("a bad bearer token"))?;
        request.metadata_mut().insert("authorization", value);
        Ok(request)
    }
}

/// Why the client could not be built.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// The channel could not be opened.
    #[error("the Yandex endpoint could not be reached: {0}")]
    Connect(String),
}

/// A Yandex Cloud gRPC client, holding a channel and a short-lived token.
pub struct Client {
    runtime: tokio::runtime::Runtime,
    channel: Channel,
    token: Arc<str>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("channel", &self.channel)
            .field("token", &self.token)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Connects to the endpoint and carries the token in every request.
    pub fn connect(endpoint: &str, token: IamToken) -> Result<Self, ClientError> {
        let runtime =
            tokio::runtime::Runtime::new().map_err(|e| ClientError::Connect(e.to_string()))?;
        let endpoint = Endpoint::from_shared(endpoint.to_owned())
            .map_err(|e| ClientError::Connect(e.to_string()))?;
        let channel = runtime
            .block_on(endpoint.connect())
            .map_err(|e| ClientError::Connect(e.to_string()))?;
        Ok(Self {
            runtime,
            channel,
            token: Arc::from(token.token.as_str()),
        })
    }

    /// Runs an async future on the client's runtime.
    ///
    /// This is the sync-to-async bridge: the [`Provider`](nerpa_core::provider::Provider)
    /// trait is synchronous, and tonic is not, so the provider's methods run
    /// their gRPC calls through here.
    #[allow(
        dead_code,
        reason = "the provider's read and prepare use it; they arrive in w-yc-provider"
    )]
    pub(crate) fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }

    fn bearer(&self) -> Bearer {
        Bearer {
            token: self.token.clone(),
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bearer_puts_the_token_in_the_header() {
        let mut bearer = Bearer {
            token: Arc::from("abc"),
        };
        let request = bearer.call(tonic::Request::new(())).expect("intercepted");
        let header = request.metadata().get("authorization").expect("a header");
        assert_eq!(header.to_str().expect("ascii"), "Bearer abc");
    }
}
