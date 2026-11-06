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

impl Response {
    /// The answer as the wire gives it: a status outside 2xx is carried as
    /// [`ApiError::Refused`] with what the API said, so a replayed exchange
    /// and a live one refuse the same way.
    ///
    /// # Errors
    ///
    /// Returns [`ApiError::Refused`] where the status is outside 2xx.
    #[must_use = "an answer is either kept or refused"]
    pub fn or_refused(self) -> Result<Self, ApiError> {
        if (200..300).contains(&self.status) {
            Ok(self)
        } else {
            // What the API said is kept to a readable length: a refusal's
            // body is a line or two, and an answer of any size is not turned
            // into an error nobody reads.
            Err(ApiError::Refused {
                status: self.status,
                said: self.body.chars().take(512).collect(),
            })
        }
    }
}

/// Why a call could not be made.
#[derive(Debug, Error)]
pub enum ApiError {
    /// The transport could not carry the request or read the answer.
    #[error("the API call failed: {0}")]
    Transport(String),
    /// The API refused the call, and what it said. A refusal's body names the
    /// field it refused, so it is carried — to a readable length — except at
    /// the token exchange, the one place a credential has just been sent and
    /// the answer is not echoed.
    #[error("the API call failed: http status: {status}; the API said: {said}")]
    Refused {
        /// The HTTP status code of the refusal.
        status: u16,
        /// What the API answered, kept to a readable length.
        said: String,
    },
}

/// The seam between the provider and the wire.
///
/// Five verbs, because that is all a cloud provider asks: read, make, change,
/// write whole and remove. A provider never builds a URL with a body for a
/// read, so the verbs keep that apart in the type rather than in a string.
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

    /// Writes a resource whole, the way the S3 door asks: a sub-resource
    /// carries its document as a PUT, not a patch. Required of every wire —
    /// a provider function that is declared is not an option, and a wire
    /// that cannot write the document is a compile error, not a live-cloud
    /// surprise.
    ///
    /// # Errors
    ///
    /// Returns [`ApiError::Transport`] when the call could not be carried.
    fn put(&self, url: &str, headers: &[(&str, &str)], body: &str) -> Result<Response, ApiError>;

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
            // A refused call is an answer, not a transport failure: the body
            // the API wrote with it is read, so a refusal says why it
            // happened instead of naming a number.
            .http_status_as_error(false)
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
        Response { status, body }.or_refused()
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
        // A JSON body is labelled JSON where the caller named no label of
        // its own: a gateway that reads the body by its content type refuses
        // a body that claims nothing, and a caller that named one sends
        // exactly that one, not two.
        if !headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
        {
            request = request.header("Content-Type", "application/json");
        }
        Self::read(request.send(body))
    }

    fn patch(&self, url: &str, headers: &[(&str, &str)], body: &str) -> Result<Response, ApiError> {
        let mut request = self.agent.patch(url);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        if !headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
        {
            request = request.header("Content-Type", "application/json");
        }
        Self::read(request.send(body))
    }

    fn put(&self, url: &str, headers: &[(&str, &str)], body: &str) -> Result<Response, ApiError> {
        let mut request = self.agent.put(url);
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
