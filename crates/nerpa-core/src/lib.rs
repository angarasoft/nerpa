//! The resource model behind the `nerpa` command.
//!
//! Nerpa holds cloud resources and the contents of the machines they run on in
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
//! * [`actor`] — who acts in a run, over what, and until when.
//! * [`address`] — resource identity, and the grammar it is written in.
//! * [`domain`] — where a resource's state can be read, and why that does not
//!   force planning to happen in waves.
//! * [`attribute`] — what a declaration says, including values that do not exist yet.
//! * [`facts`] — what a machine turns out to be, and how it is asked.
//! * [`graph`] — the desired-state graph, and the stages it is applied in.
//! * [`inventory`] — where the nodes come from.
//! * [`observed`] — what the world looks like, read before planning starts.
//! * [`schema`] — which attributes cannot be changed in place.
//! * [`secret`] — values that must never be written down.
//! * [`planner`] — desired and observed state, in one pass, into a plan.
//!   Called as `planner::plan`, not re-exported: a `plan` function beside a
//!   `plan` module reads as an accident.
//! * [`plan`] — what a run intends to do.
//! * [`provider`] — the boundary where the model stops and the world begins.
//! * [`run`] — reading the world, and making it match.
//! * [`state`] — what the last run left behind, and what it depended on.
//! * [`store`] — writing that down, and not losing it.
//! * [`resource`] — a node of the desired-state graph.
//! * [`value`] — attribute values.
//! * [`contract`] — the runtime assertions and the reasoning behind them.

pub mod actor;
pub mod address;
pub mod attribute;
pub mod contract;
pub mod domain;
pub mod facts;
pub mod fanout;
pub mod graph;
pub mod inventory;
pub mod observed;
pub mod plan;
pub mod planner;
pub mod provider;
pub mod resource;
pub mod run;
pub mod schema;
pub mod secret;
pub mod shards;
pub mod state;
pub mod store;
pub mod value;

pub use actor::{Actor, ActorError};
pub use address::{Address, AddressError, Kind, Name, Provider, Slot, Target};
pub use attribute::{Attribute, Reference};
pub use domain::ExecutionDomain;
pub use graph::{Graph, GraphError, Moved, Stage, StageKind};
pub use inventory::{Inventory, Provenance};
pub use observed::Observed;
pub use plan::{
    AttributeDelta, BlastRadius, Change, ChangeKind, Confidence, DeltaKind, Plan, PlanStage,
};
pub use resource::{Protection, Resource};
pub use schema::Schema;
pub use value::Value;
