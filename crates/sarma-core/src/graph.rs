//! The desired-state graph, and the order it has to be applied in.
//!
//! Resources are nodes, declared dependencies are edges. The graph exists so
//! that two questions can be answered before anything is touched: is this
//! configuration coherent, and in what order does it have to happen.
//!
//! # Stages
//!
//! A stage is one topological level: every resource whose dependencies are all
//! satisfied by earlier stages. Stages order *execution* — a VM is created
//! before it is configured because the graph says so. They are not planning
//! rounds. The whole graph is known here, before the first stage runs, and
//! nothing about a later stage is left to be worked out after an earlier one
//! finishes.
//!
//! # Determinism
//!
//! Every collection in this module is a `BTree*`, and every result is sorted.
//! Two runs over the same configuration must produce byte-identical stages,
//! because those stages are hashed and the hash is signed. Hash map iteration
//! order would make that untrue on alternate Tuesdays.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::address::Address;
use crate::domain::ExecutionDomain;
use crate::ensures;
use crate::resource::Resource;

/// Why a graph could not be built or ordered.
///
/// The variants carry whole addresses rather than identifiers, because the
/// message a person reads at three in the morning should not require a second
/// lookup to be useful. That makes the error type large, and clippy says so at
/// each returning function; boxing it would trade a real diagnostic for a lint
/// on a path that runs once per plan, not once per resource.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum GraphError {
    /// Two declarations claimed one address.
    #[error("{0} was declared twice")]
    Duplicate(Address),
    /// A declared dependency names something that is not here.
    #[error("{from} depends on {to}, which is not in the graph")]
    Dangling {
        /// The resource that declared the dependency.
        from: Address,
        /// The address it named, which is absent.
        to: Address,
    },
    /// Ordering is impossible. Carries everything a cycle holds up, not only
    /// the cycle proper.
    #[error("dependencies form a cycle: {}", .0.iter().map(ToString::to_string).collect::<Vec<_>>().join(" -> "))]
    Cycle(Vec<Address>),
}

/// What kind of work one stage is made of.
///
/// Reported rather than decided: it falls out of which domains the resources in
/// the stage belong to. A mixed stage is legitimate — two independent subtrees
/// can be at the same depth — and is worth naming so that plan output can say so
/// instead of implying an order that is not there.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum StageKind {
    /// Everything in this stage is reachable through an API.
    Control,
    /// Everything in this stage runs on nodes, named here.
    Target(BTreeSet<crate::address::Target>),
    /// Both, at the same depth and independent of each other.
    Mixed,
}

/// One topological level of the graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage {
    kind: StageKind,
    addresses: Vec<Address>,
}

impl Stage {
    /// What sort of work this stage is made of.
    pub fn kind(&self) -> &StageKind {
        &self.kind
    }

    /// The resources in this stage, in a stable order.
    pub fn addresses(&self) -> &[Address] {
        &self.addresses
    }
}

/// A set of resources and the ordering between them.
#[derive(Debug, Clone, Default)]
pub struct Graph {
    resources: BTreeMap<Address, Resource>,
}

impl Graph {
    /// An empty graph.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a resource, refusing a second one at the same address.
    ///
    /// Two declarations of one address are not a merge to be resolved but a
    /// question with two answers, and picking either would be a guess about
    /// which one the author meant.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError::Duplicate`] if this address is already present.
    #[allow(
        clippy::result_large_err,
        reason = "diagnostics beat a lint on a cold path"
    )]
    pub fn insert(&mut self, resource: Resource) -> Result<(), GraphError> {
        let address = resource.address().clone();
        if self.resources.contains_key(&address) {
            return Err(GraphError::Duplicate(address));
        }
        self.resources.insert(address, resource);
        Ok(())
    }

    /// How many resources are in the graph.
    #[must_use]
    pub fn len(&self) -> usize {
        self.resources.len()
    }

    /// Whether the graph holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }

    /// The resource at `address`, if it is here.
    #[must_use]
    pub fn get(&self, address: &Address) -> Option<&Resource> {
        self.resources.get(address)
    }

    /// Orders the graph into stages, or explains why it cannot be ordered.
    ///
    /// Kahn's algorithm, over sorted collections throughout so that the answer —
    /// including the cycle report — is the same on every machine and every run.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError::Dangling`] if a declared dependency names a
    /// resource that is not in the graph, and [`GraphError::Cycle`] if the
    /// dependencies cannot be ordered at all.
    ///
    /// # Panics
    ///
    /// Panics if a contract is violated — that is, if this function's own
    /// bookkeeping has become inconsistent. See [`crate::contract`].
    ///
    // ENFORCES: INV-GRAPH-001
    // ENFORCES: INV-GRAPH-002
    #[allow(
        clippy::result_large_err,
        reason = "diagnostics beat a lint on a cold path"
    )]
    pub fn stages(&self) -> Result<Vec<Stage>, GraphError> {
        let mut indegree: BTreeMap<&Address, usize> = self
            .resources
            .keys()
            .map(|address| (address, 0usize))
            .collect();
        let mut dependents: BTreeMap<&Address, BTreeSet<&Address>> = BTreeMap::new();

        for (address, resource) in &self.resources {
            for dependency in resource.depends_on() {
                let Some((known, _)) = self.resources.get_key_value(dependency) else {
                    return Err(GraphError::Dangling {
                        from: address.clone(),
                        to: dependency.clone(),
                    });
                };
                if dependents.entry(known).or_default().insert(address) {
                    *indegree
                        .get_mut(address)
                        .expect("address came from this map") += 1;
                }
            }
        }

        let mut ready: VecDeque<&Address> = indegree
            .iter()
            .filter(|(_, degree)| **degree == 0)
            .map(|(address, _)| *address)
            .collect();

        let mut stages = Vec::new();
        let mut placed = 0usize;

        while !ready.is_empty() {
            let level: Vec<&Address> = ready.drain(..).collect();
            let mut next: BTreeSet<&Address> = BTreeSet::new();

            for address in &level {
                for dependent in dependents.get(address).into_iter().flatten() {
                    let degree = indegree
                        .get_mut(*dependent)
                        .expect("dependent came from this map");
                    *degree -= 1;
                    if *degree == 0 {
                        next.insert(dependent);
                    }
                }
            }

            placed += level.len();
            stages.push(Self::stage_from(level));
            ready.extend(next);
        }

        if placed != self.resources.len() {
            // Whatever still has an incoming edge is either in a cycle or
            // downstream of one. Both belong in the report: naming only the
            // cycle proper leaves the reader wondering why unrelated resources
            // went missing from the plan.
            let stuck: Vec<Address> = indegree
                .iter()
                .filter(|(_, degree)| **degree > 0)
                .map(|(address, _)| (*address).clone())
                .collect();
            ensures!(
                !stuck.is_empty(),
                "resources went missing without a cycle to explain it"
            );
            return Err(GraphError::Cycle(stuck));
        }

        ensures!(
            stages
                .iter()
                .map(|stage| stage.addresses.len())
                .sum::<usize>()
                == self.resources.len(),
            "every resource appears in exactly one stage"
        );
        Ok(stages)
    }

    fn stage_from(level: Vec<&Address>) -> Stage {
        let mut targets = BTreeSet::new();
        let mut has_control = false;

        for address in &level {
            match address.domain() {
                ExecutionDomain::Control => has_control = true,
                ExecutionDomain::Target(target) => {
                    targets.insert(target);
                }
            }
        }

        let kind = match (has_control, targets.is_empty()) {
            (true, true) => StageKind::Control,
            (false, false) => StageKind::Target(targets),
            _ => StageKind::Mixed,
        };

        Stage {
            kind,
            addresses: level.into_iter().cloned().collect(),
        }
    }
}

impl FromIterator<Resource> for Result<Graph, GraphError> {
    #[allow(
        clippy::result_large_err,
        reason = "diagnostics beat a lint on a cold path"
    )]
    fn from_iter<I: IntoIterator<Item = Resource>>(resources: I) -> Self {
        let mut graph = Graph::new();
        for resource in resources {
            graph.insert(resource)?;
        }
        Ok(graph)
    }
}
