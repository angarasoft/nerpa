//! A recorded exchange, replayed in a test.
//!
//! The `recorded-exchange` tests pin the provider's wire format: a request must
//! be asked exactly as it was recorded, and the answer is whatever the real API
//! answered. The recording itself is made against the real account, once, and
//! committed beside the test — a live call is never made by a test.

use std::collections::VecDeque;

use crate::api::{Api, ApiError, Response};

/// One exchange: the request as it must be asked, and the answer to give.
#[derive(Debug)]
pub struct Exchange {
    /// The method the request must use.
    pub method: &'static str,
    /// The URL the request must ask, exactly.
    pub url: String,
    /// The body the request must carry, where it carries one.
    pub body: Option<String>,
    /// What to answer.
    pub response: Response,
}

/// An API that replays recorded exchanges, in order, and refuses a call it was
/// not recorded for.
#[derive(Debug, Default)]
pub struct Recorded {
    exchanges: std::sync::Mutex<VecDeque<Exchange>>,
}

impl Recorded {
    /// A recorder with these exchanges, replayed in order.
    #[must_use]
    pub fn new(exchanges: Vec<Exchange>) -> Self {
        Self {
            exchanges: std::sync::Mutex::new(exchanges.into()),
        }
    }

    /// The next exchange, if the call matches it.
    fn replay(&self, method: &str, url: &str, body: Option<&str>) -> Result<Response, ApiError> {
        let mut exchanges = self.exchanges.lock().map_err(|_| {
            ApiError::Transport("the recorded exchange store is poisoned".to_owned())
        })?;
        let exchange = exchanges.pop_front().ok_or_else(|| {
            ApiError::Transport(format!("no recorded exchange for {method} {url}"))
        })?;
        if exchange.method != method || exchange.url != url {
            return Err(ApiError::Transport(format!(
                "recorded {} {} does not match {method} {url}",
                exchange.method, exchange.url
            )));
        }
        if exchange.body.as_deref() != body {
            return Err(ApiError::Transport(format!(
                "the body for {method} {url} does not match the recording"
            )));
        }
        Ok(exchange.response)
    }
}

impl Api for Recorded {
    fn get(&self, url: &str, _headers: &[(&str, &str)]) -> Result<Response, ApiError> {
        self.replay("GET", url, None)
    }

    fn post(&self, url: &str, _headers: &[(&str, &str)], body: &str) -> Result<Response, ApiError> {
        self.replay("POST", url, Some(body))
    }

    fn patch(
        &self,
        url: &str,
        _headers: &[(&str, &str)],
        body: &str,
    ) -> Result<Response, ApiError> {
        self.replay("PATCH", url, Some(body))
    }

    fn delete(&self, url: &str, _headers: &[(&str, &str)]) -> Result<Response, ApiError> {
        self.replay("DELETE", url, None)
    }
}
