//! Resource addresses: the names Sarma writes into state and never gets to change.
//!
//! An address is the identity of one resource across every run, every plan and
//! every state file that will ever exist. Once a state file leaves this machine
//! the scheme is frozen, so the grammar here is deliberately small.
//!
//! ```text
//!   provider ":" kind "." name [ ":" slot ] [ "@" target ]
//!
//!   yandex:instance.db                     a VM, addressed by name
//!   yandex:subnet.app:ru-central1-a        two subnets of one declaration
//!   os:package.nginx@web-01                a package on one node
//!   os:package.python:3.11@web-01          two Python slots, side by side
//! ```
//!
//! # What is deliberately absent
//!
//! **There is no index.** No field holds one and no constructor accepts one,
//! because an ordinal position in a list is not an identity: removing an entry
//! from the middle renumbers everything after it, and a planner that believes
//! those numbers will happily propose destroying live resources that merely
//! moved. Repetition is expressed over stable keys, and the key that
//! distinguishes two instances of one declaration is the [`Slot`].
//!
//! # What the types rule out
//!
//! No component can be empty, and none can begin with `-`. An address therefore
//! cannot be constructed that a shell, an SSH command line or a provider CLI
//! would read as an option, no matter how carelessly it is later interpolated.

use std::fmt;

use crate::{ensures, requires};

/// Anything that can go wrong while reading an address from text.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AddressError {
    #[error("address is missing the provider prefix (expected `provider:kind.name`): {0:?}")]
    NoProvider(String),
    #[error("address is missing the resource kind (expected `provider:kind.name`): {0:?}")]
    NoKind(String),
    #[error("{component} is empty in {address:?}")]
    Empty {
        component: &'static str,
        address: String,
    },
    #[error("{component} {value:?} starts with `-`, which could be read as a command line option")]
    LeadingDash {
        component: &'static str,
        value: String,
    },
    #[error("{component} {value:?} contains {ch:?}, which is not allowed here")]
    BadCharacter {
        component: &'static str,
        value: String,
        ch: char,
    },
}

/// Which characters a component of an address may be built from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Charset {
    /// Vocabulary defined by Sarma and its providers, never by user data.
    Vocabulary,
    /// Names chosen by whoever writes the configuration.
    Chosen,
}

impl Charset {
    fn permits(self, ch: char) -> bool {
        match self {
            Charset::Vocabulary => ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_',
            Charset::Chosen => ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.',
        }
    }
}

fn validate(component: &'static str, value: &str, charset: Charset) -> Result<(), AddressError> {
    if value.is_empty() {
        return Err(AddressError::Empty {
            component,
            address: value.to_owned(),
        });
    }
    if value.starts_with('-') {
        return Err(AddressError::LeadingDash {
            component,
            value: value.to_owned(),
        });
    }
    if let Some(ch) = value.chars().find(|&ch| !charset.permits(ch)) {
        return Err(AddressError::BadCharacter {
            component,
            value: value.to_owned(),
            ch,
        });
    }
    Ok(())
}

macro_rules! segment {
    ($name:ident, $label:literal, $charset:expr, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// Reads one component, rejecting anything the grammar does not allow.
            pub fn new(value: impl Into<String>) -> Result<Self, AddressError> {
                let value = value.into();
                validate($label, &value, $charset)?;
                Ok(Self(value))
            }

            /// The component as written.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = AddressError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }
    };
}

segment!(
    Provider,
    "provider",
    Charset::Vocabulary,
    "Who owns the resource kind: `os`, `yandex`."
);
segment!(
    Kind,
    "kind",
    Charset::Vocabulary,
    "What sort of thing it is: `package`, `instance`."
);
segment!(
    Name,
    "name",
    Charset::Chosen,
    "What the configuration calls this one."
);
segment!(
    Slot,
    "slot",
    Charset::Chosen,
    "The stable key that lets two instances of one declaration coexist.\n\n\
     Borrowed from Portage, where it is the reason `python:3.11` and `python:3.12`\n\
     are two installed packages rather than a conflict. Here it plays the same part\n\
     for two application versions during a migration, or two clusters during a\n\
     cutover, without either of them pretending to be the other."
);
segment!(
    Target,
    "target",
    Charset::Chosen,
    "The node a resource lives on, for the `target` domain."
);

/// The identity of one resource.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Address {
    provider: Provider,
    kind: Kind,
    name: Name,
    slot: Option<Slot>,
    target: Option<Target>,
}

impl Address {
    /// Builds an address that lives in the control domain — reachable through an
    /// API, with no node behind it.
    pub fn control(provider: Provider, kind: Kind, name: Name) -> Self {
        Self {
            provider,
            kind,
            name,
            slot: None,
            target: None,
        }
    }

    /// Builds an address for a resource that only exists on `target`.
    pub fn on(mut self, target: Target) -> Self {
        self.target = Some(target);
        self
    }

    /// Distinguishes this instance from its siblings under the same declaration.
    pub fn in_slot(mut self, slot: Slot) -> Self {
        self.slot = Some(slot);
        self
    }

    pub fn provider(&self) -> &Provider {
        &self.provider
    }

    pub fn kind(&self) -> &Kind {
        &self.kind
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn slot(&self) -> Option<&Slot> {
        self.slot.as_ref()
    }

    pub fn target(&self) -> Option<&Target> {
        self.target.as_ref()
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}.{}", self.provider, self.kind, self.name)?;
        if let Some(slot) = &self.slot {
            write!(f, ":{slot}")?;
        }
        if let Some(target) = &self.target {
            write!(f, "@{target}")?;
        }
        Ok(())
    }
}

impl std::str::FromStr for Address {
    type Err = AddressError;

    /// Reads an address back from the form [`Display`] writes.
    ///
    /// The separators are picked so that this needs no lookahead: `:` first
    /// splits off the provider, `@` then splits off the node, and only after
    /// that does `.` separate kind from name. A name may therefore contain dots
    /// — `os:file.nginx.conf` is a file called `nginx.conf` — while no component
    /// may contain `@` or `:`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (provider, rest) = text
            .split_once(':')
            .ok_or_else(|| AddressError::NoProvider(text.to_owned()))?;

        let (locator, target) = match rest.split_once('@') {
            Some((locator, target)) => (locator, Some(Target::new(target)?)),
            None => (rest, None),
        };

        let (kind, name) = locator
            .split_once('.')
            .ok_or_else(|| AddressError::NoKind(text.to_owned()))?;

        let (name, slot) = match name.split_once(':') {
            Some((name, slot)) => (name, Some(Slot::new(slot)?)),
            None => (name, None),
        };

        let address = Address {
            provider: Provider::new(provider)?,
            kind: Kind::new(kind)?,
            name: Name::new(name)?,
            slot,
            target,
        };

        // Round-tripping is not a nicety here. The plan hash is taken over
        // canonical text, so a value that prints differently from the way it
        // parsed would produce two hashes for one plan and quietly invalidate a
        // signature that was given in good faith.
        ensures!(
            address.to_string() == text,
            "address {text:?} did not round-trip, printed as {:?}",
            address.to_string()
        );
        Ok(address)
    }
}

/// Reads an address, panicking on malformed input.
///
/// For tests and for literals written in Sarma's own source, where a bad address
/// is a bug in this repository rather than bad input from a user.
pub fn address(text: &str) -> Address {
    requires!(!text.is_empty());
    text.parse()
        .unwrap_or_else(|err| panic!("malformed address literal {text:?}: {err}"))
}
