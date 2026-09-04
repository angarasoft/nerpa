//! The resource model behind the `sarma` command.
//!
//! Sarma holds cloud resources and the contents of the machines they run on in
//! one graph, plans the whole of it in a single pass, and applies it over a
//! transport that carries an identity rather than a key.
//!
//! This crate is the vocabulary: what a resource is, what names it can have, and
//! where its state can be read. Everything that acts on the world is built on
//! top of it.
//!
//! # How the pieces fit
//!
//! ```text
//!   Address ─── the identity written into state, frozen once state exists
//!     │
//!     └── ExecutionDomain ─── read from the address, never stored beside it
//!           │
//!           ├── Control ──────── state lives behind an API
//!           └── Target(node) ─── state lives on one machine and nowhere else
//!
//!   Resource ─── an Address, the attributes it should have, what it comes
//!                after, and whether drift on it may be overwritten
//! ```
//!
//! # Correctness
//!
//! Two layers, in this order:
//!
//! 1. **Types.** No part of an [`Address`] can begin with `-`, so no amount of
//!    careless interpolation can turn a resource name into a command line
//!    option. There is no index anywhere in an address, so the renumbering that
//!    makes list-shaped declarations propose destroying live resources has
//!    nowhere to originate.
//! 2. **Contracts.** What types cannot state is written with [`requires!`],
//!    [`ensures!`] and [`invariant!`] and checked at runtime, in release builds
//!    too. See [`contract`] for why a violation panics and why that is the safe
//!    option for a tool that changes infrastructure.
//!
//! # Layout
//!
//! * [`address`] — resource identity, and the grammar it is written in.
//! * [`domain`] — where a resource's state can be read, and why that does not
//!   force planning to happen in waves.
//! * [`resource`] — a node of the desired-state graph.
//! * [`value`] — attribute values.
//! * [`contract`] — the runtime assertions and the reasoning behind them.

pub mod address;
pub mod contract;
pub mod domain;
pub mod resource;
pub mod value;

pub use address::{Address, AddressError, Kind, Name, Provider, Slot, Target};
pub use domain::ExecutionDomain;
pub use resource::{Protection, Resource};
pub use value::Value;
