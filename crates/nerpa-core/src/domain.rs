//! Where a resource's state can be read and changed.
//!
//! This is the distinction the rest of Nerpa is built on, and the one neither
//! Terraform nor Ansible expresses. A VM's state is a question you ask an API,
//! from wherever you happen to be standing. A package's state is a question you
//! can only ask by running code on one particular machine — and that machine may
//! be something this very run is about to create.
//!
//! # Why this does not force lazy planning
//!
//! The obvious reading is that a resource in the target domain cannot be planned
//! until its node exists, and therefore planning must happen in waves. It does
//! not follow, for two reasons:
//!
//! * For a node this run is creating, observed state is not *unknown*, it is
//!   *empty* — the node is new. The diff is "create everything", and that is
//!   computable before anything happens.
//! * For a node that already exists, the node is reachable, so it can be read
//!   during planning like any other source of observed state.
//!
//! What genuinely is not known before apply are individual attribute *values* —
//! an address allocated by the cloud, an identifier minted on creation. Those
//! are marked unknown and carry a reference to the resource that will supply
//! them. They never determine which resources exist.

use crate::address::{Address, Target};

/// The two places a resource's state can live.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExecutionDomain {
    /// Read and changed through an API, from wherever the run starts.
    Control,
    /// Read and changed only by executing code on one node.
    Target(Target),
}

impl ExecutionDomain {
    /// True when planning this resource needs nothing but the network the run
    /// already has.
    pub fn is_control(&self) -> bool {
        matches!(self, ExecutionDomain::Control)
    }

    /// The node this resource lives on, if any.
    pub fn target(&self) -> Option<&Target> {
        match self {
            ExecutionDomain::Control => None,
            ExecutionDomain::Target(target) => Some(target),
        }
    }
}

impl Address {
    /// Which domain this resource belongs to.
    ///
    /// Derived from the address rather than stored beside it. A separate field
    /// would be a second source of truth, and the two could disagree — at which
    /// point the planner would look for a node that the address says is not
    /// there, or skip one that is.
    // ENFORCES: INV-DOMAIN-001
    pub fn domain(&self) -> ExecutionDomain {
        match self.target() {
            None => ExecutionDomain::Control,
            Some(target) => ExecutionDomain::Target(target.clone()),
        }
    }
}
