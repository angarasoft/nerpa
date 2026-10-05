//! The HTTP boundary a provider speaks to Yandex Cloud through.
//!
//! Kept inside this crate and behind a trait so the engine never sees an HTTP
//! type, and the client stays swappable — a recorder and a replayer stand in
//! for the wire in the tests (adr-2026-070).

use thiserror::Error;

/// One exchange, narrowed to a status and a text body.
#[derive(Debug)]
pub struct Response {
    /// The HTTP status code.
    pub status: u16,
    /// The body as text, read whole.
    pub body: String,
}

/// Why a call could not be made.
#[derive(Debug, Error)]
pub enum ApiError {
    /// The transport could not carry the request or read the answer.
    #[error("the API call failed: {0}")]
    Transport(String),
}

/// The seam between the provider and the wire.
///
/// Four verbs, because that is all a cloud provider asks: read, make, change,
/// remove. A provider never builds a URL with a body for a read, so the verbs
/// keep that apart in the type rather than in a string.
pub trait Api: Send + Sync {
    /// Reads a resource or a list of them.
    ///
    /// # Errors
    ///
    /// Returns [`ApiError::Transport`] when the call could not be carried.
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, ApiError>;

    /// Makes a resource.
    ///
    /// # Errors
    ///
    /// Returns [`ApiError::Transport`] when the call could not be carried.
    fn post(&self, url: &str, headers: &[(&str, &str)], body: &str) -> Result<Response, ApiError>;

    /// Changes a resource in place.
    ///
    /// # Errors
    ///
    /// Returns [`ApiError::Transport`] when the call could not be carried.
    fn patch(&self, url: &str, headers: &[(&str, &str)], body: &str) -> Result<Response, ApiError>;

    /// Removes a resource.
    ///
    /// # Errors
    ///
    /// Returns [`ApiError::Transport`] when the call could not be carried.
    fn delete(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, ApiError>;
}

/// The real wire, over a blocking client (ureq, adr-2026-070).
pub struct Http {
    agent: ureq::Agent,
}

impl std::fmt::Debug for Http {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Http").finish_non_exhaustive()
    }
}

impl Http {
    /// An API over ureq, with a whole-call timeout.
    #[must_use]
    pub fn new(timeout: std::time::Duration) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .build();
        Self {
            agent: config.into(),
        }
    }

    fn read(
        response: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
    ) -> Result<Response, ApiError> {
        let mut response = response.map_err(|error| ApiError::Transport(error.to_string()))?;
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|error| ApiError::Transport(error.to_string()))?;
        Ok(Response { status, body })
    }
}

impl Api for Http {
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, ApiError> {
        let mut request = self.agent.get(url);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        Self::read(request.call())
    }

    fn post(&self, url: &str, headers: &[(&str, &str)], body: &str) -> Result<Response, ApiError> {
        let mut request = self.agent.post(url);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        Self::read(request.send(body))
    }

    fn patch(&self, url: &str, headers: &[(&str, &str)], body: &str) -> Result<Response, ApiError> {
        let mut request = self.agent.patch(url);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        Self::read(request.send(body))
    }

    fn delete(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, ApiError> {
        let mut request = self.agent.delete(url);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        Self::read(request.call())
    }
}
