//! Who acts in a run, and over what.
//!
//! A run without an actor does not proceed (`INV-TRUST-001`): the identity is
//! carried into the plan and its number, so approving a plan is approving it
//! *by somebody* — the detached signature of `0009` is over the number, and a
//! number that did not cover who proposed it could be moved between people.
//! The run journal beside state repeats it for a person (`RFC 0001` §5.9).
//!
//! What this is not: authentication. The engine cannot tell a real OIDC subject
//! from a string, and does not try — that a name is who it says it is belongs to
//! the grant and the identity provider (`0009`), not to a value carried here.
//! `scope` is carried for that grant to one day check; nothing reads it yet.
//! `expiry` is checked here, because checking it needs no grant: an identity
//! whose expiry has passed does not run, and an expiry that is not an `RFC3339`
//! time is refused by name rather than carried as a string.

use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Who acts in a run, over what, and until when.
///
/// Carried whole into every plan and every run journal line. The three parts
/// are the shape a grant reads (`0009`): who is limited, to what, and until
/// when. Two runs that differ only in who acts are two runs — the number says
/// so.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Actor {
    /// Who acts: a person, CI, or an agent. A name, not a credential.
    who: String,
    /// What the actor may touch. Carried now, checked when grants arrive.
    scope: String,
    /// When this identity stops being valid, as written. Checked where the
    /// actor is made — an expiry already in the past is refused — and carried
    /// into the plan and its number; absent means no expiry was given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expiry: Option<String>,
}

/// Why an actor was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ActorError {
    /// An actor that names nobody is not an actor identity, and letting it
    /// through would make `INV-TRUST-001` a claim about a string.
    #[error("an actor needs a name: `--as` was empty")]
    EmptyWho,
    /// The same for what it is limited to: a grant over an empty scope limits
    /// nothing, and the shape is the promise.
    #[error("an actor needs a scope: `--as-scope` was empty")]
    EmptyScope,
    /// The expiry is not an `RFC3339` time. Refused by name, because an expiry
    /// the comparison cannot read is the decorative flag all over again.
    #[error("an actor's expiry is not an RFC3339 time: {0:?}")]
    MalformedExpiry(String),
    /// The expiry has already passed: this identity stopped being valid before
    /// the run asked to use it.
    #[error("an actor's expiry is already in the past: {0:?}")]
    Expired(String),
}

impl Actor {
    /// An actor naming who acts, over what scope, and until when.
    ///
    /// # Errors
    ///
    /// Returns [`ActorError::EmptyWho`] or [`ActorError::EmptyScope`] for an
    /// empty name or scope: an identity that names nobody is not one, and a run
    /// carrying it would pass `INV-TRUST-001` in the letter while failing it in
    /// the fact. Returns [`ActorError::MalformedExpiry`] for an expiry that is
    /// not an `RFC3339` time, and [`ActorError::Expired`] for one already in the
    /// past: an expiry is a boundary, and a boundary nothing compares is a
    /// label.
    pub fn new(
        who: impl Into<String>,
        scope: impl Into<String>,
        expiry: Option<String>,
    ) -> Result<Self, ActorError> {
        let who = who.into();
        let scope = scope.into();
        if who.trim().is_empty() {
            return Err(ActorError::EmptyWho);
        }
        if scope.trim().is_empty() {
            return Err(ActorError::EmptyScope);
        }
        if let Some(written) = expiry.as_deref() {
            let until = DateTime::parse_from_rfc3339(written)
                .map_err(|_| ActorError::MalformedExpiry(written.to_owned()))?;
            let now: DateTime<Utc> = std::time::SystemTime::now().into();
            if until.naive_utc() < now.naive_utc() {
                return Err(ActorError::Expired(written.to_owned()));
            }
        }
        Ok(Self { who, scope, expiry })
    }

    /// Who acts.
    #[must_use]
    pub fn who(&self) -> &str {
        &self.who
    }

    /// What the actor may touch.
    #[must_use]
    pub fn scope(&self) -> &str {
        &self.scope
    }

    /// When this identity stops being valid, as written, if it was.
    #[must_use]
    pub fn expiry(&self) -> Option<&str> {
        self.expiry.as_deref()
    }
}

impl fmt::Display for Actor {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            out,
            "{} over {} until {}",
            self.who,
            self.scope,
            self.expiry.as_deref().unwrap_or("further notice")
        )
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "tests may be blunt: see docs/CODING_STANDARDS.md"
)]
mod tests {
    use super::{Actor, ActorError};

    #[test]
    fn an_actor_that_names_nobody_is_refused() {
        assert_eq!(
            Actor::new("  ", "run", None).unwrap_err(),
            ActorError::EmptyWho
        );
    }

    #[test]
    fn an_actor_over_no_scope_is_refused() {
        assert_eq!(
            Actor::new("alice", "", None).unwrap_err(),
            ActorError::EmptyScope
        );
    }

    #[test]
    fn an_expiry_in_the_future_is_carried_and_one_that_was_not_given_is_absent() {
        let given = Actor::new("alice", "run", Some("2999-06-01T00:00:00Z".to_owned())).unwrap();
        assert_eq!(given.expiry(), Some("2999-06-01T00:00:00Z"));
        assert_eq!(Actor::new("alice", "run", None).unwrap().expiry(), None);
    }

    /// The flag was read and checked by nothing (review v0013's minor-1). The
    /// refusal names the expiry it refused.
    #[test]
    fn an_expiry_already_in_the_past_is_refused_naming_it() {
        let refused =
            Actor::new("alice", "run", Some("2000-01-01T00:00:00Z".to_owned())).unwrap_err();
        assert_eq!(
            refused,
            ActorError::Expired("2000-01-01T00:00:00Z".to_owned())
        );
    }

    #[test]
    fn an_expiry_that_is_not_a_time_is_refused_naming_the_format() {
        let refused = Actor::new("alice", "run", Some("yesterday".to_owned())).unwrap_err();
        assert_eq!(refused, ActorError::MalformedExpiry("yesterday".to_owned()));
        assert!(refused.to_string().contains("RFC3339"), "{refused}");
    }
}
